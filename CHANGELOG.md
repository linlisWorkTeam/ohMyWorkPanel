# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

<!-- Add unreleased changes here. -->

### Changed

<!-- Add unreleased changes here. -->

### Fixed

<!-- Add unreleased changes here. -->

## [2.1.2] - 2026-08-24

### Added

- Added desktop update checks, SHA-256 verified downloads, and silent current-user installer handoff.
- Added reusable Modal, FormField, ButtonGroup, Badge, Toast, ContextMenu, and responsive page-layout primitives.

### Changed

- Unified all seven themes behind one semantic token contract and migrated legacy style colors to theme-aware tokens.
- Added mobile, desktop, web, container-query, and coarse-pointer behavior for reusable pages and overlays.
- Added a zero-dependency color-purity gate; non-theme source files now allow zero literal colors.

### Fixed（用户验收）

- 桌面端版本页/Wave 工作流不再「仅 Web 服务可用」：新增 12 个 Tauri 命令桥（版本板/Ask/Wave/播放/发布），语义与 web 一致
- Windows 文件系统适配：工作区不再落 `\\?\` verbatim 前缀；群工作区无效报错给出重选提示；ServerPathPicker 支持 `\` 分隔、平台化占位符、本机原生目录选择按钮
- 右键菜单：失败回复可复制（复制可见内容或错误信息）；成员「切换模型」改为二级子菜单
- 建群/卡片弹窗限高可滚动
- `linlis-super-harness` 加入系统预制角色（dsh 适配器）
- chatbot：可自定义 apiUrl + apiKey（provider=custom，OpenAI 兼容）；真正多轮对话（system+历史轮+当前消息）；默认模型统一 `deepseek-chat`（修复 opencode-go / deepseek 官方 API 均 400 无法触发的问题）；模型支持自由输入（表单可编辑 + 候选列表；右键「自定义模型…」）；迁移 v4 增加 `agent_profiles.api_url`
- chatbot 收到网页 HTML 时给出明确指引（API 地址应填 OpenAI 兼容端点如 `https://api.xxx.com/v1`，自动拼 /chat/completions；Gemini/Claude 原生 API 不适用）；自定义 API 地址保存时校验必须 http(s):// 开头；未以 /v1 结尾自动回退尝试 `{url}/v1/chat/completions`；报错信息含实际请求 URL
- **已加入的机器人可修改配置**：右键成员「编辑 API Key…」「编辑 API 地址…」（支持清除）；新增 `set_member_api_key` 命令
- vitest 排除 output/ 冻结发布快照（此前版本锁测试被快照副本误扫）

## [2.1.1] - 2026-08-23

### Added

- Added the v2.1 shell with a WeChat-style chat layout, a dockable right rail, and seven themes.
- Added CLI adapter catalog discovery through `*.adapter.json` manifests, with shell argument rejection.
- Added visual Quick Start documentation with real UI screenshots.
- Added database migrations, HTTP ACL coverage, request IDs, DTO contract tests, and shared cancel/retry services.

### Changed

- Renamed the project and repository to `ohMyWorkPanel`.
- Refined the shell, settings, group, member, message, and agent interaction surfaces.
- Consolidated agent API-key handling and encrypted newly stored keys with a local machine key.

### Fixed

- Fixed web bundle startup failures that could leave the application with a blank root element.
- Fixed UTF-8 encoding in repository documentation and release assets.

## [2.0.0] - 2026-08-21

<!-- See the v2.0.0 GitHub Release for the complete release notes. -->

[unreleased]: https://github.com/linlisWorkTeam/ohMyWorkPanel/compare/v2.1.2...HEAD
[2.1.2]: https://github.com/linlisWorkTeam/ohMyWorkPanel/releases/tag/v2.1.2
[2.1.1]: https://github.com/linlisWorkTeam/ohMyWorkPanel/releases/tag/v2.1.1
[2.0.0]: https://github.com/linlisWorkTeam/ohMyWorkPanel/releases/tag/v2.0.0
