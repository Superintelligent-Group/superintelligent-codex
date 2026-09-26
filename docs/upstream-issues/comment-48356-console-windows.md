Measured this on Windows 11 26200 with a harness that launches `codex exec` with `DETACHED_PROCESS` (no console, like the daemon) and counts visible top-level console windows during one real turn:

| Build | New console windows per turn |
|---|---|
| 0.157.0 stock | 28 |
| 0.157.1 stock (includes #48138, #48238) | 17 |
| 0.157.1 + the patch below | 0 (several runs) |

**Root cause.** A process with no console hands none to its children. So every console child it spawns *without* `CREATE_NO_WINDOW` allocates its own visible console. With 0.157.1 the remaining windows came from:

- ~10 `git.exe` per turn. `turn_metadata.rs` runs `git_workspaces` via `spawn_git_enrichment_task`: `rev-parse HEAD`, `remote -v`, and `status --porcelain` plus fsmonitor probes. `analytics/src/reducer.rs` runs a second `remote -v` on turn completion.
- The SessionStart hook's `powershell.exe`.
- `codex-computer-use.exe`.

`--no-daemon` avoids it because the in-terminal process has a console, which its children inherit.

**Root-level fix.** Fixing each spawn site one at a time keeps missing some. Once at startup, when `GetConsoleWindow()` is null, call `AllocConsoleWithOptions` (Windows 11 24H2+, resolved via `GetProcAddress`) with `ALLOC_CONSOLE_MODE_NO_WINDOW`. Save and restore any std handles the launcher set. Every child then inherits a windowless console.

It's a no-op on older Windows and for interactive launches, which already have a console. About 90 lines in `cli/src/main.rs`:
https://github.com/Superintelligent-Group/superintelligent-codex/commits/sig/0.157.1
(commit "cli: give console-less Codex a windowless console on Windows").

The window-count harness is `scripts/e2e-codex-windows.ps1` on the `sig/harness` branch of that repo. With `-Diagnose` it groups the spawned process tree by parent.

**Separately, spawn count.** Caching the per-turn git metadata keyed on `HEAD`, ref, `packed-refs` and config contents cuts git spawns from 5 to 1 per turn after the first. Only `status` still runs, since no cheap signal proves the worktree is unchanged. That removes most of these spawns even where the console fix doesn't apply.
