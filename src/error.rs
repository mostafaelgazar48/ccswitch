use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    NoHome,
    InvalidName(String),
    NotFound(String),
    AlreadyExists(String),
    NoDefault,
    DefaultMissing(String),
    ProgramNotFound { program: String, hint: &'static str },
    NeedsConfirmation(String),
    Aborted,
    Settings { path: String, message: String },
    Io { context: String, source: io::Error },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(context: impl Into<String>, source: io::Error) -> Self {
        Error::Io {
            context: context.into(),
            source,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoHome => write!(f, "could not find your home directory; set CCSWITCH_HOME"),
            Error::InvalidName(n) => write!(
                f,
                "invalid profile name '{n}': use letters, digits, '-' or '_' (max 64, must not start with '-')"
            ),
            Error::NotFound(n) => write!(f, "profile '{n}' not found; create it with `ccswitch add {n}`"),
            Error::AlreadyExists(n) => write!(f, "profile '{n}' already exists"),
            Error::NoDefault => write!(f, "no default profile; pass a name or set one with `ccswitch use <name>`"),
            Error::DefaultMissing(n) => write!(
                f,
                "default profile '{n}' no longer exists; pick another with `ccswitch use <name>`"
            ),
            Error::ProgramNotFound { program, hint } => {
                write!(f, "could not run '{program}': not found ({hint})")
            }
            Error::NeedsConfirmation(n) => write!(f, "refusing to delete '{n}' without confirmation; pass --yes"),
            Error::Aborted => write!(f, "aborted"),
            Error::Settings { path, message } => write!(f, "cannot update {path}: {message}"),
            Error::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Attaches a human-readable description to an I/O failure.
pub trait Context<T> {
    fn with_context(self, context: impl FnOnce() -> String) -> Result<T>;
}

impl<T> Context<T> for io::Result<T> {
    fn with_context(self, context: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|e| Error::io(context(), e))
    }
}
