# Best-effort adapter smoke checks. Missing CLI or auth failure => SKIPPED / WARN, exit 0.
$ErrorActionPreference = "Continue"
$TimeoutSec = 60
$Prompt = "Reply with exactly: ok"
$Results = New-Object System.Collections.Generic.List[string]

function Find-Cmd([string[]]$Names) {
  foreach ($name in $Names) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
  }
  return $null
}

function Test-Version([string]$Label, [string]$Exe) {
  try {
    & $Exe --version 2>&1 | Out-Host
    if ($LASTEXITCODE -eq 0) {
      Write-Host "[OK] $Label --version"
      return $true
    }
    Write-Host "[WARN] $Label --version exit=$LASTEXITCODE"
    return $false
  } catch {
    Write-Host "[WARN] $Label --version failed: $_"
    return $false
  }
}

function Invoke-Timed([string]$Label, [scriptblock]$Script, [object[]]$ArgumentList) {
  Write-Host "[RUN] $Label short prompt (timeout ${TimeoutSec}s)..."
  $job = Start-Job -ScriptBlock $Script -ArgumentList $ArgumentList
  if (-not (Wait-Job $job -Timeout $TimeoutSec)) {
    Stop-Job $job -ErrorAction SilentlyContinue
    Remove-Job $job -Force -ErrorAction SilentlyContinue
    Write-Host "[WARN] $Label prompt timed out / unverified"
    return "UNVERIFIED"
  }
  Receive-Job $job | Out-Host
  Remove-Job $job -Force
  Write-Host "[OK] $Label short prompt completed (inspect output/auth above)"
  return "TRIED"
}

Write-Host "=== ohMyWorkPanel adapter smoke ==="

$codex = Find-Cmd @("codex")
if (-not $codex) {
  Write-Host "[SKIPPED] codex (not installed)"
  $Results.Add("codex:SKIPPED") | Out-Null
} else {
  Test-Version "codex" $codex | Out-Null
  $status = Invoke-Timed "codex" {
    param($Exe, $Prompt)
    & $Exe exec --json --skip-git-repo-check $Prompt 2>&1
  } @($codex, $Prompt)
  $Results.Add("codex:$status") | Out-Null
}

$claude = Find-Cmd @("claude")
if (-not $claude) {
  Write-Host "[SKIPPED] claude (not installed)"
  $Results.Add("claude:SKIPPED") | Out-Null
} else {
  Test-Version "claude" $claude | Out-Null
  $status = Invoke-Timed "claude" {
    param($Exe, $Prompt)
    & $Exe -p --output-format stream-json --verbose $Prompt 2>&1
  } @($claude, $Prompt)
  $Results.Add("claude:$status") | Out-Null
}

$opencode = Find-Cmd @("opencode")
if (-not $opencode) {
  Write-Host "[SKIPPED] opencode (not installed)"
  $Results.Add("opencode:SKIPPED") | Out-Null
} else {
  Test-Version "opencode" $opencode | Out-Null
  $status = Invoke-Timed "opencode" {
    param($Exe, $Prompt)
    & $Exe run $Prompt --format json 2>&1
  } @($opencode, $Prompt)
  $Results.Add("opencode:$status") | Out-Null
}

$cursor = Find-Cmd @("agent", "cursor-agent")
if (-not $cursor) {
  Write-Host "[SKIPPED] cursor (agent/cursor-agent not installed)"
  $Results.Add("cursor:SKIPPED") | Out-Null
} else {
  Test-Version "cursor" $cursor | Out-Null
  $status = Invoke-Timed "cursor" {
    param($Exe, $Prompt)
    & $Exe -p $Prompt --output-format stream-json 2>&1
  } @($cursor, $Prompt)
  $Results.Add("cursor:$status") | Out-Null
}

Write-Host ""
Write-Host "=== Summary ==="
$Results | ForEach-Object { Write-Host $_ }
Write-Host "Smoke finished (non-blocking)."
exit 0
