//! 设置页（设计 P11）：外观 / 输入行为 / 运行时 / 桌面更新。
//!
//! 主题与发送键沿用既有存储；运行时字段以服务端返回为准。

import { useEffect, useState } from "react";

import { api } from "../api";
import { PageHeader, PageSection, PageShell } from "../components/ui";
import { sendKeyHint, type SendKeyMode } from "../chat/sendKey";
import { THEMES, useTheme } from "../theme";
import type { RuntimeSettings, UpdateCheckResult } from "../types";

function readError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isDesktopRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

interface SettingsPageProps {
  sendKeyMode: SendKeyMode;
  onSendKeyModeChange: (mode: SendKeyMode) => void;
  onError: (message: string) => void;
}

export function SettingsPage({ sendKeyMode, onSendKeyModeChange, onError }: SettingsPageProps) {
  return (
    <PageShell className="settings-page">
      <PageHeader title="设置" description="外观、输入行为、运行时与桌面更新；切换主题不会重置聊天。" />

      <PageSection title="外观" description="七套主题共用同一套语义 token。">
        <AppearanceSection />
      </PageSection>

      <PageSection title="输入行为" description="发送键偏好保存在本机，草稿按账号与群隔离。">
        <div className="settings-radios" role="radiogroup" aria-label="发送键">
          {(["enter", "ctrlEnter"] as SendKeyMode[]).map((mode) => (
            <label key={mode} className={`settings-radio${sendKeyMode === mode ? " is-active" : ""}`}>
              <input
                type="radio"
                name="send-key"
                checked={sendKeyMode === mode}
                onChange={() => onSendKeyModeChange(mode)}
              />
              <span>{mode === "enter" ? "Enter 发送" : "Ctrl/⌘+Enter 发送"}</span>
              <small>{sendKeyHint(mode)}</small>
            </label>
          ))}
        </div>
      </PageSection>

      <PageSection title="运行时" description="仅展示后端 RuntimeSettings 支持的字段；保存失败会保留输入。">
        <RuntimeSection onError={onError} />
      </PageSection>

      {isDesktopRuntime() ? (
        <PageSection title="桌面更新" description="仅在桌面端展示；Web 端不提供安装入口。">
          <UpdateSection onError={onError} />
        </PageSection>
      ) : (
        <PageSection title="桌面更新" description="仅在桌面端可用。">
          <p className="settings-hint">当前是 Web 端，不提供安装入口。</p>
        </PageSection>
      )}
    </PageShell>
  );
}

function AppearanceSection() {
  const { theme, setTheme } = useTheme();
  return (
    <div className="theme-grid">
      {THEMES.map((item) => (
        <button
          key={item.id}
          type="button"
          className={`theme-card${theme === item.id ? " is-active" : ""}`}
          aria-pressed={theme === item.id}
          onClick={() => setTheme(item.id)}
        >
          <span className="theme-swatch" aria-hidden>
            {item.swatch.map((color) => (
              <i key={color} style={{ background: color }} />
            ))}
          </span>
          <strong>{item.name}</strong>
          <small>{item.blurb}</small>
        </button>
      ))}
    </div>
  );
}

