# How-to: Update ECS After the Repository Rename

[简体中文](repository-rename.md) | **English**

The project was renamed from `workPanel` to `ohMyWorkPanel`. The new repository URL is:

```text
https://github.com/linlisWorkTeam/ohMyWorkPanel.git
```

On ECS, enter the project directory and update the Git remote first:

```bash
cd /AI/ohMyWorkPanel
git remote set-url origin https://github.com/linlisWorkTeam/ohMyWorkPanel.git
git fetch origin --prune
git pull --ff-only origin main
```

If ECS still uses `/AI/LinlisWorkPanel`, ask the operator to confirm backups, the service downtime window, and the data migration plan before moving to `/AI/ohMyWorkPanel`. Do not delete the old directory or SQLite data directly.

Service names, release directories, environment variables, and executable names also use the `ohmyworkpanel` prefix. After updating the systemd unit, run:

```bash
sudo systemctl daemon-reload
sudo systemctl restart ohmyworkpanel-canary.service
sudo systemctl is-active ohmyworkpanel-canary.service
```

Production promotion still requires the approved canary process. Do not restart the production service directly.

## Team Notification Template

> The repository has been renamed to `ohMyWorkPanel`. Update the ECS workspace Git remote to `https://github.com/linlisWorkTeam/ohMyWorkPanel.git`, then update canary according to the release checklist. The old repository URL redirects, but deployment scripts, systemd service names, and runtime directories must be checked against the renamed project.

<!-- TODO: Add the actual ECS team notification channel, owner, and migration window. -->
