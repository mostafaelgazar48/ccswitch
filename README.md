# ccswitch

[![CI](https://github.com/mostafaelgazar48/ccswitch/actions/workflows/ci.yml/badge.svg)](https://github.com/mostafaelgazar48/ccswitch/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/mostafaelgazar48/ccswitch?sort=semver)](https://github.com/mostafaelgazar48/ccswitch/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Switch between isolated [Claude Code](https://code.claude.com) profiles from one command.**

Keep your work and personal Claude accounts apart, or give each client its own setup. Each profile has
its own login, settings, history and MCP servers, and switching takes one command.

```console
$ ccswitch list
* personal
  work  (bypass)

$ ccswitch run work
```

---

## Contents

- [Features](#features)
- [Install](#install)
- [Quick start](#quick-start)
- [Commands](#commands)
- [Bypass permissions](#bypass-permissions)
- [How it works](#how-it-works)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Uninstall](#uninstall)
- [Contributing](#contributing)
- [License](#license)

## Features

- **Isolated profiles.** Each one is a separate Claude Code config directory, so accounts never mix.
- **A default profile.** `ccswitch run` uses it; `ccswitch run <name>` picks another.
- **Passes Claude's own flags through.** `ccswitch run work -- --resume`.
- **Bypass permissions per profile.** Turn prompt-free mode on for one profile and leave the others alone.
- **Safe by default.** Profile folders are private (`0700`), deleting asks for confirmation, and
  broken settings files are never overwritten.
- **One small binary.** No runtime needed, for Linux, macOS and Windows.

## Install

You need [Claude Code](https://code.claude.com/docs/en/setup) installed, so that `claude` runs in your terminal.

### Linux and macOS (recommended)

```sh
curl -fsSL https://raw.githubusercontent.com/mostafaelgazar48/ccswitch/main/install.sh | sh
```

The script picks the right build for your system, verifies its SHA-256 checksum, and installs it to
`~/.local/bin`. To choose a version or a folder:

```sh
curl -fsSL https://raw.githubusercontent.com/mostafaelgazar48/ccswitch/main/install.sh \
  | CCSWITCH_VERSION=v0.1.0 CCSWITCH_INSTALL_DIR=/usr/local/bin sh
```

### Manual download

Download the file for your system from the [latest release](https://github.com/mostafaelgazar48/ccswitch/releases/latest):

| System                        | File                                     |
|-------------------------------|------------------------------------------|
| Linux x86_64                  | `ccswitch-x86_64-unknown-linux-musl.tar.gz`  |
| Linux ARM64                   | `ccswitch-aarch64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon (M1 and later) | `ccswitch-aarch64-apple-darwin.tar.gz`   |
| macOS Intel                   | `ccswitch-x86_64-apple-darwin.tar.gz`    |
| Windows x86_64                | `ccswitch-x86_64-pc-windows-msvc.zip`    |

Every file has a matching `.sha256`, and `SHA256SUMS` lists them all.

**Linux / macOS:**

```sh
tar xzf ccswitch-*.tar.gz ccswitch
mkdir -p ~/.local/bin && mv ccswitch ~/.local/bin/
ccswitch --version
```

> **macOS:** if the file was downloaded with a browser, macOS may say it "cannot be opened". The binary
> isn't signed with an Apple Developer ID. Clear the download flag once with
> `xattr -d com.apple.quarantine ~/.local/bin/ccswitch`. The install script doesn't need this step.

**Windows:** extract `ccswitch.exe` from the zip into a folder on your `PATH`, then run
`ccswitch --version` in PowerShell.

### From source

With [Rust](https://rustup.rs) installed:

```sh
cargo install --git https://github.com/mostafaelgazar48/ccswitch
```

## Quick start

```sh
ccswitch add work          # creates the profile and opens Claude so you can log in
ccswitch add personal      # log in with your other account
ccswitch list              # * marks the default (your first profile)
ccswitch run               # start Claude with the default profile
ccswitch run personal      # start Claude with a specific profile
ccswitch use personal      # make personal the default
```

Each new profile starts empty, so you log in once per profile. Your existing `~/.claude` setup is
not touched.

## Commands

| Command | What it does |
|---|---|
| `ccswitch add <name>` | Create a profile and start Claude to log in. The first profile becomes the default. |
| `ccswitch add <name> --no-login` | Create a profile without starting Claude. |
| `ccswitch add <name> --default` | Create a profile and make it the default. |
| `ccswitch list` (`ls`) | List profiles. `*` marks the default, `(bypass)` marks bypass mode. |
| `ccswitch use <name>` | Set the default profile. |
| `ccswitch current` | Print the default profile's name. |
| `ccswitch run [name] [-- args…]` | Start Claude with a profile (default if omitted). Arguments after `--` go to Claude. |
| `ccswitch run [name] --bypass` | Start one session in bypass-permissions mode. |
| `ccswitch bypass <name> [on\|off]` | Show or change bypass mode saved on a profile. |
| `ccswitch rename <old> <new>` (`mv`) | Rename a profile. Claude may ask you to log in again. |
| `ccswitch remove <name>` (`rm`) | Delete a profile and its login. Asks first; `--yes` skips the question. |
| `ccswitch path [name]` | Print a profile's config directory. |

Run `ccswitch --help` or `ccswitch <command> --help` for details.

### Passing flags to Claude

Everything after `--` goes to `claude` unchanged:

```sh
ccswitch run -- --resume
ccswitch run work -- -p "summarize this repo"
ccswitch run personal -- --model opus
```

### Useful in scripts

```sh
cd "$(ccswitch path work)"      # open a profile's folder
echo "Using $(ccswitch current)"
alias cw='ccswitch run work --'  # shell shortcut: cw --resume
```

## Bypass permissions

Bypass mode lets Claude run tools and commands **without asking first**. Use it only in projects where
you're comfortable with that, ideally with your work committed to git.

```sh
ccswitch run --bypass           # this session only (adds --dangerously-skip-permissions)
ccswitch bypass personal on     # every session of this profile
ccswitch bypass personal        # show whether it's on or off
ccswitch bypass personal off
```

`bypass <name> on` sets `"permissions": { "defaultMode": "bypassPermissions" }` in that profile's
`settings.json`, and keeps everything else in the file. `off` removes only that setting. If you chose
another mode, such as `acceptEdits`, it stays.

If your organization's Claude Code policy sets `permissions.disableBypassPermissionsMode`, bypass mode
is blocked and Claude starts in normal mode. See the
[Claude Code permission modes docs](https://code.claude.com/docs/en/permission-modes).

## How it works

Claude Code reads its login and settings from the folder named by the `CLAUDE_CONFIG_DIR`
environment variable. ccswitch gives every profile its own folder and starts `claude` with that
variable set:

```text
~/.ccswitch/
├── default              # name of the default profile
└── profiles/
    ├── personal/        # CLAUDE_CONFIG_DIR for "personal"
    │   └── settings.json
    └── work/
```

On Linux and macOS, ccswitch replaces itself with `claude`, so Ctrl-C, the terminal and the exit code
all behave exactly as if you had run `claude` directly.

## Configuration

| Variable          | Default       | Purpose                                     |
|-------------------|---------------|---------------------------------------------|
| `CCSWITCH_HOME`   | `~/.ccswitch` | Where profiles are stored                   |
| `CCSWITCH_CLAUDE` | `claude`      | The Claude Code program to run (name or path) |

## Troubleshooting

| Message | Fix |
|---|---|
| `could not run 'claude': not found` | Install Claude Code, or point `CCSWITCH_CLAUDE` to it. |
| `no default profile` | Run `ccswitch use <name>`, or pass a name: `ccswitch run work`. |
| `default profile 'x' no longer exists` | The folder was deleted. Pick another with `ccswitch use <name>`. |
| `refusing to delete 'x' without confirmation` | You're not in an interactive terminal. Add `--yes`. |
| `cannot update …/settings.json` | That file isn't valid JSON. Fix or delete it, then retry. |
| Asked to log in again after `rename` | Expected on some systems. Log in once more. |

## Uninstall

```sh
rm ~/.local/bin/ccswitch   # the program
rm -rf ~/.ccswitch         # all profiles and their logins (optional)
```

## Contributing

Bug reports and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the development
setup, and [RELEASING.md](RELEASING.md) for how releases are published.

## License

[MIT](LICENSE) © Mostafa Elgazar

ccswitch is an independent project and is not affiliated with or endorsed by Anthropic.
