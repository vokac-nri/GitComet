# AGENTS.md

Notes for coding agents and new contributors working in this fork. Tool-agnostic: this is the
authoritative source, and `CLAUDE.md` just imports it.

Build and test commands live in [CONTRIBUTING.md](CONTRIBUTING.md) and are not repeated here.
What follows is the part that is easy to get wrong.

## This is a fork

`origin` is `vokac-nri/GitComet`, forked from `Auto-Explore/GitComet`. PRs target this fork's
`dev` branch, not upstream.

Some upstream features are **deliberately absent** because they called upstream-owned endpoints.
Do not "restore" them:

- **No startup update check.** Upstream queried `api.github.com/.../releases/latest` on every
  launch. This fork does not track upstream releases, so the request disclosed a launch signal
  for nothing.
- **No prefilled-issue crash reporting.** Upstream's "Report Issue" button opened a
  `github.com/.../issues/new?...` URL carrying the OS username, local paths and a backtrace in a
  GET query string — sent the instant the browser navigated, before the user saw the form. Crash
  logging itself is unchanged and stays local; the card shows the log path.
- **Remote markdown-preview images are off by default.** Image URLs come from repository content,
  so fetching them is a tracking-pixel vector. Gated behind "Load remote images in markdown
  previews" (Settings → Diff). The gate lives in `markdown_preview_image_source`, which every
  renderer and the decode watcher funnel through; `MarkdownPreviewImagePolicy::default()` is
  local-only so a new call site fails closed.

The app has **no telemetry, analytics, or advertising**, one HTTP stack (`ureq`/`rustls`), and no
raw sockets. Adding an outbound request is a significant change — say so explicitly in the PR.

## CI does not cover what you think it covers

Every test and clippy job runs with `--no-default-features --features gix`
(`cross-platform-tests.yml`, `APP_FEATURES`). Consequences:

| Gap | Detail |
| --- | --- |
| `gitcomet-ui-gpui` tests | `--exclude gitcomet-ui-gpui` on every workspace run. Only `smoke_tests::smoke_view_renders_without_panicking` runs in CI. |
| `gitcomet-ui-gpui` clippy | Explicitly skipped (`rust.yml`). `--all-targets -- -D warnings` reports ~49 pre-existing pedantic lints, so that is not a gate this crate passes. |
| `crates/gitcomet/src/crashlog.rs` | Behind `#[cfg(feature = "ui-gpui-runtime")]`, which the CI flags disable. **No crashlog test runs in CI.** |
| `--features benchmarks` code | Built only by `benchmark-targets.yml`. A missing field in `rows/benchmarks/` will not surface in a normal build. |

So if you touch the UI crate, the crash log, or benchmark-gated code, **you are the only gate.**
Run locally:

```bash
cargo fmt --all -- --check
cargo test -p gitcomet-ui-gpui --lib
cargo test -p gitcomet --bin gitcomet          # default features; the only config that compiles crashlog.rs
cargo clippy -p gitcomet --all-targets         # catches dead code CI cannot see
cargo check -p gitcomet-ui-gpui --features benchmarks --all-targets
```

## Known pre-existing test failures

These fail on an untouched tree on Windows. Compare failing **test names** against this list
before suspecting your change.

- `gitcomet-ui-gpui` — 3 failures in `view::panels::popover::tests::workspace`
  (`suggested_worktree_path_*`, `workspace_picker_create_row_opens_the_add_dialog_prefilled`).
  They assert hardcoded POSIX paths, so they cannot pass on Windows.
- `gitcomet-git-gix` — 5 failures in `tests/remote_management_integration.rs` (four `push_*`, plus
  `remote_add_set_url_and_remove_round_trip`). Under investigation; the evidence points at `TEMP`
  resolving to an 8.3 short path (`C:\Users\TOM~1.VOK\...`) breaking a path comparison.

Fixing either is welcome. Skipping them on Windows without first establishing that production
behaviour is correct is not — the second set may be masking a real Windows path-handling bug.

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
