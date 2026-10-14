# AGENTS.md

Notes for coding agents and new contributors working in this fork. Tool-agnostic: this is the
authoritative source, and `CLAUDE.md` just imports it.

Build and test commands live in [CONTRIBUTING.md](CONTRIBUTING.md) and are not repeated here.
What follows is the part that is easy to get wrong.

## This is a fork

`origin` is `vokac-nri/GitComet`, forked from `Auto-Explore/GitComet`. PRs target this fork's
`dev` branch, not upstream.

Some upstream features are **deliberately absent** because they called upstream-owned endpoints
or advertised. Do not "restore" them:

- **No update check, automatic or manual.** Upstream queries `api.github.com/.../releases/latest`
  (a startup setting plus a "Check for updates" menu and command-palette entry). This fork does
  not track upstream releases, so the request disclosed a launch signal for nothing. The default
  identity's `update_source` is `UpdateSource::Disabled`.
- **No prefilled-issue crash reporting.** Upstream's "Report Issue" button opened a
  `github.com/.../issues/new?...` URL carrying the OS username, local paths and a backtrace in a
  GET query string — sent the instant the browser navigated, before the user saw the form. Crash
  logging itself is unchanged and stays local; the card shows the log path. The default
  identity's `new_issue` link is `None`.
- **Remote markdown-preview images are blocked by default.** Image URLs come from repository
  content, so fetching them is a tracking-pixel vector. This uses upstream's
  `RemoteMarkdownImagePolicy` (Settings → Security / Privacy: always / ask / never) with the
  default flipped to `NeverLoad`. The policy is enforced in `MarkdownPictureContext::resolved_picture`,
  the one path that turns a source into an image element, and `MarkdownRemoteImageAccess::default()`
  is `NeverLoad`, so a new render path fails closed. Sessions that opted in through the fork's
  older on/off setting (`markdown_preview_load_remote_images`) map to `AlwaysLoad`.
- **No edition promotions.** Upstream's status bar carries a "Pro launches in N days" / "Get Pro!"
  chip, a FREE badge, and Discord, website and release links; Settings → Links has a
  "Professional Edition waitlist" row. The status bar shows the mark, name and version as plain
  text, and the default identity's `editions` link is `None`, which hides the waitlist row.
- **No survey prompt.** Upstream shows a startup toast asking users to fill in its Google Forms
  survey. The default identity's `survey` link is `None`, so the prompt never appears.

The app has **no telemetry, analytics, or advertising**, one HTTP stack (`ureq`/`rustls`), and no
raw sockets. Adding an outbound request is a significant change — say so explicitly in the PR.

## Syncing with upstream

Merge, do not rebase: `dev` is published. Add `upstream`
(`https://github.com/Auto-Explore/GitComet.git`), branch off `dev`, and
`git merge --no-ff upstream/dev`.

Conflicts are the easy part. Upstream regularly adds *new* code that reintroduces something above
without touching the fork's lines, and that merges silently. After resolving, grep the tree for
the surfaces above before building:

```bash
git grep -n -E "update_check|check_for_updates|CheckForUpdates|UpdateSource::GitHub|releases/latest|issue_url|new_issue|pro_launch|editions|community_url|survey: Some|http_client|AlwaysLoad"
```

Every hit should be one this file explains. Do not use `git merge -X ours`: it only decides
conflicting hunks, so it lets the silent reintroductions through and drops upstream fixes that
happen to sit inside a conflict.

## CI does not cover what you think it covers

Test jobs go through `scripts/ci/run.py`. The `workspace` context runs every package with
`--no-default-features --features gix,gitcomet-ui-gpui/default`, so the UI crate's full suite does
run in CI, and `rust.yml` gates its clippy (library targets) with `-D warnings`. What is left:

| Gap | Detail |
| --- | --- |
| `crates/gitcomet-app/src/crashlog.rs` | Behind `gitcomet-app`'s `ui-gpui-runtime` feature, which the test contexts do not enable. Clippy compiles it (`rust.yml`, default features); **no crashlog test runs in CI.** |
| `--features benchmarks` code | Built only by `benchmark-targets.yml`. A missing field in `rows/benchmarks/` will not surface in a normal build. |
| `#[cfg(target_os = "macos")]` code | The macOS menu bar (`app/menus.rs`) only compiles on macOS. A Windows or Linux build will not catch an unused parameter or a removed item there. |

