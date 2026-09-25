# Draft issue for openai/codex

**Title:** app-server `account/login` with `apiKey` overwrites a persisted ChatGPT login shared by every Codex process

**Labels to suggest:** bug, app-server, auth

## What version of Codex CLI is running?

codex-cli 0.157.0 (standalone installer, Windows 11 x64). The code path is
unchanged on `main` as of 4b9e0cc.

## What issue are you seeing?

After using Codex through an ACP client (Zed's `@agentclientprotocol/codex-acp`
1.12.0), the plain CLI and the background app-server daemon start failing
every turn with:

```
unexpected status 401 Unauthorized: Incorrect API key provided: sk-svcac…,
url: https://chatgpt.com/backend-api/codex/responses
```

The user had logged in with ChatGPT (`codex login`) minutes earlier, and
`auth_mode` in `~/.codex/auth.json` had been `chatgpt`.

## Root cause

`codex-acp`'s `authenticate("api-key")` calls app-server `account/login`
with `{ type: "apiKey", apiKey }`, taking the key from `CODEX_API_KEY` /
`OPENAI_API_KEY` when the client does not supply one.

`AccountRequestProcessor::login_api_key_common`
(`app-server/src/request_processors/account_processor.rs`) passes that to
`login_with_api_key(codex_home, api_key, config.cli_auth_credentials_store_mode, …)`,
which writes a fresh `AuthDotJson { auth_mode: ApiKey, tokens: None, … }` to
the persistent store. That replaces the ChatGPT tokens in the `auth.json`
shared by the CLI, the daemon, and the desktop app.

So one editor integration logging in with an API key silently logs every
other Codex surface on the machine out of ChatGPT. It also sends the API key
to the ChatGPT backend, which rejects it.

## Steps to reproduce

1. `codex login` with ChatGPT. Confirm `auth_mode: chatgpt` in `~/.codex/auth.json`.
2. With an API key in the environment, start any app-server client that
   sends `account/login { type: "apiKey" }` (e.g. Zed + codex-acp using its
   API-key auth method).
3. Run plain `codex` and send a prompt: 401 as above. `auth.json` now has
   `auth_mode: apikey` and no tokens.

## Expected behavior

An app-server client's API-key login should not destroy a persisted ChatGPT
login it did not create. The TUI treats `CODEX_API_KEY` as a per-process
override (`load_auth(enable_codex_api_key_env)` checks it before storage and
never persists it), and this path should behave the same way.

## Proposed fix

In `login_api_key_common`, when the persisted store already holds ChatGPT
tokens, save the API key with `AuthCredentialsStoreMode::Ephemeral`. The
existing precedence in `load_auth` already prefers the ephemeral store, so
that app-server process uses the key and everything else keeps ChatGPT. We
run this as a local patch:
https://github.com/Superintelligent-Group/superintelligent-codex/tree/sig/0.157.0
(commit "app-server: keep API-key logins ephemeral when a ChatGPT login is
persisted").

An explicit alternative would be a `persist: bool` on `LoginApiKeyParams`,
defaulting to false for app-server clients.
