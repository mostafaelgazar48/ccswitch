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
    let program = program_from_env("CCSWITCH_CLAUDE", "claude");
    let mut cmd = Command::new(resolve_program(&program));
    cmd.env("CLAUDE_CONFIG_DIR", config_dir).args(args);
    run(cmd).map_err(|e| spawn_error(e, &program, "install Claude Code or set CCSWITCH_CLAUDE"))
}

/// Opens VS Code with its own `user_data_dir`, so it starts a separate instance whose
/// Claude Code extension inherits `CLAUDE_CONFIG_DIR` instead of reusing a running window.
pub fn code(config_dir: &Path, user_data_dir: &Path, args: &[String]) -> Result<i32> {
    let program = program_from_env("CCSWITCH_CODE", "code");
    let mut cmd = Command::new(resolve_program(&program));
    cmd.env("CLAUDE_CONFIG_DIR", config_dir)
        .arg("--user-data-dir")
        .arg(user_data_dir)
        .args(args);
    run(cmd).map_err(|e| {
        spawn_error(
            e,
            &program,
            "install VS Code's `code` command or set CCSWITCH_CODE",
        )
    })
}

fn program_from_env(var: &str, default: &str) -> OsString {
    env::var_os(var)
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| OsString::from(default))
}

fn spawn_error(e: io::Error, program: &OsStr, hint: &'static str) -> Error {
    let name = program.to_string_lossy().into_owned();
    if e.kind() == io::ErrorKind::NotFound {
        Error::ProgramNotFound {
            program: name,
            hint,
        }
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
