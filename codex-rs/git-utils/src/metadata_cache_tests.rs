use std::path::Path;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::MetadataKind;
use super::cached_git_metadata;
use super::fingerprint_repo;
use super::fingerprint_repo_with_globals;
use super::is_cached;
use crate::get_git_remote_urls_assume_git_repo;
use crate::get_head_commit_hash;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .env("GIT_CONFIG_NOSYSTEM", "1")
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
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git stdout is UTF-8")
        .trim()
        .to_string()
}

fn repo_with_commit() -> TempDir {
    let repo = TempDir::new().expect("create repository");
    git(repo.path(), &["init", "-q", "--initial-branch=main"]);
    git(
        repo.path(),
        &["commit", "--allow-empty", "-q", "-m", "first"],
    );
    repo
}

#[tokio::test]
async fn concurrent_identical_lookups_run_once() {
    let repo = repo_with_commit();
    let runs = std::sync::Arc::new(AtomicUsize::new(0));
    let lookup = || {
        let runs = std::sync::Arc::clone(&runs);
        cached_git_metadata(repo.path(), MetadataKind::RemoteUrls, move || async move {
            runs.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Some(7_u32)
        })
    };

    let results = tokio::join!(lookup(), lookup(), lookup());

    assert_eq!(
        (results, runs.load(Ordering::SeqCst)),
        ((Some(7), Some(7), Some(7)), 1)
    );
}

#[tokio::test]
async fn failed_lookups_are_not_cached() {
    let repo = repo_with_commit();
    let first = cached_git_metadata(repo.path(), MetadataKind::HeadCommit, || async {
        None::<u32>
    })
    .await;
    let cached_after_failure = is_cached(repo.path(), MetadataKind::HeadCommit);
    let second = cached_git_metadata(repo.path(), MetadataKind::HeadCommit, || async {
        Some(3_u32)
    })
    .await;

    assert_eq!(
        (first, cached_after_failure, second),
        (None, false, Some(3))
    );
}

#[tokio::test]
async fn head_commit_follows_new_commits_and_branch_switches() {
    let repo = repo_with_commit();
    let path = repo.path();

    let first = get_head_commit_hash(path).await.map(|sha| sha.0);
    let cached_while_unchanged = is_cached(path, MetadataKind::HeadCommit);

    git(path, &["commit", "--allow-empty", "-q", "-m", "second"]);
    let cached_after_commit = is_cached(path, MetadataKind::HeadCommit);
    let after_commit = get_head_commit_hash(path).await.map(|sha| sha.0);
    let expected_after_commit = git(path, &["rev-parse", "HEAD"]);

    git(path, &["checkout", "-q", "-b", "feature", "HEAD~1"]);
    let after_switch = get_head_commit_hash(path).await.map(|sha| sha.0);

    git(path, &["checkout", "-q", "--detach", "main"]);
    let after_detach = get_head_commit_hash(path).await.map(|sha| sha.0);

    git(path, &["checkout", "-q", "main"]);
    git(path, &["pack-refs", "--all"]);
    git(path, &["reset", "-q", "--hard", "HEAD~1"]);
    let after_reset = get_head_commit_hash(path).await.map(|sha| sha.0);

    assert_eq!(
        (
            cached_while_unchanged,
            cached_after_commit,
            after_commit.clone(),
            after_switch,
            after_detach,
            after_reset,
        ),
        (
            true,
            false,
            Some(expected_after_commit.clone()),
            first.clone(),
            Some(expected_after_commit),
            first,
        )
    );
}

#[tokio::test]
async fn remote_urls_follow_remote_changes() {
    let repo = repo_with_commit();
    let path = repo.path();

    let none = get_git_remote_urls_assume_git_repo(path).await;
    // A repository without remotes is a cached answer, not a failure.
    let cached_without_remotes = is_cached(path, MetadataKind::RemoteUrls);

    git(
        path,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/org/one.git",
        ],
    );
    let added = get_git_remote_urls_assume_git_repo(path).await;

    git(
        path,
        &[
            "remote",
            "set-url",
            "origin",
            "https://example.invalid/org/two.git",
        ],
    );
    let changed = get_git_remote_urls_assume_git_repo(path).await;

    // Commits do not invalidate remote answers.
    git(path, &["commit", "--allow-empty", "-q", "-m", "second"]);
    let cached_after_commit = is_cached(path, MetadataKind::RemoteUrls);

    let origin = |remotes: Option<std::collections::BTreeMap<String, crate::SanitizedGitUrl>>| {
        remotes.and_then(|remotes| remotes.get("origin").map(|url| url.as_str().to_string()))
    };
    assert_eq!(
        (
            none,
            cached_without_remotes,
            origin(added),
            origin(changed),
            cached_after_commit,
        ),
        (
            None,
            true,
            Some("https://example.invalid/org/one.git".to_string()),
            Some("https://example.invalid/org/two.git".to_string()),
            true,
        )
    );
}

#[tokio::test]
async fn linked_worktrees_track_their_own_head() {
    let repo = repo_with_commit();
    let worktree_parent = TempDir::new().expect("create worktree parent");
    let worktree = worktree_parent.path().join("linked");
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            worktree.to_str().expect("worktree path is UTF-8"),
        ],
    );

    let before = get_head_commit_hash(&worktree).await.map(|sha| sha.0);
    git(
        &worktree,
        &["commit", "--allow-empty", "-q", "-m", "linked"],
    );
    let after = get_head_commit_hash(&worktree).await.map(|sha| sha.0);
    let main = get_head_commit_hash(repo.path()).await.map(|sha| sha.0);

    assert_eq!(
        (before.clone(), after, main),
        (
            before,
            Some(git(&worktree, &["rev-parse", "HEAD"])),
            Some(git(repo.path(), &["rev-parse", "main"])),
        )
    );
}

#[test]
fn global_config_edits_and_includes_change_the_fingerprint() {
    let repo = repo_with_commit();
    let home = TempDir::new().expect("create home");
    let global = home.path().join(".gitconfig");
    let fingerprint = || {
        fingerprint_repo_with_globals(repo.path(), false, vec![global.clone()])
            .map(|entry| entry.fingerprint)
    };

    let missing = fingerprint();
    std::fs::write(
        &global,
        "[url \"https://example.invalid/\"]\n\tinsteadOf = https://short.invalid/\n",
    )
    .expect("write global config");
    let written = fingerprint();
    std::fs::write(&global, "[include]\n\tpath = other.gitconfig\n").expect("write include");
    let with_include = fingerprint();

    assert_eq!(
        (
            missing.is_some(),
            written.is_some(),
            missing != written,
            with_include.is_none(),
        ),
        (true, true, true, true)
    );
}

#[test]
fn fresh_packed_refs_are_not_storable() {
    let repo = repo_with_commit();
    git(repo.path(), &["pack-refs", "--all"]);

    let entry = fingerprint_repo(repo.path(), true).expect("fingerprint repository");

    assert!(!entry.storable);
}

#[test]
fn repositories_outside_git_are_not_fingerprinted() {
    let dir = TempDir::new().expect("create directory");

    assert!(fingerprint_repo(dir.path(), true).is_none());
}
