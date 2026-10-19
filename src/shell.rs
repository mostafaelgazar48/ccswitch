//! Shell commands printed by `ccswitch env`, for `eval` in the user's shell.

use std::env;

use crate::cli::Shell;

const VARS: [&str; 2] = ["CLAUDE_CONFIG_DIR", "CCSWITCH_PROFILE"];

/// The shell named by `$SHELL`; PowerShell on Windows, where `$SHELL` is rarely set.
pub fn guess() -> Shell {
    let name = env::var("SHELL").unwrap_or_default();
    let name = name.rsplit(['/', '\\']).next().unwrap_or_default();
    match name {
        "fish" => Shell::Fish,
        "pwsh" | "powershell" | "pwsh.exe" | "powershell.exe" => Shell::Powershell,
        "" if cfg!(windows) => Shell::Powershell,
        _ => Shell::Sh,
    }
}

pub fn set(shell: Shell, config_dir: &str, profile: &str) -> String {
    VARS.iter()
        .zip([config_dir, profile])
        .map(|(var, value)| match shell {
            Shell::Sh => format!("export {var}={}\n", quote(shell, value)),
            Shell::Fish => format!("set -gx {var} {}\n", quote(shell, value)),
            Shell::Powershell => format!("$env:{var} = {}\n", quote(shell, value)),
        })
        .collect()
}

pub fn unset(shell: Shell) -> String {
    match shell {
        Shell::Sh => format!("unset {}\n", VARS.join(" ")),
        Shell::Fish => VARS.iter().map(|v| format!("set -e {v}\n")).collect(),
        Shell::Powershell => VARS
            .iter()
            .map(|v| format!("Remove-Item Env:{v} -ErrorAction SilentlyContinue\n"))
            .collect(),
    }
}

/// Single-quotes `value` so the shell takes it literally.
fn quote(shell: Shell, value: &str) -> String {
    match shell {
        Shell::Sh => format!("'{}'", value.replace('\'', r"'\''")),
        Shell::Fish => format!("'{}'", value.replace('\\', r"\\").replace('\'', r"\'")),
        Shell::Powershell => format!("'{}'", value.replace('\'', "''")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_for_each_shell() {
        assert_eq!(
            set(Shell::Sh, "/a b/it's", "w"),
            "export CLAUDE_CONFIG_DIR='/a b/it'\\''s'\nexport CCSWITCH_PROFILE='w'\n"
        );
        assert_eq!(
            set(Shell::Fish, r"/x\it's", "w"),
            "set -gx CLAUDE_CONFIG_DIR '/x\\\\it\\'s'\nset -gx CCSWITCH_PROFILE 'w'\n"
        );
        assert_eq!(
            set(Shell::Powershell, r"C:\it's", "w"),
            "$env:CLAUDE_CONFIG_DIR = 'C:\\it''s'\n$env:CCSWITCH_PROFILE = 'w'\n"
        );
    }

    #[test]
    fn unsets_both_vars() {
        assert_eq!(
            unset(Shell::Sh),
            "unset CLAUDE_CONFIG_DIR CCSWITCH_PROFILE\n"
        );
        assert_eq!(
            unset(Shell::Fish),
            "set -e CLAUDE_CONFIG_DIR\nset -e CCSWITCH_PROFILE\n"
        );
        assert!(unset(Shell::Powershell).contains("Remove-Item Env:CCSWITCH_PROFILE"));
    }
}
