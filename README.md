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
- [Profile per folder or shell](#profile-per-folder-or-shell)
- [Hand off a conversation](#hand-off-a-conversation)
- [Usage and cost](#usage-and-cost)
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
- **Profile per folder or shell.** A `.ccswitch` file makes a project always use its own account.
- **Hand off a conversation.** Hit the usage limit on one account? Continue the same conversation on another with `ccswitch handoff`.
- **Usage across accounts.** `ccswitch usage` adds up tokens and estimated cost for every profile, by day, model, profile or conversation.
- **Every account at a glance.** `ccswitch status` shows each profile's login, plan and MCP servers.
- **Passes Claude's own flags through.** `ccswitch run work -- --resume`.
- **Bypass permissions per profile.** Turn prompt-free mode on for one profile and leave the others alone.
- **Safe by default.** Profile folders are private (`0700`), deleting asks for confirmation, and
  broken settings files are never overwritten.
- **One small binary.** No runtime needed, for Linux, macOS and Windows.

## Install

You need [Claude Code](https://code.claude.com/docs/en/setup) installed, so that `claude` runs in your terminal.

Pick the option for your system. All files are on the
[latest release](https://github.com/mostafaelgazar48/ccswitch/releases/latest) page.

| System | Recommended | Other options |
|---|---|---|
| **Windows** | `ccswitch-<version>-windows-x86_64-setup.exe` | `ccswitch-x86_64-pc-windows-msvc.exe`, `.zip` |
| **macOS** (Apple Silicon and Intel) | install script, or `ccswitch-<version>-macos-universal.dmg` | `ccswitch-aarch64-apple-darwin.tar.gz`, `ccswitch-x86_64-apple-darwin.tar.gz` |
| **Debian / Ubuntu** | `ccswitch_<version>-1_amd64.deb` or `_arm64.deb` | install script |
| **Other Linux** | install script | `ccswitch-x86_64-unknown-linux-musl.tar.gz`, `ccswitch-aarch64-unknown-linux-musl.tar.gz` |

Every file has a matching `.sha256`, and `SHA256SUMS` lists them all.

### Windows

**Installer (recommended):** download `ccswitch-<version>-windows-x86_64-setup.exe` and run it.
It installs for your user only, so no admin rights are needed, and adds ccswitch to your `PATH`.
Open a **new** terminal afterwards and run `ccswitch --version`. To uninstall, go to
*Settings → Apps → Installed apps → ccswitch*.

> Windows SmartScreen may say "Windows protected your PC" because the installer isn't code-signed.
> Click **More info → Run anyway**.

**Portable:** download `ccswitch-x86_64-pc-windows-msvc.exe`, rename it to `ccswitch.exe`, and put it
in a folder on your `PATH`.

### macOS

**Install script (recommended, no security prompts):**

```sh
curl -fsSL https://raw.githubusercontent.com/mostafaelgazar48/ccswitch/main/install.sh | sh
```

**Disk image:** download `ccswitch-<version>-macos-universal.dmg`, open it, and double-click
**Install ccswitch.command**. It copies `ccswitch` to `/usr/local/bin` and asks for your password.
The same `.dmg` works on Apple Silicon and Intel Macs.

> The app isn't signed with an Apple Developer ID, so macOS blocks it the first time. Click
> **Done**, then open **System Settings → Privacy & Security**, scroll down, and click **Open Anyway**
> next to the ccswitch message. On older macOS you can instead right-click the file and choose **Open**.

### Debian and Ubuntu

Download the `.deb` for your machine (`amd64` for most PCs, `arm64` for ARM) and install it:

```sh
sudo apt install ./ccswitch_*_amd64.deb
ccswitch --version
```

This puts `ccswitch` in `/usr/bin`. Remove it with `sudo apt remove ccswitch`.

### Any Linux (install script)

```sh
curl -fsSL https://raw.githubusercontent.com/mostafaelgazar48/ccswitch/main/install.sh | sh
```

The script picks the right build for your system, checks its SHA-256 checksum, and installs it to
`~/.local/bin`. To choose a version or a folder:

```sh
curl -fsSL https://raw.githubusercontent.com/mostafaelgazar48/ccswitch/main/install.sh \
  | CCSWITCH_VERSION=v0.2.0 CCSWITCH_INSTALL_DIR=/usr/local/bin sh
```

Or unpack an archive yourself:

```sh
tar xzf ccswitch-*.tar.gz ccswitch
mkdir -p ~/.local/bin && mv ccswitch ~/.local/bin/
```

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
| `ccswitch use <name> --here` | Make this folder (and its subfolders) use a profile. See [Profile per folder or shell](#profile-per-folder-or-shell). |
| `ccswitch current` | Print the profile in use here. |
| `ccswitch status` (`st`) | Show each profile's account, org and plan, MCP servers, bypass mode and last use. `--json` for scripts. |
| `ccswitch sessions [name] [--all]` | List saved conversations started in this folder (`--all`: every folder), newest first. |
| `ccswitch handoff <to> [options]` | Copy a conversation to another profile and resume it there. See [Hand off a conversation](#hand-off-a-conversation). |
| `ccswitch usage [name] [--since 7d] [--by day\|model\|profile\|session]` | Tokens and estimated cost for every profile (or one). See [Usage and cost](#usage-and-cost). |
| `ccswitch env [name]` | Print commands that select a profile for this shell: `eval "$(ccswitch env work)"`. |
| `ccswitch run [name] [-- args…]` | Start Claude with a profile (default if omitted). Arguments after `--` go to Claude. |
| `ccswitch run [name] --bypass` | Start one session in bypass-permissions mode. |
| `ccswitch code [name] [-- args…]` | Open VS Code so the Claude Code extension uses a profile. Opens the current folder unless you pass arguments after `--`. |
| `ccswitch share <name\|--all> [options]` | Link shared config from `~/.claude` into profiles. See [Sharing config](#sharing-config). |
| `ccswitch add <name> --share` | Create a profile and share config in one step. |
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

## Profile per folder or shell

Which profile `run`, `code`, `path` and `env` use when you don't name one:

1. `$CCSWITCH_PROFILE`, set for the current shell by `ccswitch env`;
2. the nearest `.ccswitch` file in this folder or a parent folder;
3. the default profile (`ccswitch use <name>`).

```sh
cd ~/clients/acme
ccswitch use acme --here      # writes ./.ccswitch; this folder and its subfolders now use "acme"
ccswitch run                  # starts Claude logged in as acme
ccswitch current              # prints "acme" (stderr says it came from .ccswitch)
```

`.ccswitch` holds only the profile name, so you can commit it or add it to `.gitignore`. If it names a
profile that doesn't exist, ccswitch stops with an error instead of falling back to another account.

`ccswitch env` sets `CLAUDE_CONFIG_DIR` and `CCSWITCH_PROFILE` in your current shell. After that,
plain `claude`, scripts and other tools use the profile too:

```sh
eval "$(ccswitch env work)"           # bash, zsh
ccswitch env work | source            # fish
ccswitch env work | Invoke-Expression # PowerShell
eval "$(ccswitch env --unset)"        # back to normal
```

The shell is guessed from `$SHELL`; pass `--shell sh|fish|powershell` to choose. To show the profile in
your prompt (bash/zsh): `PS1='${CCSWITCH_PROFILE:+($CCSWITCH_PROFILE) }'$PS1`.

## Hand off a conversation

Each account keeps its own conversation history, so when one account hits its usage limit, Claude
can't continue that conversation on another. `handoff` copies it over and resumes it:

```sh
ccswitch handoff personal                    # latest conversation in this folder → "personal"
ccswitch handoff personal --from work        # take it from a profile other than the one in use
ccswitch sessions work                       # list conversations and their ids
ccswitch handoff personal --session 61252340 # pick one by id (the first characters are enough)
ccswitch handoff personal --move             # remove it from the old profile afterwards
ccswitch handoff personal --no-resume        # copy only
ccswitch handoff personal -- --model opus    # more flags for Claude after --
```

It copies the conversation log, its subagent and tool output folder, and its rewind history
(`file-history`). The old profile keeps its copy unless you pass `--move`. Quit Claude in the old
profile first, so nothing is written to the conversation while it's copied. If the conversation was
started in another folder, Claude is started there, because `--resume` looks up conversations by
folder.

## Usage and cost

`usage` reads the conversation logs Claude Code keeps in each profile and adds up the tokens, for all
your accounts in one table:

```console
$ ccswitch usage --since 30d --by profile
PROFILE   REQUESTS  INPUT     OUTPUT  CACHE WRITE   CACHE READ        TOTAL  EST. COST
work         1,081  2,180    626,604    2,992,198  239,578,663  243,199,645     $79.09
personal       842  1,690    512,210    2,410,120  101,220,431  104,144,451     $41.27
TOTAL        1,923  3,870  1,138,814    5,402,318  340,799,094  347,344,096    $120.36
```

- `--since`: `7d` (default), `30d`, a date such as `2026-10-01`, or `all`. Days are UTC.
- `--by`: `day` (default), `model`, `profile` or `session` (`profile/session-id`, ready for `handoff`).
- `ccswitch usage work` covers one profile; `--json` prints the same numbers for scripts.

The cost is an **estimate at Claude API list prices**, including cache writes and reads. Pro, Max and
Team plans are not billed per token, so for them it shows what the same work would cost on the API.
Models missing from the price table are counted but not priced, and the footer says how many. A
conversation copied by `handoff` is counted once.

## VS Code

The Claude Code extension for VS Code reads `CLAUDE_CONFIG_DIR` from the environment VS Code was
started with. `ccswitch code` sets it for you:

```sh
ccswitch code                         # default profile, current folder
ccswitch code work                    # "work" profile, current folder
ccswitch code work -- ~/projects/app  # arguments after -- go to `code`
```

VS Code normally hands a new folder to the instance that's already running, which still has the old
environment. So each profile gets its own VS Code instance, started with `--user-data-dir` pointing to
`vscode-data/` inside the profile. You can have a work window and a personal window open side by side.

That instance shares your installed extensions, but starts with its own settings, keybindings and
recent folders. Turn on Settings Sync in it if you want your usual setup.

Only one profile at a time? You can skip `ccswitch code` and set it once in your VS Code user settings:

```json
"claudeCode.environmentVariables": [
  { "name": "CLAUDE_CONFIG_DIR", "value": "/home/you/.ccswitch/profiles/work" }
]
```

`ccswitch path work` prints the folder to use. This setting only works in user settings, not in a
project's `.vscode/settings.json`.

## Sharing config

Profiles are isolated, but you may want the same CLAUDE.md, skills and agents everywhere.
`share` symlinks them from one folder (`~/.claude`, or `$CCSWITCH_SHARE_FROM`, or `--from DIR`), so an
edit in one place shows up in every profile.

```sh
ccswitch share work                     # link the defaults into one profile
ccswitch share --all                    # every profile
ccswitch add client --share             # share while creating
ccswitch share work --status            # what is shared right now
ccswitch share work --dry-run           # preview
ccswitch share work --force             # replace its own copy, kept as <name>.bak
ccswitch share work --undo              # remove the links
ccswitch share work --only skills,hooks
```

Default items: `CLAUDE.md`, `skills`, `agents`, `commands`, `plugins`, `hooks`, `output-styles`,
`keybindings.json`. A profile's own copy is never overwritten unless you pass `--force`.

`--only settings.json` copies the `hooks`, `enabledPlugins`, `env`, `statusLine`, `outputStyle` and
`extraKnownMarketplaces` keys into the profile's file. It is a copy, so run it again after you change
the source. Everything else, including `permissions` and the bypass setting, stays per profile.

Logins, history and projects (`.credentials.json`, `projects`, `history.jsonl`, …) are never shared.
On Windows, folders use a junction if symlinks aren't allowed; linking a single file needs Developer
Mode or administrator rights.

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
    │   ├── settings.json
    │   └── vscode-data/ # VS Code instance for `ccswitch code personal`
    └── work/
```

On Linux and macOS, ccswitch replaces itself with `claude`, so Ctrl-C, the terminal and the exit code
all behave exactly as if you had run `claude` directly.

## Configuration

| Variable          | Default       | Purpose                                     |
|-------------------|---------------|---------------------------------------------|
| `CCSWITCH_HOME`   | `~/.ccswitch` | Where profiles are stored                   |
| `CCSWITCH_PROFILE` | (none)       | Profile for this shell; set by `ccswitch env` |
| `CCSWITCH_CLAUDE` | `claude`      | The Claude Code program to run (name or path) |
| `CCSWITCH_CODE`   | `code`        | The VS Code program to run (name or path)   |

## Troubleshooting

| Message | Fix |
|---|---|
| `could not run 'claude': not found` | Install Claude Code, or point `CCSWITCH_CLAUDE` to it. |
| `could not run 'code': not found` | In VS Code, run **Shell Command: Install 'code' command in PATH**, or point `CCSWITCH_CODE` to it (for example `codium` or `cursor`). |
| `no default profile` | Run `ccswitch use <name>`, or pass a name: `ccswitch run work`. |
| `profile 'x' (from …/.ccswitch) not found` | Create the profile, or edit or delete that `.ccswitch` file. With `$CCSWITCH_PROFILE`, run `eval "$(ccswitch env --unset)"`. |
| `default profile 'x' no longer exists` | The folder was deleted. Pick another with `ccswitch use <name>`. |
| `refusing to delete 'x' without confirmation` | You're not in an interactive terminal. Add `--yes`. |
| `cannot update …/settings.json` | That file isn't valid JSON. Fix or delete it, then retry. |
| Asked to log in again after `rename` | Expected on some systems. Log in once more. |

## Uninstall

| Installed with | Remove with |
|---|---|
| Install script or archive | `rm ~/.local/bin/ccswitch` |
| `.dmg` | `sudo rm /usr/local/bin/ccswitch` |
| `.deb` | `sudo apt remove ccswitch` |
| Windows installer | *Settings → Apps → Installed apps → ccswitch → Uninstall* |

Your profiles stay in `~/.ccswitch` (Windows: `%USERPROFILE%\.ccswitch`). Delete that folder too if
you want to remove all profiles and their logins.

## Contributing

Bug reports and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the development
setup, and [RELEASING.md](RELEASING.md) for how releases are published.

## License

[MIT](LICENSE) © Mostafa Elgazar

ccswitch is an independent project and is not affiliated with or endorsed by Anthropic.
