use std::io::{self, BufRead, IsTerminal, Write};

use crate::{
    cli::{Command, Toggle},
    error::{Context, Error, Result},
    launch,
    store::Store,
};

/// VS Code's user data for `ccswitch code`, kept inside the profile so rename and remove cover it.
const VSCODE_DATA_DIR: &str = "vscode-data";

/// Executes a command and returns the process exit code.
pub fn run(command: Command) -> Result<i32> {
    let store = Store::from_env()?;
    match command {
        Command::Add {
            name,
            no_login,
            default,
        } => add(&store, &name, no_login, default),
        Command::List => list(&store),
        Command::Use { name } => {
            store.set_default(&name)?;
            println!("Default profile: {name}");
            Ok(0)
        }
        Command::Current => {
            let (name, _) = store.resolve(None)?;
            println!("{name}");
            Ok(0)
        }
        Command::Run {
            name,
            bypass,
            mut args,
        } => {
            let (_, path) = store.resolve(name.as_deref())?;
            if bypass {
                args.insert(0, "--dangerously-skip-permissions".to_owned());
            }
            launch::claude(&path, &args)
        }
        Command::Code { name, mut args } => {
            let (_, path) = store.resolve(name.as_deref())?;
            if args.is_empty() {
                args.push(".".to_owned());
            }
            launch::code(&path, &path.join(VSCODE_DATA_DIR), &args)
        }
        Command::Bypass { name, state } => bypass(&store, &name, state),
        Command::Rename { old, new } => {
            store.rename(&old, &new)?;
            println!("Renamed '{old}' to '{new}'");
            Ok(0)
        }
        Command::Remove { name, yes } => remove(&store, &name, yes),
        Command::Path { name } => {
            let (_, path) = store.resolve(name.as_deref())?;
            println!("{}", path.display());
            Ok(0)
        }
    }
}

fn add(store: &Store, name: &str, no_login: bool, make_default: bool) -> Result<i32> {
    let path = store.create(name)?;
    let make_default = make_default
        || matches!(
            store.resolve(None),
            Err(Error::NoDefault | Error::DefaultMissing(_))
        );
    if make_default {
        store.set_default(name)?;
    }
    println!(
        "Created profile '{name}'{}",
        if make_default { " (default)" } else { "" }
    );
    if no_login {
        return Ok(0);
    }
    println!("Starting Claude Code; log in when prompted.");
    launch::claude(&path, &[])
}

fn list(store: &Store) -> Result<i32> {
    let names = store.list()?;
    let default = store.default_name()?;
    if names.is_empty() {
        eprintln!("No profiles yet. Create one with `ccswitch add <name>`.");
        return Ok(0);
    }
    for name in &names {
        let marker = if default.as_deref() == Some(name.as_str()) {
            "*"
        } else {
            " "
        };
        let bypass = if store.bypass(name).unwrap_or(false) {
            "  (bypass)"
        } else {
            ""
        };
        println!("{marker} {name}{bypass}");
    }
    if let Some(stale) = default.filter(|d| !names.contains(d)) {
        eprintln!("warning: default profile '{stale}' no longer exists");
    }
    Ok(0)
}

fn bypass(store: &Store, name: &str, state: Option<Toggle>) -> Result<i32> {
    match state {
        None => {
            let on = store.bypass(name)?;
            println!(
                "Bypass permissions for '{name}': {}",
                if on { "on" } else { "off" }
            );
        }
        Some(Toggle::On) => {
            store.set_bypass(name, true)?;
            println!("Bypass permissions on for '{name}': Claude will run tools without asking.");
        }
        Some(Toggle::Off) => {
            store.set_bypass(name, false)?;
            println!("Bypass permissions off for '{name}'");
        }
    }
    Ok(0)
}

fn remove(store: &Store, name: &str, yes: bool) -> Result<i32> {
    store.existing(name)?;
    if !yes && !confirm(name)? {
        return Err(Error::Aborted);
    }
    store.remove(name)?;
    println!("Removed profile '{name}'");
    Ok(0)
}

fn confirm(name: &str) -> Result<bool> {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        return Err(Error::NeedsConfirmation(name.to_owned()));
    }
    eprint!("Delete profile '{name}' and its saved login? [y/N] ");
    io::stderr().flush().ok();
    let mut answer = String::new();
    stdin
        .lock()
        .read_line(&mut answer)
        .with_context(|| "reading confirmation".to_owned())?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}
