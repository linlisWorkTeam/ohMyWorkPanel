# Design 1105 前台移植

参考站：https://ohmyworkpanel-design-1105.icedblkamericano.chatgpt.site/

## 文件

- `preview.css`：参考站 `style.css` 的逐字节移植，仅加 `.pv-root` 作用域前缀；颜色/尺寸/字体未改。
  作为 vendor 设计稿，颜色字面量在 `scripts/check-color-purity.mjs` 中列入例外。
- `PreviewWorkbench.tsx`：主壳 + 协作页（review bar / rail / sidebar / header / tabs / transcript / composer / inspector）。
- `PreviewPages.tsx`：交付 / 目标 / 内容 / 场景 / 扩展 / Agent 环境 / 可观测 / 设置。
- 开关：默认启用；`?legacy=1` 或 `localStorage omwp.design=legacy` 回旧壳。

## 覆盖

| 页面 | 数据源 | dark | light |
|---|---|---|---|
| 协作 | group state / version board / goal | ✅ | ✅ |
| 交付 | VersionBoard | ✅ | ✅ |
| 目标 | listGoals | ✅ | ✅ |
| 内容 | marketing campaigns | ✅ | ✅ |
| 场景 | members / runs | ✅ | ✅ |
| 扩展 | listGroupExtensions | ✅ | ✅ |
| Agent 环境 | members / detectAgent | ✅ | ✅ |
| 可观测 | listLogs / runs | ✅ | ✅ |
| 设置 | getSettings / updateSettings | ✅ | ✅ |

## 验证方法

- 主壳：CDP 读取参考站与本地 computed style（字体/行高/颜色/内边距/尺寸/圆角），逐选择器 diff。
  结果：`rail 60 / sidebar 234 / header 86 / tabs 43 / summary 44 / inspector 298`，字号 13px，无横向溢出。
- 断点：1024（56/205/1fr/265）、720（单列）、390（单列），逐页 `scrollWidth === innerWidth`。
- 压力：200% zoom 无溢出；`prefers-reduced-motion` 下 transition 0s；粗指针 44px 规则保留。
- 截图：`output/final-{page}-{dark,light}.png`（18 张）、`pv-phone-*.png`、`pv-*-dialog.png`。

## 已知偏差

- 消息/成员行宽度与高度随真实内容变化（长群名、长角色文案、日志条数），非样式差异。
- 参考站协作区有滚动条（10px），本地数据少时无滚动条，宽度差 10px。
- 登录/邀请页未迁移：参考站没有登录页，仍用旧壳样式（待产品确认是否重绘）。
- 顶栏保留参考站的「演示数据 · 操作仅在此页面生效」；真实应用操作会写入数据库（待确认文案）。
