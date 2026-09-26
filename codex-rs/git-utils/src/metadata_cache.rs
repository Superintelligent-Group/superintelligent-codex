//! Per-process cache for Git repository metadata that cannot change without a
//! filesystem change.
//!
//! Every turn asks Git for the HEAD commit, the configured remotes and the
//! effective `core.fsmonitor` value, and analytics asks for the remotes again
//! when the turn completes. Each answer costs a `git` process, which is tens of
//! milliseconds on Windows. The answers only change when repository files
//! change, so this module fingerprints those files and reuses a previous answer
//! while the fingerprint is identical.
//!
//! The fingerprint is deliberately a superset of what Git reads:
//!
//! - `HEAD` (the per-worktree one), the loose ref file it points at (both the
//!   per-worktree and the common directory spelling), compared by content;
//! - `packed-refs`, compared by length and modification time. An entry whose
//!   `packed-refs` was modified within [`RACY_WINDOW`] of the lookup is never
//!   stored, mirroring Git's own "racily clean" rule for timestamp granularity;
//! - the repository `config`, `config.worktree`, and every candidate global
//!   config file (`$HOME`, `%USERPROFILE%`, `%HOMEDRIVE%%HOMEPATH%`, XDG),
//!   compared by content, so `url.<base>.insteadOf` rewrites and remote edits
//!   invalidate the entry.
//!
//! Anything the fingerprint cannot describe disables caching for that lookup:
//! `include`/`includeIf` directives in any fingerprinted config, reftable
//! repositories, symbolic refs that point at other symbolic refs, and the
//! environment variables that redirect repository or config discovery. System
//! config is not fingerprinted (its location depends on the Git installation),
//! so every entry also expires after [`MAX_ENTRY_AGE`].
//!
//! Failed Git invocations are never cached. Set
//! `CODEX_DISABLE_GIT_METADATA_CACHE` to any value to bypass the cache.

use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;
use futures::future::WeakShared;

/// Environment variable that bypasses the cache entirely.
pub const DISABLE_GIT_METADATA_CACHE_ENV: &str = "CODEX_DISABLE_GIT_METADATA_CACHE";

/// Environment variables that make Git discover the repository or its config
/// somewhere the fingerprint does not look.
const BYPASS_ENV_VARS: &[&str] = &[
    DISABLE_GIT_METADATA_CACHE_ENV,
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_CONFIG",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_NAMESPACE",
];

/// Files modified this recently may change again within the same timestamp
/// tick without a visible metadata change.
const RACY_WINDOW: Duration = Duration::from_secs(2);

/// Upper bound on how long an entry is trusted, covering inputs the
/// fingerprint cannot see (system config, Git upgrades).
const MAX_ENTRY_AGE: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum MetadataKind {
    HeadCommit,
    RemoteUrls,
    GitInfo,
    FsmonitorOverride,
}

