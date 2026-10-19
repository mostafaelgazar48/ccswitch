use std::{
    collections::HashSet,
    env, fs,
    io::{self, BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use serde_json::json;

use crate::{
    archive, claude_json,
    cli::{Command, GroupBy, HandoffArgs, McpCommand, ShareArgs, Toggle},
    date,
    error::{Context, Error, Result},
    launch, sessions, share, shell,
    store::{self, Source, Store},
    usage::{self, Totals},
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
        Command::Sessions { name, all, limit } => {
            list_sessions(&store, name.as_deref(), all, limit)
        }
        Command::Handoff(args) => handoff(&store, args),
        Command::Usage {
            name,
            since,
            by,
            json,
        } => show_usage(&store, name, &since, by, json),
        Command::Clone {
            from,
            new,
            no_login,
        } => clone(&store, &from, &new, no_login),
        Command::Export { name, output } => export(&store, &name, output),
        Command::Import {
            file,
            name,
            share,
            no_login,
        } => import(&store, &file, name, share, no_login),
        Command::Mcp(McpCommand::List { name, all }) => mcp_list(&store, name.as_deref(), all),
        Command::Mcp(McpCommand::Copy {
            from,
            to,
            servers,
            force,
        }) => mcp_copy(&store, &from, &to, &servers, force),
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
    print_table(&header, &rows, header.len());
    Ok(0)
}

/// Columns separated by two spaces, without trailing spaces. Columns from `right_from` on are
/// right-aligned.
fn print_table<R: AsRef<[String]>>(header: &[String], rows: &[R], right_from: usize) {
    let mut widths = vec![0; header.len()];
    for row in std::iter::once(header).chain(rows.iter().map(AsRef::as_ref)) {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    for row in std::iter::once(header).chain(rows.iter().map(AsRef::as_ref)) {
        let line: Vec<String> = row
            .iter()
            .zip(&widths)
            .enumerate()
            .map(|(i, (cell, w))| {
                if i >= right_from {
                    format!("{cell:>w$}")
                } else {
                    format!("{cell:w$}")
                }
            })
            .collect();
        println!("{}", line.join("  ").trim_end());
    }
}

fn current_dir() -> Result<PathBuf> {
    env::current_dir().with_context(|| "finding the current folder".to_owned())
}

fn list_sessions(store: &Store, name: Option<&str>, all: bool, limit: usize) -> Result<i32> {
    let (name, profile) = store.resolve(name)?;
    let folder = if all { None } else { Some(current_dir()?) };
    let found = sessions::list(&profile, folder.as_deref())?;
    if found.is_empty() {
        let where_ = if all {
            ""
        } else {
            " for this folder (try --all)"
        };
        eprintln!("No sessions in '{name}'{where_}.");
        return Ok(0);
    }
    let now = SystemTime::now();
    let mut header = vec!["ID", "UPDATED", "TITLE"];
    if all {
        header.push("FOLDER");
    }
    let rows: Vec<Vec<String>> = found
        .iter()
        .take(limit)
        .map(|s| {
            let mut row = vec![
                s.id.clone(),
                date::ago(s.modified, now),
                s.title.clone().unwrap_or_else(|| "(untitled)".to_owned()),
            ];
            if all {
                row.push(s.cwd.clone().unwrap_or_else(|| s.slug.clone()));
            }
            row
        })
        .collect();
    let header: Vec<String> = header.into_iter().map(String::from).collect();
    print_table(&header, &rows, header.len());
    if found.len() > limit {
        eprintln!("({} more; use -n to show more)", found.len() - limit);
    }
    Ok(0)
}

fn handoff(store: &Store, args: HandoffArgs) -> Result<i32> {
    let (from_name, from) = store.resolve(args.from.as_deref())?;
    let to = store.existing(&args.to)?;
    if from_name == args.to {
        return Err(Error::SameProfile(from_name));
    }
    let here = current_dir()?;
    let session = match &args.session {
        Some(id) => sessions::find(&from, id)?,
        None => sessions::list(&from, Some(&here))?
            .into_iter()
            .next()
            .ok_or_else(|| {
                Error::NoSession(format!("no sessions for this folder in '{from_name}'"))
            })?,
    };
    if session.exists_in(&to) && !args.force {
        return Err(Error::SessionExists {
            id: session.id,
            profile: args.to,
        });
    }
    let age = SystemTime::now()
        .duration_since(session.modified)
        .unwrap_or_default();
    if age < Duration::from_secs(60) {
        eprintln!(
            "warning: this conversation was updated just now; if Claude is still running it in '{from_name}', quit it there first"
        );
    }
    session.copy(&from, &to)?;
    if args.move_session {
        session.remove(&from)?;
    }
    println!(
        "{} \"{}\" ({}) from '{from_name}' to '{}'",
        if args.move_session { "Moved" } else { "Copied" },
        session.title.as_deref().unwrap_or("untitled"),
        session.id,
        args.to
    );
    if args.no_resume {
        println!(
            "Continue it with: ccswitch run {} -- --resume {}",
            args.to, session.id
        );
        return Ok(0);
    }
    // Claude finds a session through the folder it was started in.
    if sessions::slug(&here) != session.slug {
        if let Some(dir) = session.cwd.as_deref().filter(|d| Path::new(d).is_dir()) {
            env::set_current_dir(dir).with_context(|| format!("changing to {dir}"))?;
            eprintln!("Resuming in {dir}");
        }
    }
    let mut claude_args = vec!["--resume".to_owned(), session.id];
    claude_args.extend(args.args);
    launch::claude(&to, &claude_args)
}

fn show_usage(
    store: &Store,
    name: Option<String>,
    since: &str,
    by: GroupBy,
    as_json: bool,
) -> Result<i32> {
    let (since_day, since_time) =
        date::since(since, SystemTime::now()).ok_or_else(|| Error::BadSince(since.to_owned()))?;
    let names = match name {
        Some(name) => vec![name],
        None => store.list()?,
    };
    let mut requests = Vec::new();
    let mut seen = HashSet::new();
    for name in &names {
        let profile = store.existing(name)?;
        for request in usage::requests(&profile, &since_day, since_time, &mut seen)? {
            requests.push((name, request));
        }
    }
    let groups = usage::group(requests.iter().map(|(profile, r)| {
        let key = match by {
            GroupBy::Day => r.day.clone(),
            GroupBy::Model => r.model.clone(),
            GroupBy::Profile => (*profile).clone(),
            GroupBy::Session => format!("{profile}/{}", r.session),
        };
        (key, &r.totals)
    }));
    let mut rows: Vec<(String, Totals)> = groups.into_iter().collect();
    if by != GroupBy::Day {
        rows.sort_by(|a, b| {
            b.1.cost
                .total_cmp(&a.1.cost)
                .then(b.1.tokens().cmp(&a.1.tokens()))
        });
    }
    let mut total = Totals::default();
    for (_, t) in &rows {
        total.add(t);
    }

    if as_json {
        let object = |t: &Totals| {
            json!({
                "requests": t.requests,
                "input": t.input,
                "output": t.output,
                "cacheWrite": t.cache_write,
                "cacheRead": t.cache_read,
                "totalTokens": t.tokens(),
                "estimatedCostUsd": (t.cost * 100.0).round() / 100.0,
                "unpricedRequests": t.unpriced,
            })
        };
        let rows: Vec<_> = rows
            .iter()
            .map(|(key, t)| {
                let mut o = object(t);
                o["key"] = key.clone().into();
                o
            })
            .collect();
        let out = json!({ "since": since_day, "rows": rows, "total": object(&total) });
        println!(
            "{}",
            serde_json::to_string_pretty(&out).expect("JSON always serializes")
        );
        return Ok(0);
    }
    if rows.is_empty() {
        eprintln!("No usage since {since_day}.");
        return Ok(0);
    }
    let by_name = match by {
        GroupBy::Day => "DAY",
        GroupBy::Model => "MODEL",
        GroupBy::Profile => "PROFILE",
        GroupBy::Session => "SESSION",
    };
    let cells = |key: &str, t: &Totals| {
        vec![
            key.to_owned(),
            usage::thousands(t.requests),
            usage::thousands(t.input),
            usage::thousands(t.output),
            usage::thousands(t.cache_write),
            usage::thousands(t.cache_read),
            usage::thousands(t.tokens()),
            format!("${:.2}", t.cost),
        ]
    };
    let header: Vec<String> = [
        by_name,
        "REQUESTS",
        "INPUT",
        "OUTPUT",
        "CACHE WRITE",
        "CACHE READ",
        "TOTAL",
        "EST. COST",
    ]
    .map(String::from)
    .to_vec();
    let mut table: Vec<Vec<String>> = rows.iter().map(|(k, t)| cells(k, t)).collect();
    table.push(cells("TOTAL", &total));
    print_table(&header, &table, 1);
    eprintln!(
        "Since {since_day} (UTC). Cost is estimated at API list prices; subscription plans are not billed per token."
    );
    if total.unpriced > 0 {
        eprintln!(
            "{} request(s) from models without a known price are not in the cost.",
            total.unpriced
        );
    }
    Ok(0)
}

fn clone(store: &Store, from: &str, new: &str, no_login: bool) -> Result<i32> {
    let source = store.existing(from)?;
    let servers = claude_json::mcp_servers(&claude_json::read(&source)?);
    let dest = store.create(new)?;
    let skip = |rel: &Path| {
        let mut parts = rel.components();
        match (parts.next(), parts.next()) {
            (Some(name), None) => {
                let is_link = fs::symlink_metadata(source.join(rel))
                    .is_ok_and(|m| m.file_type().is_symlink());
                !share::is_portable(&name.as_os_str().to_string_lossy(), is_link)
            }
            _ => false,
        }
    };
    let result = store::copy_tree(&source, &dest, &skip).and_then(|()| {
        if servers.is_empty() {
            return Ok(());
        }
        let mut json = serde_json::Map::new();
        claude_json::set_mcp_servers(&mut json, servers.clone());
        claude_json::write(&dest, &json)
    });
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&dest);
        return Err(e);
    }
    println!(
        "Created profile '{new}' from '{from}' with its settings and {} MCP server(s); the login and history were not copied.",
        servers.len()
    );
    if no_login {
        return Ok(0);
    }
    println!("Starting Claude Code; log in when prompted.");
    launch::claude(&dest, &[])
}

fn export(store: &Store, name: &str, output: Option<PathBuf>) -> Result<i32> {
    let profile = store.existing(name)?;
    let out = output.unwrap_or_else(|| PathBuf::from(format!("{name}.ccswitch.tar.gz")));
    let (manifest, summary) = archive::export(&profile, name, &out)?;
    println!(
        "Exported '{name}' to {} ({} file(s), {} MCP server(s)). Logins and history are not included.",
        out.display(),
        summary.files,
        manifest.mcp_servers.len()
    );
    if !manifest.shared.is_empty() {
        println!(
            "Shared items are not included, only named: {}. `ccswitch import --share` links them again.",
            manifest.shared.join(", ")
        );
    }
    if summary.skipped_links > 0 {
        eprintln!(
            "warning: left out {} link(s) inside folders",
            summary.skipped_links
        );
    }
    if !manifest.mcp_servers.is_empty() {
        eprintln!("note: MCP server settings can hold API keys; keep this file private.");
    }
    Ok(0)
}

fn import(
    store: &Store,
    file: &Path,
    name: Option<String>,
    share_items: bool,
    no_login: bool,
) -> Result<i32> {
    let mut chosen = name;
    let (manifest, profile, summary) = archive::import(file, |manifest| {
        let name = chosen.get_or_insert_with(|| manifest.name.clone());
        store.create(name)
    })?;
    let name = chosen.expect("set when the profile was created");
    println!(
        "Created profile '{name}' from {} ({} file(s), {} MCP server(s))",
        file.display(),
        summary.files,
        manifest.mcp_servers.len()
    );
    if !manifest.shared.is_empty() {
        if share_items {
            let linked = share::check_items(&manifest.shared).and_then(|items| {
                let opts = share::Opts {
                    force: false,
                    dry_run: false,
                };
                share::link(&share::default_source()?, &profile, &items, opts)
            });
            match linked {
                Ok(results) => {
                    for (item, outcome) in results {
                        println!("  {item}: {}", outcome.describe(false));
                    }
                }
                Err(e) => eprintln!("warning: could not share config: {e}"),
            }
        } else {
            println!(
                "It shared {} from ~/.claude; link them with `ccswitch share {name} --only {}`.",
                manifest.shared.join(", "),
                manifest.shared.join(",")
            );
        }
    }
    if no_login {
        return Ok(0);
    }
    println!("Starting Claude Code; log in when prompted.");
    launch::claude(&profile, &[])
}

fn mcp_list(store: &Store, name: Option<&str>, all: bool) -> Result<i32> {
    let names = if all {
        store.list()?
    } else {
        vec![store.resolve(name)?.0]
    };
    for name in &names {
        let servers = claude_json::mcp_servers(&claude_json::read(&store.existing(name)?)?);
        let indent = if all {
            println!("{name}:");
            "  "
        } else {
            ""
        };
        if servers.is_empty() {
            println!("{indent}(no MCP servers)");
            continue;
        }
        let width = servers.keys().map(|k| k.chars().count()).max().unwrap_or(0);
        for (server, config) in &servers {
            println!(
                "{indent}{server:width$}  {}",
                claude_json::describe_server(config)
            );
        }
    }
    Ok(0)
}

fn mcp_copy(store: &Store, from: &str, to: &str, servers: &[String], force: bool) -> Result<i32> {
    let source = claude_json::mcp_servers(&claude_json::read(&store.existing(from)?)?);
    let dest_path = store.existing(to)?;
    if from == to {
        return Err(Error::SameProfile(to.to_owned()));
    }
    let mut dest = claude_json::read(&dest_path)?;
    let mut target = claude_json::mcp_servers(&dest);
    let names: Vec<String> = if servers.is_empty() {
        source.keys().cloned().collect()
    } else {
        servers.to_vec()
    };
    if names.is_empty() {
        println!("'{from}' has no MCP servers to copy.");
        return Ok(0);
    }
    let mut changed = false;
    for server in &names {
        let config = source.get(server).ok_or_else(|| Error::McpMissing {
            server: server.clone(),
            profile: from.to_owned(),
        })?;
        let outcome = match target.get(server) {
            Some(existing) if existing == config => "already there",
            Some(_) if !force => "kept (it has its own; use --force to replace it)",
            Some(_) => "replaced",
            None => "copied",
        };
        if matches!(outcome, "copied" | "replaced") {
            target.insert(server.clone(), config.clone());
            changed = true;
        }
        println!("{server}: {outcome}");
    }
    if changed {
        claude_json::set_mcp_servers(&mut dest, target);
        claude_json::write(&dest_path, &dest)?;
        eprintln!("Restart Claude in '{to}' if it's running, or it may overwrite this change.");
    }
    Ok(0)
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
