# Upstream Codex boundary

Audited repository: `https://github.com/openai/codex`

Audited commit: `31ffe2bc9adccfe5fd3d29208250f796a13aa7a0`

The upstream tree already contains a context manager, conversation-summary
readback, compaction implementations, and compaction analytics. The SIG
harness therefore owns the cross-provider durable packet and governance
lineage, rather than reimplementing Codex's internal compaction engine.

Relevant official documentation:

- OpenAI Responses compaction: https://developers.openai.com/api/reference/java/resources/responses/methods/compact
- Claude Code memory: https://code.claude.com/docs/en/memory

Claude Code's project instructions and auto memory are useful local ergonomics,
but they are context supplied to the model, not enforcement. SIG hooks,
leases, receipts, and database invariants remain the authority for governed
actions.

## SIG fork and patch stack

Fork: `https://github.com/Superintelligent-Group/superintelligent-codex`
(local checkout `C:\Github\superintelligent-codex`, remote `sig`).

The fork is a patch stack, not a diverging codebase. `main` mirrors
upstream and never carries SIG commits. Each upstream release gets a
`sig/<version>` branch cut from its `rust-v<version>` tag, with our patches
on top. `patches/codex/stack.json` is the ledger: every patch declares
whether it is an upstream `backport` or a `sig` patch, why it exists, and
the condition under which it is dropped. `node --test` enforces that shape.

Install the current stack over the local standalone CLI and daemon:

```powershell
scripts/install-sig-codex.ps1            # build + install
scripts/install-sig-codex.ps1 -Restore   # put upstream binaries back
```

Roll forward when upstream ships a release:

1. `git fetch origin tag rust-v<new>` in the fork checkout.
2. `git switch -c sig/<new> rust-v<new>`.
3. Cherry-pick each patch still listed in `stack.json`; skip any backport
   the new tag already contains.
4. Update `stack.json` (base tag/commit, branch, remaining patches), run
   `node --test`, push `sig/<new>` to the `sig` remote, reinstall.
5. Decline Codex's own update prompt for versions the stack has not been
   rolled onto yet; the updater replaces our binary.

Upstream does not accept external pull requests (`docs/contributing.md` in
openai/codex). SIG patches go upstream as issue reports with root cause and
proposed fix; drafts live in `docs/upstream-issues/`.