function RuntimeSection({ onError }: { onError: (message: string) => void }) {
  const [settings, setSettings] = useState<RuntimeSettings | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "saving">("loading");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let alive = true;
    void (async () => {
      try {
        const value = await api.getSettings();
        if (alive) {
          setSettings(value);
          setStatus("ready");
        }
      } catch (error) {
        if (alive) {
          onError(readError(error));
          setStatus("ready");
        }
      }
    })();
    return () => {
      alive = false;
    };
  }, [onError]);

  if (!settings) return <p className="settings-hint">读取中…</p>;

  const update = (patch: Partial<RuntimeSettings>) => {
    setSettings({ ...settings, ...patch });
    setSaved(false);
  };

  const save = async () => {
    setStatus("saving");
    try {
      const next = await api.updateSettings(settings);
      setSettings(next);
      setSaved(true);
    } catch (error) {
      onError(readError(error));
    } finally {
      setStatus("ready");
    }
  };

  return (
    <div className="settings-form">
      <div className="settings-grid">
        <label>并发运行上限
          <input type="number" min={1} value={settings.maxConcurrentRuns}
            onChange={(e) => update({ maxConcurrentRuns: Number(e.target.value) })} />
        </label>
        <label>单次运行超时（秒）
          <input type="number" min={1} value={settings.runTimeoutSeconds}
            onChange={(e) => update({ runTimeoutSeconds: Number(e.target.value) })} />
        </label>
        <label>项目群上下文消息数
          <input type="number" min={1} value={settings.contextMessageLimit}
            onChange={(e) => update({ contextMessageLimit: Number(e.target.value) })} />
        </label>
        <label>聊天群上下文消息数
          <input type="number" min={1} value={settings.chatContextMessageLimit ?? 12}
            onChange={(e) => update({ chatContextMessageLimit: Number(e.target.value) })} />
        </label>
        <label>最大委派深度
          <input type="number" min={0} value={settings.maxDelegationDepth}
            onChange={(e) => update({ maxDelegationDepth: Number(e.target.value) })} />
        </label>
        <label className="settings-check">
          <input type="checkbox" checked={settings.heartbeatAuto ?? true}
            onChange={(e) => update({ heartbeatAuto: e.target.checked })} />
          自动心跳
        </label>
        <label>前台心跳间隔（秒）
          <input type="number" min={1} value={settings.heartbeatFocusSeconds ?? 15}
            onChange={(e) => update({ heartbeatFocusSeconds: Number(e.target.value) })} />
        </label>
        <label>后台心跳间隔（秒）
          <input type="number" min={1} value={settings.heartbeatBackgroundSeconds ?? 30}
            onChange={(e) => update({ heartbeatBackgroundSeconds: Number(e.target.value) })} />
        </label>
      </div>
      <div className="settings-actions">
        <button type="button" className="pm-btn primary sm" disabled={status === "saving"} onClick={() => void save()}>
          {status === "saving" ? "保存中…" : "保存运行时设置"}
        </button>
        {saved && <span className="settings-hint">已保存并生效。</span>}
      </div>
    </div>
  );
}

function UpdateSection({ onError }: { onError: (message: string) => void }) {
  const [check, setCheck] = useState<UpdateCheckResult | null>(null);
  const [busy, setBusy] = useState<"check" | "install" | null>(null);
  const [installer, setInstaller] = useState<string | null>(null);

  const runCheck = async () => {
    setBusy("check");
    try {
      setCheck(await api.checkForUpdate());
    } catch (error) {
      onError(readError(error));
    } finally {
      setBusy(null);
    }
  };

  const install = async () => {
    if (!check?.downloadUrl) return;
    setBusy("install");
    try {
      const path = await api.downloadUpdate(check.downloadUrl, check.sha256 ?? null);
      setInstaller(path);
    } catch (error) {
      onError(readError(error));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="settings-form">
      <p className="settings-hint">
        {check
          ? check.error
            ? `检查失败：${check.error}`
            : check.hasUpdate
              ? `当前 ${check.currentVersion}，最新 ${check.latestVersion ?? "—"}`
              : `当前 ${check.currentVersion}，已是最新`
          : "尚未检查更新。"}
      </p>
      {check?.notes && <pre className="settings-notes">{check.notes}</pre>}
      <div className="settings-actions">
        <button type="button" className="pm-btn sm" disabled={busy !== null} onClick={() => void runCheck()}>
          {busy === "check" ? "检查中…" : "检查更新"}
        </button>
        {check?.hasUpdate && !installer && (
          <button type="button" className="pm-btn primary sm" disabled={busy !== null} onClick={() => void install()}>
            {busy === "install" ? "下载中…" : "下载并安装"}
          </button>
        )}
        {installer && (
          <button type="button" className="pm-btn primary sm" onClick={() => void api.exitAndInstall(installer)}>
            退出并安装
          </button>
        )}
      </div>
    </div>
  );
}
