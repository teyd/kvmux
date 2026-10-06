# kvmux agent rules

kvmux switches monitor inputs over DDC/CI when a chosen USB device connects. One binary:
it runs in the background with a tray icon and opens a GPUI settings window on demand.
The settings window picks a USB device and per-monitor inputs, and writes `config.toml`.

## Layout

- `crates/kvmux-core`: config, input sources, device traits. No GUI dependencies.
- `crates/kvmux`: the binary: daemon, tray, updater, settings window.

## Commands

Run `mise run ci` before every commit. It runs fmt, clippy, tests and cargo-deny.
Rust version lives in `mise.toml` and `rust-toolchain.toml`; keep them equal.

## Git

- Work on a branch per issue: `feat/<issue>-<slug>`. Never push to `main`.
- Logical commits: one coherent change each, builds and passes tests on its own.
  No "wip" or "fix typo" commits; rewrite history before opening the PR.
- Conventional Commits for every commit message: `type(scope): subject`.
  Types: feat fix docs style refactor perf test build ci chore revert.
- PRs are rebase-merged, so each commit lands on `main` as written.

## Skills

Before any UI work, load the `gpui-kit` and `gpui-kit-design-guides` skills (in `.agents/skills`, pinned by `skills-lock.json`).

## Rules

- Hardware access (USB, DDC) goes behind traits in `kvmux-core` with fakes for tests.
- Identify monitors by EDID id and USB devices by vendor id, product id and serial, never by list position.
- GUI follows the gpui-kit design guides: theme tokens only (no raw colors, no `px()`),
  sentence-case labels, primary button only for the form's default commit.
- No `unsafe` outside platform modules, and each block needs a SAFETY comment.
- Config is strict: `deny_unknown_fields`, versioned, written atomically.
- Do not run external commands from config.

## Needs a human review, do not change on your own

`.github/`, `.agents/`, `skills-lock.json`, `scripts/`, `mise.toml`, `rust-toolchain.toml`, `deny.toml`, `Cargo.lock`,
anything in the updater, signing keys, or release code. Do not add or bump dependencies
without saying why in the PR; fastframe crates are pinned by tag and reviewed by a human.
