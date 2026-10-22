import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const [script, ...args] = process.argv.slice(2);
if (!script) {
  console.error("usage: node scripts/run-bash.mjs <script> [args...]");
  process.exit(2);
}

const candidates = process.platform === "win32"
  ? [
      process.env.GIT_BASH,
      "C:\\Program Files\\Git\\bin\\bash.exe",
      "C:\\Program Files\\Git\\usr\\bin\\bash.exe",
    ]
  : [process.env.BASH, "/bin/bash", "bash"];
const executable = candidates.find((candidate) => candidate &&
  (candidate === "bash" || existsSync(candidate)));
if (!executable) {
  console.error("Git Bash or bash is required to run repository gates");
  process.exit(127);
}

const result = spawnSync(executable, [script, ...args], {
  cwd: process.cwd(),
  env: process.env,
  stdio: "inherit",
});
if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
