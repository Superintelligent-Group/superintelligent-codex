#![allow(clippy::expect_used)]

//! Measures how many `git` processes one turn's metadata collection spawns.
//!
//! A turn runs `get_head_commit_hash`, `get_git_remote_urls_assume_git_repo`
//! and `get_has_changes_in_repo` concurrently (core `turn_metadata.rs`), and
//! analytics resolves the remotes again when the turn completes
//! (`accepted_line_repo_hash_for_cwd`). The measurement runs in a child
//! process with an isolated global Git config so the spawn counter only sees
//! this sequence.

use std::path::Path;
use std::process::Command;

use codex_git_utils::DISABLE_GIT_METADATA_CACHE_ENV;
use codex_git_utils::get_git_remote_urls_assume_git_repo;
use codex_git_utils::get_has_changes_in_repo;
use codex_git_utils::get_head_commit_hash;
use codex_git_utils::git_command_spawn_count;
use codex_git_utils::set_git_metadata_cache_enabled;
use pretty_assertions::assert_eq;

const CHILD_REPO_ENV: &str = "CODEX_GIT_SPAWN_TEST_REPO";
/// Makes the child disable the cache through the SIG config switch instead of the env var.
const CHILD_CONFIG_OFF_ENV: &str = "CODEX_GIT_SPAWN_TEST_CONFIG_OFF";

async fn simulated_turn(repo: &Path) -> u64 {
    let before = git_command_spawn_count();
    let (head, remotes, has_changes) = tokio::join!(
        get_head_commit_hash(repo),
        get_git_remote_urls_assume_git_repo(repo),
        get_has_changes_in_repo(repo, repo),
    );
    assert!(head.is_some() && remotes.is_some() && has_changes.is_some());
    assert!(get_git_remote_urls_assume_git_repo(repo).await.is_some());
    git_command_spawn_count() - before
}

fn isolated_git(home: &Path, repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .envs(isolated_env(home))
        .args([
            "-c",
            "user.name=Codex Tests",
            "-c",
            "user.email=codex-tests@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(repo)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

fn isolated_env(home: &Path) -> Vec<(&'static str, std::ffi::OsString)> {
    vec![
        ("HOME", home.as_os_str().to_owned()),
        ("USERPROFILE", home.as_os_str().to_owned()),
        ("XDG_CONFIG_HOME", home.join("xdg").into_os_string()),
        ("GIT_CONFIG_NOSYSTEM", "1".into()),
    ]
}

/// Returns the spawn counts of two consecutive turns.
fn measure(cache_disabled: bool, config_off: bool) -> (u64, u64) {
    let home = tempfile::tempdir().expect("create home");
    let repo = tempfile::tempdir().expect("create repository");
    isolated_git(
        home.path(),
        repo.path(),
        &["init", "-q", "--initial-branch=main"],
    );
    isolated_git(
        home.path(),
        repo.path(),
        &["commit", "--allow-empty", "-q", "-m", "first"],
    );
    isolated_git(
        home.path(),
        repo.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/org/repo.git",
        ],
    );

    let mut child = Command::new(std::env::current_exe().expect("find test executable"));
    child
        .args(["per_turn_git_spawns", "--exact", "--nocapture"])
        .envs(isolated_env(home.path()))
        .env_remove("HOMEDRIVE")
        .env_remove("HOMEPATH")
        .env(CHILD_REPO_ENV, repo.path());
    if cache_disabled {
        child.env(DISABLE_GIT_METADATA_CACHE_ENV, "1");
    } else {
        child.env_remove(DISABLE_GIT_METADATA_CACHE_ENV);
    }
    if config_off {
        child.env(CHILD_CONFIG_OFF_ENV, "1");
    } else {
        child.env_remove(CHILD_CONFIG_OFF_ENV);
    }
    let output = child.output().expect("run child test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "child failed: {stdout}");
    let counts = stdout
        .lines()
        .find_map(|line| line.strip_prefix("GIT_SPAWNS "))
        .expect("child reports spawn counts");
    let (first, second) = counts.split_once(' ').expect("two counts");
    (
        first.parse().expect("first count"),
        second.parse().expect("second count"),
    )
}

#[tokio::test]
async fn per_turn_git_spawns() {
    if let Some(repo) = std::env::var_os(CHILD_REPO_ENV) {
        let repo = Path::new(&repo);
        if std::env::var_os(CHILD_CONFIG_OFF_ENV).is_some() {
            set_git_metadata_cache_enabled(/*enabled*/ false);
        }
        let first = simulated_turn(repo).await;
        let second = simulated_turn(repo).await;
        println!("GIT_SPAWNS {first} {second}");
        return;
    }

    let uncached = measure(/*cache_disabled*/ true, /*config_off*/ false);
    let cached = measure(/*cache_disabled*/ false, /*config_off*/ false);
    // SIG `sig.git_metadata_cache = false` must reproduce the uncached (upstream) spawn pattern.
    let config_off = measure(/*cache_disabled*/ false, /*config_off*/ true);
    assert_eq!(config_off, uncached, "sig.git_metadata_cache = false");
    println!("git spawns per turn (first, steady state): uncached={uncached:?} cached={cached:?}");

    // Uncached: rev-parse HEAD, remote -v, fsmonitor config probe,
    // status --porcelain, and the second remote -v from analytics.
    // Cached: the first turn dedupes the second remote -v; later turns only
    // run status --porcelain.
    assert_eq!((uncached, cached), ((5, 5), (4, 1)));
}
