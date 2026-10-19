//! Sharing config between profiles by linking it to one source folder (default `~/.claude`).

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use crate::{
    error::{Context, Error, Result},
    settings::{self, Settings},
    store::write_atomic,
};

/// Shared when no `--only` is given. `settings.json` is opt-in; see `SETTINGS`.
pub const DEFAULT_ITEMS: &[&str] = &[
    "CLAUDE.md",
    "skills",
    "agents",
    "commands",
    "plugins",
    "hooks",
    "output-styles",
    "keybindings.json",
];

/// Merged key by key instead of linked, because the file also holds per-profile keys such as
/// `permissions.defaultMode` (bypass).
pub const SETTINGS: &str = "settings.json";
const SHARED_SETTINGS_KEYS: &[&str] = &[
    "hooks",
    "enabledPlugins",
    "extraKnownMarketplaces",
    "env",
    "statusLine",
    "outputStyle",
];

/// What `clone` and `export` copy, besides MCP servers. Everything else in a profile is login,
/// history, caches or runtime state. `plugins` holds installed copies and caches with
/// machine-specific paths; Claude reinstalls plugins from `settings.json`.
const PORTABLE: &[&str] = &[
    "CLAUDE.md",
    "skills",
    "agents",
    "commands",
    "hooks",
    "output-styles",
    "keybindings.json",
    SETTINGS,
];

/// Whether `clone` and `export` copy this top-level profile item. A shared item (a link) is
/// always kept, since it costs nothing.
pub fn is_portable(name: &str, is_link: bool) -> bool {
    PORTABLE.contains(&name) || (is_link && DEFAULT_ITEMS.contains(&name))
}

/// Logins, history and per-profile state and caches. Sharing these would defeat isolation.
const PRIVATE: &[&str] = &[
    ".credentials.json",
    ".claude.json",
    "projects",
    "history.jsonl",
    "todos",
    "sessions",
    "shell-snapshots",
    "statsig",
    "ide",
    "vscode-data",
    "file-history",
    "session-env",
    "backups",
    "cache",
    "paste-cache",
    "daemon",
];

#[derive(Clone, Copy)]
pub struct Opts {
    /// Move a profile's own copy aside (to `<name>.bak`) and link anyway.
    pub force: bool,
    /// Report what would happen without touching anything.
    pub dry_run: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Linked,
    AlreadyLinked,
    /// Linked after moving the profile's own copy to the named backup.
    Replaced(String),
    /// The profile has its own copy here; left untouched.
    Kept,
    /// Nothing at the source to share.
    Missing,
    Unlinked,
    NotLinked,
    /// Settings keys copied from the source.
    Merged(usize),
    InSync,
    OutOfSync(usize),
    /// Settings were copied, not linked, so there is nothing to undo.
    Copied,
    /// Status only: the profile has its own copy.
    Own,
    /// Status only: the source has it but the profile doesn't.
    Unshared,
}

impl Outcome {
    pub fn describe(&self, dry_run: bool) -> String {
        let would = |done: &str, plan: &str| if dry_run { plan } else { done }.to_owned();
        match self {
            Outcome::Linked => would("linked", "would link"),
            Outcome::AlreadyLinked => "linked".to_owned(),
            Outcome::Replaced(backup) => {
                let (done, plan) = ("linked", "would link");
                format!(
                    "{} (own copy moved to {backup})",
                    if dry_run { plan } else { done }
                )
            }
            Outcome::Kept => "kept (profile has its own; use --force to replace it)".to_owned(),
            Outcome::Missing => "skipped (not in source)".to_owned(),
            Outcome::Unlinked => would("unlinked", "would unlink"),
            Outcome::NotLinked => "skipped (not linked to source)".to_owned(),
            Outcome::Merged(n) => format!(
                "{} {n} key(s)",
                if dry_run { "would copy" } else { "copied" }
            ),
            Outcome::InSync => "in sync".to_owned(),
            Outcome::OutOfSync(n) => format!("{n} key(s) differ from source"),
            Outcome::Copied => "skipped (copied, not linked)".to_owned(),
            Outcome::Own => "own copy (not shared)".to_owned(),
            Outcome::Unshared => "not shared".to_owned(),
        }
    }
}

