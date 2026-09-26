//! Negative cache for remote marketplace revisions that deterministically failed to activate.
//!
//! When a configured Git marketplace's remote revision clones cleanly but is not a valid
//! marketplace (missing manifest, name mismatch), retrying it on every automatic run only
//! repeats the same network clone and the same failure. The rejection is recorded per
//! marketplace and keyed by the configured source plus the rejected remote revision, so a new
//! remote revision or a changed source is always retried.

use super::ConfiguredGitMarketplace;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

/// Lives beside the per-marketplace install roots. Marketplace names cannot contain `.`, so
/// this directory cannot collide with an installed marketplace.
const REJECTED_REVISIONS_DIR: &str = ".upgrade-rejections";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
struct RejectedMarketplaceRevision {
    source: String,
    ref_name: Option<String>,
    sparse_paths: Vec<String>,
    rejected_revision: String,
    reason: String,
    rejected_at: String,
}

/// Returns the recorded rejection reason when `remote_revision` of this exact configured
/// marketplace already failed validation.
pub(super) fn rejected_revision_reason(
    install_root: &Path,
    marketplace: &ConfiguredGitMarketplace,
    remote_revision: &str,
) -> Option<String> {
    let contents = std::fs::read_to_string(rejection_path(install_root, &marketplace.name)).ok()?;
    let record = serde_json::from_str::<RejectedMarketplaceRevision>(&contents).ok()?;
    (record.rejected_revision == remote_revision
        && record.source == marketplace.source
        && record.ref_name == marketplace.ref_name
        && record.sparse_paths == marketplace.sparse_paths)
        .then_some(record.reason)
}

/// Best-effort: failing to persist the rejection only costs a retry on the next run.
pub(super) fn record_rejected_revision(
    install_root: &Path,
    marketplace: &ConfiguredGitMarketplace,
    remote_revision: &str,
    reason: &str,
) {
    let record = RejectedMarketplaceRevision {
        source: marketplace.source.clone(),
        ref_name: marketplace.ref_name.clone(),
        sparse_paths: marketplace.sparse_paths.clone(),
        rejected_revision: remote_revision.to_string(),
        reason: reason.to_string(),
        rejected_at: chrono::Utc::now().to_rfc3339(),
    };
    let path = rejection_path(install_root, &marketplace.name);
    let result = serde_json::to_string_pretty(&record)
        .map_err(std::io::Error::other)
        .and_then(|contents| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, contents)
        });
    if let Err(err) = result {
        warn!(
            marketplace = %marketplace.name,
            path = %path.display(),
            error = %err,
            "failed to record rejected marketplace revision"
        );
    }
}

pub(super) fn clear_rejected_revision(install_root: &Path, marketplace_name: &str) {
    let path = rejection_path(install_root, marketplace_name);
    if let Err(err) = std::fs::remove_file(&path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        warn!(
            marketplace = marketplace_name,
            path = %path.display(),
            error = %err,
            "failed to clear rejected marketplace revision"
        );
    }
}

fn rejection_path(install_root: &Path, marketplace_name: &str) -> PathBuf {
    install_root
        .join(REJECTED_REVISIONS_DIR)
        .join(format!("{marketplace_name}.json"))
}
