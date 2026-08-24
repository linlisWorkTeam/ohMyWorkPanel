# 构建桌面安装包（Windows 本地）

面向：ohMyWorkPanel 桌面版（Tauri 2 / NSIS）增量出包。发版总流程见
[`docs/release-checklist.md`](../release-checklist.md)（灰度→生产）；本文只讲「桌面安装包」。

## 何时出包

- **本轮验收对话期间**：所有 bugfix 统一收拢为一个版本（当前 2.1.2），**替代式发布**——
  同名 `ohMyWorkPanel_2.1.2_x64-setup.exe` 覆盖 `.local-panel/release/`，不逐条升版本号。
- 每轮对话结束后正常走版本流水线（见 `docs/version-pipeline.md`），以新版本号发布。

## 前置

- Windows + Node 20+ + Rust stable + WebView2（Tauri 依赖）
- `pnpm install`
- 安装包不打包外部 Agent CLI（Codex/Claude/OpenCode/OpenClaw/Cursor/DSh 需在执行机器单独安装登录）

## 命令

一键助手（推荐）：

```powershell
powershell -File scripts/build-desktop.ps1        # 门禁 + 构建 + 覆盖发布 + 校验和
powershell -File scripts/build-desktop.ps1 -SkipGate   # 跳过门禁快速出包
```

手工分步：

```powershell
# 1) 门禁（快，~20s）
pnpm run test:gate
# 2) 增量编译开关：release 增量缓存，小改动可省 30–40% 构建时间
#    （首次全量构建仍需 15–30 分钟；增量后小改动约 1 分钟）
$env:CARGO_PROFILE_RELEASE_INCREMENTAL = "true"
# 3) 构建（前台约 1–1.5 分钟；建议后台跑以免卡住终端）
pnpm tauri build
```

- 产物：`src-tauri\target\release\bundle\nsis\ohMyWorkPanel_<版本>_x64-setup.exe`
- `CARGO_PROFILE_RELEASE_INCREMENTAL=true`：Rust release 增量编译。**只用于本地反复出包**；
  对甲方/CI 的一体化构建可省略（避免产物体积与偶发慢链接）。

## 替代式发布（当前约定）

```powershell
$rel = ".local-panel\release"
Copy-Item src-tauri\target\release\bundle\nsis\ohMyWorkPanel_<版本>_x64-setup.exe $rel\ -Force
Get-FileHash "$rel\ohMyWorkPanel_<版本>_x64-setup.exe" -Algorithm SHA256  # 更新 SHA256SUMS.txt
```

每次出包必须：
- [ ] 全量门禁（`pnpm run test:gate` + `pnpm build`（tsc）+ `cargo check --lib`（gui 特性，覆盖
      `commands.rs`——`--no-default-features` 门禁**不编译它**，曾漏过 4 个编译错）
- [ ] 覆盖同名单文件 + 更新 `SHA256SUMS.txt`
- [ ] 产物版本资源（`(Get-Item ...exe).VersionInfo`）= 期望版本

## 排障：读桌面端运行日志

桌面日志在应用自己的数据库（不在工作区）：
`%APPDATA%\com.ohmyworkpanel.app\ohmyworkpanel.sqlite3`（logs / run_events / task_runs）。

```powershell
cd src-tauri
cargo run --bin dblog -- "$env:APPDATA\com.ohmyworkpanel.app\ohmyworkpanel.sqlite3" 25
```

- dblog 只读打开（运行中可用），导出最近 run_events / task_runs（含 error）/ logs。
- chatbot 失败还会落 `debug_chatbot` 事件（provider/model/apiUrl/error），一跑 dblog 即可定位。
- 工具源码：`src-tauri/src/bin/dblog.rs`。

## 已知问题与排查

### 安装位置漂移到 `%TEMP%\<app>-<版本>-install-check-<hash>`

症状：卸载注册表 `InstallLocation` / `UninstallString` 指向 `%TEMP%\ohmyworkpanel-v2.1.1-install-check-…`
（而不是 `%LOCALAPPDATA%\ohMyWorkPanel`）；`%TEMP%` 被清理后应用"消失"。

成因：NSIS 安装器会把 payload 解到 `%TEMP%` 的 install-check 暂存位做安装校验；
若安装中途状态被污染（残留 install-check 目录 + 旧注册表指向），后续安装可能沿用该暂存位并登记为正式安装。

修复（已验证有效）：

```powershell
# 1) 关闭 app → 运行暂存位里的 uninstall.exe（或先删该目录 + 注册表项）
# 2) 用显式 /D= 强制正确安装位（/D 必须是最后一个参数，路径不含引号）
$setup = ".local-panel\release\ohMyWorkPanel_<版本>_x64-setup.exe"
Start-Process $setup -ArgumentList "/S", "/D=C:\Users\<用户名>\AppData\Local\ohMyWorkPanel" -Wait
# 3) 验证：注册表 InstallLocation 应为 %LOCALAPPDATA%\ohMyWorkPanel，且 %TEMP% 无残留
```

清干净后默认路径即恢复正常（裸装也会落到 `%LOCALAPPDATA%`）。后续自更新安装同样使用显式 `/D=` 规避。

## 发布更新清单（桌面端「检查更新」）

桌面端设置 →更新可检查新版本。清单为 JSON（发布方托管在任意 URL）：

```json
{ "version": "2.1.3", "notes": "修复 xxx", "url": "https://…/ohMyWorkPanel_2.1.3_x64-setup.exe", "sha256": "<hex>" }
```

- 出包时自动生成：`scripts/build-desktop.ps1 -ManifestBaseUrl https://…/ohmyworkpanel` 会在 `.local-panel\release\update.json` 生成清单（version/notes/url/sha256 已填好，托管时改 notes 可手动补一句）。
- 桌面端「检查更新」逻辑：curl 拉清单 → 与当前版本数值比较（忽略 -beta/+build 后缀）→ 有新版提示 + 显示/复制下载链接；未配置清单 URL 时静默跳过。
- 本地自测：清单 URL 填 `file:///D:/AI/LinlisWorkPanel/.local-panel/release/update.json` 即可验证。
- 安装新包建议用显式 `/D=`（见上节已知问题），后续自更新安装同样处理。

## 发布记录约定

每次桌面发布在 `docs/epitaph/` 写一条发布记录（如 `2026-08-23-v2.1.1-release-verify.md`）：
版本、安装包大小、SHA-256、验证要点（SmartScreen 未签名提示、数据目录
`%APPDATA%\com.ohmyworkpanel.app`、覆盖安装保留数据）。