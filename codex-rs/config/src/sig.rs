//! `[sig]` table: individually switchable SIG fork optimizations.
//!
//! Every key defaults to `true` (the SIG patch is active). Setting a key to
//! `false` restores the upstream Codex behavior for that optimization, so each
//! patch can be A/B tested on the same binary with `-c sig.<key>=false`.
//!
//! The Windows windowless-console patch runs before configuration is loaded and
//! is therefore controlled by the `SIG_CODEX_WINDOWLESS_CONSOLE=0` environment
//! variable instead of a key in this table.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

/// Raw `[sig]` table as written in `config.toml`.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SigToml {
    /// Serve a stale on-disk model catalog while refreshing it in the background.
    pub models_serve_stale: Option<bool>,
    /// Skip re-cloning marketplace revisions that already failed validation.
    pub marketplace_reject_cache: Option<bool>,
    /// Probe the PowerShell version at session start instead of on the first turn.
    pub powershell_probe_prewarm: Option<bool>,
    /// Run shadow skill selection off the turn critical path.
    pub skills_shadow_offpath: Option<bool>,
    /// Open and migrate the runtime SQLite databases concurrently.
    pub parallel_state_db_open: Option<bool>,
    /// Classify websocket server closes and scope the HTTP fallback to one turn.
    pub websocket_turn_scoped_fallback: Option<bool>,
    /// Reuse pooled HTTP clients for model API requests.
    pub http_client_pool: Option<bool>,
    /// Cache per-turn git metadata keyed on repository file fingerprints.
    pub git_metadata_cache: Option<bool>,
    /// Let the first turn proceed without waiting for optional MCP servers.
    pub mcp_nonblocking_first_turn: Option<bool>,
    /// Derive the prompt cache key from the stable request prefix instead of the thread id.
    pub shared_prompt_cache_key: Option<bool>,
    /// Reuse the session-start world-state snapshot for the first turn.
    pub world_state_once: Option<bool>,
}

/// Resolved `[sig]` settings. `Default` enables every optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SigConfig {
    pub models_serve_stale: bool,
    pub marketplace_reject_cache: bool,
    pub powershell_probe_prewarm: bool,
    pub skills_shadow_offpath: bool,
    pub parallel_state_db_open: bool,
    pub websocket_turn_scoped_fallback: bool,
    pub http_client_pool: bool,
    pub git_metadata_cache: bool,
    pub mcp_nonblocking_first_turn: bool,
    pub shared_prompt_cache_key: bool,
    pub world_state_once: bool,
}

impl SigConfig {
    /// All SIG optimizations enabled (the fork default).
    pub const ALL_ON: Self = Self {
        models_serve_stale: true,
        marketplace_reject_cache: true,
        powershell_probe_prewarm: true,
        skills_shadow_offpath: true,
        parallel_state_db_open: true,
        websocket_turn_scoped_fallback: true,
        http_client_pool: true,
        git_metadata_cache: true,
        mcp_nonblocking_first_turn: true,
        shared_prompt_cache_key: true,
        world_state_once: true,
    };

    /// Every SIG optimization disabled: upstream Codex behavior.
    pub const UPSTREAM: Self = Self {
        models_serve_stale: false,
        marketplace_reject_cache: false,
        powershell_probe_prewarm: false,
        skills_shadow_offpath: false,
        parallel_state_db_open: false,
        websocket_turn_scoped_fallback: false,
        http_client_pool: false,
        git_metadata_cache: false,
        mcp_nonblocking_first_turn: false,
        shared_prompt_cache_key: false,
        world_state_once: false,
    };
}

impl Default for SigConfig {
    fn default() -> Self {
        Self::ALL_ON
    }
}

impl From<SigToml> for SigConfig {
    fn from(toml: SigToml) -> Self {
        let on = Self::ALL_ON;
        Self {
            models_serve_stale: toml.models_serve_stale.unwrap_or(on.models_serve_stale),
            marketplace_reject_cache: toml
                .marketplace_reject_cache
                .unwrap_or(on.marketplace_reject_cache),
            powershell_probe_prewarm: toml
                .powershell_probe_prewarm
                .unwrap_or(on.powershell_probe_prewarm),
            skills_shadow_offpath: toml
                .skills_shadow_offpath
                .unwrap_or(on.skills_shadow_offpath),
            parallel_state_db_open: toml
                .parallel_state_db_open
                .unwrap_or(on.parallel_state_db_open),
            websocket_turn_scoped_fallback: toml
                .websocket_turn_scoped_fallback
                .unwrap_or(on.websocket_turn_scoped_fallback),
            http_client_pool: toml.http_client_pool.unwrap_or(on.http_client_pool),
            git_metadata_cache: toml.git_metadata_cache.unwrap_or(on.git_metadata_cache),
            mcp_nonblocking_first_turn: toml
                .mcp_nonblocking_first_turn
                .unwrap_or(on.mcp_nonblocking_first_turn),
            shared_prompt_cache_key: toml
                .shared_prompt_cache_key
                .unwrap_or(on.shared_prompt_cache_key),
            world_state_once: toml.world_state_once.unwrap_or(on.world_state_once),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn missing_table_enables_every_optimization() {
        assert_eq!(SigConfig::from(SigToml::default()), SigConfig::ALL_ON);
    }

    #[test]
    fn explicit_false_disables_only_that_key() {
        let toml: SigToml = toml::from_str("http_client_pool = false").expect("parse");
        let resolved = SigConfig::from(toml);
        assert_eq!(
            resolved,
            SigConfig {
                http_client_pool: false,
                ..SigConfig::ALL_ON
            }
        );
    }
}