/// The folder to share from: `CCSWITCH_SHARE_FROM`, else `~/.claude`.
pub fn default_source() -> Result<PathBuf> {
    match env::var_os("CCSWITCH_SHARE_FROM") {
        Some(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => Ok(dirs::home_dir().ok_or(Error::NoHome)?.join(".claude")),
    }
}

/// The items to act on: the defaults, or the names given. Only plain file or folder names are
/// accepted, and never logins or history.
pub fn check_items(items: &[String]) -> Result<Vec<String>> {
    if items.is_empty() {
        return Ok(DEFAULT_ITEMS.iter().map(|s| (*s).to_owned()).collect());
    }
    for item in items {
        let plain = !item.is_empty() && item != "." && item != ".." && !item.contains(['/', '\\']);
        if !plain || PRIVATE.contains(&item.as_str()) {
            return Err(Error::UnknownItem(item.clone()));
        }
    }
    Ok(items.to_vec())
}

fn require_source(source: &Path) -> Result<()> {
    if source.is_dir() {
        Ok(())
    } else {
        Err(Error::SourceMissing(source.display().to_string()))
    }
}

/// Links each item of `profile` to the same item in `source`.
pub fn link(
    source: &Path,
    profile: &Path,
    items: &[String],
    opts: Opts,
) -> Result<Vec<(String, Outcome)>> {
    require_source(source)?;
    items
        .iter()
        .map(|item| {
            let outcome = if item == SETTINGS {
                merge_settings(source, profile, opts.dry_run)?
            } else {
                link_one(source, profile, item, opts)?
            };
            Ok((item.clone(), outcome))
        })
        .collect()
}

fn link_one(source: &Path, profile: &Path, item: &str, opts: Opts) -> Result<Outcome> {
    let from = source.join(item);
    let to = profile.join(item);
    if !from.exists() {
        return Ok(Outcome::Missing);
    }
    let mut backup = None;
    match fs::symlink_metadata(&to) {
        Ok(meta) if meta.file_type().is_symlink() && points_to(&to, &from) => {
            return Ok(Outcome::AlreadyLinked)
        }
        Ok(_) if !opts.force => return Ok(Outcome::Kept),
        Ok(_) => {
            let moved = backup_path(&to);
            if !opts.dry_run {
                fs::rename(&to, &moved)
                    .with_context(|| format!("moving {} aside", to.display()))?;
            }
            backup = moved.file_name().map(|n| n.to_string_lossy().into_owned());
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::io(format!("reading {}", to.display()), e)),
    }
    if !opts.dry_run {
        symlink(&from, &to).with_context(|| format!("linking {}", to.display()))?;
    }
    Ok(match backup {
        Some(name) => Outcome::Replaced(name),
        None => Outcome::Linked,
    })
}

/// `<name>.bak`, or `<name>.bak2`, `.bak3`… when that is taken.
fn backup_path(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    (1..)
        .map(|n| {
            let suffix = if n == 1 { String::new() } else { n.to_string() };
            path.with_file_name(format!("{name}.bak{suffix}"))
        })
        .find(|p| fs::symlink_metadata(p).is_err())
        .expect("an unused backup name exists")
}

/// Removes links to `source`; real files and links to elsewhere are left alone.
pub fn unlink(
    source: &Path,
    profile: &Path,
    items: &[String],
    opts: Opts,
) -> Result<Vec<(String, Outcome)>> {
    items
        .iter()
        .map(|item| {
            if item == SETTINGS {
                return Ok((item.clone(), Outcome::Copied));
            }
            let to = profile.join(item);
            if !is_link_to(&to, &source.join(item)) {
                return Ok((item.clone(), Outcome::NotLinked));
            }
            if !opts.dry_run {
                remove_link(&to).with_context(|| format!("removing {}", to.display()))?;
            }
            Ok((item.clone(), Outcome::Unlinked))
        })
        .collect()
}

/// What is currently shared, without changing anything.
pub fn status(source: &Path, profile: &Path, items: &[String]) -> Result<Vec<(String, Outcome)>> {
    items
        .iter()
        .map(|item| {
            let outcome = if item == SETTINGS {
                match settings_diff(source, profile)? {
                    None => Outcome::Missing,
                    Some(keys) if keys.is_empty() => Outcome::InSync,
                    Some(keys) => Outcome::OutOfSync(keys.len()),
                }
            } else {
                let to = profile.join(item);
                if !source.join(item).exists() {
                    Outcome::Missing
                } else if is_link_to(&to, &source.join(item)) {
                    Outcome::AlreadyLinked
                } else if fs::symlink_metadata(&to).is_ok() {
                    Outcome::Own
                } else {
                    Outcome::Unshared
                }
            };
            Ok((item.clone(), outcome))
        })
        .collect()
}

/// Copies the shared keys from the source's `settings.json` into the profile's, leaving every
/// other key (such as `permissions`) as it was.
fn merge_settings(source: &Path, profile: &Path, dry_run: bool) -> Result<Outcome> {
    let Some(keys) = settings_diff(source, profile)? else {
        return Ok(Outcome::Missing);
    };
    if keys.is_empty() {
        return Ok(Outcome::InSync);
    }
    if !dry_run {
        let file = profile.join(SETTINGS);
        let mut target = read_settings(&file)?.unwrap_or_default();
        let from = read_settings(&source.join(SETTINGS))?.unwrap_or_default();
        for key in &keys {
            target.insert((*key).to_owned(), from[*key].clone());
        }
        write_atomic(&file, &settings::to_string(&target))?;
    }
    Ok(Outcome::Merged(keys.len()))
}

/// Shared keys whose value in the profile differs from the source; `None` if the source has no
/// settings file.
fn settings_diff(source: &Path, profile: &Path) -> Result<Option<Vec<&'static str>>> {
    let Some(from) = read_settings(&source.join(SETTINGS))? else {
        return Ok(None);
    };
    let target = read_settings(&profile.join(SETTINGS))?.unwrap_or_default();
    Ok(Some(
        SHARED_SETTINGS_KEYS
            .iter()
            .copied()
            .filter(|key| from.get(*key).is_some_and(|v| target.get(*key) != Some(v)))
            .collect(),
    ))
}

