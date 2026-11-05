# Design 1105 前台移植

参考站：https://ohmyworkpanel-design-1105.icedblkamericano.chatgpt.site/

- `preview.css`：参考站 `style.css` 的逐字节移植，仅做 `.pv-root` 作用域前缀；颜色/尺寸/字体未改动。
  该文件是外部设计稿的 vendor 样式，颜色字面量为像素一致所必需，已在 `scripts/check-color-purity.mjs` 中列入例外。
- `PreviewWorkbench.tsx`：1:1 DOM 结构与类名的 React 实现，数据来自真实 group state。
- 开关：`?design=1105` 或 `localStorage.setItem("omwp.design","1105")`。
- 状态：主壳 + 协作页已对齐；其余页签为迁移占位，下一阶段逐页替换。
