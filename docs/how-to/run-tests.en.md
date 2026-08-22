# How-to: Run Tests

[简体中文](run-tests.md) | **English**

## Frontend Tests

```bash
pnpm test
```

## Rust Unit Tests

Run from the repository root:

```bash
cd src-tauri
cargo test --no-default-features --lib
```

## Complete Gate

Run before a canary release:

```bash
pnpm run test:gate
```

This command includes frontend Vitest, Rust library tests, and the extension-host purity check. Real Agent CLI smoke tests are best-effort and are not part of the complete gate.

## When Tests Fail

1. Record the command, complete error, and current commit.
2. Confirm that `pnpm install` has completed and the Rust toolchain is available.
3. If the failure comes from an external CLI that is not installed or authenticated, mark it as an environment issue instead of claiming the test passed.
4. Add a matching test for behavior changes, or explain in the PR why a test is not applicable.

See the [testing strategy](../testing-strategy.md) for test scope and coverage.
