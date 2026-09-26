# Draft issue for openai/codex

**Title:** app-server `account/login/start {type: "apiKey"}` overwrites the persisted ChatGPT login shared by every Codex surface

## What version of Codex CLI is running?

codex-cli 0.157.0 and 0.157.1 (standalone installer, Windows 11 x64). The
code path is unchanged on `main`.

## What subscription do you have?

ChatGPT (signed in with `codex login`).

## What issue are you seeing?

When any app-server client sends `account/login/start` with
`{ "type": "apiKey", "apiKey": ... }`, the app-server writes a fresh
API-key-only `auth.json` to the persistent credential store. That replaces the
ChatGPT tokens that the CLI, the background daemon and the desktop app all
share.

One client choosing API-key auth for its own session silently switches every
other Codex surface on the machine from ChatGPT to that API key. Every
surface then bills to the API platform instead of the ChatGPT plan, and all of
them fail at once if the key is invalid or revoked.

Editor integrations built on the app-server protocol hit this path. For
example, `@agentclientprotocol/codex-acp` implements its `api-key` auth
method as `account/login/start {type: "apiKey"}`, falling back to
`CODEX_API_KEY` / `OPENAI_API_KEY`.

## Steps to reproduce

Hermetic, using a throwaway `CODEX_HOME` and a synthetic, never-sent ChatGPT
`auth.json`:

1. Seed `$CODEX_HOME/auth.json` with `auth_mode: "chatgpt"` and tokens.
2. Start `codex app-server` with that `CODEX_HOME`, then `initialize`.
3. Send `account/login/start { "type": "apiKey", "apiKey": "sk-dummy" }`.
4. Read `$CODEX_HOME/auth.json`.

**Actual:** `auth.json` now has `auth_mode: "apikey"` and no ChatGPT tokens.
**Expected:** the ChatGPT login persisted for other processes is unchanged,
while this session uses the API key (`account/read` reports `apiKey`).

The script we used:
https://github.com/Superintelligent-Group/superintelligent-codex/blob/sig/harness/scripts/e2e-codex-auth-isolation.mjs

Stock 0.157.0 result:
`{"sessionAccount":"apiKey","persistedAuthMode":"apikey","authJsonUnchanged":false}`

## Root cause

`login_api_key_common` in
`app-server/src/request_processors/account_processor.rs` passes the key to
`login_with_api_key(codex_home, api_key, config.cli_auth_credentials_store_mode, ...)`.
That persists `AuthDotJson { auth_mode: ApiKey, tokens: None, ... }` to the
same store the ChatGPT login lives in.

By contrast, `CODEX_API_KEY` is treated as a per-process override:
`load_auth(enable_codex_api_key_env)` checks it before storage and never
persists it. An app-server client's API-key login should behave the same way.

## Proposed fix

In `login_api_key_common`, when the persisted store already holds ChatGPT
tokens, save the API key with `AuthCredentialsStoreMode::Ephemeral`. The
existing precedence in `load_auth` already prefers the ephemeral store, so
that app-server process uses the key and everything else keeps ChatGPT.

We run this as a patch with a regression test
(`login_account_api_key_keeps_persisted_chatgpt_login` in
`app-server/tests/suite/v2/account.rs`). With it, the same script reports
`{"sessionAccount":"apiKey","persistedAuthMode":"chatgpt","authJsonUnchanged":true}`.
Patch:
https://github.com/Superintelligent-Group/superintelligent-codex/commits/sig/0.157.1

An explicit alternative is a `persist: bool` on `LoginApiKeyParams`,
defaulting to false for app-server clients.

## Not the cause of the 2026-09-25 `sk-svcacct` 401 outage

We first suspected this bug in the widespread `Incorrect API key provided:
sk-svcacct...` 401 from 2026-09-25 (#48237), but that was the server-side
outage. This report is only about the local credential overwrite.
