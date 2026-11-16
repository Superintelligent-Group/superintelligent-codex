# SIG Codex roll to rust-v0.169.0 (SUP-3170)

Base: `rust-v0.169.0` (`d32063f96c2dff1dc96af839b3952abb6acfeda4`). Source: `rust-v0.157.1..sig/0.157.1` (12 commits, tip `4c6e8cc62364b089b94d466a20235fb1c5d03b16`).

Rows follow the harness ledger, with the missing git-utils patch appended. The design numbered the console/auth-test pair differently; commit order here follows the ledger and source history.

| Old SHA | New SHA | Decision | Evidence |
|---|---|---|---|
| `70849817e86c40793a83eb6a682577a4a4ea3e26` | `ef7f0c480b1507dc16ae3f6f8e4466a9131fb0aa` | pick | **app-server: keep API-key logins ephemeral when a ChatGPT login is persisted**. Tag still passes configured persistent store mode to `login_with_api_key`; regression `login_account_api_key_keeps_persisted_chatgpt_login` checks byte-for-byte auth file preservation through login and logout, and API-key process auth (app-server account gate pending). |
| `b3f7fc95abeac72f71cbcbfe364cab152c3ddec4` | `fdaf18fd995da8184683a3ce1b9f220d3af5a75a` | pick | **cli: give console-less Codex a windowless console on Windows**. Tag has no `windows_console` module or startup allocation; retain root console fix. CLI suite and detached Windows console reproduction remain pending. |
| `510e66c444821bacbe1f7d4a8d4e923053bcfe87` | `0559022be02c95fcb721a5d61fae2e7682be75c9` | pick | **app-server: test that API-key login keeps a persisted ChatGPT login**. Companion reproduction for auth patch; retained against upstream account changes including `faeb379646` (#59823). Account gate pending. |
| `6953235ed86e7437b11f11579bd8dda85a603b10` | `5b50d274666430f08c210ee0c0a93f5f0682edf5` | port | **plugins: stop re-cloning rejected marketplace revisions on every launch**. Upstream `af6e8d42eb` (#58957) caches only name mismatches; missing-manifest rejection still clones. Preserve `c875667eb8` object reuse and `69727ae170` staging cleanup. Port missing-manifest cache with checked-out-revision/symlink checks and hashed sources. Typed I/O failures are not cached; `transient_manifest_read_failure_does_not_block_same_revision_recovery` injects a read failure and retries the same revision. Plugin regression gate pending. |
| `09d997bea5bba90cb7e895c5c2d05bb74b9c66e6` | `c61ecad5f272af41e2ac3fa4e52473399fe2a19f` | port | **models-manager: serve stale model catalog while revalidating in background**. Upstream catalog refactors retain blocking expired-cache fetch in `load_catalog`. Reproduction `stale_cache_is_served_immediately_and_revalidated_in_background`; provider identity/version/revocation tests preserved; new blocked-account-switch regression and identity-scoped background slots. Gate pending. |
| `21b0c2dcb8d18615b1d9bd2b65d74a560a671d27` | `8282ba409658ecd72b6e3385e1dc22670a632bef` | pick | **cli: use mimalloc as the global allocator on Windows**. Tag CLI has no Windows global allocator. Preserve original mimalloc-only-on-Windows behavior; impact remains unmeasured. CLI gate pending; Cargo-generated lockfile closure is committed in this allocator patch: mimalloc 0.1.52 and libmimalloc-sys 0.1.49, plus Cargo workspace version normalization; no existing external dependency drift. |
| `96706c73a398ff80f5de7449c219fbde653297f4` | `b07a070dae105bd19617e9f5b3f3e801f1267a58` | port | **perf(core): prewarm PowerShell version probe at session start**. Tag `powershell_version` still probes on first use; session startup no prewarm. Preserve new shell/credential-broker startup. Unit reproductions in environment_tests exercise shared in-flight probe and parsing; core-lib gate pending. |
| `50d3ec0cac8a68ccc9415abdc59410bcdd233d97` | dropped | drop | **perf(skills): run shadow skill selection off the turn critical path**. Superseded by `0f8df3d2140a0e120361eb4f0f806ce930b8a312` (#49812): background blocking workers, shared capacity=2, frozen inputs and queued invocation observations. Upstream tests retained; skills gate pending. |
| `642610c71e9e12628467f4f9ba553f1f859c05bd` | dropped | drop | **perf(state): open and migrate runtime SQLite DBs concurrently**. `9e77fb4ad5ff4975f25c4d1d121aece3c40c5176` (#59015) changes migration safety; it does NOT parallelize runtime DB opens. Deliberately retain upstream sequential startup and `caae161dd0` (#60466) fail-soft log DB handling rather than re-derive an unmeasured startup optimization. State gate pending. |
| `07ffd242ab7c319230c654a1d35be68f95e4480f` | `7c756f0e12fc69b903e1afe62b9c8ad26c97778a` | port (classifier only) | **fix(websocket): classify server closes and scope HTTP fallback to the turn**. `0a2dda4b5e` (#58950) was reverted by `44604499214999fd863013ab85947a6cb9283391` (#58989). Target tag includes `websocket_fallback_is_sticky_across_turns`. Source-bound Rust policy reproduction in `repro-0.169.0-sticky-fallback.py` extracts immutable-base methods; PASS (1 source-bound policy test, no network). Keep close code/reason and credential classification only; preserve upstream session-scoped fallback. Full network gate pending. |
| `a17f011b5361b8fdd23da367fb8ecb58cd0c1c7b` | `0e565aed99006d78f0feb6aa83e153d73adaf747` | port | **perf(client): reuse pooled HTTP clients for model API requests**. `e343a12d5c` (#57575) and subsequent provider extraction moved construction to `ResolvedProviderRequest::http_transport`. Core now pools using each freshly resolved setup factory, URL, effective redirect policy, default headers, environment, 5m TTL and 16-entry bound. Auth/routing still resolve per attempt. Cache and local SSE tests pending. |
| `4c6e8cc62364b089b94d466a20235fb1c5d03b16` | `de85b6765fdf5388503218454f31b6898a303479` | port | **perf(git-utils): cache per-turn git metadata keyed on repo file fingerprints**. Tag still spawns Git for stable metadata. Preserve upstream `--ignore-submodules=dirty` security policy while caching metadata/fsmonitor probes. `tests/turn_git_spawns.rs` reproduces process counts; fingerprint invalidation tests cover repo/config/ref changes. The continuation review found partial probe failures were cached; retain partial answers but cache only fully populated GitInfo, with a per-probe failure/recovery regression. Git-utils gate pending. |

## Validation

Initial gated review identified three upstream API compile incompatibilities and two behavior issues (cross-account refresh suppression and logout deleting preserved credentials). The respective patch commits now include fixes and regression coverage. The second review confirmed those fixes and found missing lockfile generation (resolved in the allocator patch during continuation) plus transient marketplace I/O rejection caching (fixed with typed classification and a recovery regression). Review receipts bind exact heads in the final stdout report.

After the initial Rusty V8 dependency checkout failed with a Windows path-length error, the retry uses `CARGO_HOME=C:/c169`, a junction to the existing Cargo cache. It changes no global Git settings. Failed setup log: `C:/tmp/sig/fork-roll-0169-check-longpath-fail.log`.

All Cargo commands use `RUSTC_WRAPPER=` (the host's sccache wrapper fails), `CARGO_TARGET_DIR=C:/cargo-target/sig-0.169.0`, `CARGO_BUILD_JOBS=6`, `--locked`, and the MSVC environment. Dependency downloads are setup egress; model/network tests must use loopback fixtures and no live model calls. Continuation gate results and durations are recorded in `C:/tmp/sig/fork-roll-0169b-gates/results.json`; the runner stops at the first failure.

The harness integrated script is a command reference. This roll adds git-utils and CLI coverage and does not restore Cargo.lock after Cargo regenerates it. Green suites will not be rerun.

| Command | Result |
|---|---|
| `cargo check -p codex-cli` | Continuation PASS with --locked and no wrapper (600.9 s) |
| `cargo test -p codex-api` | Continuation PASS with --locked (668.3 s) |
| `cargo test -p codex-core --lib` | Continuation 1: 2895 passed, 3 failed, 3 ignored. Continuation 2: all 3 targeted tests pass on the stack; 2 fail at the same assertions on stock. Full suite not repeated. |
| `cargo test -p codex-core --test all -- websocket_fallback client_websockets models_cache_ttl injected_models_cache` | Continuation 2 PASS: 96 tests |
| `cargo test -p codex-models-manager` | Continuation 2 PASS: 64 tests after test-only port fix; stock PASS: 60 |
| `cargo test -p codex-app-server --test all -- suite::v2::account` | Stock PASS: 83. Replacement-test notification fix and full retry are recorded in the continuation receipt below. |
| `cargo test -p codex-state` | Continuation receipt below |
| `cargo test -p codex-skills-extension` | Continuation receipt below |
| `cargo test -p codex-core-plugins` | Continuation receipt below |
| `cargo test -p codex-git-utils` (includes `--test turn_git_spawns`) | Continuation receipt below |
| `cargo test -p codex-cli` | Continuation receipt below |

The authoritative continuation receipt is `C:/tmp/sig/fork-roll-0169c-handoff.json`, with every attempt in `C:/tmp/sig/fork-roll-0169c-gates/results.json`. It records commands with `--locked`, tested heads, results, counts, durations, and logs; unfinished gates remain explicit.

## Build and review

Release command: `cargo build --release --locked -p codex-cli -p codex-code-mode-host`. Its completion status, binary paths, `--version` results and SHA-256 are recorded in the continuation handoff and final stdout report. No binaries are installed. Tests and version checks use an isolated temporary `CODEX_HOME`.

Gated review uses this branch base, not origin/main or a harness base. Exact final reviewed head and output are recorded in the final stdout report.

The harness ledger and `[sig]` configuration are outside this roll. New commit mapping above is the handoff to R0. This branch may only be pushed to `sig/0.169.0` after all gates pass; otherwise it is a WIP under `sig/0.169.0-wip`.

Continuation review corrections also clear a shadowing ephemeral API key after a successful replacement login (browser/device-code coverage shares the completion path), and preserve fsmonitor probe failure status so a transient fallback is retried. The device-code test covers API-key-to-ChatGPT replacement, and Git cache tests cover per-probe recovery without repository changes. The first core-lib attempt exposed an obsolete test helper; the client patch now uses the current upstream test constructor.

The subsequent review identified that Bazel did not forward macro features to rust_binary. The allocator patch now forwards crate_features to the executable, so CLI mimalloc configuration reaches main.rs. Source configuration validation passed; Bazel compilation is unavailable on this host (neither bazel nor bazelisk is installed).

The next review found unbounded synchronous fingerprint reads of repository metadata. The git-utils patch now rejects nonregular files and symlinks before reading, checks opened file type and size, and caps content at 1 MiB (including growth during reads). HEAD, gitdir and commondir use the same bounded path. Regressions cover oversized files and Unix FIFO/device inputs; Unix-only tests remain unexecuted on this Windows host.

## Continuation 2 evidence (2026-11-15)

The three remaining `turn_metadata::tests` failures were rerun together, serially, on `5f7fdbdb3baf6f281be22e2859233f2f985795c3`: **3 passed, 0 failed** (8.19 s test execution; 41.75 s including Cargo). No source fix or bisect is justified by that passing reproduction. The full core lib suite is not being repeated.

The stock comparison uses a new detached worktree at `C:/tmp/sig/fork-roll-0169c-stock`, tag `rust-v0.169.0` (`d32063f96c2dff1dc96af839b3952abb6acfeda4`), and the independent target directory `C:/cargo-target/stock-0.169.0`. Its initial `cargo test --locked` could not start: the tagged lockfile records workspace versions as `0.0.0`, while the manifests declare `0.169.0`. `cargo update --workspace --offline` normalized that temporary worktree's lockfile using Cargo; all **1,310 external package records are identical**. Stock source remains unchanged. The normalized, locked stock test finished **1 passed, 2 failed** (19.23 s execution; 2351.34 s including the cold build).

| Targeted metadata test | Stack `5f7fdbdb3b` | Stock `d32063f96c` |
|---|---|---|
| `detached_memory_responses_metadata_starts_an_independent_root_turn` | PASS | FAIL: missing `workspace path`, same assertion as continuation 1 |
| `git_enrichment_discovers_the_repository_when_it_starts` | PASS | PASS |
| `turn_metadata_state_preserves_subagent_parent_after_git_enrichment` | PASS | FAIL: 2 s Git enrichment timeout, same assertion as continuation 1 |

Two failures therefore reproduce on stock and are upstream/environmental; the third passes on both trees. This does not establish the precise environmental cause, but provides no failing stack-only case to bisect. No upstream tests, deadlines, or production behavior were changed. The ordered gate ladder resumes at core-network.

The core-network gate passed **96 tests**. Models-manager then failed to compile its tests: the port's switching-account fixture lacked upstream's `has_command_auth`, and two retained originator assertions still accessed fields moved into `inner` by the port. The stock suite passed **60 tests**. The minimal fix updates only those two test files (5 insertions, 2 deletions), folded into the stale-catalog patch with a fixup and interactive autosquash. That checkpoint used patch `2c43515933a7b6221c99b3020a1260bbd910c74b`; current patch identities are in the table above. No production code changed in this continuation. Models-manager then passed its complete suite (**64 tests**, including the port's account-switch and background-refresh regressions). App-server account is compiling; later gates and release remain pending at this checkpoint. Commit identities before and after autosquash are preserved in `C:/tmp/sig/fork-roll-0169c-sha-map.json`.

Continuation logs, exact commands, heads, durations and lockfile audit are under `C:/tmp/sig/fork-roll-0169c-gates/` (`results.jsonl`, `stock-lock-audit.json`, `stock-lock-normalization.diff`). Tests use an isolated `CODEX_HOME`, no API-key environment variables, and their local mock fixtures.

At the 100-minute checkpoint, app-server account had finished **84 passed, 1 failed**. The failure is the port's added `replaces_ephemeral_api_key` device-code case: the test left the API-key `account/login/completed` notification buffered, then consumed its `login_id: None` while expecting the subsequent device-code login ID. The log shows the earlier API-key completion before `account/updated`; upstream's ordinary API-key test explicitly consumes both. The stock account suite is compiling before any fix is applied. Remaining gates, release, and the final-head review have not run. `sig/0.169.0-wip` was verified at checkpoint `8378e86071728bbf9b74b2cd738e2519c431321c`; the release branch remains unpublished.

The stock account suite subsequently passed **83 tests**. The replacement case now consumes and validates the API-key completion before waiting for the device-code completion (13 added test lines); the device-code login-ID and account-mode assertions are retained. The fix is folded into the account-test patch with a fixup and interactive autosquash. No production behavior changed. The complete account gate is being rerun before the remaining gates advance.

That first retry exposed a second buffered notification: an initial ChatGPT account update could be consumed in place of the API-key account update, leaving the latter queued for device-code login. The test now drains through and validates the API-key update before starting device-code login, and checks the active account after completion. The isolated replacement case passed (3.05 s execution); its working-tree trial patch and digest are preserved in the attempt receipt. The complete correction is autosquashed into the account-test patch. The full account retry and later gates run on that rewritten stack; their results and the exact final-head review remain in the authoritative continuation receipt.

The complete account retry passed **85 tests**, and state passed **242 unit tests plus 1 doctest**. The first continuation-2 gated review then returned two P2 findings at `4b06ca403ae7446872c3376c40063a43977bfbd4`: lexical Git-cache discovery could fingerprint a different repository through a linked working directory, and a failed catalog-cache write could allow an older disk snapshot to replace newer in-memory results. The skills gate was interrupted to address review.

Those fixes are folded into their respective patches: Git fingerprint discovery canonicalizes the physical directory first, and catalog cache loading compares snapshots under the write lock while preserving revocation. Regressions cover a directory link between two repositories (a junction on Windows), and successful refresh followed by cache-write and subsequent refresh failures. The affected models-manager and git-utils crate suites and a new exact-head gated review are required after these production changes. The remaining ladder and release status stay explicit in the continuation handoff; the earlier clean review is not treated as approval of this head.

Those affected suites passed: models-manager **65**, git-utils **54 unit tests plus its spawn-count integration test**. The post-review metadata run failed detached memory and repository discovery; the stock rerun failed detached memory only. A cache-bypass diagnostic passed all three, leaving the cache-enabled discovery timeout unclassified. The ladder remains stopped at that discrepancy.

The second review returned two further P2 findings at `5adb657619c16119efaabd2e894e5a7de9f1407c`. The console patch now restores NULL/invalid standard handles exactly after allocation, and the Git-cache patch opens Unix metadata with `O_NONBLOCK | O_NOFOLLOW` before descriptor validation. Cargo regenerated the lockfile offline; its only new edge is the existing `libc` dependency for git-utils on Unix. These last fixes are autosquashed, but their detached-console/Unix-race validation and the remaining ladder/release are unfinished at the budget boundary. A final-head review is attempted within the remaining budget; its actual exit and output are preserved in the handoff. No release or clean-review claim follows from these edits alone.
