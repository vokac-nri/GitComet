use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::path_utils::git_dir_for_workdir;
use std::path::Path;

/// Open the repository backing the worktree at `workdir`.
///
/// This is the single point in the crate that turns a worktree path into an
/// open [`gix::Repository`]. It routes through [`git_dir_for_workdir`] so that a
/// worktree whose directory ends in `.git` (e.g. `/path/myrepo.git`) is opened
/// via its inner `.git` entry rather than being misread by gix as a bare git
/// directory.
///
/// gix 0.85 offers no open-option to force this: when a path ends in `.git` it
/// sets `looks_like_git_dir` and refuses to append `.git`, and `open_path_as_is`
/// only governs the opposite branch. Resolving the path ourselves is therefore
/// the intended fix — so keep every worktree open going through here.
///
/// The raw [`gix::open::Error`] is returned so callers can map it to their own
/// error type or treat "not a repository" as absence.
// This is a thin forwarder over `gix::open`, which itself returns the large
// `gix::open::Error` by value; boxing here would only complicate every caller's
// match without a real payoff.
#[allow(clippy::result_large_err)]
pub(crate) fn open_worktree_repo(
    workdir: &Path,
) -> std::result::Result<gix::Repository, gix::open::Error> {
    gix::open_opts(git_dir_for_workdir(workdir), open_options())
}

/// The options every repository in this crate is opened with.
///
/// The only departure from `gix::open`'s defaults is that repository-local
/// configuration is always readable. gix derives a trust level from the *owner*
/// of the git directory, and at anything below `Trust::Full` it silently drops
/// every config section whose source is the repository itself — `remote.*`,
/// `branch.*`, `core.*`, all of it. Values are still parsed, so the repository
/// opens and reads fine; it just looks like nobody ever configured a remote.
///
/// Windows produces such directories routinely: anything created by an elevated
/// process is owned by `BUILTIN\Administrators` rather than by the user, and on
/// domain-joined machines that includes ordinary `git init` and `git clone`
/// output. gix's ownership probe rejects that owner for a user who is not in
/// the Administrators group, while the `git` CLI works in the very same
/// repository without a word about dubious ownership.
///
/// That divergence is the bug: GitComet drives the `git` CLI for every
/// operation that touches a remote, and that CLI already reads this exact
/// config with full trust (still gated by its own `safe.directory` check), so
/// reading it here extends no trust the process was not extending already. This
/// crate's use of gix is read-only — refs, objects, the index, config values —
/// and never spawns a program named by configuration.
fn open_options() -> gix::open::Options {
    gix::open::Options::default().filter_config_section(trust_repository_config)
}

/// Accept every configuration section, including repository-local ones that
/// gix's default filter would drop at reduced trust. See [`open_options`].
fn trust_repository_config(_meta: &gix::config::file::Metadata) -> bool {
    true
}

/// Translate a failed [`open_worktree_repo`] into the crate's error type.
///
/// `context` names the operation that was opening the repository and is only
/// used for the catch-all `Backend` message; the two cases callers act on —
/// "not a repository" and I/O — map to their own kinds so they stay
/// distinguishable. Callers that treat a missing repository as absence rather
/// than an error match on [`gix::open::Error`] themselves instead.
#[allow(clippy::result_large_err)]
pub(crate) fn map_open_error(error: gix::open::Error, context: &str) -> Error {
    match error {
        gix::open::Error::NotARepository { .. } => Error::new(ErrorKind::NotARepository),
        gix::open::Error::Io(io) => Error::new(ErrorKind::Io(io.kind())),
        error => Error::new(ErrorKind::Backend(format!("{context}: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::open_worktree_repo;
    use std::path::Path;
    use std::process::Command;

    fn run_git(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    fn repo_with_origin(root: &Path) -> std::path::PathBuf {
        let repo = root.join("work");
        std::fs::create_dir_all(&repo).expect("create work dir");
        run_git(&repo, &["init"]);
        run_git(
            &repo,
            &["remote", "add", "origin", "https://example.invalid/x.git"],
        );
        repo
    }

    /// A git directory owned by someone other than the current user (on Windows,
    /// anything created by an elevated process is owned by `BUILTIN\Administrators`)
    /// drops gix to reduced trust, and gix's default section filter then hides every
    /// repository-local config section. The repository still opens, so the failure is
    /// silent: no remotes, no upstreams, and `push` falls back to a bare `git push`.
    /// Trust cannot be forced low by owning a directory differently in a test, so pin
    /// the filtering behaviour by asking gix for reduced trust directly.
    #[test]
    fn repository_local_config_survives_reduced_trust() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let repo = repo_with_origin(dir.path());

        let untrusted = gix::open_opts(
            &repo,
            gix::open::Options::default().with(gix::sec::Trust::Reduced),
        )
        .expect("open with gix defaults");
        assert!(
            untrusted.remote_names().is_empty(),
            "precondition: gix's default filter hides repository-local config at reduced trust"
        );

        let trusted = gix::open_opts(&repo, super::open_options().with(gix::sec::Trust::Reduced))
            .expect("open with crate options");
        assert!(
            trusted.remote_names().iter().any(|name| name == "origin"),
            "crate options must keep repository-local config readable, got {:?}",
            trusted.remote_names()
        );
    }

    /// The same thing through the real entry point, on a repository whose ownership
    /// the test does not control.
    #[test]
    fn open_worktree_repo_sees_configured_remotes() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let repo = repo_with_origin(dir.path());

        let opened = open_worktree_repo(&repo).expect("open worktree repo");
        assert!(
            opened.remote_names().iter().any(|name| name == "origin"),
            "expected origin, got {:?}",
            opened.remote_names()
        );
    }
}
