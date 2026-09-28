use std::{
    env,
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::error::{Error, Result};

/// Runs Claude Code against `config_dir` and returns its exit code.
/// On Unix this replaces the current process and only returns on failure.
pub fn claude(config_dir: &Path, args: &[String]) -> Result<i32> {
    let program = env::var_os("CCSWITCH_CLAUDE")
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| OsString::from("claude"));
    let mut cmd = Command::new(resolve_program(&program));
    cmd.env("CLAUDE_CONFIG_DIR", config_dir).args(args);
    run(cmd).map_err(|e| spawn_error(e, &program))
}

fn spawn_error(e: io::Error, program: &OsStr) -> Error {
    let name = program.to_string_lossy().into_owned();
    if e.kind() == io::ErrorKind::NotFound {
        Error::ClaudeNotFound(name)
    } else {
        Error::io(format!("running {name}"), e)
    }
}

#[cfg(unix)]
fn run(mut cmd: Command) -> io::Result<i32> {
    use std::os::unix::process::CommandExt;
    // exec hands the terminal, signals and exit status straight to Claude.
    Err(cmd.exec())
}

#[cfg(not(unix))]
fn run(mut cmd: Command) -> io::Result<i32> {
    Ok(cmd.status()?.code().unwrap_or(1))
}

#[cfg(not(windows))]
fn resolve_program(program: &OsStr) -> PathBuf {
    PathBuf::from(program)
}

/// `Command` only appends `.exe` when searching PATH, but npm installs Claude as `claude.cmd`.
#[cfg(windows)]
fn resolve_program(program: &OsStr) -> PathBuf {
    let path = Path::new(program);
    if path.extension().is_some() || path.components().count() > 1 {
        return path.to_path_buf();
    }
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .flat_map(|dir| ["exe", "cmd", "bat"].map(|ext| dir.join(program).with_extension(ext)))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| path.to_path_buf())
}
