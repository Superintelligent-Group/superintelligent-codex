# SIG Codex Harness

This repository is a small, provider-neutral context and session harness for
SIG's governed pods. It is intentionally separate from the upstream Codex
source tree at `C:\\Github\\superintelligent-codex` so that we can improve
session ergonomics without creating a long-lived fork of Codex's product code.

The first slice is a deterministic context packet reducer. It keeps the facts
that must survive compaction—objective, invariants, blockers, next actions,
decisions, evidence references, and budget lineage—in a compact, hashable
packet. Codex, Claude Code, and cloud workers can consume the same packet.

## Invariants

- Compaction may remove detail, but never removes an invariant, unresolved
  blocker, or owner-visible next action.
- Every packet has a stable content hash and a parent hash when it is derived.
- Evidence references remain pointers; the harness never turns an assertion
  into proof.
- Provider identity, spend reservation, lease, and session identifiers are
  metadata only and never authorize work by themselves.
- A missing or malformed packet fails closed.
- A byte budget may remove only evidence and decision history; if the mandatory
  envelope cannot fit, compaction fails closed instead of silently dropping it.

## Thin slice

```text
SIG work-session entry point
        -> context packet (JSON + hash)
        -> Codex or Claude session
        -> checkpoint / result packet
        -> next packet after compaction
```

The next integration is to attach this packet to SUP-1092 work-session
commands and persist its hash beside the existing command/run/seat lineage.
The harness does not claim settlement or provider usage; those remain governed
by the SIG control plane and the provider receipt path.

`compactPacket(packet, { maxBytes })` gives each session a deterministic context
budget. It drops oldest evidence first and then oldest decisions, while keeping
the authoritative facts and parent hash lineage intact.

## Upstream relationship

The current upstream Codex checkout is pinned by commit in
`docs/upstream-codex.md`. The harness consumes stable session artifacts and
does not patch upstream until an integration seam has a focused test and a
measured benefit.

## Development

```powershell
node --test
```
