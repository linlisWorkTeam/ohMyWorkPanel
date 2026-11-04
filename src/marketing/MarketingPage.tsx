//! 内容页（设计 P08）：campaign 列表 + 详情 + 创建/修订/批准/导出。
//!
//! 事实以服务端为准：所有写操作后读回；未批准的 campaign 不提供可用导出。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { api } from "../api";
import type { Member } from "../types";
import type {
  CampaignExport,
  CampaignStatus,
  ChannelDraft,
  ContentCampaign,
  CreateCampaignInput,
} from "./types";

const STATUS_LABEL: Record<CampaignStatus, string> = {
  collecting: "收集证据",
  planning: "规划中",
  writing: "撰写中",
  validating: "校验中",
  awaiting_user: "待确认",
  changes_requested: "需修改",
  approved: "已批准",
  no_content: "无可发布内容",
  failed: "失败",
};

const CHANNEL_LABEL: Record<ChannelDraft["channel"], string> = {
  xiaohongshu: "小红书",
  x: "X",
  zhihu: "知乎",
  bilibili: "B 站",
  github_release: "GitHub Release",
};

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function formatTime(ts: number): string {
  if (!Number.isFinite(ts)) return "—";
  return new Date(ts).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

interface MarketingPageProps {
  groupId: string;
  members: Member[];
  senderMemberId: string | null;
  canManage: boolean;
}

export function MarketingPage({ groupId, members, senderMemberId, canManage }: MarketingPageProps) {
  const [campaigns, setCampaigns] = useState<ContentCampaign[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const generation = useRef(0);

  const agents = useMemo(() => members.filter((member) => member.kind === "agent"), [members]);

  const load = useCallback(
    async (preferredId?: string | null) => {
      const current = ++generation.current;
      setLoading(true);
      try {
        const list = await api.listMarketingCampaigns(groupId);
        if (current !== generation.current) return;
        setCampaigns(list);
        setSelectedId((previous) => {
          const wanted = preferredId ?? previous;
          if (wanted && list.some((item) => item.id === wanted)) return wanted;
          const active = list.find((item) => item.status !== "approved" && item.status !== "failed");
          return (active ?? list[0])?.id ?? null;
        });
        setError(null);
      } catch (cause) {
        if (current === generation.current) setError(errorText(cause));
      } finally {
        if (current === generation.current) setLoading(false);
      }
    },
    [groupId],
  );

  useEffect(() => {
    setCampaigns([]);
    setSelectedId(null);
    void load(null);
    return () => {
      generation.current += 1;
    };
  }, [load]);

  const selected = campaigns.find((item) => item.id === selectedId) ?? null;

  const act = async (action: () => Promise<unknown>, preferredId?: string | null) => {
    setBusy(true);
    try {
      await action();
      await load(preferredId ?? selectedId);
    } catch (cause) {
      setError(errorText(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="content-page">
      <aside className="content-list" aria-label="内容 campaign 列表">
        <div className="content-list-head">
          <h2>内容</h2>
          {canManage && senderMemberId && (
            <button type="button" className="pm-btn primary sm" onClick={() => setShowCreate(true)}>
              新建
            </button>
          )}
        </div>
        {loading && campaigns.length === 0 ? (
          <p className="content-hint">加载中…</p>
        ) : campaigns.length === 0 ? (
          <div className="content-empty">
            <p>还没有内容 campaign。</p>
            {canManage && senderMemberId ? (
              <button type="button" className="pm-btn sm" onClick={() => setShowCreate(true)}>
                创建第一个 campaign
              </button>
            ) : (
              <p className="content-hint">需要群主或管理员创建。</p>
            )}
          </div>
        ) : (
          <ul className="content-items">
            {campaigns.map((campaign) => (
              <li key={campaign.id}>
                <button
                  type="button"
                  className={`content-row${campaign.id === selectedId ? " is-active" : ""}`}
                  aria-current={campaign.id === selectedId}
                  onClick={() => setSelectedId(campaign.id)}
                >
                  <span className="content-row-title">
                    {campaign.brief?.coreMessage?.trim() ||
                      campaign.snapshot.commits[0]?.subject ||
                      `campaign ${campaign.id.slice(0, 8)}`}
                  </span>
                  <span className="content-row-meta">
                    <span className="content-status" data-status={campaign.status}>
                      {STATUS_LABEL[campaign.status]}
                    </span>
                    <span>r{campaign.revision}</span>
                    <span>{campaign.drafts.length} 渠道</span>
                    <span>{formatTime(campaign.updatedAt)}</span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </aside>

      <section className="content-detail" aria-label="campaign 详情">
        {error && (
          <div className="content-error" role="alert">
            {error}
          </div>
        )}
        {showCreate ? (
          <CreateCampaignPanel
            groupId={groupId}
            agents={agents}
            requestedBy={senderMemberId}
            onCancel={() => setShowCreate(false)}
            onCreated={async (campaign) => {
              setShowCreate(false);
              await load(campaign?.id ?? null);
            }}
            onError={setError}
          />
        ) : selected ? (
          <CampaignDetail
            campaign={selected}
            actorMemberId={senderMemberId}
            canManage={canManage}
            busy={busy}
            onAction={act}
            onError={setError}
          />
        ) : (
          <div className="content-empty">
            <p>选择左侧 campaign 查看详情，或新建一个。</p>
          </div>
        )}
      </section>
    </div>
  );
}

function CampaignDetail({
  campaign,
  actorMemberId,
  canManage,
  busy,
  onAction,
  onError,
}: {
  campaign: ContentCampaign;
  actorMemberId: string | null;
  canManage: boolean;
  busy: boolean;
  onAction: (action: () => Promise<unknown>, preferredId?: string | null) => Promise<void>;
  onError: (message: string) => void;
}) {
  const [feedback, setFeedback] = useState("");
  const [exported, setExported] = useState<CampaignExport | null>(null);
  const canExport = campaign.status === "approved" && campaign.drafts.length > 0;
  const canRevise = canManage && actorMemberId && campaign.status !== "approved";

  return (
    <div className="campaign-detail">
      <header className="campaign-head">
        <div>
          <h2>{campaign.brief?.coreMessage?.trim() || "内容 campaign"}</h2>
          <div className="campaign-meta">
            <span className="content-status" data-status={campaign.status}>
              {STATUS_LABEL[campaign.status]}
            </span>
            <span>来源 {campaign.sourceMode === "include_uncommitted" ? "含未提交改动" : "已提交"}</span>
            {campaign.baseRef && <span>base {campaign.baseRef}</span>}
            <span>head {campaign.headRef || "—"}</span>
            <span>{campaign.snapshot.evidence.length} 条证据</span>
            <span>修订 r{campaign.revision}</span>
          </div>
        </div>
        <div className="campaign-actions">
          {canManage && actorMemberId && campaign.status !== "approved" && (
            <button
              type="button"
              className="pm-btn primary sm"
              disabled={busy}
              onClick={() => void onAction(() => api.approveMarketingCampaign(campaign.id, actorMemberId), campaign.id)}
            >
              批准
            </button>
          )}
          <button
            type="button"
            className="pm-btn sm"
            disabled={busy || !canExport}
            title={canExport ? undefined : "只有已批准且有内容的 campaign 可以导出"}
            onClick={() =>
              void onAction(async () => {
                const result = await api.exportMarketingCampaign(campaign.id);
                setExported(result);
              }, campaign.id)
            }
          >
            导出
          </button>
        </div>
      </header>

      {campaign.errorMessage && <p className="content-error-inline">{campaign.errorMessage}</p>}

      {campaign.snapshot.truncated && (
        <p className="content-hint">证据已截断：仓库较大，仅收集了部分提交与文件。</p>
      )}

      {campaign.brief ? (
        <section className="campaign-section">
          <h3>内容 brief</h3>
          <div className="campaign-brief">
            <p>
              <strong>可发布性</strong>{" "}
              {campaign.brief.publishability === "publish"
                ? "可发布"
                : campaign.brief.publishability === "hold"
                  ? "暂缓"
                  : "无可发布内容"}
              {campaign.brief.reason ? ` · ${campaign.brief.reason}` : ""}
            </p>
            {campaign.brief.audience.length > 0 && (
              <p>
                <strong>受众</strong> {campaign.brief.audience.join("、")}
              </p>
            )}
            <p>
              <strong>核心信息</strong> {campaign.brief.coreMessage}
            </p>
            {campaign.brief.updates.length > 0 && (
              <ul className="campaign-updates">
                {campaign.brief.updates.map((update) => (
                  <li key={update.id}>
                    <strong>{update.title}</strong>
                    {update.summary ? ` · ${update.summary}` : ""}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>
      ) : (
        <p className="content-hint">brief 尚未生成（等待规划 Agent 完成）。</p>
      )}

      <section className="campaign-section">
        <h3>渠道文案</h3>
        {campaign.drafts.length === 0 ? (
          <p className="content-hint">还没有草稿。</p>
        ) : (
          <div className="campaign-drafts">
            {campaign.drafts.map((draft) => (
              <article key={draft.channel} className="campaign-draft">
                <header>
                  <span className="draft-channel">{CHANNEL_LABEL[draft.channel] ?? draft.channel}</span>
                  <strong>{draft.title}</strong>
                </header>
                <pre>{draft.body}</pre>
                {draft.claimRefs.length > 0 && (
                  <p className="content-hint">引用 {draft.claimRefs.length} 条证据</p>
                )}
              </article>
            ))}
          </div>
        )}
      </section>

      {campaign.validation.length > 0 && (
        <section className="campaign-section">
          <h3>校验</h3>
          <ul className="campaign-findings">
            {campaign.validation.map((finding, index) => (
              <li key={`${finding.code}-${index}`} data-severity={finding.severity}>
                [{finding.severity}] {finding.message}
              </li>
            ))}
          </ul>
        </section>
      )}

      {canRevise && (
        <section className="campaign-section">
          <h3>要求修改</h3>
          {campaign.feedback && (
            <p className="content-hint">
              上次反馈（{campaign.feedbackBy ?? "—"}）：{campaign.feedback}
            </p>
          )}
          <textarea
            rows={3}
            value={feedback}
            placeholder="指出需要补充或修改的内容"
            onChange={(event) => setFeedback(event.target.value)}
          />
          <button
            type="button"
            className="pm-btn sm"
            disabled={busy || !feedback.trim()}
            onClick={() =>
              void onAction(async () => {
                await api.reviseMarketingCampaign(campaign.id, actorMemberId ?? "", feedback.trim());
                setFeedback("");
              }, campaign.id).catch((cause) => onError(errorText(cause)))
            }
          >
            提交修订
          </button>
        </section>
      )}

      {exported && (
        <section className="campaign-section">
          <h3>导出：{exported.filename}</h3>
          <pre className="campaign-export">{exported.markdown}</pre>
        </section>
      )}
    </div>
  );
}

function CreateCampaignPanel({
  groupId,
  agents,
  requestedBy,
  onCancel,
  onCreated,
  onError,
}: {
  groupId: string;
  agents: Member[];
  requestedBy: string | null;
  onCancel: () => void;
  onCreated: (campaign: ContentCampaign | null) => Promise<void> | void;
  onError: (message: string) => void;
}) {
  const [sourceMode, setSourceMode] = useState<"committed" | "include_uncommitted">("committed");
  const [baseRef, setBaseRef] = useState("");
  const [planner, setPlanner] = useState("");
  const [writer, setWriter] = useState("");
  const [saving, setSaving] = useState(false);

  const submit = async () => {
    if (!requestedBy || saving) return;
    setSaving(true);
    try {
      const input: CreateCampaignInput = {
        groupId,
        requestedBy,
        sourceMode,
        plannerAgentId: planner || undefined,
        writerAgentId: writer || undefined,
        baseRef: baseRef.trim() || undefined,
      };
      const campaign = await api.createMarketingCampaign(input);
      await onCreated(campaign);
    } catch (cause) {
      onError(errorText(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="campaign-create">
      <h2>新建内容 campaign</h2>
      <p className="content-hint">
        基于当前仓库快照生成对外内容；需要群里已配置可用的规划 / 撰写 Agent。
      </p>
      <label className="content-field">
        <span>来源模式</span>
        <select
          value={sourceMode}
          onChange={(event) => setSourceMode(event.target.value as "committed" | "include_uncommitted")}
        >
          <option value="committed">仅已提交内容</option>
          <option value="include_uncommitted">包含未提交改动</option>
        </select>
      </label>
      <label className="content-field">
        <span>base ref（可选）</span>
        <input value={baseRef} placeholder="例如 origin/main" onChange={(event) => setBaseRef(event.target.value)} />
      </label>
      <label className="content-field">
        <span>规划 Agent（可选）</span>
        <select value={planner} onChange={(event) => setPlanner(event.target.value)}>
          <option value="">自动</option>
          {agents.map((agent) => (
            <option key={agent.id} value={agent.id}>
              {agent.displayName} · {agent.adapter ?? "—"}
            </option>
          ))}
        </select>
      </label>
      <label className="content-field">
        <span>撰写 Agent（可选）</span>
        <select value={writer} onChange={(event) => setWriter(event.target.value)}>
          <option value="">自动</option>
          {agents.map((agent) => (
            <option key={agent.id} value={agent.id}>
              {agent.displayName} · {agent.adapter ?? "—"}
            </option>
          ))}
        </select>
      </label>
      <div className="campaign-create-actions">
        <button type="button" className="pm-btn primary sm" disabled={saving || !requestedBy} onClick={() => void submit()}>
          {saving ? "创建中…" : "创建"}
        </button>
        <button type="button" className="pm-btn sm" disabled={saving} onClick={onCancel}>
          取消
        </button>
      </div>
    </div>
  );
}
