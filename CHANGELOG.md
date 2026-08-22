# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

<!-- Add unreleased changes here. -->

### Changed

<!-- Add unreleased changes here. -->

### Fixed

<!-- Add unreleased changes here. -->

## [2.1.1] - 2026-08-23

### Added

- Added the v2.1 shell with a WeChat-style chat layout, a dockable right rail, and seven themes.
- Added CLI adapter catalog discovery through `*.adapter.json` manifests, with shell argument rejection.
- Added visual Quick Start documentation with real UI screenshots.
- Added database migrations, HTTP ACL coverage, request IDs, DTO contract tests, and shared cancel/retry services.

### Changed

- Renamed the project and repository to `ohMyWorkPanel`.
- Refined the shell, settings, group, member, message, and agent interaction surfaces.
- Consolidated agent API-key handling and encrypted newly stored keys with a local machine key.

### Fixed

- Fixed web bundle startup failures that could leave the application with a blank root element.
- Fixed UTF-8 encoding in repository documentation and release assets.

## [2.0.0] - 2026-08-21

<!-- See the v2.0.0 GitHub Release for the complete release notes. -->

[unreleased]: https://github.com/linlisWorkTeam/ohMyWorkPanel/compare/v2.1.1...HEAD
[2.1.1]: https://github.com/linlisWorkTeam/ohMyWorkPanel/releases/tag/v2.1.1
[2.0.0]: https://github.com/linlisWorkTeam/ohMyWorkPanel/releases/tag/v2.0.0
