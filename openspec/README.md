# OpenSpec 使用约定（ohMyWorkPanel）

本目录是**行为契约的单一事实源（SSOT）**。从 2026-10-21 起，新功能先在这里立 change，再动代码。

## 与既有文档的分工

| 文档 | 回答什么 | 变化频率 |
|---|---|---|
| `docs/version-pipeline.md` | **方向与优先级**：我们要去哪、下一站做什么 | 每次立项 |
| `openspec/specs/**` | **当前行为契约**：系统现在承诺做什么 | 每次归档 change |
| `openspec/changes/**` | **进行中的变更**：这次要改什么、为什么、怎么做 | 一次变更生命周期 |
| `docs/superpowers/**` | 设计过程与实现计划归档（历史 + 深度方案） | 低频 |
| `docs/epitaph/**` | 会话交接（做过什么、坑在哪） | 每次会话 |

规则冲突时：**方向**看 version-pipeline，**行为**看 openspec/specs。

## 目录

```text
openspec/
├── project.md              # 愿景、原则、架构地图、术语
├── specs/
│   ├── README.md           # capability 清单与迁移状态
│   └── <capability>/spec.md
└── changes/
    ├── 0001-agent-playground/
    │   ├── proposal.md     # 为什么做、范围
    │   ├── design.md       # 怎么做、架构决策
    │   ├── tasks.md        # 可勾选的任务清单
    │   └── specs/          # delta specs（ADDED/MODIFIED/REMOVED）
    └── archive/            # 已归档变更（带日期前缀）
```

## 工作流

1. **立项**：在 `changes/NNNN-slug/` 写 `proposal.md`，确定 scope 与 non-goals。
2. **写行为**：需要改行为时补 delta spec（`specs/<capability>/spec.md` 下用 `## ADDED/MODIFIED/REMOVED Requirements`）。
3. **设计**：`design.md` 记录技术路线与权衡；实现细节只写这里，不写进 spec。
4. **拆任务**：`tasks.md` 每条任务都要写清怎么验证。
5. **实现**：代码 + 测试；任务逐条勾选。
6. **归档**：把 delta 合并进 `specs/`，change 移到 `changes/archive/<YYYY-MM-DD>-<slug>/`。

## 写作规则

- Requirement 只写**可观察行为**，用 RFC 2119 关键词（MUST/SHALL/SHOULD/MAY）。
- Scenario 用 `GIVEN / WHEN / THEN / AND`，必须可测。
- 不写类名、函数名、库选型——那是 design 的事。
- 中文正文，标识符与代码路径用英文原样。

## 迁移状态

现有行为契约正在按 capability 从 `docs/superpowers/specs/` 与代码行为增量迁移，清单见 [`specs/README.md`](specs/README.md)。未迁移的 capability 不代表没有契约，只代表契约还散落在旧文档里。
