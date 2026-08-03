import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { DirListing } from "./types";

interface Props {
  value: string;
  onChange: (path: string) => void;
  onError?: (msg: string) => void;
}

export function ServerPathPicker({ value, onChange, onError }: Props) {
  const [listing, setListing] = useState<DirListing | null>(null);
  const [loading, setLoading] = useState(false);
  const [manual, setManual] = useState(value);

  const load = useCallback(async (path: string) => {
    setLoading(true);
    try {
      const result = await api.listServerDir(path || "/");
      setListing(result);
      setManual(result.path);
    } catch (e: unknown) {
      onError?.(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [onError]);

  useEffect(() => {
    void load(value || "/");
  }, []); // eslint-disable-line react-hooks/exhaustive-deps -- initial browse only

  const crumbs = (listing?.path ?? "/").split("/").filter(Boolean);

  return (
    <div className="server-path-picker">
      <div className="path-input">
        <input
          value={manual}
          onChange={(e) => setManual(e.target.value)}
          placeholder="服务器绝对路径，例如 /AI/LinlisWorkPanel"
          required
        />
        <button type="button" onClick={() => { onChange(manual.trim()); void load(manual.trim() || "/"); }}>
          打开
        </button>
      </div>
      <div className="path-crumbs">
        <button type="button" className="crumb" onClick={() => void load("/")}>/</button>
        {crumbs.map((part, i) => {
          const path = "/" + crumbs.slice(0, i + 1).join("/");
          return (
            <button key={path} type="button" className="crumb" onClick={() => void load(path)}>
              {part}
            </button>
          );
        })}
        {loading && <span className="path-loading">加载中…</span>}
      </div>
      <div className="path-entries">
        {listing?.parent != null && (
          <button type="button" className="path-entry up" onClick={() => void load(listing.parent!)}>
            .. 上级
          </button>
        )}
        {listing?.entries.filter((e) => e.isDir).map((entry) => (
          <button
            key={entry.path}
            type="button"
            className="path-entry"
            onClick={() => void load(entry.path)}
            onDoubleClick={() => { onChange(entry.path); void load(entry.path); }}
          >
            📁 {entry.name}
          </button>
        ))}
      </div>
      <div className="path-confirm">
        <code>{listing?.path ?? manual}</code>
        <button
          type="button"
          className="pm-btn primary sm"
          onClick={() => onChange((listing?.path ?? manual).trim())}
        >
          使用此目录
        </button>
      </div>
      <p className="form-hint">仅可选择服务器上已存在的目录（Agent 在该机器上执行）。</p>
    </div>
  );
}
