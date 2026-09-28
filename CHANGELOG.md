# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mostafaelgazar48/ccswitch/releases/tag/v0.1.0
