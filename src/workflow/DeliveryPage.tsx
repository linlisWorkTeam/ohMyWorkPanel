//! 交付页（设计 P06）：次级导航「路线图 / 版本」。
//!
//! 路线图复用 PmPanel（含功能看板），版本复用重构后的 VersionView（列表 + 详情）。

import { useState } from "react";

import type { Group, Member } from "../types";
import { PmPanel } from "./PmPanel";
import { VersionView } from "./VersionView";

type DeliveryTab = "roadmap" | "versions";

interface DeliveryPageProps {
  group: Group;
  members: Member[];
  senderMemberId: string | null;
  canManage: boolean;
  onError: (msg: string) => void;
  onGotoChat?: () => void;
}

export function DeliveryPage({ group, members, senderMemberId, canManage, onError, onGotoChat }: DeliveryPageProps) {
  const [tab, setTab] = useState<DeliveryTab>("versions");

  return (
    <div className="delivery-page">
      <header className="delivery-head">
        <div>
          <h2>交付</h2>
          <p>路线图与版本流水线；版本 Release 不等于部署，生产晋升仍需管理员批准。</p>
        </div>
        <nav className="delivery-nav" aria-label="交付分区">
          <button
            type="button"
            className={`delivery-tab${tab === "roadmap" ? " is-active" : ""}`}
            aria-current={tab === "roadmap" ? "page" : undefined}
            onClick={() => setTab("roadmap")}
          >
            路线图
          </button>
          <button
            type="button"
            className={`delivery-tab${tab === "versions" ? " is-active" : ""}`}
            aria-current={tab === "versions" ? "page" : undefined}
            onClick={() => setTab("versions")}
          >
            版本
          </button>
        </nav>
      </header>

      <div className="delivery-body">
        {tab === "roadmap" ? (
          <PmPanel
            groupId={group.id}
            members={members}
            adminMemberId={group.adminMemberId}
            onError={onError}
          />
        ) : (
          <VersionView
            group={group}
            members={members}
            senderMemberId={senderMemberId}
            canManage={canManage}
            onError={onError}
            onGotoChat={onGotoChat}
          />
        )}
      </div>
    </div>
  );
}
