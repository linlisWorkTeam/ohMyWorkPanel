//! 可观测页（设计 P10）：日志 / 经验 / 指标 / 运维，全局入口。
//!
//! 能力差异显式表达：桌面没有 metrics 接口，会显示「仅 Web」而不是假 0。

import { useCallback, useEffect, useState } from "react";

import { api } from "../api";
import { PageHeader, PageSection, PageShell } from "../components/ui";
import type { Member, MetricsSample, OpsJobState, ReleaseStatus } from "../types";
import { ExperiencePanel } from "./ExperiencePanel";
import { LogsPanel } from "./LogsPanel";

type ObservabilityTab = "logs" | "experiences" | "metrics" | "ops";

const TABS: { id: ObservabilityTab; label: string }[] = [
  { id: "logs", label: "日志" },
  { id: "experiences", label: "经验" },
  { id: "metrics", label: "指标" },
  { id: "ops", label: "运维" },
];

function readError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function formatTime(ts: number | null | undefined): string {
  if (!ts) return "—";
  return new Date(ts).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

interface ObservabilityPageProps {
  groupId: string | null;
  members: Member[];
  ownerId: string | null;
  isAdmin: boolean;
  onError: (message: string) => void;
}

export function ObservabilityPage({ groupId, members, ownerId, isAdmin, onError }: ObservabilityPageProps) {
  const [tab, setTab] = useState<ObservabilityTab>("logs");

  return (
    <PageShell className="observability-page">
      <PageHeader
        title="可观测"
        description="日志、经验、指标与运维操作；写操作按权限展示。"
        actions={
          <nav className="obs-tabs" aria-label="可观测分区">
            {TABS.map((item) => (
              <button
                key={item.id}
                type="button"
                className={`obs-tab${tab === item.id ? " is-active" : ""}`}
                aria-current={tab === item.id ? "page" : undefined}
                onClick={() => setTab(item.id)}
              >
                {item.label}
              </button>
            ))}
          </nav>
        }
      />

      {tab === "logs" && (
        <PageSection title="日志" description="按级别与来源过滤；总数与当前已加载条数分开统计。">
          <LogsPanel onError={onError} />
        </PageSection>
      )}

      {tab === "experiences" && (
        <PageSection title="经验" description="保存可复用的排查与实现经验，按群隔离。">
          {groupId && ownerId ? (
            <ExperiencePanel groupId={groupId} members={members} ownerId={ownerId} onError={onError} />
          ) : (
            <p className="obs-hint">先选择一个群，再查看该群的经验。</p>
          )}
        </PageSection>
      )}

      {tab === "metrics" && (
        <PageSection title="指标" description="进程级采样；桌面端没有该接口。">
          <MetricsSection onError={onError} />
        </PageSection>
      )}

      {tab === "ops" && (
        <PageSection title="运维" description="灰度槽位与门禁任务；前台不提供生产发布按钮。">
          <OpsSection isAdmin={isAdmin} onError={onError} />
        </PageSection>
      )}
    </PageShell>
  );
}

function MetricsSection({ onError }: { onError: (message: string) => void }) {
  const [sample, setSample] = useState<MetricsSample | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "unsupported" | "error">("loading");
  const [message, setMessage] = useState("");

  useEffect(() => {
    let alive = true;
    void (async () => {
      try {
        const result = await api.getMetricsLatest();
        if (!alive) return;
        setSample(result);
        setStatus("ready");
      } catch (error) {
        if (!alive) return;
        const text = readError(error);
        setMessage(text);
        setStatus(/web-only|not supported|unsupported/i.test(text) ? "unsupported" : "error");
        if (!/web-only/i.test(text)) onError(text);
      }
    })();
    return () => {
      alive = false;
    };
  }, [onError]);

  if (status === "loading") return <p className="obs-hint">读取中…</p>;
  if (status === "unsupported") return <p className="obs-hint">指标仅在 Web 端可用；桌面端不提供该接口。</p>;
  if (status === "error") return <p className="obs-error">{message}</p>;
  if (!sample) return <p className="obs-hint">暂无采样。</p>;
  return (
    <dl className="obs-metrics">
      <div>
        <dt>RSS</dt>
        <dd>{sample.rssMib.toFixed(1)} MiB</dd>
      </div>
      <div>
        <dt>CPU</dt>
        <dd>{sample.cpuPct.toFixed(1)}%</dd>
      </div>
      <div>
        <dt>采样时间</dt>
        <dd>{formatTime(sample.ts)}</dd>
      </div>
    </dl>
  );
}

function OpsSection({ isAdmin, onError }: { isAdmin: boolean; onError: (message: string) => void }) {
  const [release, setRelease] = useState<ReleaseStatus | null>(null);
  const [job, setJob] = useState<OpsJobState | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const [nextRelease, nextJob] = await Promise.all([api.opsReleaseStatus(), api.opsJob()]);
      setRelease(nextRelease);
      setJob(nextJob);
    } catch (error) {
      onError(readError(error));
    }
  }, [onError]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!job?.running) return;
    const id = window.setInterval(() => void load(), 3000);
    return () => window.clearInterval(id);
  }, [job?.running, load]);

  const run = async (action: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    try {
      await action();
      await load();
    } catch (error) {
      onError(readError(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="obs-ops">
      <div className="obs-slots">
        <div>
          <h3>生产 :8080</h3>
          <p>{release ? `HTTP ${release.prod.httpStatus ?? "—"} · ${String(release.prod.release?.deployedAt ?? "—")}` : "读取中…"}</p>
        </div>
        <div>
          <h3>灰度 :8081</h3>
          <p>{release ? `HTTP ${release.canary.httpStatus ?? "—"} · ${String(release.canary.release?.deployedAt ?? "—")}` : "读取中…"}</p>
        </div>
      </div>
      {release?.note && <p className="obs-hint">{release.note}</p>}

      <div className="obs-job">
        <h3>最近任务</h3>
        {job ? (
          <>
            <p>
              {job.running ? "运行中" : job.exitCode === null ? "未运行" : `结束（exit ${job.exitCode}）`}
              {job.kind ? ` · ${job.kind}` : ""}
              {job.startedAt ? ` · 开始 ${formatTime(job.startedAt)}` : ""}
            </p>
            {job.log && <pre>{job.log.slice(-4000)}</pre>}
          </>
        ) : (
          <p className="obs-hint">读取中…</p>
        )}
      </div>

      {isAdmin && (
        <div className="obs-actions">
          <button
            type="button"
            className="pm-btn sm"
            disabled={busy || job?.running}
            onClick={() => {
              if (window.confirm("运行完整测试门禁（test:gate）？")) void run(() => api.opsRunTestGate());
            }}
          >
            运行门禁
          </button>
          <button
            type="button"
            className="pm-btn sm"
            disabled={busy || job?.running}
            onClick={() => {
              if (window.confirm("部署到灰度槽位 :8081？不会影响生产。")) void run(() => api.opsDeployCanary());
            }}
          >
            部署灰度
          </button>
          <button type="button" className="pm-btn sm" disabled={busy} onClick={() => void load()}>
            刷新
          </button>
        </div>
      )}
    </div>
  );
}
