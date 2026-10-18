use std::{
    env, fs,
    io::{self, BufRead, IsTerminal, Write},
    path::Path,
    time::SystemTime,
};

use serde_json::json;

use crate::{
    claude_json,
    cli::{Command, ShareArgs, Toggle},
    date,
    error::{Context, Error, Result},
    launch, share, shell,
    store::{Source, Store},
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
            share,
        } => add(&store, &name, no_login, default, share),
        Command::List => list(&store),
        Command::Use { name, here: false } => {
            store.set_default(&name)?;
            println!("Default profile: {name}");
            Ok(0)
        }
        Command::Use { name, here: true } => {
            let dir =
                env::current_dir().with_context(|| "finding the current folder".to_owned())?;
            let file = store.pin_folder(&dir, &name)?;
            println!("This folder now uses '{name}' ({})", file.display());
            Ok(0)
        }
        Command::Current => {
            let (name, _, source) = store.resolve_with_source(None)?;
            println!("{name}");
            if source != Source::Default {
                eprintln!("(from {})", source.describe());
            }
            Ok(0)
        }
        Command::Status { json } => status(&store, json),
        Command::Env { name, shell, unset } => {
            let shell = shell.unwrap_or_else(shell::guess);
            if unset {
                print!("{}", shell::unset(shell));
            } else {
                let (name, path) = store.resolve(name.as_deref())?;
                print!("{}", shell::set(shell, &path.display().to_string(), &name));
            }
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
        Command::Share(args) => share(&store, &args),
        Command::Path { name } => {
            let (_, path) = store.resolve(name.as_deref())?;
            println!("{}", path.display());
            Ok(0)
        }
    }
}

fn add(store: &Store, name: &str, no_login: bool, make_default: bool, share: bool) -> Result<i32> {
    let path = store.create(name)?;
    let make_default = make_default
        || matches!(
            store.resolve_default(),
            Err(Error::NoDefault | Error::DefaultMissing(_))
        );
    if make_default {
        store.set_default(name)?;
    }
    println!(
        "Created profile '{name}'{}",
        if make_default { " (default)" } else { "" }
    );
    if share {
        share_new(&path);
    }
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

fn status(store: &Store, as_json: bool) -> Result<i32> {
    let names = store.list()?;
    let default = store.default_name()?;
    let now = SystemTime::now();
    let mut rows = Vec::new();
    let mut objects = Vec::new();
    for name in &names {
        let profile = store.existing(name)?;
        let config = claude_json::read(&profile);
        let account = config.as_ref().ok().and_then(claude_json::account);
        let mcp = config
            .as_ref()
            .map_or(0, |c| claude_json::mcp_servers(c).len());
        let bypass = store.bypass(name).unwrap_or(false);
        let last_used = fs::metadata(profile.join("history.jsonl"))
            .and_then(|m| m.modified())
            .ok();
        let is_default = default.as_deref() == Some(name.as_str());
        if as_json {
            objects.push(json!({
                "name": name,
                "default": is_default,
                "email": account.as_ref().map(|a| &a.email),
                "organization": account.as_ref().map(|a| &a.org),
                "plan": account.as_ref().map(|a| &a.plan),
                "mcpServers": mcp,
                "bypass": bypass,
                "lastUsed": last_used
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()),
            }));
            continue;
        }
        let (email, org) = match (&config, &account) {
            (Err(_), _) => (".claude.json unreadable".to_owned(), "-".to_owned()),
            (_, None) => ("not logged in".to_owned(), "-".to_owned()),
            (_, Some(a)) => (a.email.clone(), a.org_and_plan()),
        };
        rows.push([
            if is_default { "*" } else { " " }.to_owned(),
            name.clone(),
            email,
            org,
            mcp.to_string(),
            if bypass { "on" } else { "off" }.to_owned(),
            last_used.map_or("never".to_owned(), |t| date::ago(t, now)),
        ]);
    }
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&objects).expect("JSON always serializes")
        );
        return Ok(0);
    }
    if names.is_empty() {
        eprintln!("No profiles yet. Create one with `ccswitch add <name>`.");
        return Ok(0);
    }
    let header = [
        "",
        "NAME",
        "ACCOUNT",
        "ORG / PLAN",
        "MCP",
        "BYPASS",
        "LAST USED",
    ]
    .map(String::from);
    print_table(&header, &rows);
    Ok(0)
}

/// Left-aligned columns separated by two spaces, without trailing spaces.
fn print_table<const N: usize>(header: &[String; N], rows: &[[String; N]]) {
    let mut widths = [0; N];
    for row in std::iter::once(header).chain(rows) {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    for row in std::iter::once(header).chain(rows) {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(cell, w)| format!("{cell:w$}"))
            .collect();
        println!("{}", line.join("  ").trim_end());
    }
}

fn share(store: &Store, args: &ShareArgs) -> Result<i32> {
    let names = match &args.name {
        Some(name) => vec![name.clone()],
        None => store.list()?,
    };
    if names.is_empty() {
        eprintln!("No profiles yet. Create one with `ccswitch add <name>`.");
        return Ok(0);
    }
    let profiles = names
        .iter()
        .map(|name| store.existing(name))
        .collect::<Result<Vec<_>>>()?;
    let items = share::check_items(&args.only)?;
    let source = match &args.from {
        Some(dir) => dir.clone(),
        None => share::default_source()?,
    };
    let opts = share::Opts {
        force: args.force,
        dry_run: args.dry_run,
    };
    for (name, profile) in names.iter().zip(&profiles) {
        let results = if args.status {
            share::status(&source, profile, &items)?
        } else if args.undo {
            share::unlink(&source, profile, &items, opts)?
        } else {
            share::link(&source, profile, &items, opts)?
        };
        println!("{name}:");
        for (item, outcome) in results {
            println!("  {item}: {}", outcome.describe(args.dry_run));
        }
    }
    Ok(0)
}

/// Links the default items into a freshly created profile. A problem here is reported but does not
/// undo the profile.
fn share_new(profile: &Path) {
    let opts = share::Opts {
        force: false,
        dry_run: false,
    };
    let result = share::default_source().and_then(|source| {
        let items = share::check_items(&[])?;
        share::link(&source, profile, &items, opts)
    });
    match result {
        Ok(results) => {
            let linked: Vec<_> = results
                .iter()
                .filter(|(_, o)| *o == share::Outcome::Linked)
                .map(|(item, _)| item.as_str())
                .collect();
            println!("Shared from your Claude folder: {}", linked.join(", "));
        }
        Err(e) => eprintln!("warning: could not share config: {e}"),
    }
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
