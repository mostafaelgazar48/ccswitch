use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use crate::{
    error::{Context, Error, Result},
    settings::{self, Settings},
};

pub const MAX_NAME_LEN: usize = 64;

pub fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidName(name.to_owned()))
    }
}

/// Profiles on disk: `<root>/profiles/<name>/` plus a `<root>/default` file naming the default.
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn from_env() -> Result<Self> {
        let root = match env::var_os("CCSWITCH_HOME") {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => dirs::home_dir().ok_or(Error::NoHome)?.join(".ccswitch"),
        };
        Ok(Self::new(root))
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }

    fn default_file(&self) -> PathBuf {
        self.root.join("default")
    }

    /// Where a profile lives, whether or not it exists yet.
    pub fn path(&self, name: &str) -> Result<PathBuf> {
        validate_name(name)?;
        Ok(self.profiles_dir().join(name))
    }

    /// Path of a profile that must already exist.
    pub fn existing(&self, name: &str) -> Result<PathBuf> {
        let path = self.path(name)?;
        if path.is_dir() {
            Ok(path)
        } else {
            Err(Error::NotFound(name.to_owned()))
        }
    }

    pub fn create(&self, name: &str) -> Result<PathBuf> {
        let path = self.path(name)?;
        let profiles = self.profiles_dir();
        private_dir(&profiles, true).with_context(|| format!("creating {}", profiles.display()))?;
        match private_dir(&path, false) {
            Ok(()) => Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                Err(Error::AlreadyExists(name.to_owned()))
            }
            Err(e) => Err(Error::io(format!("creating {}", path.display()), e)),
        }
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let dir = self.profiles_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(format!("reading {}", dir.display()), e)),
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| validate_name(n).is_ok())
            .collect();
        names.sort();
        Ok(names)
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let path = self.existing(name)?;
        fs::remove_dir_all(&path).with_context(|| format!("removing {}", path.display()))?;
        if self.default_name()?.as_deref() == Some(name) {
            self.clear_default()?;
        }
        Ok(())
    }

    pub fn rename(&self, old: &str, new: &str) -> Result<()> {
        let from = self.existing(old)?;
        let to = self.path(new)?;
        if to.exists() {
            return Err(Error::AlreadyExists(new.to_owned()));
        }
        fs::rename(&from, &to)
            .with_context(|| format!("renaming {} to {}", from.display(), to.display()))?;
        if self.default_name()?.as_deref() == Some(old) {
            write_atomic(&self.default_file(), new)?;
        }
        Ok(())
    }

    /// The name stored as default; it may refer to a profile that was deleted by hand.
    pub fn default_name(&self) -> Result<Option<String>> {
        let file = self.default_file();
        match fs::read_to_string(&file) {
            Ok(s) => Ok(Some(s.trim().to_owned()).filter(|s| !s.is_empty())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::io(format!("reading {}", file.display()), e)),
        }
    }

    pub fn set_default(&self, name: &str) -> Result<()> {
        self.existing(name)?;
        write_atomic(&self.default_file(), name)
    }

    fn clear_default(&self) -> Result<()> {
        let file = self.default_file();
        match fs::remove_file(&file) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                Err(Error::io(format!("removing {}", file.display()), e))
            }
            _ => Ok(()),
        }
    }

    /// Whether the profile starts Claude in bypass-permissions mode.
    pub fn bypass(&self, name: &str) -> Result<bool> {
        let (_, settings) = self.settings(name)?;
        Ok(settings::is_bypass(&settings))
    }

    pub fn set_bypass(&self, name: &str, on: bool) -> Result<()> {
        let (file, mut settings) = self.settings(name)?;
        settings::set_bypass(&mut settings, on)
            .map_err(|message| settings_error(&file, message))?;
        write_atomic(&file, &settings::to_string(&settings))
    }

    fn settings(&self, name: &str) -> Result<(PathBuf, Settings)> {
        let file = self.existing(name)?.join("settings.json");
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(Error::io(format!("reading {}", file.display()), e)),
        };
        let settings = settings::parse(&text).map_err(|message| settings_error(&file, message))?;
        Ok((file, settings))
    }

    /// The named profile, or the default when no name is given.
    pub fn resolve(&self, name: Option<&str>) -> Result<(String, PathBuf)> {
        if let Some(name) = name {
            return Ok((name.to_owned(), self.existing(name)?));
        }
        let name = self.default_name()?.ok_or(Error::NoDefault)?;
        match self.existing(&name) {
            Ok(path) => Ok((name, path)),
            Err(Error::NotFound(_) | Error::InvalidName(_)) => Err(Error::DefaultMissing(name)),
            Err(e) => Err(e),
        }
    }
}

fn settings_error(file: &Path, message: String) -> Error {
    Error::Settings {
        path: file.display().to_string(),
        message,
    }
}

/// Creates a directory only the current user can read; profiles hold login credentials.
fn private_dir(path: &Path, recursive: bool) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(recursive);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)
        .and_then(|()| fs::rename(&tmp, path))
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempStore {
        store: Store,
        root: PathBuf,
    }

    impl TempStore {
        fn new(tag: &str) -> Self {
            let root = env::temp_dir().join(format!("ccswitch-unit-{}-{tag}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            Self {
                store: Store::new(&root),
                root,
            }
        }
    }

    impl Drop for TempStore {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn name_validation() {
        for ok in ["work", "a", "my_profile-2", &"x".repeat(MAX_NAME_LEN)] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "-x",
            "../x",
            "a/b",
            "a b",
            "é",
            ".",
            &"x".repeat(MAX_NAME_LEN + 1),
        ] {
            assert!(
                matches!(validate_name(bad), Err(Error::InvalidName(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn create_list_and_duplicate() {
        let t = TempStore::new("create");
        assert!(t.store.list().unwrap().is_empty());
        t.store.create("b").unwrap();
        t.store.create("a").unwrap();
        assert_eq!(t.store.list().unwrap(), ["a", "b"]);
        assert!(matches!(t.store.create("a"), Err(Error::AlreadyExists(_))));
    }

    #[test]
    fn resolve_uses_default() {
        let t = TempStore::new("resolve");
        assert!(matches!(t.store.resolve(None), Err(Error::NoDefault)));
        t.store.create("work").unwrap();
        t.store.set_default("work").unwrap();
        assert_eq!(t.store.resolve(None).unwrap().0, "work");
        assert!(matches!(
            t.store.resolve(Some("nope")),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn stale_default_is_reported() {
        let t = TempStore::new("stale");
        t.store.create("work").unwrap();
        t.store.set_default("work").unwrap();
        fs::remove_dir_all(t.store.path("work").unwrap()).unwrap();
        assert!(matches!(t.store.resolve(None), Err(Error::DefaultMissing(n)) if n == "work"));
    }

    #[test]
    fn remove_and_rename_keep_default_in_sync() {
        let t = TempStore::new("sync");
        t.store.create("old").unwrap();
        t.store.set_default("old").unwrap();
        t.store.rename("old", "new").unwrap();
        assert_eq!(t.store.default_name().unwrap().as_deref(), Some("new"));
        t.store.remove("new").unwrap();
        assert_eq!(t.store.default_name().unwrap(), None);
        assert!(t.store.list().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn profiles_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let t = TempStore::new("perms");
        let path = t.store.create("work").unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}
