import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { PageShell } from "../components/ui";
import type { LogEntry, LogLevel } from "../types";

const PAGE = 50;

const LEVEL_LABEL: Record<string, string> = { debug: "调试", info: "信息", warn: "警告", error: "错误" };

const fmtTime = (ts: number) =>
  new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(ts);

const readErr = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function LogsPanel({ onError }: { onError: (msg: string) => void }) {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [level, setLevel] = useState<LogLevel | "">("");
  const [source, setSource] = useState("");
  const [loading, setLoading] = useState(true);
  const [expanded, setExpanded] = useState<string | null>(null);

  const load = useCallback(async (offset: number, append: boolean) => {
    try {
      const [entries, count] = await Promise.all([
        api.listLogs({ limit: PAGE, offset, level: level || undefined, source: source.trim() || undefined }),
        api.countLogs(level || undefined, source.trim() || undefined),
      ]);
      setLogs((prev) => (append ? [...prev, ...entries] : entries));
      setTotal(count.count);
    } catch (e) {
      onError(readErr(e));
    } finally {
      setLoading(false);
    }
  }, [level, source, onError]);

  useEffect(() => { setLoading(true); void load(0, false); }, [load]);

  const handleClear = async () => {
    if (!confirm("确定清空所有日志？此操作不可恢复。")) return;
    try {
      await api.clearLogs();
      await load(0, false);
    } catch (e) {
      onError(readErr(e));
    }
  };

  return (
    <PageShell className="pm-panel" density="compact">
      <div className="pm-body">
        <div className="exp-toolbar">
          <select className="pm-select" value={level} onChange={(e) => setLevel(e.target.value as LogLevel | "")}>
            <option value="">全部级别</option>
            <option value="debug">调试</option>
            <option value="info">信息</option>
            <option value="warn">警告</option>
            <option value="error">错误</option>
          </select>
          <input
            className="pm-input sm exp-search"
            value={source}
            onChange={(e) => setSource(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); setLoading(true); void load(0, false); } }}
            placeholder="按来源过滤…"
          />
          <button className="pm-btn sm" onClick={() => { setLoading(true); void load(0, false); }}>刷新</button>
          <button className="pm-btn sm log-clear" onClick={() => void handleClear()}>清空</button>
        </div>
        {loading ? (
          <div className="pm-loading">加载中…</div>
        ) : logs.length === 0 ? (
          <div className="pm-empty"><p>暂无日志</p></div>
        ) : (
          <>
            <div className="pm-section-header"><span className="pm-count">共 {total} 条{total > logs.length ? `，已加载 ${logs.length}` : ""}</span></div>
            {logs.map((entry) => (
              <div key={entry.id} className="log-row">
                <div className="log-row-head">
                  <span className={`log-badge log-${entry.level}`}>{LEVEL_LABEL[entry.level] ?? entry.level}</span>
                  <span className="log-source">{entry.source}</span>
                  <span className="pm-muted">{fmtTime(entry.createdAt)}</span>
                </div>
                <div className="log-message">{entry.message}</div>
                {entry.details && (
                  expanded === entry.id ? (
                    <>
                      <pre className="log-details">{entry.details}</pre>
                      <button className="pm-link sm" onClick={() => setExpanded(null)}>收起</button>
                    </>
                  ) : (
                    <button className="pm-link sm" onClick={() => setExpanded(entry.id)}>详情</button>
                  )
                )}
              </div>
            ))}
            {logs.length < total && (
              <button className="pm-btn sm log-more" onClick={() => void load(logs.length, true)}>加载更多</button>
            )}
          </>
        )}
      </div>
    </PageShell>
  );
}
