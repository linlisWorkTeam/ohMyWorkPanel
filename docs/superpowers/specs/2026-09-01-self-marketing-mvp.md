---
date: 2026-09-01
topic: self-marketing-mvp
status: accepted
version: v2.2.0
track: H-project-communication
---

# Self-Marketing MVP 设计规格

## 目标与边界

让绑定 Git 工作区的工作群把“最近发生了什么”转换为可审核、可追溯的对外内容。MVP 只做：采集仓库证据、判断是否值得传播、生成统一 Content Brief、生成五类渠道草稿、确定性校验、群聊人工审核和 Markdown 导出。

不做平台账号连接、自动发布、定时任务、互动数据回流、A/B 测试、增长 CRM，也不引入新的通用 DAG/工作流引擎。

默认只把已提交内容当作公开事实。用户可显式选择包含工作区 diff，但该部分必须标记为 `unreleased`，不得写成已发布。

## 现有能力复用

- Group 的 `workspace_path`、成员和管理员权限作为 campaign 作用域。
- 现有 Adapter 调度、`task_runs`、流式消息与 A2A 能力承载 Planner / Writer 执行。
- 群聊消息承载发起、进度和审核结果，不新增第二套协作界面。
- SQLite 迁移、Web/Tauri 双 API、前端右栏扩展和语义主题 token 沿用现有约束。
- `git_inspect` 现有 Git 探测只作底层参考；Self-Marketing 使用受限、可测试的独立 collector，不扩大任意 shell 能力。

## 最小编排

```text
用户在群聊发起
       │
       ▼
Context Collector ──无可宣传更新──► no_content
       │ RepositorySnapshot + Evidence[]
       ▼
Content Planner ──► ContentBrief（所有事实带 evidence_refs）
       │
       ▼
Channel Writer ──► 5 个 ChannelDraft
       │
       ▼
Deterministic Validator ──失败──► changes_requested
       │通过
       ▼
awaiting_user ──要求修改──► Writer ──► Validator
       │批准
       ▼
approved ──► Markdown export
```

Planner 与 Writer 是产品角色，不要求新的执行引擎；MVP 可由同一群成员承担两个角色，但保存时仍记录各阶段 run id，后续可无迁移地拆成不同 Agent。Reviewer 在 MVP 是确定性校验器加人类终审，不增加第三个 LLM Agent。

## 数据归属

```text
<workspace>/
  docs/marketing/
    project-context.md       # 项目定位、受众、边界，可版本控制
    brand-guide.md           # 语气、禁用词、承诺边界，可版本控制
    channels/
      xiaohongshu.md
      x-twitter.md
      zhihu.md
      bilibili-script.md
      github-release.md
  .ohmyworkpanel/
    marketing/exports/       # 已批准内容导出，默认被项目忽略
```

运行态 campaign、快照 JSON、草稿、校验结果和审核记录保存在 ohMyWorkPanel SQLite。项目配置若不存在则使用内置保守模板；MVP 不擅自写入用户仓库。

## 状态模型

```ts
type CampaignStatus =
  | "collecting"
  | "planning"
  | "writing"
  | "validating"
  | "awaiting_user"
  | "changes_requested"
  | "approved"
  | "no_content"
  | "failed";

interface ContentCampaign {
  id: string;
  groupId: string;
  status: CampaignStatus;
  sourceMode: "committed" | "include_uncommitted";
  baseRef?: string;
  headRef: string;
  snapshot: RepositorySnapshot;
  brief?: ContentBrief;
  drafts: ChannelDraft[];
  validation: ValidationFinding[];
  plannerRunId?: string;
  writerRunId?: string;
  requestedBy: string;
  feedbackBy?: string;
  approvedBy?: string;
  createdAt: string;
  updatedAt: string;
}
```

MVP 使用一个新增 `content_campaigns` 表保存上述结构中的稳定列和 JSON payload，不拆成多表，不改现有表语义。

## Content Brief Schema

