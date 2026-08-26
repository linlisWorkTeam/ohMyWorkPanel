#!/usr/bin/env bash
# Deterministic guardrails for AI-authored changes. This validates repository
# structure and submission metadata; it never commits, pushes, or promotes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

usage() {
  cat <<'TXT'
Usage:
  scripts/ai-harness.sh check
  scripts/ai-harness.sh commit-message "type: summary"
  scripts/ai-harness.sh submit

check          Fast deterministic structure, documentation, and shell checks.
commit-message Validate an AI commit subject without creating a commit.
submit         Validate HEAD and clean state, then run build, colors, and test gate.
TXT
}

fail() { echo "AI HARNESS FAIL: $*" >&2; exit 1; }

validate_commit_message() {
  local subject="${1:-}" last
  [[ -n "${subject}" ]] || fail "commit subject is required"
  [[ ${#subject} -le 72 ]] || fail "commit subject exceeds 72 characters"
  [[ "${subject}" =~ ^(feat|fix|refactor|docs|test|chore|build|ci|perf|revert)(\([a-z0-9._/-]+\))?:[[:space:]][^[:space:]].*$ ]] \
    || fail "commit subject must use an allowed conventional prefix"
  last="${subject: -1}"
  [[ ! "${last}" =~ [.!?\;:。！？；：] ]] || fail "commit subject should not end with punctuation"
}

check_root_files() {
  local unexpected
  unexpected="$({
    find src -maxdepth 1 -type f \( -name '*.ts' -o -name '*.tsx' \) -printf '%f\n' \
      | grep -Ev '^(App|main|api|api-tauri|api-web|types|theme)\.tsx?$|^(appHooksOrder|colorPurity|themeChromeKeepAlive|themeTokens|uiDemoParity|webApiAlias)\.test\.ts$' || true
  })"
  [[ -z "${unexpected}" ]] || fail "business files must not be flat in src/: ${unexpected//$'\n'/, }"

  unexpected="$(find src-tauri/src -maxdepth 1 -type f -name '*.rs' -printf '%f\n' \
    | grep -Ev '^(lib|main|main_server|models|db|db_migrations|commands|scheduler|web|workflow|extensions|a2a|live_prompt|codex_proxy|context_policy|context_seams|event_sender|fs_browse|git_inspect|memory|message_content|ocr|orchestrator|wiki_context)\.rs$' || true)"
  [[ -z "${unexpected}" ]] || fail "new Rust business modules need a domain directory: ${unexpected//$'\n'/, }"
}

check_tracked_hygiene() {
  local backups added_markers
  backups="$(git ls-files '*.bak' '*.bak.*' '*.old' '*_final' '*_final.*')"
  [[ -z "${backups}" ]] || fail "tracked backup files are forbidden: ${backups//$'\n'/, }"

  local diff_base=""
  if [[ -n "${AI_HARNESS_BASE_REF:-}" ]] && git rev-parse --verify "${AI_HARNESS_BASE_REF}^{commit}" >/dev/null 2>&1; then
    diff_base="${AI_HARNESS_BASE_REF}"
  elif git rev-parse --verify origin/main^{commit} >/dev/null 2>&1; then
    diff_base="origin/main"
  elif git rev-parse --verify HEAD^ >/dev/null 2>&1; then
    diff_base="HEAD^"
  fi

  added_markers="$({
    if [[ -n "${diff_base}" ]]; then
      git diff --unified=0 "${diff_base}...HEAD" -- src src-tauri/src
    fi
    git diff --unified=0 HEAD -- src src-tauri/src
  } | grep '^+' | grep -v '^+++' \
    | grep -E '(^|[^A-Za-z0-9_])(TODO|FIXME)([^A-Za-z0-9_]|$)' \
    | grep -vE '(issue|spec|docs/|根据项目实际补充)' || true)"
  if [[ -n "${added_markers}" ]]; then
    fail "source TODO/FIXME comments must reference an issue, spec, or documentation"
  fi
}

check_docs_links() {
  python3 - <<'PY'
from pathlib import Path
import re

broken = []
for path in [Path("README.md"), *Path("docs").rglob("*.md")]:
    text = path.read_text(encoding="utf-8")
    for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", text):
        relative = target.split("#", 1)[0]
        if not relative or "://" in relative or relative.startswith(("mailto:", "/")):
            continue
        if not (path.parent / relative).resolve().exists():
            broken.append(f"{path} -> {target}")
if broken:
    raise SystemExit("broken Markdown links:\n" + "\n".join(broken))
print("AI harness: Markdown links OK")
PY
}

run_fast_checks() {
  check_root_files
  check_tracked_hygiene
  check_docs_links
  bash -n scripts/*.sh
  local script
  for script in scripts/*.mjs scripts/*.cjs; do
    [[ -e "${script}" ]] || continue
    node --check "${script}"
  done
  echo "AI harness: fast checks OK"
}

mode="${1:-check}"
case "${mode}" in
  check)
    [[ $# -eq 1 || $# -eq 0 ]] || { usage; exit 2; }
    run_fast_checks
    ;;
  commit-message)
    [[ $# -eq 2 ]] || { usage; exit 2; }
    validate_commit_message "$2"
    echo "AI harness: commit message OK"
    ;;
  submit)
    [[ $# -eq 1 ]] || { usage; exit 2; }
    run_fast_checks
    validate_commit_message "$(git log -1 --pretty=%s)"
    [[ -z "$(git status --porcelain)" ]] || fail "submit requires a clean committed worktree"
    pnpm run test:gate
    echo "AI harness: submission OK"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    usage
    exit 2
    ;;
esac
