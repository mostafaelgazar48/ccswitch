# Contributing to ccswitch

Thanks for helping! This guide covers the development setup and the pull-request flow.

## Setup

1. Install Rust with [rustup](https://rustup.rs) (stable toolchain).
2. Clone and build:

   ```sh
   git clone https://github.com/mostafaelgazar48/ccswitch.git
   cd ccswitch
   cargo build
   ```

3. Run your local build without touching your real profiles:

   ```sh
   export CCSWITCH_HOME=/tmp/ccswitch-dev
   cargo run -- add test --no-login
   cargo run -- list
   ```

## Project layout

| Path | Contents |
|---|---|
| `src/main.rs` | Entry point: parses arguments, prints errors, sets the exit code |
| `src/cli.rs` | Command-line definitions (clap) |
| `src/app.rs` | What each command does |
| `src/store.rs` | Profiles on disk: create, list, rename, remove, default |
| `src/settings.rs` | Safe edits to a profile's `settings.json` |
| `src/launch.rs` | Finding and starting `claude` |
| `src/error.rs` | Error type and user-facing messages |
| `tests/cli.rs` | End-to-end tests that run the real binary against a fake `claude` |

## Before you open a pull request

Run the same checks as CI:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

- Add a test for new behavior. Unit tests live next to the code; command behavior goes in `tests/cli.rs`.
- Update `README.md` if you add or change a command or flag.
- Add a line under **Unreleased** in `CHANGELOG.md`.

## Pull-request flow

1. Fork the repo, or create a branch if you have write access: `git switch -c fix/short-description`.
2. Commit with clear messages, e.g. `Add ccswitch export command`.
3. Push and open a pull request against `main`. Fill in the template.
4. CI runs formatting, lint and tests on Linux, macOS and Windows. All checks must pass.
5. A maintainer reviews and squash-merges.

## Reporting bugs

Open an [issue](https://github.com/mostafaelgazar48/ccswitch/issues/new/choose) with the command you
ran, the full output, `ccswitch --version`, your OS, and `claude --version`.
