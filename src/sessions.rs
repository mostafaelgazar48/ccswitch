//! Claude Code's saved conversations inside a profile: `projects/<slug>/<id>.jsonl`, plus an
//! optional `projects/<slug>/<id>/` folder (subagents, tool output) and `file-history/<id>/`
//! (rewind snapshots).

use std::{
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde_json::Value;

use crate::{
    error::{Context, Error, Result},
    store,
};

const PROJECTS: &str = "projects";
const FILE_HISTORY: &str = "file-history";

pub struct Session {
    pub id: String,
    /// The `projects/` subfolder, named after the folder Claude ran in.
    pub slug: String,
    /// The folder Claude ran in, as recorded in the log.
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub modified: SystemTime,
}

/// Claude's folder name for a project: every character but ASCII letters and digits becomes `-`.
pub fn slug(dir: &Path) -> String {
    dir.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Sessions in `profile`, newest first; only those started in `folder` when one is given.
pub fn list(profile: &Path, folder: Option<&Path>) -> Result<Vec<Session>> {
    let projects = profile.join(PROJECTS);
    let slugs = match folder {
        Some(dir) => vec![slug(dir)],
        None => read_dir_names(&projects)?,
    };
    let mut sessions = Vec::new();
    for slug in slugs {
        for name in read_dir_names(&projects.join(&slug))? {
            let Some(id) = name.strip_suffix(".jsonl") else {
                continue;
            };
            let file = projects.join(&slug).join(&name);
            let modified = fs::metadata(&file)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let (cwd, title) = scan(&file)?;
            sessions.push(Session {
                id: id.to_owned(),
                slug: slug.clone(),
                cwd,
                title,
                modified,
            });
        }
    }
    sessions.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.id.cmp(&b.id)));
    Ok(sessions)
}

/// The session whose id is `id` or starts with it.
pub fn find(profile: &Path, id: &str) -> Result<Session> {
    let mut matches: Vec<Session> = list(profile, None)?
        .into_iter()
        .filter(|s| s.id.starts_with(id))
        .collect();
    if let Some(exact) = matches.iter().position(|s| s.id == id) {
        return Ok(matches.swap_remove(exact));
    }
    match matches.len() {
        0 => Err(Error::NoSession(format!("no session '{id}'"))),
        1 => Ok(matches.remove(0)),
        n => Err(Error::NoSession(format!(
            "'{id}' matches {n} sessions; give more of the id"
        ))),
    }
}

impl Session {
    fn log(&self, profile: &Path) -> PathBuf {
        profile
            .join(PROJECTS)
            .join(&self.slug)
            .join(format!("{}.jsonl", self.id))
    }

    /// The log plus whichever of its folders exist, relative to the profile.
    fn parts(&self, profile: &Path) -> Vec<PathBuf> {
        let parts = [
            Path::new(PROJECTS)
                .join(&self.slug)
                .join(format!("{}.jsonl", self.id)),
            Path::new(PROJECTS).join(&self.slug).join(&self.id),
            Path::new(FILE_HISTORY).join(&self.id),
        ];
        parts
            .into_iter()
            .filter(|p| fs::symlink_metadata(profile.join(p)).is_ok())
            .collect()
    }

    /// Whether `profile` already has a session with this id in the same project.
    pub fn exists_in(&self, profile: &Path) -> bool {
        self.log(profile).exists()
    }

    /// Copies the session from one profile to another, replacing any copy already there.
    pub fn copy(&self, from: &Path, to: &Path) -> Result<()> {
        for part in self.parts(from) {
            let dest = to.join(&part);
            remove_any(&dest)?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            store::copy_tree(&from.join(&part), &dest, &|_| false)?;
        }
        Ok(())
    }

    pub fn remove(&self, profile: &Path) -> Result<()> {
        for part in self.parts(profile) {
            remove_any(&profile.join(part))?;
        }
        Ok(())
    }
}

fn remove_any(path: &Path) -> Result<()> {
    let result = match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    };
    result.with_context(|| format!("removing {}", path.display()))
}

/// The folder Claude ran in and the latest title Claude gave the conversation.
fn scan(file: &Path) -> Result<(Option<String>, Option<String>)> {
    let reader = BufReader::new(
        fs::File::open(file).with_context(|| format!("reading {}", file.display()))?,
    );
    let (mut cwd, mut title) = (None, None);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let wanted = (cwd.is_none() && line.contains("\"cwd\"")) || line.contains("\"ai-title\"");
        if !wanted {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if cwd.is_none() {
            cwd = value.get("cwd").and_then(Value::as_str).map(String::from);
        }
        if let Some(t) = value.get("aiTitle").and_then(Value::as_str) {
            title = Some(t.to_owned());
        }
    }
    Ok((cwd, title))
}

fn read_dir_names(dir: &Path) -> Result<Vec<String>> {
    match fs::read_dir(dir) {
        Ok(entries) => Ok(entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::io(format!("reading {}", dir.display()), e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_matches_claude() {
        assert_eq!(
            slug(Path::new("/media/x/my space/my-work/cc_switch.v2")),
            "-media-x-my-space-my-work-cc-switch-v2"
        );
    }
}
