# Reference：Web API

当前仓库已有一份薄 API 索引：[`docs/api-web.md`](../api-web.md)。它列出鉴权、群组、成员、消息、任务、设置和 WebSocket 路径，但不维护完整 schema。

## 使用原则

- JSON 字段通常使用 camelCase；
- 除注册、登录和明确标注公开的健康检查外，API 通常需要 `Authorization: Bearer <JWT>`；
- Web 部署中的文件系统路径是服务器路径，不是浏览器本机路径；
- 具体请求和响应字段以当前代码和实际服务为准。

## 参考入口

- [API 路由薄索引](../api-web.md)
- [前端 API 封装](../../src/api-web.ts)
- [发布与实时检查](../release-checklist.md)

<!-- TODO: 根据项目实际补充稳定 API 的请求示例、响应示例、错误码和版本兼容策略。 -->
