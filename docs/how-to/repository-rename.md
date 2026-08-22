# How-to：仓库改名后更新 ECS

项目已从 `workPanel` 改名为 `ohMyWorkPanel`，新的仓库地址是：

```text
https://github.com/linlisWorkTeam/ohMyWorkPanel.git
```

在 ECS 上进入项目目录后，先更新 Git remote：

```bash
cd /AI/ohMyWorkPanel
git remote set-url origin https://github.com/linlisWorkTeam/ohMyWorkPanel.git
git fetch origin --prune
git pull --ff-only origin master
```

如果 ECS 仍使用旧目录 `/AI/LinlisWorkPanel`，请先由运维确认备份、服务停机窗口和数据迁移方案，再迁移到 `/AI/ohMyWorkPanel`。不要直接删除旧目录或 SQLite 数据。

服务名、发布目录、环境变量和可执行文件也已改为 `ohmyworkpanel` 前缀；更新 systemd unit 后执行：

```bash
sudo systemctl daemon-reload
sudo systemctl restart ohmyworkpanel-canary.service
sudo systemctl is-active ohmyworkpanel-canary.service
```

生产服务的 promote 仍须遵守审批和灰度流程，不要直接重启生产服务。

## 通知模板

> 仓库已改名为 `ohMyWorkPanel`。请将 ECS 工作区的 Git remote 更新为 `https://github.com/linlisWorkTeam/ohMyWorkPanel.git`，再按发布清单更新 canary。旧仓库地址会跳转，但部署脚本、systemd 服务名和运行目录应按本次改名后的文档核对。

<!-- TODO: 补充 ECS 团队通知渠道、负责人和实际迁移窗口。 -->
