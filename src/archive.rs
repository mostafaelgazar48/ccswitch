//! `ccswitch export` / `import`: a profile's config as a `.tar.gz`, without its login or history.
//!
//! The archive holds `ccswitch-export.json` first, then the profile's files under `profile/`.
//! Shared items (links into `~/.claude`) are not followed; their names go in the manifest so
//! `import --share` can link them again on the new machine.

use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde_json::{json, Map, Value};

use crate::{
    claude_json,
    error::{Context, Error, Result},
    share,
};

const MANIFEST: &str = "ccswitch-export.json";
const ROOT: &str = "profile";
const FORMAT: u64 = 1;

pub struct Manifest {
    /// The profile's name when it was exported.
    pub name: String,
    /// Items that were links to the shared folder.
    pub shared: Vec<String>,
    pub mcp_servers: Map<String, Value>,
}

pub struct Summary {
    pub files: usize,
    /// Links below the top level, which are left out.
    pub skipped_links: usize,
}

pub fn export(profile: &Path, name: &str, out: &Path) -> Result<(Manifest, Summary)> {
    let mut entries = Vec::new();
    let mut manifest = Manifest {
        name: name.to_owned(),
        shared: Vec::new(),
        mcp_servers: claude_json::mcp_servers(&claude_json::read(profile)?),
    };
    let mut summary = Summary {
        files: 0,
        skipped_links: 0,
    };
    walk(
        profile,
        Path::new(""),
        &mut entries,
        &mut manifest,
        &mut summary,
    )?;

    let file = create_private(out)?;
    let result = (|| {
        let mut tar = tar::Builder::new(GzEncoder::new(file, Compression::default()));
        let text = serde_json::to_vec_pretty(&json!({
            "format": FORMAT,
            "name": manifest.name,
            "shared": manifest.shared,
            "mcpServers": manifest.mcp_servers,
        }))
        .expect("JSON always serializes");
        let mut header = tar::Header::new_gnu();
        header.set_size(text.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        tar.append_data(&mut header, MANIFEST, text.as_slice())?;
        for (rel, is_dir) in &entries {
            let name = Path::new(ROOT).join(rel);
            if *is_dir {
                tar.append_dir(&name, profile.join(rel))?;
            } else {
                tar.append_path_with_name(profile.join(rel), &name)?;
            }
        }
        tar.into_inner()?.finish().map(drop)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(out);
        return Err(Error::io(format!("writing {}", out.display()), e));
    }
    Ok((manifest, summary))
}

/// Collects what to export, in a stable order.
fn walk(
    profile: &Path,
    rel: &Path,
    entries: &mut Vec<(PathBuf, bool)>,
    manifest: &mut Manifest,
    summary: &mut Summary,
) -> Result<()> {
    let dir = profile.join(rel);
    let mut names: Vec<_> = fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .flatten()
        .map(|e| e.file_name())
        .collect();
    names.sort();
    for name in names {
        let rel = rel.join(&name);
        let path = profile.join(&rel);
        let meta =
            fs::symlink_metadata(&path).with_context(|| format!("reading {}", path.display()))?;
        let top = rel.components().count() == 1;
        if top && !share::is_portable(&name.to_string_lossy(), meta.file_type().is_symlink()) {
            continue;
        }
        if meta.file_type().is_symlink() {
            if top {
                manifest.shared.push(name.to_string_lossy().into_owned());
            } else {
                summary.skipped_links += 1;
            }
        } else if meta.is_dir() {
            entries.push((rel.clone(), true));
            walk(profile, &rel, entries, manifest, summary)?;
        } else if meta.is_file() {
            entries.push((rel, false));
            summary.files += 1;
        }
    }
    Ok(())
}

fn create_private(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .with_context(|| format!("creating {}", path.display()))
}

/// Unpacks an export. `create` gets the manifest and returns the new profile's folder; if
/// anything fails after that, the folder is removed again.
pub fn import(
    file: &Path,
    create: impl FnOnce(&Manifest) -> Result<PathBuf>,
) -> Result<(Manifest, PathBuf, Summary)> {
    let bad = |message: &str| Error::Archive {
        file: file.display().to_string(),
        message: message.to_owned(),
    };
    let reader = fs::File::open(file).with_context(|| format!("reading {}", file.display()))?;
    let mut archive = tar::Archive::new(GzDecoder::new(reader));
    let mut entries = archive
        .entries()
        .map_err(|_| bad("not a ccswitch export"))?;

    let mut first = entries
        .next()
        .and_then(io::Result::ok)
        .ok_or_else(|| bad("not a ccswitch export"))?;
    if first.path().ok().as_deref() != Some(Path::new(MANIFEST)) {
        return Err(bad("not a ccswitch export"));
    }
    let mut text = String::new();
    first
        .read_to_string(&mut text)
        .map_err(|_| bad("unreadable manifest"))?;
    let manifest = parse_manifest(&text).ok_or_else(|| bad("unreadable manifest"))?;

    let profile = create(&manifest)?;
    let result = (|| {
        let mut summary = Summary {
            files: 0,
            skipped_links: 0,
        };
        for entry in entries {
            let mut entry = entry.map_err(|e| bad(&e.to_string()))?;
            let path = entry.path().map_err(|e| bad(&e.to_string()))?.into_owned();
            let rel = path
                .strip_prefix(ROOT)
                .ok()
                .filter(|rel| {
                    rel.components().all(|c| matches!(c, Component::Normal(_)))
                        && rel.components().next().is_some()
                })
                .ok_or_else(|| bad(&format!("unsafe path {}", path.display())))?
                .to_path_buf();
            let top = rel
                .components()
                .next()
                .map(|c| c.as_os_str().to_string_lossy());
            if !top.is_some_and(|name| share::is_portable(&name, false)) {
                return Err(bad(&format!(
                    "contains {}, which is never imported",
                    rel.display()
                )));
            }
            let dest = profile.join(&rel);
            match entry.header().entry_type() {
                tar::EntryType::Directory => {
                    fs::create_dir_all(&dest)
                        .with_context(|| format!("creating {}", dest.display()))?;
                }
                tar::EntryType::Regular => {
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)
                            .with_context(|| format!("creating {}", parent.display()))?;
                    }
                    entry
                        .unpack(&dest)
                        .with_context(|| format!("writing {}", dest.display()))?;
                    summary.files += 1;
                }
                _ => return Err(bad(&format!("{} is a link or special file", rel.display()))),
            }
        }
        if !manifest.mcp_servers.is_empty() {
            let mut json = Map::new();
            claude_json::set_mcp_servers(&mut json, manifest.mcp_servers.clone());
            claude_json::write(&profile, &json)?;
        }
        Ok(summary)
    })();
    match result {
        Ok(summary) => Ok((manifest, profile, summary)),
        Err(e) => {
            let _ = fs::remove_dir_all(&profile);
            Err(e)
        }
    }
}

fn parse_manifest(text: &str) -> Option<Manifest> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v.get("format")?.as_u64()? != FORMAT {
        return None;
    }
    let shared = v
        .get("shared")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect();
    Some(Manifest {
        name: v.get("name")?.as_str()?.to_owned(),
        shared,
        mcp_servers: v.get("mcpServers")?.as_object()?.clone(),
    })
}
