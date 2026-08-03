import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { Experience, Member } from "./types";

interface ExperiencePanelProps {
  groupId: string;
  members: Member[];
  ownerId: string;
  onError: (msg: string) => void;
}

const fmtTime = (ts: number) =>
  new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).format(ts);

const readErr = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function ExperiencePanel({ groupId, members, ownerId, onError }: ExperiencePanelProps) {
  const [experiences, setExperiences] = useState<Experience[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);

  const load = useCallback(async (q: string) => {
    try {
      setExperiences(await api.queryExperiences(groupId, q.trim() || undefined, 50));
    } catch (e) {
      onError(readErr(e));
    } finally {
      setLoading(false);
    }
  }, [groupId, onError]);

  useEffect(() => { setLoading(true); void load(""); }, [load]);

  const memberName = (id: string) => members.find((m) => m.id === id)?.displayName ?? "未知成员";

  const handleDelete = async (id: string) => {
    if (!confirm("确定删除这条经验？")) return;
    try {
      await api.deleteExperience(id);
      await load(query);
    } catch (e) {
      onError(readErr(e));
    }
  };

  return (
    <div className="pm-panel">
      <div className="pm-body">
        <div className="exp-toolbar">
          <input
            className="pm-input sm exp-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); void load(query); } }}
            placeholder="搜索经验内容…"
          />
          <button className="pm-btn sm" onClick={() => void load(query)}>搜索</button>
          <button className="pm-btn primary sm" onClick={() => setShowForm(!showForm)}>＋ 记录</button>
        </div>
        {showForm && (
          <ExperienceForm
            onSubmit={async (title, content, tags) => {
              try {
                await api.saveExperience({ groupId, sourceMemberId: ownerId, title, content, tags: tags || undefined });
                setShowForm(false);
                await load(query);
              } catch (e) {
                onError(readErr(e));
              }
            }}
            onCancel={() => setShowForm(false)}
          />
        )}
        {loading ? (
          <div className="pm-loading">加载中…</div>
        ) : experiences.length === 0 ? (
          <div className="pm-empty"><p>{query ? "没有匹配的经验" : "还没有沉淀经验"}</p></div>
        ) : (
          <>
            <div className="pm-section-header"><span className="pm-count">共 {experiences.length} 条</span></div>
            {experiences.map((exp) => (
              <div key={exp.id} className="exp-card">
                <div className="exp-card-header">
                  <strong>{exp.title}</strong>
                  <span className="pm-muted">{fmtTime(exp.createdAt)}</span>
                </div>
                <p className="exp-card-content">{exp.content}</p>
                <div className="exp-card-meta">
                  <span className="pm-assignee">👤 {memberName(exp.sourceMemberId)}</span>
                  {exp.tags.split(",").map((t) => t.trim()).filter(Boolean).map((tag) => (
                    <span key={tag} className="pm-tag">{tag}</span>
                  ))}
                  <span className="exp-card-actions">
                    <button className="pm-link danger sm" onClick={() => void handleDelete(exp.id)}>删除</button>
                  </span>
                </div>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}

function ExperienceForm({ onSubmit, onCancel }: {
  onSubmit: (title: string, content: string, tags: string) => void;
  onCancel: () => void;
}) {
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [tags, setTags] = useState("");
  return (
    <div className="pm-inline-form">
      <input className="pm-input" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="经验标题" />
      <textarea className="pm-textarea" rows={3} value={content} onChange={(e) => setContent(e.target.value)} placeholder="经验内容（可复用的做法、坑、结论）" />
      <input className="pm-input sm" value={tags} onChange={(e) => setTags(e.target.value)} placeholder="标签，逗号分隔（可选）" />
      <div className="pm-inline-actions">
        <button className="pm-btn quiet sm" onClick={onCancel}>取消</button>
        <button
          className="pm-btn primary sm"
          disabled={!title.trim() || !content.trim()}
          onClick={() => onSubmit(title.trim(), content.trim(), tags.trim())}
        >保存</button>
      </div>
    </div>
  );
}
