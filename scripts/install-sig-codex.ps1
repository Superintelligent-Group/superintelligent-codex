<#
.SYNOPSIS
  Build the SIG Codex patch stack and install it over the local standalone
  Codex CLI and app-server daemon binaries.

.DESCRIPTION
  Reads patches/codex/stack.json for the expected fork branch, builds
  codex.exe from C:\Github\superintelligent-codex, backs up each installed
  binary once (codex.exe.upstream), swaps ours in, restarts the daemon, and
  runs `codex doctor`. Running binaries are renamed aside rather than
  overwritten, so open Codex sessions keep working.

  -Restore puts the upstream backups back.
#>
param(
  [string]$CodexRepo = 'C:\Github\superintelligent-codex',
  # Dedicated build dir; worktrees must not share one (stale outputs).
  [string]$TargetDir = 'C:\cargo-target\sig-codex-release',
  [switch]$SkipBuild,
  [switch]$Restore
)

$ErrorActionPreference = 'Stop'
$stack = Get-Content (Join-Path $PSScriptRoot '..\patches\codex\stack.json') -Raw | ConvertFrom-Json
$version = $stack.base.tag -replace '^rust-v', ''
$triple = "$version-x86_64-pc-windows-msvc"
$packages = Join-Path $env:USERPROFILE '.codex\packages'
$targets = @(
  (Join-Path $packages "standalone\releases\$triple\bin\codex.exe"),
  (Join-Path $packages "app-server-daemon\releases\$triple\bin\codex.exe"),
  (Join-Path $env:LOCALAPPDATA 'Programs\OpenAI\Codex\bin\codex.exe')
) | Where-Object { Test-Path $_ }

if (-not $targets) { throw "No installed Codex $version binaries found under $packages" }

function Replace-Binary([string]$Target, [string]$Source) {
  # Windows allows renaming a running executable but not overwriting it.
  $aside = "$Target.old-$(Get-Date -Format yyyyMMddHHmmss)"
  Move-Item -LiteralPath $Target -Destination $aside
  Copy-Item -LiteralPath $Source -Destination $Target
  Remove-Item -LiteralPath $aside -ErrorAction SilentlyContinue
}

codex app-server daemon stop | Out-Null

if ($Restore) {
  foreach ($target in $targets) {
    $backup = "$target.upstream"
    if (Test-Path $backup) { Replace-Binary $target $backup; Write-Host "restored $target" }
  }
} else {
  Push-Location $CodexRepo
  try {
    $branch = git rev-parse --abbrev-ref HEAD
    if ($branch -ne $stack.branch) { throw "$CodexRepo is on '$branch', expected '$($stack.branch)'" }
    if (git status --porcelain --untracked-files=no) { throw "$CodexRepo has uncommitted changes" }
    $env:CARGO_TARGET_DIR = $TargetDir
    $built = Join-Path $TargetDir 'release\codex.exe'
    if (-not $SkipBuild) {
      # Git for Windows ships a coreutils link.exe that shadows the MSVC linker.
      $env:PATH = ($env:PATH -split ';' | Where-Object { $_ -notmatch 'Git\\usr\\bin' }) -join ';'
      Push-Location codex-rs
      try { cargo build --release -p codex-cli --bin codex; if ($LASTEXITCODE) { throw 'cargo build failed' } }
      finally { Pop-Location }
    }
    if (-not (Test-Path $built)) { throw "No build at $built" }
  } finally { Pop-Location }

  foreach ($target in $targets) {
    $backup = "$target.upstream"
    if (-not (Test-Path $backup)) { Copy-Item -LiteralPath $target -Destination $backup }
    Replace-Binary $target $built
    Write-Host "installed $target"
  }
}

codex app-server daemon start
codex doctor --summary