```json
{
  "schemaVersion": 1,
  "campaignId": "uuid",
  "publishability": "publish|hold|no_content",
  "reason": "为什么值得或不值得宣传",
  "audience": ["目标读者"],
  "coreMessage": "一条可验证的核心信息",
  "updates": [
    {
      "title": "更新标题",
      "summary": "事实描述",
      "userValue": "对用户的实际价值",
      "evidenceRefs": ["ev-001"],
      "releaseState": "released|committed|unreleased"
    }
  ],
  "proofPoints": [
    {"id": "proof-001", "text": "证据化要点", "evidenceRefs": ["ev-001"]}
  ],
  "doNotClaim": ["不能从证据推出的结论"],
  "channelAngles": {
    "xiaohongshu": "角度",
    "x": "角度",
    "zhihu": "角度",
    "bilibili": "角度",
    "github_release": "角度"
  }
}
```

`RepositorySnapshot` 保存 `repositoryRoot`，其 `evidence[]` 至少包含 `id`、`kind`、`path/ref`、`excerpt`、`content_hash`、`release_state`。`ChannelDraft` 保存去重后的 `claim_refs`，引用 Content Brief 中实际使用的 update/proof point；Reviewer 会逐一验证其证据链。MVP 不做逐句 span 标注，避免为了标注系统而扩大范围。

## 事实与风格护栏

1. Collector 使用路径白名单、字节上限、commit 数上限和命令超时；忽略密钥、vendor、构建产物和二进制文件。
2. Snapshot 完成后冻结；修改请求默认复用同一快照，用户显式刷新才重新采集。
3. Planner 输出严格 JSON；所有 update/proof point 必须有有效 evidence ref。
4. Writer 只能使用 brief 中的事实；每个 draft 的 claim ref 必须能回溯到 brief 和 snapshot。
5. Validator 检查 schema、悬空引用、渠道长度、禁用词、绝对化承诺、未提交内容措辞和必要免责声明。
6. 内置禁用表达包含“颠覆、革命性、行业第一、完美、彻底解决、零风险”等；Brand Guide 可追加。
7. 允许并鼓励 `no_content`。没有用户价值或证据不足时，不为了产出而宣传。
8. LLM 校验结果不等于批准；只有有权限的人类能把状态改为 `approved`。

## 双 API 与权限

新增 API 必须同时覆盖 Tauri command 与 Web 路由，保持现有 API 不变：

- `POST /api/groups/{group_id}/marketing/campaigns`
- `GET /api/groups/{group_id}/marketing/campaigns`
- `GET /api/marketing/campaigns/{campaign_id}`
- `POST /api/marketing/campaigns/{campaign_id}/revise`
- `POST /api/marketing/campaigns/{campaign_id}/approve`
- `GET /api/marketing/campaigns/{campaign_id}/export`

创建和修改请求要求群成员；批准要求群管理员或 campaign 发起人。所有写操作记录 actor 与时间。导出只读返回 Markdown；写入 workspace export 作为后续增强。

## 用户流程

1. 用户在工作群点击“宣传”或输入 `/market`，选择基准范围（默认最近一个 tag/最近 20 commits）和是否包含未提交 diff。
2. 系统展示证据采集摘要；无内容则在群聊说明原因。
3. Planner 生成 brief，Writer 生成五渠道草稿，Validator 给出阻断项和警告。
4. 群聊出现 campaign 卡片；用户查看 Content Brief 与各渠道草稿。
5. 用户填写修改意见，Writer 基于冻结 brief 重写；若要求改变事实，必须刷新 snapshot/brief。
6. 用户批准后导出单一 Markdown 包。MVP 到此结束，不代替用户发布。

## 验收标准

- 在真实 Git 工作区可生成有 hash 的受限 snapshot，且敏感/超大文件不会进入 payload。
- 无值得宣传更新时稳定返回 `no_content`。
- 五渠道草稿都能追溯到同一 brief；悬空证据、夸大词和未发布误述会阻止批准。
- 群聊中可完成发起、查看、要求修改、批准、导出。
- Web 与 Tauri 路径行为等价；旧群聊、调度、SQLite 数据兼容。
- 前端通过主题颜色、窄容器与触控门禁；后端和前端新增单元/集成测试。

## 后续路线

- P1：GitHub App / `gh` 的 PR、Release 证据；可保存项目级模板编辑 UI。
- P2：显式平台连接、草稿箱投递、逐渠道审批与幂等发布记录。
- P3：抓取曝光/互动指标，以只读 feedback snapshot 进入下一次 brief。
- P4：定时触发、内容日历、实验和归因；仍保留事实追溯、人类审批和平台级 kill switch。