This fork also runs less CI than upstream, on purpose:

- **Native tests** run on `ubuntu22-x64`, `macos15-arm64` and `windows-x64` only, and **benchmark
  targets** compile on `ubuntu22-x64` only. Upstream's Linux and Windows ARM, macOS 26, macOS
  Intel and Fedora lanes are removed. The Fedora lane was the only one with `git-annex`, so the
  annex integration tests now skip in CI. Upstream syncs that touch these matrices will
  conflict; keep the trimmed list.
- **Package deployment** (`deploy-*.yml`, `deployment-ci.yml`) and the weekly `perf.yml` are
  disabled in the repo's Actions settings, not deleted, so syncs still merge cleanly. They publish
  to upstream-owned channels with secrets this fork does not have.
- **`release-manual-main.yml` does not work here as-is.** It only runs from `main`, chains into
  the disabled deploy workflows, and `build-release-artifacts.yml` fails without upstream's Azure
  Windows-signing secrets.

So if you touch the crash log, benchmark-gated code, or macOS-only code, **you are the only gate**
(or CI on the right OS is). Run locally:

```bash
cargo fmt --all -- --check
cargo test -p gitcomet-ui-gpui --lib
cargo test -p gitcomet-app                     # default features; the only config that compiles crashlog.rs
cargo clippy -p gitcomet-app --all-targets     # catches dead code CI cannot see
cargo check -p gitcomet-ui-gpui --features benchmarks --all-targets
```

## Known pre-existing test failures

These fail on Windows (Git for Windows, with its default system `core.autocrlf=true`) with
upstream's code, not because of anything this fork changes. Compare failing **test names**
against this list before suspecting your change. `gitcomet-ui-gpui` currently has none.

- `gitcomet-git-gix` lib, `repo::history_transaction::tests`:
  `checked_out_branch_is_synchronized_and_dirty_work_is_preserved` and
  `explicit_range_does_not_move_other_refs_when_update_refs_is_configured`. Line-ending
  conversion from the *system* git config makes the fixture worktree look dirty. They pass with
  `GIT_CONFIG_NOSYSTEM=1`.
- `gitcomet-git-gix` lib: `repo::status::worker_limit::tests::global_attributes_and_nested_overrides_control_the_worker_policy`,
  `repo::log::tests::worktree_file_source_memo_invalidates_on_global_attributes_change` (CRLF in
  the read content) and `repo::line_stats::tests::empty_supplied_status_does_not_launch_clean_filter_or_rescan_worktree`.
  They fail even with `GIT_CONFIG_NOSYSTEM=1`, and also with `open_worktree_repo` swapped back to a
  plain `gix::open`, so the fork's `open.rs` is not the cause. Not yet diagnosed.

Fixing any of these is welcome. Skipping them on Windows without first establishing that
production behaviour is correct is not; the undiagnosed ones may be masking a real Windows bug.

## Verification is expensive; plan for it

`gitcomet-ui-gpui` takes **8–9 minutes** to compile from any touched state (35+ vendored
tree-sitter grammars, ~3,400 tests). The full workspace test run takes **40+ minutes**.

- Use `cargo check -p gitcomet-ui-gpui --all-targets` (~20s warm) while iterating; save
  `cargo test` for when you believe you are done.
- Do not stash-and-rebuild to find out whether a failure is pre-existing. Read the assertion
  output, or check whether the crate can even reach your change through its dependency graph —
  both are seconds, a rebuild is minutes.
- Commit each logical unit as soon as it is verified, before starting the next. Overlapping files
  make two half-finished changes very hard to separate afterwards.
- Kill stale processes before building: a leftover `gitcomet.exe` locks
  `target/debug/gitcomet.exe` and cargo fails with "failed to remove file".

## Commit messages

Explain **why** the change is needed, not just what changed; several commits in this fork's
history are worth reading as examples.

Do not reference AI or agentic coding tools in commit messages, PR titles/descriptions, issue
comments, or branch names — no `Co-Authored-By` trailers, no "generated with" footers, no in-body
mentions. (The README's acknowledgement that AI tools were used on the project is a separate,
project-level statement and stays.)
