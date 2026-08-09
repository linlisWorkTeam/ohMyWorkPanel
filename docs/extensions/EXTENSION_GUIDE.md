# WorkPanel 扩展宿主指南（V1.3.0）

平台只做 **发现 / 开关 / 同源反代 / 页签壳 / A2A skill 校验**。扩展业务留在扩展仓。

需求来源：AIHotel `2026-08-06-workpanel-extension-host-v130-requirements.md`。  
认领：[`docs/superpowers/specs/2026-08-06-extension-host-v130-claim.md`](../superpowers/specs/2026-08-06-extension-host-v130-claim.md)。

## 1. 发现

| 来源 | 说明 |
|---|---|
| `LINLIS_EXTENSION_ROOTS` | `:` 或 `;` 分隔的绝对路径，每项含 `extension.manifest.json` |
| `LINLIS_PANELLIVE_ROOT` | 兼容回落，默认 `/AI/WorkPanelLive`，始终并入列表 |
| `/AI/AIHotel` | 若磁盘上存在 manifest 则自动加入（可用 ROOTS 覆盖顺序） |

重启 Web 服务后生效。

## 2. Manifest

沿用 PanelLive 形状（`id` / `contributes.tabs[]` / `contributes.a2aSkills` / `runtime.defaultPort` / `healthPath`）。平台 **不** 另起 schema。

页签 `entry` 经同源反代加载：`{baseUrl}{entry}?groupId=`，禁止前端直连扩展端口。

## 3. API

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/groups/{id}/extensions` | 全部已发现扩展状态 |
| PUT | `/api/groups/{id}/extensions/{extId}` | `{enabled}`；启用前 health 失败 → 4xx |
| PUT | `/api/groups/{id}/extensions/panellive` | 兼容别名 |
| ANY | `/api/extensions/{extId}/{*path}` | 反代到 `127.0.0.1:{defaultPort}` |

## 4. 前端

- 顶栏遍历 `extensions[].tabs`（`ExtensionPanel` iframe）
- PanelLive 语音桥：`ExtensionPanel` 内对 `panellive`+`live` 委托 `LivePanel`（迁移期允许）
- 设置 → **Extend**：列出全部扩展开关与 health

## 5. A2A

按各扩展 `a2aSkills`（支持 `prefix.*`）校验；未声明 skill → 拒绝。`live.*` 仍走既有控制面；其它扩展 skill 宿主 ack（业务在扩展侧）。

## 6. 禁止事项

- ❌ `proxy_<ext>` / `<ext>Tab` / `HotelPanel` 等扩展专用平台代码
- ❌ iframe 直连 `127.0.0.1:port`
- ❌ 剧本 / NPC / 判定等业务进平台仓

## 7. 纯净度（`test:gate`）

```bash
bash scripts/check-extension-purity.sh
```

白名单：`extensions.rs`、`ExtensionPanel.tsx`、迁移期 `LivePanel` / `liveBridge` / `liveVoice`、文档与测试。
