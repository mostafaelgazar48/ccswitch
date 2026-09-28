# Publishing and releasing ccswitch

This guide has two parts:

- **Part 1:** put the project on GitHub. You only do this once.
- **Part 2:** publish a release. Repeat this for every new version.

Releases are automated. You push a version tag, and GitHub Actions tests, builds and publishes for you.

```text
 you                            GitHub Actions (release.yml)
 ───                            ─────────────────────────────
 1. bump version in Cargo.toml
 2. update CHANGELOG.md
 3. commit + push to main  ───▶  CI: fmt, clippy, tests (Linux, macOS, Windows)
 4. git tag v0.2.0 + push  ───▶  check tag == Cargo.toml version
                                 run CI again
                                 build 5 binaries (Linux x2, macOS x2, Windows)
                                 create GitHub Release with binaries, checksums
                                 and notes from CHANGELOG.md
```

---

## Part 1: First-time publish

### 1.1 Log in with the right GitHub account

The repo lives under **mostafaelgazar48**. Check which account the GitHub CLI uses:

```sh
gh auth status
```

If it shows a different account, log in as `mostafaelgazar48`:

```sh
gh auth login              # choose GitHub.com, HTTPS, log in with a browser
gh auth switch --user mostafaelgazar48   # if you have several accounts
```

### 1.2 Create the local git repository

From the project folder:

```sh
git init -b main
git add .
git status                 # check: no target/ or dist/ folders listed
git commit -m "Initial release of ccswitch"
```

### 1.3 Create the GitHub repository and push

```sh
gh repo create mostafaelgazar48/ccswitch --public --source . --push \
  --description "Switch between isolated Claude Code profiles from one command"
```

Or on the website: create an **empty** public repo named `ccswitch` at
<https://github.com/new>, with no README or license, then:

```sh
git remote add origin https://github.com/mostafaelgazar48/ccswitch.git
git push -u origin main
```

### 1.4 Check that CI passes

Open <https://github.com/mostafaelgazar48/ccswitch/actions>. The **CI** workflow should turn green in
a few minutes. Fix any failures before you release.

### 1.5 Recommended repository settings

In **Settings** on GitHub:

- **General → Features:** keep Issues on.
- **General → Pull Requests:** allow squash merging, and turn on *Automatically delete head branches*.
- **Branches → Add branch ruleset** for `main`: require a pull request, and require these status checks:
  `Format & lint`, `Test (ubuntu-latest)`, `Test (macos-latest)`, `Test (windows-latest)`.
- **Actions → General → Workflow permissions:** the default *Read repository contents* is fine.
  The release job asks for write access by itself.
- **About** (gear icon on the main page): add the description and the topics
  `claude`, `claude-code`, `cli`, `rust`, `profiles`.

Then publish the first release, `v0.1.0`, with Part 2. `Cargo.toml` and `CHANGELOG.md` are already
set up for it, so you can go straight to step 2.4.

---

## Part 2: Publish a release

Versions follow [Semantic Versioning](https://semver.org):

| Change | Example | New version |
|---|---|---|
| Bug fix only | fix a crash | `0.1.0 → 0.1.1` |
| New command or flag | add `ccswitch export` | `0.1.1 → 0.2.0` |
| Breaking change (renamed command, moved files) | | `0.2.0 → 1.0.0` (or `0.3.0` before 1.0) |

### 2.1 Start from an up-to-date `main`

```sh
git switch main
git pull
```

### 2.2 Bump the version

Edit `version` in `Cargo.toml`, e.g. `version = "0.2.0"`, then refresh the lock file:

```sh
cargo check
```

### 2.3 Update the changelog

In `CHANGELOG.md`, rename `## [Unreleased]` to the new version and date. Add a new empty
`## [Unreleased]` above it, and update the links at the bottom:

```markdown
## [Unreleased]

## [0.2.0] - 2026-10-15

### Added
- `ccswitch export` command.

### Fixed
- ...

[Unreleased]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/mostafaelgazar48/ccswitch/compare/v0.1.0...v0.2.0
```

The text under the version heading becomes the release notes on GitHub.

### 2.4 Commit, tag and push

```sh
cargo test                                   # last local check
git commit -am "Release v0.2.0"
git push
git tag -a v0.2.0 -m "ccswitch v0.2.0"
git push origin v0.2.0
```

The tag must be `v` + the exact version in `Cargo.toml`, or the release stops at the first step.

### 2.5 Watch the release

```sh
gh run watch                                 # or open the Actions tab
```

It takes about 5–10 minutes. When it finishes, the release appears at
<https://github.com/mostafaelgazar48/ccswitch/releases> with these files:

```text
ccswitch-x86_64-unknown-linux-musl.tar.gz    (+ .sha256)
ccswitch-aarch64-unknown-linux-musl.tar.gz   (+ .sha256)
ccswitch-x86_64-apple-darwin.tar.gz          (+ .sha256)
ccswitch-aarch64-apple-darwin.tar.gz         (+ .sha256)
ccswitch-x86_64-pc-windows-msvc.zip          (+ .sha256)
SHA256SUMS
```

### 2.6 Check the published release

```sh
curl -fsSL https://raw.githubusercontent.com/mostafaelgazar48/ccswitch/main/install.sh | sh
ccswitch --version                           # should print the new version
```

---

## Pre-releases

To let people try a version before the final release, tag it with a suffix, e.g. set
`version = "0.2.0-rc.1"` in `Cargo.toml` and tag `v0.2.0-rc.1`. Any tag with a `-` is published as a
**pre-release**. The "latest" download links and the install script skip it, unless someone asks for
it with `CCSWITCH_VERSION=v0.2.0-rc.1`.

## When a release fails

| Problem | What to do |
|---|---|
| `Tag vX does not match Cargo.toml version` | Delete the tag (below), fix `Cargo.toml`, commit, tag again. |
| Tests or a build failed | Delete the tag, fix the problem on `main`, tag again. |
| Release published with a mistake | Publish a new patch version. Don't replace files in a published release. |

Delete a tag, and its release if one was created:

```sh
gh release delete v0.2.0 --yes --cleanup-tag   # if the release exists
git push origin :refs/tags/v0.2.0              # otherwise, delete the remote tag
git tag -d v0.2.0                              # delete the local tag
```

## Optional: publish to crates.io

To let people install with `cargo install ccswitch`:

1. Check that the name is free at <https://crates.io/crates/ccswitch>.
2. Log in: `cargo login` with a token from <https://crates.io/settings/tokens>.
3. After the GitHub release: `cargo publish --dry-run`, then `cargo publish`.
