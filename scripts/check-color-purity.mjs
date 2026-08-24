import { readFileSync, readdirSync, statSync } from "node:fs";
import { relative, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const SRC = resolve(ROOT, "src");
const COLOR_LITERAL = /#[0-9a-f]{3,8}\b|\b(?:rgb|rgba|hsl|hsla)\s*\(|(?<![-\w])(?:white|black)(?![-\w])/gi;
const THEME_SOURCES = new Set([
  "src/theme.tsx",
  "src/themes.css",
  "src/shell/tokens.css",
]);

function walk(directory) {
  const files = [];
  for (const name of readdirSync(directory)) {
    const path = resolve(directory, name);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      if (name !== "node_modules" && name !== "stubs") files.push(...walk(path));
      continue;
    }
    if (/\.(?:css|tsx|ts)$/.test(name) && !/\.test\.[^.]+$/.test(name)) files.push(path);
  }
  return files;
}

function withoutComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, " "))
    .replace(/(^|[^:])\/\/.*$/gm, (comment) => comment.replace(/[^\n]/g, " "));
}

function inspect(path) {
  const file = relative(ROOT, path).replaceAll("\\", "/");
  if (THEME_SOURCES.has(file)) return [];
  const source = withoutComments(readFileSync(path, "utf8"));
  const findings = [];
  for (const match of source.matchAll(COLOR_LITERAL)) {
    const line = source.slice(0, match.index).split("\n").length;
    findings.push({ file, line, literal: match[0] });
  }
  return findings;
}

const findings = walk(SRC).flatMap(inspect);
const grouped = new Map();
for (const finding of findings) {
  const rows = grouped.get(finding.file) ?? [];
  rows.push(finding);
  grouped.set(finding.file, rows);
}

if (process.argv.includes("--report-baseline")) {
  const report = Object.fromEntries([...grouped].sort(([a], [b]) => a.localeCompare(b)).map(([file, rows]) => [file, rows.length]));
  console.log(JSON.stringify(report, null, 2));
  process.exit(0);
}

const violations = [];
for (const [file, rows] of grouped) {
  violations.push(...rows);
}

if (violations.length > 0) {
  console.error(`Color purity failed: ${violations.length} literal(s). Use semantic var(--lp-*) tokens.`);
  for (const finding of violations.slice(0, 80)) {
    console.error(`  ${finding.file}:${finding.line} ${finding.literal}`);
  }
  if (violations.length > 80) console.error(`  ... ${violations.length - 80} more`);
  process.exit(1);
}

console.log("Color purity passed: 0 violations.");
