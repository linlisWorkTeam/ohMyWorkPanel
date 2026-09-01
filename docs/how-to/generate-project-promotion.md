# 在群聊中生成项目宣传草稿

Self-Marketing 把项目最近的 Git 进展整理成一个可追溯的 Content Brief，再生成小红书、X、知乎、B 站脚本和 GitHub Release 五类草稿。它只生成和审核内容，不会登录或发布到外部平台。

## 前提

- 使用绑定了 Git 工作区的项目群；普通聊天群不支持。
- 群中至少有一个可运行的 Agent。可让同一个 Agent 同时承担 Planner 和 Writer。
- 默认只采集已提交内容。只有明确勾选后，才会读取未提交的文本 diff，并把相关事实标为 `unreleased`。

## 生成并审核

1. 在项目群输入 `/market`，或点击输入框工具栏的菱形按钮。
2. 选择 Planner、Writer 和可选的 Git 基准 ref。留空时优先使用最近 tag，否则读取最近 20 个 commit。
3. 如确实要讨论尚未提交的改动，显式勾选“包含未提交改动”。
4. 提交后在群聊卡片中查看 Content Brief、证据引用、五渠道草稿和确定性校验结果。
5. 内容需要调整时填写修改意见。修改只重写 Writer 草稿，复用冻结的 Brief，不会悄悄更换事实源。
6. 确认事实和表达都合适后批准，并下载 Markdown 包。批准不等于发布。

校验出现阻断项时不能批准。常见原因包括证据引用悬空、缺少渠道、X 正文超过 280 字、使用绝对化营销词，或把 `unreleased` 内容写成已经发布。

## 可选项目配置

不创建配置也可以使用内置保守规则。需要稳定项目定位和语气时，可自行把以下文本文件加入项目仓库：

```text
docs/marketing/
  project-context.md
  brand-guide.md
  channels/
    xiaohongshu.md
    x-twitter.md
    zhihu.md
    bilibili-script.md
    github-release.md
```

`project-context.md` 记录项目定位、目标用户和能力边界；`brand-guide.md` 记录语气、禁用承诺和术语。可在 Brand Guide 中写一行 `禁用词：词一，词二` 追加确定性检查。渠道文件只写结构和语气要求，不应复制可能过期的产品事实。

采集器不会读取 `.env*`、凭据/密钥文件、锁文件、依赖目录和构建产物；单条命令、文件、证据数和总差异范围均有上限。完整数据契约见 [Self-Marketing 参考](../reference/self-marketing.md)。
