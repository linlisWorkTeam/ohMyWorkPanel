import { FormEvent, useEffect, useState } from "react";
import { api, setAuthToken } from "../api";
import { saveAuthUser } from "./authSession";
import type { InvitePreview } from "../types";
import { Brand, ThemeSwitcher } from "../theme";

function readError(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  if (typeof reason === "string") return reason;
  return "操作失败";
}

export function parseInviteTokenFromPath(pathname: string): string | null {
  const m = pathname.match(/^\/invite\/([^/]+)\/?$/);
  return m?.[1] ?? null;
}

export function InviteLanding({
  token,
  onDone,
}: {
  token: string;
  onDone: () => void;
}) {
  const [preview, setPreview] = useState<InvitePreview | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [mode, setMode] = useState<"login" | "register">("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    void api
      .getInvitePreview(token)
      .then((p) => {
        if (!disposed) setPreview(p);
      })
      .catch((reason) => {
        if (!disposed) setLoadError(readError(reason));
      });
    return () => {
      disposed = true;
    };
  }, [token]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!preview?.valid) return;
    setBusy(true);
    setError(null);
    try {
      const auth =
        mode === "login"
          ? await api.login(username.trim(), password)
          : await api.register(username.trim(), password);
      setAuthToken(auth.token);
      saveAuthUser({
        userId: auth.user_id,
        username: auth.username,
        isAdmin: Boolean(auth.isAdmin ?? auth.is_admin),
      });
      await api.acceptInvite(token);
      window.history.replaceState({}, "", "/");
      onDone();
    } catch (reason) {
      setError(readError(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="auth-screen invite-landing">
      <header className="auth-header">
        <Brand />
        <ThemeSwitcher />
      </header>
      <div className="auth-card">
        <h1>加入群组</h1>
        {loadError && <p className="error">{loadError}</p>}
        {preview && !preview.valid && (
          <p className="error">{preview.reason || "邀请无效"}</p>
        )}
        {preview?.valid && (
          <>
            <p>
              邀请你以 <strong>{preview.displayName}</strong> 加入
              「{preview.groupName}」
            </p>
            <p className="muted">
              链接有效期至{" "}
              {new Date(preview.expiresAt).toLocaleString("zh-CN")}
            </p>
            <div className="auth-tabs">
              <button
                type="button"
                className={mode === "login" ? "active" : ""}
                onClick={() => setMode("login")}
              >
                登录后加入
              </button>
              <button
                type="button"
                className={mode === "register" ? "active" : ""}
                onClick={() => setMode("register")}
              >
                注册后加入
              </button>
            </div>
            <form onSubmit={submit}>
              <input
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="用户名"
                autoComplete="username"
                required
              />
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="密码"
                autoComplete={mode === "login" ? "current-password" : "new-password"}
                required
              />
              {error && <p className="error">{error}</p>}
              <button type="submit" disabled={busy}>
                {busy ? "处理中…" : mode === "login" ? "登录并加入" : "注册并加入"}
              </button>
            </form>
          </>
        )}
      </div>
    </div>
  );
}
