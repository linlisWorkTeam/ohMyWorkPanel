# Contributing

感谢参与 LinlisWorkPanel。请保持改动小而清晰，并优先补充与行为变化对应的文档。

## 本地开发环境

要求：

- Node.js 20+
- pnpm
- Rust stable（仅桌面模式、Rust 后端或 Rust 测试需要）
- Windows 桌面开发需要 WebView2

安装依赖并启动浏览器开发模式：

```bash
pnpm install
pnpm dev
```

常用检查命令：

```bash
pnpm test
pnpm run test:gate
cd src-tauri
cargo test --no-default-features --lib
```

真实 Agent CLI 的 smoke 检查是尽力项，不应把未安装或未登录的 CLI 当作代码测试失败。

## 提交 Pull Request

1. 从最新默认分支创建分支，并只提交与问题相关的文件。
2. 修改行为时同步更新对应文档；文档只使用 Markdown，不引入额外文档构建工具。
3. 在本地运行与改动相关的测试，并在 PR 描述中记录命令和结果。
4. 提交 PR，说明背景、改动、验证方式和已知限制。
5. 根据评审意见修改后再合并；不要把密钥、登录态、数据库或构建产物提交到仓库。

## 提交信息建议

建议使用简短的类型前缀：

```text
docs: reorganize user documentation
fix: handle empty group state
feat: add adapter setting
test: cover message parsing
chore: update dependencies
```

<!-- TODO: 根据项目实际补充默认分支、代码所有者、PR 模板和必需检查。 -->