fn read_settings(file: &Path) -> Result<Option<Settings>> {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(format!("reading {}", file.display()), e)),
    };
    settings::parse(&text)
        .map(Some)
        .map_err(|message| Error::Settings {
            path: file.display().to_string(),
            message,
        })
}

fn is_link_to(link: &Path, target: &Path) -> bool {
    fs::symlink_metadata(link).is_ok_and(|m| m.file_type().is_symlink()) && points_to(link, target)
}

fn points_to(link: &Path, target: &Path) -> bool {
    fs::read_link(link).is_ok_and(|t| {
        let t = if t.is_relative() {
            link.parent().map_or(t.clone(), |p| p.join(&t))
        } else {
            t
        };
        t == target
    })
}

#[cfg(unix)]
pub fn symlink(from: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(from, to)
}

/// Folders fall back to a junction, which needs no special rights. Files need Developer Mode or
/// administrator rights.
#[cfg(windows)]
pub fn symlink(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        std::os::windows::fs::symlink_dir(from, to).or_else(|e| {
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(to)
                .arg(from)
                .stdout(std::process::Stdio::null())
                .status();
            match status {
                Ok(s) if s.success() => Ok(()),
                _ => Err(e),
            }
        })
    } else {
        std::os::windows::fs::symlink_file(from, to).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("{e}; turn on Developer Mode or run as administrator to link files"),
            )
        })
    }
}

#[cfg(unix)]
fn remove_link(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

#[cfg(windows)]
fn remove_link(path: &Path) -> io::Result<()> {
    fs::remove_dir(path).or_else(|_| fs::remove_file(path))
}
