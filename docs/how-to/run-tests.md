# How-to：运行测试

## 前端测试

```bash
pnpm test
```

## Rust 单元测试

在仓库根目录执行：

```bash
cd src-tauri
cargo test --no-default-features --lib
```

## 完整门禁

发布灰度前运行：

```bash
pnpm run test:gate
```

该命令包含前端 Vitest、Rust library 测试和扩展宿主纯度检查。真实 Agent CLI smoke 是独立的尽力检查，不属于完整门禁。

## 测试失败时

1. 记录失败命令、完整错误和当前 commit。
2. 确认依赖已执行 `pnpm install`，Rust 工具链可用。
3. 如果失败来自未安装或未登录的外部 CLI，请标记为环境问题，不要伪装成通过。
4. 行为变更提交前，补充对应测试或在 PR 中说明为什么不适用。

更多测试范围说明见 [`docs/testing-strategy.md`](../testing-strategy.md)。
