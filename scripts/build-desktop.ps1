# Desktop installer build helper (Windows).
# Usage: powershell -ExecutionPolicy Bypass -File scripts/build-desktop.ps1 [-SkipGate] [-ManifestBaseUrl <url>] [-ManifestNotes "<txt>"]
# Steps: gate (unless -SkipGate) -> incremental release build -> replace-publish to .local-panel\release + SHA256SUMS
#        + optional update manifest (update.json) beside the installer for the "check for update" feature.
# See docs/how-to/build-desktop-package.md.
param([switch]$SkipGate, [string]$ManifestBaseUrl = "", [string]$ManifestNotes = "")
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $SkipGate) {
  Write-Host "[1/4] gate: vitest"
  & pnpm exec vitest run --pool=forks --maxWorkers=1
  if ($LASTEXITCODE -ne 0) { throw "vitest failed" }
  Write-Host "[1/4] gate: cargo test (no-default-features --lib)"
  Push-Location "$root\src-tauri"
  & cargo test --no-default-features --lib
  if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
  Pop-Location
  Write-Host "[1/4] gate: pnpm build (tsc)"
  & pnpm build
  if ($LASTEXITCODE -ne 0) { throw "tsc build failed" }
  Write-Host "[1/4] gate: cargo check --lib (gui feature - covers commands.rs)"
  Push-Location "$root\src-tauri"
  & cargo check --lib
  Pop-Location
  if ($LASTEXITCODE -ne 0) { throw "cargo check (gui) failed" }
}

Write-Host "[2/4] build: CARGO_PROFILE_RELEASE_INCREMENTAL=true pnpm tauri build"
$env:CARGO_PROFILE_RELEASE_INCREMENTAL = "true"
& pnpm tauri build
if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

$version = (Get-Content "$root\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
$artifact = "$root\src-tauri\target\release\bundle\nsis\ohMyWorkPanel_${version}_x64-setup.exe"
if (-not (Test-Path $artifact)) { throw "artifact not found: $artifact" }

Write-Host "[3/4] replace-publish to .local-panel\release"
$rel = "$root\.local-panel\release"
New-Item -ItemType Directory -Force -Path $rel | Out-Null
Copy-Item $artifact "$rel\ohMyWorkPanel_${version}_x64-setup.exe" -Force
$hash = (Get-FileHash "$rel\ohMyWorkPanel_${version}_x64-setup.exe" -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content "$rel\SHA256SUMS.txt" -Value "$hash  ohMyWorkPanel_${version}_x64-setup.exe" -Encoding ascii

Write-Host "[4/4] done"
Write-Host "installer: $rel\ohMyWorkPanel_${version}_x64-setup.exe"
Write-Host "sha256:    $hash"

if ($ManifestBaseUrl.Trim() -ne "") {
  $base = $ManifestBaseUrl.Trim().TrimEnd("/")
  $manifest = @{
    version = $version
    notes = $ManifestNotes.Trim()
    url = "$base/ohMyWorkPanel_${version}_x64-setup.exe"
    sha256 = $hash
  } | ConvertTo-Json -Compress
  Set-Content "$rel\update.json" $manifest -Encoding ascii
  Write-Host "update.json:  $rel\update.json"
  # 同步一份到前端 dist（本地灰度 Web 以 /update.json 服务，桌面端自动发现用）
  New-Item -ItemType Directory -Force -Path "$root\dist" | Out-Null
  Copy-Item "$rel\update.json" "$root\dist\update.json" -Force
  Write-Host "dist/update.json synced (local gray web serves /update.json)"
}