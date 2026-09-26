<#
.SYNOPSIS
  E2E: count console windows that appear while a console-less Codex runs a
  real turn.

.DESCRIPTION
  The terminal-window spam only happens when Codex itself has no console
  (the detached app-server daemon, desktop-app hosts): every console child it
  spawns without CREATE_NO_WINDOW then allocates its own visible console.
  Launching from an ordinary shell hides the bug, because children inherit
  that shell's console.

  So this launches `codex exec` with DETACHED_PROCESS, using the user's real
  config so every configured stdio MCP server starts, and samples visible
  top-level console windows (conhost and Windows Terminal) every 100 ms.
  Exit 0 = no new console windows and the turn answered.
#>
param(
  [string]$Codex = 'codex',
  [string]$Prompt = 'Reply with exactly: SIG-E2E-OK',
  [int]$TimeoutSeconds = 240
)

Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class SigE2E {
  delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr hWnd, StringBuilder s, int n);
  public static long[] ConsoleWindows() {
    var found = new List<long>();
    EnumWindows((h, _) => {
      if (!IsWindowVisible(h)) return true;
      var s = new StringBuilder(256); GetClassName(h, s, 256);
      var c = s.ToString();
      if (c == "ConsoleWindowClass" || c == "CASCADIA_HOSTING_WINDOW_CLASS") found.Add(h.ToInt64());
      return true;
    }, IntPtr.Zero);
    return found.ToArray();
  }

  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  struct STARTUPINFO {
    public int cb; public string lpReserved, lpDesktop, lpTitle;
    public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
    public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
  }
  [StructLayout(LayoutKind.Sequential)]
  struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }
  [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  static extern bool CreateProcess(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inherit,
    uint flags, IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

  // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: no console at all, like the daemon.
  public static int StartDetached(string commandLine, string cwd) {
    var si = new STARTUPINFO(); si.cb = Marshal.SizeOf(si);
    PROCESS_INFORMATION pi;
    if (!CreateProcess(null, new StringBuilder(commandLine), IntPtr.Zero, IntPtr.Zero, false,
        0x00000008 | 0x00000200, IntPtr.Zero, cwd, ref si, out pi))
      throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    CloseHandle(pi.hThread); CloseHandle(pi.hProcess);
    return pi.dwProcessId;
  }
}
'@

$codexPath = (Get-Command $Codex).Source
$outFile = Join-Path ([IO.Path]::GetTempPath()) "sig-codex-e2e-$([guid]::NewGuid()).txt"
$before = [SigE2E]::ConsoleWindows()
$cmd = "`"$codexPath`" exec --skip-git-repo-check -o `"$outFile`" `"$Prompt`""
$procId = [SigE2E]::StartDetached($cmd, (Get-Location).Path)
$proc = Get-Process -Id $procId

$seen = @{}
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
while (-not $proc.HasExited -and (Get-Date) -lt $deadline) {
  foreach ($h in [SigE2E]::ConsoleWindows()) { if ($before -notcontains $h) { $seen[$h] = $true } }
  Start-Sleep -Milliseconds 100
}
$timedOut = -not $proc.HasExited
if ($timedOut) { Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue }

$answer = if (Test-Path $outFile) { (Get-Content $outFile -Raw).Trim() } else { $null }
Remove-Item $outFile -ErrorAction SilentlyContinue
$ok = $answer -match 'SIG-E2E-OK'
[pscustomobject]@{
  codex = $codexPath
  newConsoleWindows = $seen.Count
  timedOut = $timedOut
  turnSucceeded = $ok
} | ConvertTo-Json -Compress
if ($seen.Count -eq 0 -and $ok) { exit 0 } else { exit 1 }
