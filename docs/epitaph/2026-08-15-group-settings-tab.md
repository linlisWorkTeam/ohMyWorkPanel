---
date: 2026-08-15
topic: group-settings-tab
branch: master
status: active
---

# Epitaph: 群设置页（公告 + 工作目录）

## Built this session
- V1.3.0 顶栏「项目」改为「版本」后，公告/群工作区编辑入口消失。
- 新增顶栏 **设置** + 成员栏 **群设置**，打开 `GroupSettingsView`：群公告、群工作目录、Agent 工作区覆盖。
- 标题栏增加「公告 / 工作目录在「设置」」跳转。

## Key files
| 文件 | 说明 |
|---|---|
| `src/GroupSettingsView.tsx` | 设置页 |
| `src/App.tsx` | 顶栏/成员栏入口 |

## Do not regress
- 勿把旧「项目」看板当主入口（版本页保留）
- 聊天群无工作区编辑，仍可改公告
