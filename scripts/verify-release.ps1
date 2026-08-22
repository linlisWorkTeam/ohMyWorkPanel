param(
  [string]$ExpectedVersion = "2.1.1",
  [string]$InstallerPath
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

$package = Get-Content (Join-Path $repoRoot "package.json") -Raw | ConvertFrom-Json
$tauri = Get-Content (Join-Path $repoRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$cargoText = Get-Content (Join-Path $repoRoot "src-tauri\Cargo.toml") -Raw

$versions = @{
  "package.json" = [string]$package.version
  "src-tauri/tauri.conf.json" = [string]$tauri.version
  "src-tauri/Cargo.toml" = ([regex]::Match($cargoText, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value)
}

foreach ($entry in $versions.GetEnumerator()) {
  if ($entry.Value -ne $ExpectedVersion) {
    throw "$($entry.Key) version is '$($entry.Value)', expected '$ExpectedVersion'."
  }
}

$webviewMode = $tauri.bundle.windows.webviewInstallMode.type
if ($webviewMode -ne "offlineInstaller") {
  throw "Windows WebView2 mode is '$webviewMode', expected 'offlineInstaller'."
}

Write-Output "Version: $ExpectedVersion"
Write-Output "WebView2: $webviewMode"

if ($InstallerPath) {
  $installer = (Resolve-Path $InstallerPath).Path
  $hash = Get-FileHash -Algorithm SHA256 -LiteralPath $installer
  $size = (Get-Item -LiteralPath $installer).Length
  Write-Output "Installer: $installer"
  Write-Output "SizeBytes: $size"
  Write-Output "SHA256: $($hash.Hash.ToLowerInvariant())"
}

