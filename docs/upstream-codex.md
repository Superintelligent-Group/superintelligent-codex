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
