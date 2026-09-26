# SIG Codex substrate: fast, assured, programmable context

Status: design, 2026-09-25. Source evidence: the three research reports behind
this doc (Codex app-server protocol and context internals at `sig/0.157.0`,
the swarm sandbox's context/world-model code, Superintelligent-CLM). Paths are
relative to each repo. Nothing here is built yet unless marked **shipped**.

## Thesis

Run Codex's app-server as SIG's execution substrate: one long-lived server,
driven over its JSON-RPC protocol by a SIG client that is also the approver.
Every fork patch is a config key whose default is upstream behavior. The event
stream becomes a hash-chained ledger and a proof bundle. Context becomes a
typed, budgeted, level-of-detail (LOD) structure instead of free-text
summaries, with a lossless archive behind it.

```text
SIG control plane (budgets, leases, policy)
        |  context packet (hash)
        v
sig-codex-client ── JSON-RPC ──> codex app-server (patched, config-gated)
   | approver: item/*/requestApproval          | SIG extension (compiled in):
   | ledger:   item/*, turn/*, tokenUsage      |   context contributor (LOD)
   v                                           |   compaction strategy (typed patch)
proof bundle = packet hash + config hash       |   turn admission (budget gate)
             + ledger head + git tree          v
             + validate --attest result    model tier: frontier | local GPU
                                           + CLM scorer (rank / verify / route)
```

## Layer 0: performance (Amdahl first)

Measure, then attack the largest share. The log DB (`~/.codex/logs_2.sqlite`)
timestamps every event and serves as the profiler.

**Shipped on `sig/0.157.0`:**

- Stale-while-revalidate model list.
- mimalloc on Windows.
- Marketplace negative cache.
- Windowless console.
- PowerShell version probe prewarmed at session start.
- Skills shadow selection off the critical path.
- Runtime SQLite DBs opened concurrently.

**In progress:** websocket close handling with a per-turn (not per-session)
HTTP fallback, and a pooled HTTP client.

A PowerShell *parser* prewarm is not needed: production uses an in-process
tree-sitter parser (upstream #39602), so no process is spawned.

**Next:** persistent shell (unified exec) as the Windows default, then
marketplace check interval, then merging the app-server's boot `/models` fetch
with the stale-while-revalidate refresh.

Summarization is the biggest Amdahl target once the transport is fixed.
Adopt the sandbox's **two-speed update** (`core/planning/async_compression.py`):

- A deterministic structural record is written at once.
- The LLM summary runs in the background on a frozen snapshot.
- The summary merges only at safe points.

Compaction then leaves the turn's critical path.

## Layer 1: configurability

- Every behavior patch gets a key. Defaults equal upstream, so the fork stays
  drop-in and each patch can be disabled alone.
- Home: a `sig: Option<SigToml>` table beside `features`/`memories` in
  `config/src/config_toml.rs` (~:485), resolved in `core/src/config/mod.rs`
  (~:4237).
- Initial keys:
  - `sig.models.cache_ttl_secs` (`models-manager/src/manager.rs:32`)
  - `sig.models.serve_stale`
  - `sig.plugins.marketplace_auto_upgrade` and
    `sig.plugins.marketplace_check_interval` (`core-plugins/src/manager.rs:2784`)
  - `sig.network.websocket_fallback = "per_turn" | "session"`
  - `sig.windows.windowless_console`
  - `sig.context.strategy` (layer 3)
- Policy that the agent must not override lives in `requirements.toml`:
  sandbox, login methods, network. Tunables are per-thread `config` overrides
  on `thread/start`.

## Layer 2: assurance (usable today, no fork patch needed)

The protocol already has the seams (`app-server-protocol/src/protocol/`):

- **Approver.** The client answers these requests with
  Accept / AcceptForSession / Decline / Cancel, applying SIG policy (budget,
  protected paths, leases) and recording each decision:
  - `item/commandExecution/requestApproval`
  - `item/fileChange/requestApproval`
  - `item/permissions/requestApproval`
- **Admission.** An extension `TurnStartAdmission` gates every turn
  (`ext/extension-api/.../turn_admission.rs:10`). This is the budget gate.
- **Ledger.** Every `item/*`, `turn/*`, `thread/tokenUsage/updated` and
  `serverRequest/resolved` event is appended to the harness's hash-chained
  `session-ledger`.
- **Proof bundle.** The bundle contains:
  - the context packet hash
  - the effective config hash (`config/read` with layers)
  - the ledger head
  - the result git tree hash
  - the `pnpm validate --attest` result, run as a recorded step in the thread
- **Gaps (small fork patches):**
  - Per-turn cost: extend `ThreadTokenUsageUpdatedNotification`. Until then,
    SIG prices tokens itself.
  - Patch approvals carry no diff: correlate by `itemId`, or add the changes
    to `FileChangeRequestApprovalParams` (`item.rs:1627`).

## Layer 3: programmable, compositional context

**Target:** the context is a typed structure the harness composes and the
agent can query, with LOD chosen by salience under a budget.

**Primitives to port from the swarm sandbox** (tested there unless noted):

| Primitive | Source | Role in Codex |
|---|---|---|
| Pinned header charged first | context packet (P8), `prepare_task_context` (P4) | objective, scopes, budget, lessons: never compacted |
| Typed world-model patch, merge by field kind | `core/runtime/world_model.py` (P1) | compaction output is a validated patch, not prose |
| Lessons survive forever | `context_compressor.py` (P3) | stalled-stage lessons {trigger, cause, recommendation} |
| Per-entry lifespans and importance | `agents/coding/context_editor.py` (P5) | tool output becomes a receipt; errors pinned until resolved |
| Lossy prompt, lossless queryable archive | RLM compaction (P6) | full transcript kept; agent re-expands on demand |
| Two-speed update | `async_compression.py` (P2) | layer 0 Amdahl win |

**New work, designed in the sandbox docs but never built there:**

- A per-category budget with min / ideal / max / priority.
- Freshness by content hash with dependency invalidation.

Together these are the auto-LOD engine: each context item exists at several
levels of detail (full, summary, one-line receipt, pointer). A budgeted
knapsack picks one level per item by salience.

**Codex seams:**

- Contribute context with an extension `ContextContributor`,
  `TurnInputContributor` and `ResponseItemInjector`.
- Registration is compiled in at `app-server/src/extensions.rs:67`. That is
  the smallest fork patch.
- Two fork patches are needed for full control:
  1. A `CompactionContributor` hook at `run_auto_compact`
     (`core/src/session/turn.rs:1440`) and `tasks/compact.rs:36`. It returns
     the replacement history, which is our typed patch plus pinned header plus
     recent window.
  2. A prompt-time LOD transform at `session/turn.rs:513-517`, before
     `run_sampling_request`. It rewrites what is sent without mutating
     history, so the archive stays lossless.
- Archive access for the agent is a dynamic tool (`item/tool/call`, executed
  by the client), e.g. `context.expand(id, level)` and `history.search(q)`.
  This is the RLM `SHOW_VARS()/history` pattern, done governed.

## Layer 4: model tiers and the CLM

**Local GPU (RTX 3080, 10 GB).** Codex supports `ollama` / `lmstudio`
providers or any `[model_providers.x]`, but `wire_api` must be `responses`:
the local server has to speak `/v1/responses`. Remote compaction is OpenAI /
Azure / Bedrock only (`model-provider/src/provider.rs:410`), so local threads
use local compaction or our layer-3 hook.

The sandbox proved small models become reliable through protocol, not size:

- one format with enumerated actions
- a forgiving parser
- one worked example
- explicit state signals
- tests run by the runtime, never self-reported

Those fixes live on the sandbox branch `rebase/pr6-hatchet-local-swarm`, not
`dev`. The local tier should get a harness with exactly those properties.

**CLM** (`Contrastive-LM/CLM`, our fork is unmodified). It is a dual-encoder
scorer: frozen Qwen3-8B embeddings plus about 20M-parameter heads, cosine
score, softmax over candidates. README figures: about 28 ms per new state,
1-2 ms cached (RTX 4090). It needs about 16 GB at bf16, so it doesn't fit the
3080 unquantized.

Its value is at **decision points**, a state plus candidates:

1. Best-of-N selection over local-model patches. README: DeepSWE 81.6% on 38
   held-out tasks.
2. Next-action ranking.
3. Context chunk ranking for layer-3 LOD.
4. Escalation routing local → frontier.

"Calibrated" is only a learned temperature; routing needs real calibration
(temperature or isotonic fit, ECE) on our outcomes first.

**Compute plan:**

- **Kaggle (free, about 30 GPU-h/week, 2×T4, fp16).**
  - Validate fp16 against the published bf16 DeepSWE embeddings, then rerun
    their best-of-N eval.
  - Offline study 1: best-of-N over swarm runs with pass/fail labels.
  - Head fine-tunes.
  - No hosted endpoints (Kaggle terms).
- **CommonQuant AWS (`g6.xlarge` / `g5.xlarge`, 24 GB, bf16 as trained,
  scale-to-zero, hard cap)** for live in-loop studies 2-3.

## Build order

Each step is a patch or harness change with a test and a before/after
measurement against stock.

1. Integrate the layer-0 patches; release build; full e2e on the installed
   binary (auth isolation, zero console windows detached, real turn, daemon
   under a job object).
2. `sig-codex-client` (harness, Node):
   - connect, start or fork a thread from a context packet
   - act as the approver
   - write the ledger
   - emit the proof bundle
   - one real job end to end
3. Layer-1 `sig` config table; convert each behavior patch to a key.
4. SIG extension crate: turn admission (budget), token-usage pricing, pinned
   header contributor.
5. Compaction hook plus typed world-model patch plus lessons plus lossless
   archive tool (layer 3 core).
6. LOD engine (multi-level items, budgeted knapsack), first with a heuristic
   salience score, then the CLM ranker.
7. Local tier with the sandbox protocol fixes; CLM best-of-N; calibrated
   escalation.

## Open decisions

- Marketplace check interval default (proposed 6 h).
- CQ spend cap for the live CLM endpoint (proposed $50 for the first study).
- Which swarm runs may leave SIG for Kaggle (scrub secrets; opt-in only).
