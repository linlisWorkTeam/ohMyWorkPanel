# Self-Marketing 参考

## 状态

| 状态 | 含义 |
|---|---|
| `collecting` | 正在冻结 Git 证据快照 |
| `planning` | Planner 正在判断传播价值并生成 Brief |
| `writing` | Writer 正在生成五渠道草稿 |
| `awaiting_user` | 校验通过，等待人类审核 |
| `changes_requested` | 校验阻断或用户已要求修改 |
| `approved` | 有权限的人已批准，可导出 |
| `no_content` | 没有候选更新，或 Planner 判断不值得宣传 |
| `failed` | 采集、Agent 输出解析或运行失败 |

## 证据和 Brief 契约

`RepositorySnapshot` 固定记录 `baseRef`、`headRef`、`sourceMode`、commit、文件路径、受限证据摘录和内容 hash。每个 `Evidence` 都有稳定的本次快照内 ID（如 `ev-001`）及 `committed` / `unreleased` 状态。

Planner 必须返回 `ContentBrief`：

```json
{
  "schemaVersion": 1,
  "campaignId": "campaign-id",
  "publishability": "publish",
  "reason": "为什么值得传播",
  "audience": ["目标读者"],
  "coreMessage": "一条可验证的核心信息",
  "updates": [{
    "id": "update-1",
    "title": "更新标题",
    "summary": "事实描述",
    "userValue": "实际价值",
    "evidenceRefs": ["ev-001"],
    "releaseState": "committed"
  }],
  "proofPoints": [{
    "id": "proof-1",
    "text": "证据化要点",
    "evidenceRefs": ["ev-001"]
  }],
  "doNotClaim": ["证据无法支持的说法"],
  "channelAngles": {
    "xiaohongshu": "角度",
    "x": "角度",
    "zhihu": "角度",
    "bilibili": "角度",
    "github_release": "角度"
  }
}
```

`publishability=publish` 仅表示值得生成供用户审核的草稿，并不表示内容或版本已经发布。已提交但未发版的开发进展可以进入草稿阶段，但必须保留 `committed` / `unreleased` 边界；`hold` 表示当前证据或时机不足、暂不生成草稿，`no_content` 表示没有值得传播的更新。

Writer 返回 `DraftBundle`，必须恰好覆盖 `xiaohongshu`、`x`、`zhihu`、`bilibili`、`github_release`。每个草稿的 `claimRefs` 只能引用同一 Brief 内的 update / proof point ID。

## 权限与生命周期

- 创建、读取和要求修改：当前群成员。
- 批准：群管理员、群主或 campaign 发起人。
- 修改请求复用冻结的 Snapshot 和 Brief，只新建 Writer run。
- 批准是产品内审核状态，不会调用任何外部平台 API。
- Campaign 运行态和审核记录保存在 SQLite `content_campaigns`；项目配置仍属于项目仓库。

Web 路由见 [Web API 索引](../api-web.md)，实现规格与边界见 [Self-Marketing MVP 设计规格](../superpowers/specs/2026-09-01-self-marketing-mvp.md)。