impl MetadataKind {
    /// Whether answers of this kind depend on HEAD and refs. Every kind
    /// depends on config.
    fn depends_on_refs(self) -> bool {
        match self {
            Self::HeadCommit | Self::GitInfo => true,
            Self::RemoteUrls | Self::FsmonitorOverride => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FileStamp {
    Missing,
    Content(Vec<u8>),
    Metadata {
        len: u64,
        modified: Option<SystemTime>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepoFingerprint {
    files: Vec<(PathBuf, FileStamp)>,
}

struct Fingerprinted {
    fingerprint: RepoFingerprint,
    /// False when a timestamp-compared file is too fresh to trust.
    storable: bool,
}

struct CacheEntry {
    fingerprint: RepoFingerprint,
    stored_at: Instant,
    value: Arc<dyn Any + Send + Sync>,
}

type CacheKey = (PathBuf, MetadataKind);
type CachedValue = Arc<dyn Any + Send + Sync>;
type SharedRun = Shared<BoxFuture<'static, Option<CachedValue>>>;

struct InFlight {
    fingerprint: RepoFingerprint,
    run: WeakShared<BoxFuture<'static, Option<CachedValue>>>,
}

#[derive(Default)]
struct MetadataCache {
    entries: Mutex<HashMap<CacheKey, CacheEntry>>,
    in_flight: Mutex<HashMap<CacheKey, InFlight>>,
}

fn cache() -> &'static MetadataCache {
    static CACHE: OnceLock<MetadataCache> = OnceLock::new();
    CACHE.get_or_init(MetadataCache::default)
}

/// Returns the cached value for `(cwd, kind)` when the repository fingerprint
/// is unchanged, otherwise runs `compute` and caches a successful (`Some`)
/// result. Concurrent lookups for the same key and fingerprint share one run
/// so identical Git invocations within a turn execute once.
pub(crate) async fn cached_git_metadata<T, F, Fut>(
    cwd: &Path,
    kind: MetadataKind,
    compute: F,
) -> Option<T>
where
    T: Clone + Send + Sync + 'static,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Option<T>> + Send + 'static,
{
    if cache_bypassed_by_env() {
        return compute().await;
    }

    // Fingerprint before running Git: if files change while Git runs, the
    // stored fingerprint is older than the value and the next lookup misses.
    let Some(Fingerprinted {
        fingerprint,
        storable,
    }) = fingerprint_repo(cwd, kind.depends_on_refs())
    else {
        return compute().await;
    };

    let key = (cwd.to_path_buf(), kind);
    if let Some(value) = lookup(&key, &fingerprint) {
        return Some(value);
    }

    let run = shared_run(key, fingerprint, storable, compute);
    run.await?.downcast_ref::<T>().cloned()
}

/// Joins an outstanding run for the same key and fingerprint, or starts one
/// that stores its successful result.
fn shared_run<T, F, Fut>(
    key: CacheKey,
    fingerprint: RepoFingerprint,
    storable: bool,
    compute: F,
) -> SharedRun
where
    T: Clone + Send + Sync + 'static,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Option<T>> + Send + 'static,
{
    let mut in_flight = cache()
        .in_flight
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(run) = in_flight
        .get(&key)
        .filter(|entry| entry.fingerprint == fingerprint)
        .and_then(|entry| entry.run.upgrade())
        .filter(|run| run.peek().is_none())
    {
        return run;
    }
    in_flight.retain(|_, entry| entry.run.upgrade().is_some_and(|run| run.peek().is_none()));

    let compute = compute();
    let store_key = key.clone();
    let store_fingerprint = fingerprint.clone();
    let run = async move {
        let value: CachedValue = Arc::new(compute.await?);
        if storable {
            cache()
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(
                    store_key,
                    CacheEntry {
                        fingerprint: store_fingerprint,
                        stored_at: Instant::now(),
                        value: Arc::clone(&value),
                    },
                );
        }
        Some(value)
    }
    .boxed()
    .shared();
    if let Some(weak) = run.downgrade() {
        in_flight.insert(
            key,
            InFlight {
                fingerprint,
                run: weak,
            },
        );
    }
    run
}

fn lookup<T: Clone + 'static>(key: &CacheKey, fingerprint: &RepoFingerprint) -> Option<T> {
    let mut entries = cache()
        .entries
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = entries.get(key)?;
    if entry.fingerprint != *fingerprint || entry.stored_at.elapsed() > MAX_ENTRY_AGE {
        entries.remove(key);
        return None;
    }
    entry.value.downcast_ref::<T>().cloned()
}

/// Whether a lookup for `(cwd, kind)` would currently be answered from cache.
#[cfg(test)]
fn is_cached(cwd: &Path, kind: MetadataKind) -> bool {
    let Some(entry) = fingerprint_repo(cwd, kind.depends_on_refs()) else {
        return false;
    };
    cache()
        .entries
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&(cwd.to_path_buf(), kind))
        .is_some_and(|cached| {
            cached.fingerprint == entry.fingerprint && cached.stored_at.elapsed() <= MAX_ENTRY_AGE
        })
}

fn cache_bypassed_by_env() -> bool {
    BYPASS_ENV_VARS
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

/// Builds the fingerprint for the repository that contains `cwd`, or `None`
/// when the repository layout is outside what the fingerprint can describe.
fn fingerprint_repo(cwd: &Path, include_refs: bool) -> Option<Fingerprinted> {
    // Unit tests must not depend on the developer's global Git config (an
    // `include` there would disable caching); they cover global files through
    // `fingerprint_repo_with_globals` directly.
    let globals = if cfg!(test) {
        Vec::new()
    } else {
        global_config_candidates()
    };
    fingerprint_repo_with_globals(cwd, include_refs, globals)
}

fn fingerprint_repo_with_globals(
    cwd: &Path,
    include_refs: bool,
    global_configs: Vec<PathBuf>,
) -> Option<Fingerprinted> {
    let base = if cwd.is_dir() { cwd } else { cwd.parent()? };
    let (repo_root, dot_git) = crate::info::find_ancestor_git_entry(base)?;
    let git_dir = resolve_git_dir(&repo_root, &dot_git)?;
    let common_dir = resolve_common_dir(&git_dir)?;
    if git_dir.join("reftable").exists() || common_dir.join("reftable").exists() {
        return None;
    }

    let mut files = Vec::new();
    let mut storable = true;
    if include_refs {
        fingerprint_refs(&git_dir, &common_dir, &mut files, &mut storable)?;
    }

    let mut config_files = vec![common_dir.join("config"), git_dir.join("config.worktree")];
    config_files.extend(global_configs);
    config_files.dedup();
    for path in config_files {
        let stamp = content_stamp(&path)?;
        if let FileStamp::Content(bytes) = &stamp
            && contains_include_directive(bytes)
        {
            return None;
        }
        files.push((path, stamp));
    }

    Some(Fingerprinted {
        fingerprint: RepoFingerprint { files },
        storable,
    })
}

/// Adds HEAD, the loose ref it points at and `packed-refs`.
fn fingerprint_refs(
    git_dir: &Path,
    common_dir: &Path,
    files: &mut Vec<(PathBuf, FileStamp)>,
    storable: &mut bool,
) -> Option<()> {
    let head_path = git_dir.join("HEAD");
    let head = std::fs::read(&head_path).ok()?;
    if let Some(refname) = head.trim_ascii().strip_prefix(b"ref:") {
        let refname = std::str::from_utf8(refname.trim_ascii()).ok()?;
        if refname.is_empty() || refname.split('/').any(|part| part == "..") {
            return None;
        }
        for dir in [git_dir, common_dir] {
            let ref_path = dir.join(refname);
            let stamp = content_stamp(&ref_path)?;
            if let FileStamp::Content(bytes) = &stamp
                && bytes.trim_ascii().starts_with(b"ref:")
            {
                return None;
            }
            files.push((ref_path, stamp));
        }
    }
    files.push((head_path, FileStamp::Content(head)));

    let packed_refs = common_dir.join("packed-refs");
    let (stamp, fresh) = metadata_stamp(&packed_refs)?;
    *storable &= !fresh;
    files.push((packed_refs, stamp));
    Some(())
}

fn resolve_git_dir(repo_root: &Path, dot_git: &Path) -> Option<PathBuf> {
    let metadata = std::fs::metadata(dot_git).ok()?;
    if metadata.is_dir() {
        return Some(dot_git.to_path_buf());
    }
    let contents = std::fs::read_to_string(dot_git).ok()?;
    let target = contents.trim().strip_prefix("gitdir:")?.trim();
    if target.is_empty() || target.contains('\n') {
        return None;
    }
    let git_dir = repo_root.join(target);
    git_dir.is_dir().then_some(git_dir)
}

fn resolve_common_dir(git_dir: &Path) -> Option<PathBuf> {
    match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(contents) => {
            let target = contents.trim();
            if target.is_empty() || target.contains('\n') {
                return None;
            }
            let common_dir = git_dir.join(target);
            common_dir.is_dir().then_some(common_dir)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(git_dir.to_path_buf()),
        Err(_) => None,
    }
}

fn global_config_candidates() -> Vec<PathBuf> {
    let mut homes = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        homes.push(PathBuf::from(home));
    }
    if cfg!(windows) {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            homes.push(PathBuf::from(profile));
        }
        if let (Some(drive), Some(path)) =
            (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
        {
            let mut home = drive;
            home.push(path);
            homes.push(PathBuf::from(home));
        }
    }

    let mut candidates = Vec::new();
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(xdg).join("git").join("config"));
    }
    for home in homes {
        candidates.push(home.join(".config").join("git").join("config"));
        candidates.push(home.join(".gitconfig"));
    }
    candidates
}

