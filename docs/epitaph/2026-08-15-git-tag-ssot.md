---
date: 2026-08-15
topic: v1.3-git-tag-ssot
branch: master
status: active
---

# Epitaph: 补打 v1.2.0 / v1.3.0 并对齐 SSOT

## Built this session
- **Git tags**：`v1.2.0` → `0750306`（Live 会话/聊天一致，v1.2.0 基线）；`v1.3.0` → `8e3869d`（版本页 + Ask/Wave）。**不要把 HEAD 打成 v1.3.0**。
- **SSOT**：`docs/version-pipeline.md` 去掉「tag 待补」；`package.json` / `src-tauri/Cargo.toml` / `tauri.conf.json` / `Cargo.lock` 包版本 `0.1.0` → `1.3.0`。
- Tags 已 `git push origin v1.2.0 v1.3.0`。版本页 `git_inspect` 读工作区仓库，prod/canary 刷新即可看到新 tag（不必等 promote）。

## Key files
| 文件 | 说明 |
|---|---|
| `docs/version-pipeline.md` | 产品版本 SSOT |
| `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` | 包版本对齐 1.3.0 |

## Locked product decisions
| 项 | 选择 |
|---|---|
| v1.2.0 钉点 | `0750306`，不是「跳过 1.2 把 1.3 打在 HEAD」 |
| v1.3.0 钉点 | `8e3869d`（工作流实现），HEAD 的 drain/4.6/群设置为 1.3.0+ |
| 历史 epitaph「v1.3 双槽位」 | 不是 git tag `v1.3.0` |

## Known pitfalls
- 三个「1.3」易混：git tag `v1.3.0`、产品里程碑「WorkPanel V1.3.0」、epitaph「v1.3 双槽位」。
- 改 tag 立刻影响版本页；移动/删除已推送 tag 需人工明确授权。

## How to run / verify
```bash
git tag -l --sort=v:refname
git log -1 --oneline v1.2.0   # 0750306
git log -1 --oneline v1.3.0   # 8e3869d
git describe --tags           # v1.3.0-N-g…（HEAD 在 tag 之后）
```

## Do not regress
- 勿把 `v1.3.0` 移到 HEAD。
- 勿把历史平台小步 epitaph（v1.2 Experience、v1.3 双槽位）改写成 git tag 语义。

## Open follow-ups
- 下一产品小版本立项后再打 tag；1.3.0+ 补丁暂不另打。
- 包版本进生产二进制需灰度通过后 **批准 promote**（tag 本身已生效）。
