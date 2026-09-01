import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { api } from "../api";
import { readError } from "../components/uiShared";
import type { Member } from "../types";
import type { ChannelDraft, ContentCampaign, CreateCampaignInput } from "./types";
import type { MarketingMarker } from "./markers";
import "./marketing.css";

const STATUS_LABEL: Record<ContentCampaign["status"], string> = {
  collecting: "采集证据",
  planning: "策划中",
  writing: "写作中",
  validating: "校验中",
  awaiting_user: "待你审核",
  changes_requested: "需要修改",
  approved: "已批准",
  no_content: "本轮不宣传",
  failed: "生成失败",
};

const CHANNEL_LABEL: Record<ChannelDraft["channel"], string> = {
  xiaohongshu: "小红书",
  x: "X / Twitter",
  zhihu: "知乎",
  bilibili: "B站脚本",
  github_release: "GitHub Release",
};

const ACTIVE = new Set<ContentCampaign["status"]>(["collecting", "planning", "writing", "validating"]);

function downloadMarkdown(filename: string, markdown: string) {
  const url = URL.createObjectURL(new Blob([markdown], { type: "text/markdown;charset=utf-8" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

async function copyText(value: string) {
  await navigator.clipboard.writeText(value);
}

export function MarketingMessage({
  marker,
  actorMemberId,
  onError,
}: {
  marker: MarketingMarker;
  actorMemberId: string | null;
  onError?: (message: string) => void;
}) {
  if (marker.kind === "internal") {
    const label = marker.stage === "planning"
      ? "Content Planner 正在核对仓库证据"
      : marker.stage === "revising"
        ? "Channel Writer 正在按审核意见修改"
        : "Channel Writer 正在生成五渠道草稿";
    return <div className="marketing-progress"><span className="marketing-pulse" />{label}</div>;
  }
  return <MarketingCampaignCard campaignId={marker.campaignId} actorMemberId={actorMemberId} onError={onError} />;
}

export function MarketingCampaignCard({
  campaignId,
  actorMemberId,
  onError,
}: {
  campaignId: string;
  actorMemberId: string | null;
  onError?: (message: string) => void;
}) {
  const [campaign, setCampaign] = useState<ContentCampaign | null>(null);
  const [channel, setChannel] = useState<ChannelDraft["channel"]>("xiaohongshu");
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);
  const onErrorRef = useRef(onError);

  useEffect(() => {
    onErrorRef.current = onError;
  }, [onError]);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    const load = async () => {
      try {
        const next = await api.getMarketingCampaign(campaignId);
        if (cancelled) return;
        setCampaign(next);
        if (ACTIVE.has(next.status)) timer = window.setTimeout(load, 1800);
      } catch (reason) {
        if (!cancelled) onErrorRef.current?.(readError(reason));
      }
    };
    void load();
    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [campaignId]);

  const selected = useMemo(
    () => campaign?.drafts.find((draft) => draft.channel === channel) ?? campaign?.drafts[0],
    [campaign, channel],
  );
  if (!campaign) return <div className="marketing-progress"><span className="marketing-pulse" />加载 Campaign…</div>;

  const errors = campaign.validation.filter((finding) => finding.severity === "error");
  const warnings = campaign.validation.filter((finding) => finding.severity === "warning");
  const canRevise = Boolean(actorMemberId && ["awaiting_user", "changes_requested"].includes(campaign.status));
  const canApprove = Boolean(actorMemberId && campaign.status === "awaiting_user" && errors.length === 0);

  const revise = async () => {
    if (!actorMemberId || !feedback.trim()) return;
    setBusy(true);
    try {
      setCampaign(await api.reviseMarketingCampaign(campaign.id, actorMemberId, feedback.trim()));
      setFeedback("");
    } catch (reason) {
      onError?.(readError(reason));
    } finally {
      setBusy(false);
    }
  };

  const approve = async () => {
    if (!actorMemberId) return;
    setBusy(true);
    try {
      setCampaign(await api.approveMarketingCampaign(campaign.id, actorMemberId));
    } catch (reason) {
      onError?.(readError(reason));
    } finally {
      setBusy(false);
    }
  };

  const exportDrafts = async () => {
    setBusy(true);
    try {
      const bundle = await api.exportMarketingCampaign(campaign.id);
      downloadMarkdown(bundle.filename, bundle.markdown);
    } catch (reason) {
      onError?.(readError(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <article className="marketing-card" data-status={campaign.status}>
      <header className="marketing-card-head">
        <div>
          <span className="marketing-eyebrow">SELF-MARKETING</span>
          <strong>{STATUS_LABEL[campaign.status]}</strong>
        </div>
        <span className="marketing-revision">r{campaign.revision}</span>
      </header>

      <div className="marketing-facts">
        <span>HEAD {campaign.headRef.slice(0, 8)}</span>
        <span>{campaign.snapshot.commits.length} commits</span>
        <span>{campaign.snapshot.evidence.length} evidence</span>
        {campaign.sourceMode === "include_uncommitted" && <span className="marketing-unreleased">含未提交内容</span>}
      </div>

      {campaign.brief && (
        <details className="marketing-brief" open={campaign.drafts.length === 0}>
          <summary>Content Brief · {campaign.brief.coreMessage || "暂无核心信息"}</summary>
          <p>{campaign.brief.reason}</p>
          {campaign.brief.updates.map((update) => (
            <div className="marketing-update" key={update.id}>
              <strong>{update.title}</strong>
              <p>{update.summary}</p>
              <small>{update.releaseState} · {update.evidenceRefs.join(", ")}</small>
            </div>
          ))}
          {campaign.brief.doNotClaim.length > 0 && (
            <p className="marketing-boundary">不要声称：{campaign.brief.doNotClaim.join("；")}</p>
          )}
        </details>
      )}

      {campaign.drafts.length > 0 && (
        <section className="marketing-drafts">
          <div className="marketing-tabs" role="tablist" aria-label="宣传渠道">
            {campaign.drafts.map((draft) => (
              <button
                key={draft.channel}
                type="button"
                role="tab"
                aria-selected={(selected?.channel ?? channel) === draft.channel}
                onClick={() => setChannel(draft.channel)}
              >
                {CHANNEL_LABEL[draft.channel]}
              </button>
            ))}
          </div>
          {selected && (
            <div className="marketing-draft" role="tabpanel">
              <div className="marketing-draft-title">
                <strong>{selected.title}</strong>
                <button type="button" onClick={() => void copyText(selected.body)}>复制</button>
              </div>
              <pre>{selected.body}</pre>
              <small>事实引用：{selected.claimRefs.join(", ")}</small>
            </div>
          )}
        </section>
      )}

      {(errors.length > 0 || warnings.length > 0) && (
        <details className="marketing-validation" open={errors.length > 0}>
          <summary>{errors.length} 个阻断项 · {warnings.length} 个提醒</summary>
          {[...errors, ...warnings].map((finding, index) => (
            <p key={`${finding.code}-${index}`} data-severity={finding.severity}>
              {finding.message} <small>{finding.path}</small>
            </p>
          ))}
        </details>
      )}

      {campaign.errorMessage && <p className="marketing-error">{campaign.errorMessage}</p>}

      {canRevise && (
        <div className="marketing-review">
          <textarea
            rows={3}
            value={feedback}
            onChange={(event) => setFeedback(event.target.value)}
            placeholder="具体说明要改什么；事实范围变化请重新发起 Campaign。"
          />
          <button type="button" disabled={busy || !feedback.trim()} onClick={() => void revise()}>要求修改</button>
        </div>
      )}
      <footer className="marketing-actions">
        {canApprove && <button type="button" disabled={busy} onClick={() => void approve()}>批准内容</button>}
        {campaign.status === "approved" && <button type="button" disabled={busy} onClick={() => void exportDrafts()}>导出 Markdown</button>}
      </footer>
    </article>
  );
}

export function MarketingLaunchForm({
  groupId,
  requestedBy,
  members,
  onCreated,
  onCancel,
  onError,
}: {
  groupId: string;
  requestedBy: string;
  members: Member[];
  onCreated: (campaign: ContentCampaign) => void;
  onCancel: () => void;
  onError: (message: string) => void;
}) {
  const agents = members.filter((member) => member.isActive && (member.kind === "agent" || member.kind === "chatbot"));
  const preferredPlanner = agents.find((member) => /planner|策划|传播/i.test(`${member.displayName} ${member.roleDescription}`)) ?? agents[0];
  const preferredWriter = agents.find((member) => /writer|写作|文案/i.test(`${member.displayName} ${member.roleDescription}`)) ?? preferredPlanner;
  const [plannerAgentId, setPlannerAgentId] = useState(preferredPlanner?.id ?? "");
  const [writerAgentId, setWriterAgentId] = useState(preferredWriter?.id ?? "");
  const [includeUncommitted, setIncludeUncommitted] = useState(false);
  const [baseRef, setBaseRef] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!plannerAgentId || !writerAgentId) return;
    setBusy(true);
    const input: CreateCampaignInput = {
      groupId,
      requestedBy,
      plannerAgentId,
      writerAgentId,
      sourceMode: includeUncommitted ? "include_uncommitted" : "committed",
      baseRef: baseRef.trim() || undefined,
    };
    try {
      onCreated(await api.createMarketingCampaign(input));
    } catch (reason) {
      onError(readError(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="marketing-launch modal-form" onSubmit={(event) => void submit(event)}>
      <p className="form-hint">系统会冻结一份仓库证据快照，再由 Planner 和 Writer 生成可追溯草稿。不会自动发布。</p>
      <label>Content Planner
        <select value={plannerAgentId} onChange={(event) => setPlannerAgentId(event.target.value)} required>
          {agents.map((agent) => <option key={agent.id} value={agent.id}>{agent.displayName}</option>)}
        </select>
      </label>
      <label>Channel Writer
        <select value={writerAgentId} onChange={(event) => setWriterAgentId(event.target.value)} required>
          {agents.map((agent) => <option key={agent.id} value={agent.id}>{agent.displayName}</option>)}
        </select>
      </label>
      <label>基准 ref（可选）
        <input value={baseRef} onChange={(event) => setBaseRef(event.target.value)} placeholder="默认：最近 Git tag" />
      </label>
      <label className="marketing-check">
        <input type="checkbox" checked={includeUncommitted} onChange={(event) => setIncludeUncommitted(event.target.checked)} />
        包含未提交 diff（草稿会强制标记为未发布）
      </label>
      {agents.length === 0 && <p className="marketing-error">群内没有可用 Agent，请先添加或启用一个 Agent。</p>}
      <div className="marketing-launch-actions">
        <button type="button" onClick={onCancel}>取消</button>
        <button type="submit" disabled={busy || agents.length === 0}>{busy ? "采集证据中…" : "生成宣传内容"}</button>
      </div>
    </form>
  );
}
