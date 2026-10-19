# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-10-19

### Added
- `ccswitch share <name>` links CLAUDE.md, skills, agents, commands, plugins, hooks, output-styles and keybindings.json from `~/.claude` into a profile. Logins, history and projects stay separate.
- `share --all` applies to every profile, `--status` shows what is shared, `--dry-run` previews, `--undo` removes the links, and `--force` moves a profile's own copy to `<name>.bak` before linking.
- `share --only settings.json` copies the hooks, enabledPlugins, env, statusLine, outputStyle and extraKnownMarketplaces keys. Other keys, including the bypass setting, stay per profile.
- `--only` accepts any plain file or folder name. Logins, history and projects are refused.
- `ccswitch add <name> --share` shares as the profile is created.
- `CCSWITCH_SHARE_FROM` sets the folder to share from (default `~/.claude`).
- On Windows, folders fall back to a junction when symlinks aren't allowed.
- `ccswitch status` (`st`) shows each profile's account email, org and plan, MCP server count, bypass mode and last use. `--json` prints the same as JSON.
- `ccswitch use <name> --here` writes a `.ccswitch` file, so the folder and its subfolders use that profile.
- `ccswitch env [name]` prints shell commands that set `CLAUDE_CONFIG_DIR` and `CCSWITCH_PROFILE` for `eval`, for sh, fish and PowerShell. `--unset` clears them.
- `CCSWITCH_PROFILE` picks the profile for a shell. Without a name, commands use it first, then the nearest `.ccswitch` file, then the default.
- `ccswitch handoff <to>` copies the latest conversation in this folder (or `--session <id>`) to another profile, with its subagent output and rewind history, and resumes it there. `--from`, `--move`, `--force`, `--no-resume`, and Claude flags after `--`.
- `ccswitch sessions [name]` lists saved conversations with their titles; `--all` covers every folder.
- `ccswitch usage [name]` adds up tokens (input, output, cache write, cache read) and their estimated API cost from every profile's conversation logs. `--since 7d|30d|YYYY-MM-DD|all`, `--by day|model|profile|session`, `--json`. Repeated log lines and conversations copied by `handoff` are counted once.
- `ccswitch clone <from> <new>` creates a profile with another's settings, CLAUDE.md, skills, agents, commands, hooks, output styles, keybindings and MCP servers. The login, history, caches and plugin downloads are not copied. Shared links stay links.
- `ccswitch export <name>` saves the same config to a private `.tar.gz`, and `ccswitch import <file>` creates a profile from it (`--as`, `--share`, `--no-login`). Import refuses links, paths outside the profile and anything that isn't config.
- `ccswitch mcp list [name|--all]` shows user-scope MCP servers; `ccswitch mcp copy <from> <to> [servers…]` copies them, keeping the target's own servers unless `--force`.

### Changed
- `ccswitch current` prints the profile in use in this folder or shell, and says on stderr where the choice came from.

## [0.2.0] - 2026-09-28

### Added
- `ccswitch code [name] [-- args…]` opens VS Code so the Claude Code extension uses that profile. Each profile gets its own VS Code instance (`--user-data-dir`), so windows for different profiles can be open at once.
- `CCSWITCH_CODE` environment variable to run a different editor binary, such as `codium` or `cursor`.

## [0.1.1] - 2026-09-28

### Added
- Windows installer (`…-windows-x86_64-setup.exe`). It installs per user without admin rights and adds ccswitch to `PATH`.
- Standalone Windows `ccswitch.exe` download.
- macOS disk image (`…-macos-universal.dmg`) with one binary for Apple Silicon and Intel, plus a double-click installer.
- Debian/Ubuntu packages (`.deb`) for amd64 and arm64.

## [0.1.0] - 2026-09-28

### Added
- `add`, `list` (`ls`), `use`, `current`, `run`, `rename` (`mv`), `remove` (`rm`) and `path` commands.
- `bypass <profile> [on|off]` saves bypass-permissions mode in a profile; `run --bypass` enables it for one session.
- The first profile you add becomes the default automatically.
- `CCSWITCH_HOME` and `CCSWITCH_CLAUDE` environment variables.
- Prebuilt binaries for Linux (x86_64, arm64), macOS (Intel, Apple Silicon) and Windows, plus an install script.

### Security
- Profile directories are created with `0700` permissions on Unix, because they hold login credentials.

[Unreleased]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mostafaelgazar48/ccswitch/releases/tag/v0.1.0
