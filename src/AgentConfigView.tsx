import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import type { AgentEnvStatus, AgentConfigBundle, ImportReport } from "./api-web";

interface Props {
  onError: (msg: string) => void;
  onStatusChange?: (hasBundle: boolean) => void;
}

const CLI_LABELS: Record<string, string> = {
  codex: "Codex CLI",
  cursor: "Cursor CLI",
  claude: "Claude Code",
  opencode: "OpenCode",
  dsh: "DeepSeek Harness",
};

function download(name: string, content: string) {
  const blob = new Blob([content], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result ?? ""));
    r.onerror = () => reject(new Error("读取文件失败"));
    r.readAsText(file);
  });
}

/** Agent 配置中心：一键导入 / 导出配置包 / 环境自检 / CLI 自动安装（仅管理员）。 */
export function AgentConfigView({ onError, onStatusChange }: Props) {
  const [status, setStatus] = useState<AgentEnvStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [importText, setImportText] = useState("");
  const [autoInstall, setAutoInstall] = useState(true);
  const [importing, setImporting] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [exportText, setExportText] = useState("");
  const [exporting, setExporting] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await api.agentConfigStatus();
      setStatus(s);
      onStatusChange?.(Boolean(s.bundleImportedAt));
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [onError, onStatusChange]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const doInstall = async (cli: string) => {
    setInstalling(cli);
    try {
      const out = await api.agentConfigInstall(cli);
      if (!out.ok) onError(`${CLI_LABELS[cli] ?? cli} 安装未成功：${out.detail}`);
      void refresh();
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setInstalling(null);
    }
  };

  const parseBundle = (): AgentConfigBundle | null => {
    const text = importText.trim();
    if (!text) {
      onError("请先粘贴或选择一份 Agent 配置包（JSON）。");
      return null;
    }
    try {
      const obj = JSON.parse(text);
      if (typeof obj !== "object" || obj === null || Array.isArray(obj)) {
        throw new Error("配置包应为 JSON 对象");
      }
      return obj as AgentConfigBundle;
    } catch (e: unknown) {
      onError(`配置包 JSON 解析失败：${e instanceof Error ? e.message : String(e)}`);
      return null;
    }
  };

  const doImport = async () => {
    const bundle = parseBundle();
    if (!bundle) return;
    setImporting(true);
    setReport(null);
    try {
      const rep = await api.agentConfigImport(bundle, autoInstall);
      setReport(rep);
      void refresh();
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setImporting(false);
    }
  };

  const doExport = async (includeSecrets: boolean) => {
    setExporting(true);
    try {
      const bundle = await api.agentConfigExport(includeSecrets);
      const text = JSON.stringify(bundle, null, 2);
      setExportText(text);
      download(
        `linlis-agent-config-${includeSecrets ? "with-secrets" : "no-secrets"}-${Date.now()}.json`,
        text,
      );
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setExporting(false);
    }
  };

  const onPickFile = async (f: File | null) => {
    if (!f) return;
    try {
      setImportText(await readFileAsText(f));
    } catch (e: unknown) {
      onError(e instanceof Error ? e.message : String(e));
    }
  };

  if (loading) {
    return <div className="project-workflow agent-config-view"><p className="wf-hint">正在检测 Agent 环境…</p></div>;
  }

  const knownClis = (status?.clis ?? []).filter((c) => Boolean(CLI_LABELS[c.cli]));

  return (
    <div className="project-workflow agent-config-view">
      <section className="wf-section">
        <h2>环境自检</h2>
        <p className="wf-hint">
          检测本机（服务器）已安装的 Agent CLI、Node.js 与 Codex shim（
          <code>127.0.0.1:{status?.shimPort ?? 18888}</code>
          ）。缺的 CLI 可点「自动安装」（best-effort，官方方式）。
        </p>
        <div className="room-grid">
          <div className={`room-card ${status?.nodePath ? "ok" : "warn"}`}>
            <div className="room-card-title">
              <span className="status-dot" />
              Node.js
            </div>
            <div className="room-card-detail">
              {status?.nodePath ? status.nodePath : "未找到（Codex shim / CLI 安装依赖 node）"}
            </div>
          </div>
          <div className={`room-card ${status?.shimUp ? "ok" : "warn"}`}>
            <div className="room-card-title">
              <span className="status-dot" />
              Codex shim :{status?.shimPort ?? 18888}
            </div>
            <div className="room-card-detail">
              {status?.shimUp
                ? "就绪（面板内嵌，启动时自动拉起）"
                : "未监听——启动后自动拉起；若仍失败，请确认 node 与 scripts/codex-deepseek-proxy.cjs" }
            </div>
          </div>
          <div className={`room-card ${status?.codexKeySet ? "ok" : "warn"}`}>
            <div className="room-card-title">
              <span className="status-dot" />
              Codex API Key
            </div>
            <div className="room-card-detail">
              {status?.codexKeySet ? "已配置（成员/环境/ ~/.codex/auth.json）" : "缺失——导入含密钥的配置包即可"}
            </div>
          </div>
          <div className={`room-card ${status?.bundleImportedAt ? "ok" : ""}`}>
            <div className="room-card-title">
              <span className="status-dot" />
              已导入配置包
            </div>
            <div className="room-card-detail">
              {status?.bundleImportedAt
                ? new Date(status.bundleImportedAt).toLocaleString("zh-CN")
                : "尚未导入；启动后自动重放：" + (status?.autoApply ? "已开启" : "已关闭")}
            </div>
          </div>
          {knownClis.map((c) => (
            <div key={c.cli} className={`room-card ${c.present ? "ok" : "warn"}`}>
              <div className="room-card-title">
                <span className="status-dot" />
                {CLI_LABELS[c.cli]}{" "}
                <code>{c.cli}</code>
              </div>
              <div className="room-card-detail cls-row">
                <span>{c.present ? (c.path || "已安装") : "未安装"}</span>
                {!c.present && (
                  <button
                    className="pm-btn sm"
                    disabled={installing === c.cli}
                    onClick={() => void doInstall(c.cli)}
                  >
                    {installing === c.cli ? "安装中…" : "自动安装"}
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="wf-section">
        <h2>一键导入（本地 / 新安装）</h2>
        <p className="wf-hint">
          粘贴从服务器导出的配置包（<code>linlis-agent-config-*.json</code>），或选择该文件。
          导入会：写 <code>~/.codex</code>、<code>~/.claude</code>、<code>~/.cursor</code>、
          通用 <code>files</code>（备份后合并）→ 同步成员（agent_profiles）→ 持久化并随启动自动重放。
        </p>
        <textarea
          className="agent-config-textarea"
          rows={7}
          spellCheck={false}
          placeholder='粘贴 JSON，例如 {"codex":{"enabled":true,"apiKey":"sk-…","model":"deepseek-v4-flash"},…}'
          value={importText}
          onChange={(e) => setImportText(e.target.value)}
        />
        <div className="wf-ws-edit">
          <button className="pm-btn sm quiet" onClick={() => fileRef.current?.click()}>
            选择文件…
          </button>
          <input
            ref={fileRef}
            type="file"
            accept=".json,application/json"
            style={{ display: "none" }}
            onChange={(e) => void onPickFile(e.target.files?.[0] ?? null)}
          />
          <label className="settings-check">
            <input
              type="checkbox"
              checked={autoInstall}
              onChange={(e) => setAutoInstall(e.target.checked)}
            />
            导入时自动安装缺失的 CLI
          </label>
          <button className="pm-btn primary" disabled={importing} onClick={() => void doImport()}>
            {importing ? "正在配置…" : "一键导入并配置"}
          </button>
        </div>

        {report && (
          <div className={`agent-config-report ${report.ok ? "ok" : "err"}`}>
            <h4>导入结果：{report.ok ? "成功" : "部分失败"}（装 {report.installed.length} · 缺 {report.missing.length}）</h4>
            <ol>
              {report.steps.map((s, i) => (
                <li key={i} className={`step-${s.status}`}>
                  <strong>{s.name}</strong>
                  <span className="step-badge">{s.status}</span>
                  <div className="step-detail">{s.detail}</div>
                </li>
              ))}
            </ol>
            {report.warnings.length > 0 && (
              <p className="wf-hint">提醒：{report.warnings.join("；")}</p>
            )}
          </div>
        )}
      </section>

      <section className="wf-section">
        <h2>导出配置包（在服务器 / 已 vibecoding 好的环境执行一次）</h2>
        <p className="wf-hint">
          把当前环境的 Agent 配置收敛成一份可移植包：codex（base_url / model / key）、claude
          （base_url / token）、cursor（cli-config / mcp）、成员映射。到本地「一键导入」即可。
        </p>
        <div className="wf-ws-edit">
          <button className="pm-btn primary" disabled={exporting} onClick={() => void doExport(true)}>
            {exporting ? "导出中…" : "导出配置包（含密钥）"}
          </button>
          <button className="pm-btn sm quiet" disabled={exporting} onClick={() => void doExport(false)}>
            导出（不含密钥）
          </button>
        </div>
        {exportText && (
          <textarea
            className="agent-config-textarea"
            rows={6}
            readOnly
            spellCheck={false}
            value={exportText}
          />
        )}
      </section>

      <section className="wf-section">
        <h2>使用说明（release 开箱即用）</h2>
        <ol className="wf-ol">
          <li><strong>服务器（首台）</strong>：在已 vibecoding 配好 Agent 的 ECS 上登录面板 → 「Agent 配置」→ 「导出配置包（含密钥）」→ 保存 <code>linlis-agent-config-with-secrets.json</code>。</li>
          <li><strong>本地 / 新机器</strong>：安装面板 release → 「Agent 配置」→ 粘贴该包 → 「一键导入并配置」→ 缺失 CLI 自动安装，缺哪个就点「自动安装」。</li>
          <li>此后每次启动会 <strong>自动重放</strong>（幂等补写缺失配置），新用户无需重新 vibecoding。</li>
          <li>扩展性：未知 CLI 可用包内 <code>files</code>（home 相对路径 → 内容）额外携带配置；密钥字段不含密钥时导入不会误写。</li>
        </ol>
      </section>
    </div>
  );
}