/// Reads a small file. Missing files are part of the fingerprint; other read
/// errors make the repository uncacheable.
fn content_stamp(path: &Path) -> Option<FileStamp> {
    match std::fs::read(path) {
        Ok(bytes) => Some(FileStamp::Content(bytes)),
        Err(error) if is_missing(&error) => Some(FileStamp::Missing),
        Err(_) => None,
    }
}

/// Stats a potentially large file. Returns whether it is too fresh to trust.
fn metadata_stamp(path: &Path) -> Option<(FileStamp, bool)> {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata.modified().ok();
            let fresh = modified.is_none_or(|modified| {
                SystemTime::now()
                    .duration_since(modified)
                    .map_or(true, |age| age < RACY_WINDOW)
            });
            Some((
                FileStamp::Metadata {
                    len: metadata.len(),
                    modified,
                },
                fresh,
            ))
        }
        Err(error) if is_missing(&error) => Some((FileStamp::Missing, false)),
        Err(_) => None,
    }
}

fn is_missing(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
    )
}

fn contains_include_directive(config: &[u8]) -> bool {
    config
        .windows(b"include".len())
        .any(|window| window.eq_ignore_ascii_case(b"include"))
}

#[cfg(test)]
#[path = "metadata_cache_tests.rs"]
mod tests;
