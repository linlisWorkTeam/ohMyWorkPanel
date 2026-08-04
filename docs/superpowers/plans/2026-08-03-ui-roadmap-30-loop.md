# Loop plan: UI polish + Roadmap completeness (30 × 20min)

## Goal
好看、交互强、易用的界面；完整 roadmap，使 Agent 能按路线图 checklist 自行推进并完成任务。

## Constraints
- 只部署灰度 `:8081`；不碰生产 `data/`
- 每 tick 一个可验证切片；`pnpm run test:gate` 后 `deploy-canary`（或 skip 若未改后端）
- 进度：`.linlis/loop/ui-roadmap-30.json`

## Tick themes (adaptive)
1. Roadmap 条：进度%、当前 checklist 光标、编排状态即时刷新
2–8. 项目视图视觉系统（tokens / 动效 / 空态 / 响应式）
9–16. PmPanel：功能↔路线图绑定、checklist 可操作性、一键启动编排
17–24. Orchestrator 完备：失败恢复、指派缺失提示、完成收尾、WS 局部更新
25–30. 端到端冒烟 + 打磨 + 墓志铭

## Done when
- 项目视图可一眼看懂进度与当前任务
- 绑定 feature/task 后点「启动」能串行派发 @Agent 直至路线图项 done
- 暂停/失败/继续/取消行为清晰
