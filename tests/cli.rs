#![cfg(unix)]

use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

/// An isolated CCSWITCH_HOME plus a fake `claude` that records how it was called and exits 7.
struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let dir = env::temp_dir().join(format!("ccswitch-it-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("fake-claude");
        fs::write(
            &fake,
            "#!/bin/sh\nprintf '%s\\n' \"$CLAUDE_CONFIG_DIR\" \"$@\" > \"$(dirname \"$0\")/called\"\nexit 7\n",
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ccswitch"));
        cmd.args(args)
            .env("CCSWITCH_HOME", self.dir.join("home"))
            .env("CCSWITCH_CLAUDE", self.dir.join("fake-claude"))
            .env_remove("CCSWITCH_PROFILE")
            .current_dir(&self.dir)
            .stdin(Stdio::null());
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }

    fn profile(&self, name: &str) -> String {
        self.dir
            .join("home/profiles")
            .join(name)
            .display()
            .to_string()
    }

    fn called(&self) -> Vec<String> {
        fs::read_to_string(self.dir.join("called"))
            .unwrap()
            .lines()
            .map(String::from)
            .collect()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn add_launches_login_and_first_profile_becomes_default() {
    let s = Sandbox::new("add");
    let out = s.run(&["add", "work"]);
    assert_eq!(out.status.code(), Some(7), "exit code is Claude's");
    assert!(stdout(&out).contains("(default)"));
    assert_eq!(s.called(), [s.profile("work")]);

    assert!(s.run(&["add", "home", "--no-login"]).status.success());
    assert_eq!(stdout(&s.run(&["list"])), "  home\n* work\n");
}

#[test]
fn run_passes_config_dir_and_args() {
    let s = Sandbox::new("run");
    s.run(&["add", "a", "--no-login"]);
    s.run(&["add", "b", "--no-login"]);
    assert!(s.run(&["use", "b"]).status.success());

    let out = s.run(&["run", "--", "--resume", "two words"]);
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(
        s.called(),
        [s.profile("b"), "--resume".into(), "two words".into()]
    );

    s.run(&["run", "a"]);
    assert_eq!(s.called(), [s.profile("a")]);
}

#[test]
fn errors_are_readable() {
    let s = Sandbox::new("errors");
    let out = s.run(&["run"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        stderr(&out),
        "ccswitch: no default profile; pass a name or set one with `ccswitch use <name>`\n"
    );

    assert!(stderr(&s.run(&["add", "../evil"])).contains("invalid profile name"));
    assert!(stderr(&s.run(&["use", "ghost"])).contains("profile 'ghost' not found"));

    s.run(&["add", "a", "--no-login"]);
    assert!(stderr(&s.run(&["add", "a"])).contains("already exists"));
}

#[test]
fn missing_claude_binary_is_explained() {
    let s = Sandbox::new("nobin");
    s.run(&["add", "a", "--no-login"]);
    let out = s
        .cmd(&["run"])
        .env("CCSWITCH_CLAUDE", "definitely-not-claude-xyz")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("could not run 'definitely-not-claude-xyz': not found"));
}

#[test]
fn code_opens_vscode_with_its_own_data_dir() {
    let s = Sandbox::new("code");
    s.run(&["add", "work", "--no-login"]);
    let code = |args: &[&str]| {
        s.cmd(args)
            .env("CCSWITCH_CODE", s.dir.join("fake-claude"))
            .output()
            .unwrap()
    };
    let data_dir = format!("{}/vscode-data", s.profile("work"));

    assert_eq!(code(&["code"]).status.code(), Some(7));
    assert_eq!(
        s.called(),
        [
            s.profile("work"),
            "--user-data-dir".into(),
            data_dir.clone(),
            ".".into()
        ]
    );

    code(&["code", "work", "--", "/some/project", "--new-window"]);
    assert_eq!(
        s.called(),
        [
            s.profile("work"),
            "--user-data-dir".into(),
            data_dir,
            "/some/project".into(),
            "--new-window".into()
        ]
    );
}

#[test]
fn missing_code_binary_is_explained() {
    let s = Sandbox::new("nocode");
    s.run(&["add", "a", "--no-login"]);
    let out = s
        .cmd(&["code"])
        .env("CCSWITCH_CODE", "definitely-not-code-xyz")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("could not run 'definitely-not-code-xyz': not found"));
    assert!(stderr(&out).contains("CCSWITCH_CODE"));
}

#[test]
fn remove_needs_confirmation_and_clears_default() {
    let s = Sandbox::new("remove");
    s.run(&["add", "a", "--no-login"]);

    let out = s.run(&["remove", "a"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("pass --yes"));

    assert!(s.run(&["rm", "a", "--yes"]).status.success());
    assert!(stdout(&s.run(&["list"])).is_empty());
    assert!(stderr(&s.run(&["current"])).contains("no default profile"));
}

#[test]
fn rename_and_path_follow_default() {
    let s = Sandbox::new("rename");
    s.run(&["add", "old", "--no-login"]);
    assert!(s.run(&["mv", "old", "new"]).status.success());
    assert_eq!(stdout(&s.run(&["current"])), "new\n");
    assert_eq!(stdout(&s.run(&["path"])), format!("{}\n", s.profile("new")));
}

#[test]
fn stale_default_is_reported() {
    let s = Sandbox::new("stale");
    s.run(&["add", "a", "--no-login"]);
    fs::remove_dir_all(s.profile("a")).unwrap();
    assert!(stderr(&s.run(&["run"])).contains("default profile 'a' no longer exists"));
    // A new profile replaces a stale default.
    assert!(stdout(&s.run(&["add", "b", "--no-login"])).contains("(default)"));
}

#[test]
fn bypass_is_saved_per_profile() {
    let s = Sandbox::new("bypass");
    s.run(&["add", "a", "--no-login"]);
    s.run(&["add", "b", "--no-login"]);
    let settings = PathBuf::from(s.profile("a")).join("settings.json");
    fs::write(&settings, r#"{"model": "opus"}"#).unwrap();

    assert_eq!(
        stdout(&s.run(&["bypass", "a"])),
        "Bypass permissions for 'a': off\n"
    );
    assert!(s.run(&["bypass", "a", "on"]).status.success());
    assert_eq!(
        stdout(&s.run(&["bypass", "a"])),
        "Bypass permissions for 'a': on\n"
    );
    assert_eq!(stdout(&s.run(&["list"])), "* a  (bypass)\n  b\n");

    let json = fs::read_to_string(&settings).unwrap();
    assert!(json.contains(r#""model": "opus""#), "{json}");
    assert!(
        json.contains(r#""defaultMode": "bypassPermissions""#),
        "{json}"
    );

    assert!(s.run(&["bypass", "a", "off"]).status.success());
    assert_eq!(
        fs::read_to_string(&settings).unwrap(),
        "{\n  \"model\": \"opus\"\n}\n"
    );
}

#[test]
fn bypass_errors() {
    let s = Sandbox::new("bypass-errors");
    assert!(stderr(&s.run(&["bypass", "ghost", "on"])).contains("profile 'ghost' not found"));
    s.run(&["add", "a", "--no-login"]);
    fs::write(
        PathBuf::from(s.profile("a")).join("settings.json"),
        "{broken",
    )
    .unwrap();
    let out = s.run(&["bypass", "a", "on"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("cannot update"), "{}", stderr(&out));
    assert!(!s.run(&["bypass", "a", "maybe"]).status.success());
}

#[test]
fn run_bypass_adds_flag_for_one_session() {
    let s = Sandbox::new("run-bypass");
    s.run(&["add", "a", "--no-login"]);
    s.run(&["run", "--bypass", "--", "--resume"]);
    assert_eq!(
        s.called(),
        [
            s.profile("a"),
            "--dangerously-skip-permissions".into(),
            "--resume".into()
        ]
    );
    assert_eq!(
        stdout(&s.run(&["bypass", "a"])),
        "Bypass permissions for 'a': off\n"
    );
}

#[test]
fn share_links_items_and_undo_removes_them() {
    let sb = Sandbox::new("share");
    assert!(sb.run(&["add", "work", "--no-login"]).status.success());
    let src = sb.dir.join("shared");
    fs::create_dir_all(src.join("skills")).unwrap();
    fs::write(src.join("CLAUDE.md"), "hi").unwrap();
    fs::write(src.join("settings.json"), "{}").unwrap();
    let profile = PathBuf::from(sb.profile("work"));
    fs::write(profile.join("agents"), "own").unwrap();
    fs::create_dir_all(src.join("agents")).unwrap();

    let from = src.display().to_string();
    let out = sb.run(&["share", "work", "--from", &from]);
    assert!(out.status.success());
    assert_eq!(fs::read_to_string(profile.join("CLAUDE.md")).unwrap(), "hi");
    assert!(fs::symlink_metadata(profile.join("skills"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(fs::symlink_metadata(profile.join("agents"))
        .unwrap()
        .file_type()
        .is_file());
    assert!(!profile.join("settings.json").exists());
    assert!(sb.run(&["share", "work", "--from", &from]).status.success());

    assert!(sb
        .run(&["share", "work", "--from", &from, "--undo"])
        .status
        .success());
    assert!(!profile.join("CLAUDE.md").exists());
    assert!(profile.join("agents").is_file());

    let bad = sb.run(&["share", "work", "--from", &from, "--only", "projects"]);
    assert!(!bad.status.success());
}

fn is_link(path: &std::path::Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

fn shared_source(sb: &Sandbox) -> PathBuf {
    let src = sb.dir.join("shared");
    fs::create_dir_all(src.join("skills")).unwrap();
    fs::write(src.join("CLAUDE.md"), "hi").unwrap();
    src
}

#[test]
fn share_force_moves_own_copy_to_backup() {
    let sb = Sandbox::new("share-force");
    sb.run(&["add", "work", "--no-login"]);
    let src = shared_source(&sb);
    let profile = PathBuf::from(sb.profile("work"));
    fs::write(profile.join("CLAUDE.md"), "mine").unwrap();
    fs::write(profile.join("CLAUDE.md.bak"), "older").unwrap();
    let from = src.display().to_string();

    let out = sb.run(&["share", "work", "--from", &from, "--force"]);
    assert!(out.status.success());
    assert!(is_link(&profile.join("CLAUDE.md")));
    assert_eq!(
        fs::read_to_string(profile.join("CLAUDE.md.bak")).unwrap(),
        "older"
    );
    assert_eq!(
        fs::read_to_string(profile.join("CLAUDE.md.bak2")).unwrap(),
        "mine"
    );
}

#[test]
fn share_dry_run_changes_nothing() {
    let sb = Sandbox::new("share-dry");
    sb.run(&["add", "work", "--no-login"]);
    let src = shared_source(&sb);
    let profile = PathBuf::from(sb.profile("work"));
    let from = src.display().to_string();

    let out = sb.run(&["share", "work", "--from", &from, "--dry-run"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("would link"));
    assert!(!profile.join("skills").exists());
    assert!(!profile.join("CLAUDE.md").exists());
}

#[test]
fn share_all_and_status() {
    let sb = Sandbox::new("share-all");
    sb.run(&["add", "a", "--no-login"]);
    sb.run(&["add", "b", "--no-login"]);
    let src = shared_source(&sb);
    let from = src.display().to_string();
    fs::write(PathBuf::from(sb.profile("b")).join("skills"), "own").unwrap();

    assert!(sb
        .run(&["share", "--all", "--from", &from])
        .status
        .success());
    assert!(is_link(&PathBuf::from(sb.profile("a")).join("skills")));
    assert!(is_link(&PathBuf::from(sb.profile("b")).join("CLAUDE.md")));

    let text = stdout(&sb.run(&["share", "--all", "--from", &from, "--status"]));
    assert!(text.contains("a:") && text.contains("b:"));
    assert!(text.contains("skills: own copy (not shared)"));
    assert!(text.contains("CLAUDE.md: linked"));
    assert!(text.contains("hooks: skipped (not in source)"));

    assert!(!sb.run(&["share"]).status.success());
    assert!(!sb.run(&["share", "a", "--all"]).status.success());
}

#[test]
fn add_share_links_from_env_source() {
    let sb = Sandbox::new("add-share");
    let src = shared_source(&sb);
    let out = sb
        .cmd(&["add", "work", "--no-login", "--share"])
        .env("CCSWITCH_SHARE_FROM", &src)
        .output()
        .unwrap();
    assert!(out.status.success());
    let profile = PathBuf::from(sb.profile("work"));
    assert!(is_link(&profile.join("skills")));
    assert!(is_link(&profile.join("CLAUDE.md")));

    let missing = sb
        .cmd(&["add", "other", "--no-login", "--share"])
        .env("CCSWITCH_SHARE_FROM", sb.dir.join("projects"))
        .output()
        .unwrap();
    assert!(missing.status.success(), "profile is still created");
    assert!(String::from_utf8_lossy(&missing.stderr).contains("could not share"));
}

#[test]
fn share_settings_merges_shared_keys_only() {
    let sb = Sandbox::new("share-settings");
    sb.run(&["add", "work", "--no-login"]);
    sb.run(&["bypass", "work", "on"]);
    let src = sb.dir.join("shared");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("settings.json"),
        r#"{"hooks":{"x":1},"env":{"A":"1"},"permissions":{"defaultMode":"plan"},"model":"opus"}"#,
    )
    .unwrap();
    let from = src.display().to_string();

    let out = sb.run(&["share", "work", "--from", &from, "--only", "settings.json"]);
    assert!(out.status.success());
    let text = fs::read_to_string(PathBuf::from(sb.profile("work")).join("settings.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["hooks"]["x"], 1);
    assert_eq!(json["env"]["A"], "1");
    assert_eq!(json["permissions"]["defaultMode"], "bypassPermissions");
    assert!(json.get("model").is_none());

    let again = stdout(&sb.run(&["share", "work", "--from", &from, "--only", "settings.json"]));
    assert!(again.contains("in sync"));
}

#[test]
fn share_refuses_logins_and_paths() {
    let sb = Sandbox::new("share-private");
    sb.run(&["add", "work", "--no-login"]);
    let src = shared_source(&sb);
    let from = src.display().to_string();
    for bad in [".credentials.json", "projects", "../x", "a/b", ".."] {
        let out = sb.run(&["share", "work", "--from", &from, "--only", bad]);
        assert!(!out.status.success(), "{bad}");
    }
}

#[test]
fn remove_and_rename_leave_the_source_alone() {
    let sb = Sandbox::new("share-remove");
    sb.run(&["add", "work", "--no-login"]);
    let src = shared_source(&sb);
    fs::write(src.join("skills/s.md"), "x").unwrap();
    let from = src.display().to_string();
    sb.run(&["share", "work", "--from", &from]);

    assert!(sb.run(&["rename", "work", "job"]).status.success());
    assert!(PathBuf::from(sb.profile("job"))
        .join("skills/s.md")
        .exists());
    assert!(sb.run(&["remove", "job", "--yes"]).status.success());
    assert!(src.join("skills/s.md").exists());
    assert!(src.join("CLAUDE.md").exists());
}

#[test]
fn status_shows_each_account() {
    let sb = Sandbox::new("status");
    sb.run(&["add", "work", "--no-login"]);
    sb.run(&["add", "home", "--no-login"]);
    sb.run(&["bypass", "work", "on"]);
    fs::write(
        PathBuf::from(sb.profile("work")).join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"me@acme.com","organizationName":"Acme","organizationType":"claude_team"},
            "mcpServers":{"a":{},"b":{}}}"#,
    )
    .unwrap();
    fs::write(PathBuf::from(sb.profile("work")).join("history.jsonl"), "").unwrap();

    let out = sb.run(&["status"]);
    assert!(out.status.success());
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].contains("ACCOUNT"));
    assert!(
        lines[1].contains("home")
            && lines[1].contains("not logged in")
            && lines[1].contains("never")
    );
    assert!(lines[2].starts_with("*  work"), "{text}");
    for want in ["me@acme.com", "Acme (team)", "2", "on", "just now"] {
        assert!(lines[2].contains(want), "{want} in {text}");
    }

    let json: serde_json::Value =
        serde_json::from_slice(&sb.run(&["st", "--json"]).stdout).unwrap();
    assert_eq!(json[1]["email"], "me@acme.com");
    assert_eq!(json[1]["mcpServers"], 2);
    assert_eq!(json[0]["email"], serde_json::Value::Null);
}

#[test]
fn folder_file_and_env_pick_the_profile() {
    let sb = Sandbox::new("folder");
    sb.run(&["add", "personal", "--no-login"]);
    sb.run(&["add", "client", "--no-login"]);
    let project = sb.dir.join("project");
    let nested = project.join("src/deep");
    fs::create_dir_all(&nested).unwrap();

    let out = sb
        .cmd(&["use", "client", "--here"])
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        fs::read_to_string(project.join(".ccswitch")).unwrap(),
        "client\n"
    );

    let current = sb.cmd(&["current"]).current_dir(&nested).output().unwrap();
    assert_eq!(stdout(&current), "client\n");
    assert!(stderr(&current).contains(".ccswitch"));
    sb.cmd(&["run"]).current_dir(&nested).output().unwrap();
    assert_eq!(sb.called(), [sb.profile("client")]);
    assert_eq!(
        stdout(&sb.run(&["current"])),
        "personal\n",
        "outside the folder"
    );

    let env = |args: &[&str]| {
        sb.cmd(args)
            .current_dir(&nested)
            .env("CCSWITCH_PROFILE", "personal")
            .output()
            .unwrap()
    };
    env(&["run"]);
    assert_eq!(
        sb.called(),
        [sb.profile("personal")],
        "$CCSWITCH_PROFILE wins"
    );
    assert!(stderr(&env(&["current"])).contains("$CCSWITCH_PROFILE"));

    fs::write(project.join(".ccswitch"), "gone\n").unwrap();
    let out = sb.cmd(&["run"]).current_dir(&nested).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("profile 'gone' (from "),
        "{}",
        stderr(&out)
    );
    assert!(
        sb.cmd(&["run", "client"])
            .current_dir(&nested)
            .output()
            .unwrap()
            .status
            .code()
            == Some(7)
    );
    assert!(!sb.run(&["use", "ghost", "--here"]).status.success());
}

#[test]
fn env_prints_shell_commands() {
    let sb = Sandbox::new("env");
    sb.run(&["add", "work", "--no-login"]);
    let out = sb.run(&["env", "work", "--shell", "bash"]);
    assert_eq!(
        stdout(&out),
        format!(
            "export CLAUDE_CONFIG_DIR='{}'\nexport CCSWITCH_PROFILE='work'\n",
            sb.profile("work")
        )
    );
    let fish = sb
        .cmd(&["env"])
        .env("SHELL", "/usr/bin/fish")
        .output()
        .unwrap();
    assert!(stdout(&fish).starts_with("set -gx CLAUDE_CONFIG_DIR '"));
    assert_eq!(
        stdout(&sb.run(&["env", "--unset", "--shell", "sh"])),
        "unset CLAUDE_CONFIG_DIR CCSWITCH_PROFILE\n"
    );
    assert!(stdout(&sb.run(&["env", "--shell", "pwsh"])).starts_with("$env:CLAUDE_CONFIG_DIR = '"));
    assert!(!sb.run(&["env", "work", "--unset"]).status.success());
}

/// Claude's `projects/` folder name for the sandbox, which is the commands' working folder.
fn sandbox_slug(sb: &Sandbox) -> String {
    fs::canonicalize(&sb.dir)
        .unwrap()
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Writes a saved conversation with a subagent folder and rewind history into `profile`.
fn fake_session(sb: &Sandbox, profile: &str, id: &str, title: &str) -> PathBuf {
    let root = PathBuf::from(sb.profile(profile));
    let project = root.join("projects").join(sandbox_slug(sb));
    fs::create_dir_all(project.join(id).join("subagents")).unwrap();
    let cwd = fs::canonicalize(&sb.dir).unwrap();
    fs::write(
        project.join(format!("{id}.jsonl")),
        format!(
            "{{\"type\":\"user\",\"cwd\":{cwd:?},\"sessionId\":\"{id}\"}}\n{{\"type\":\"ai-title\",\"aiTitle\":\"{title}\"}}\n"
        ),
    )
    .unwrap();
    fs::write(project.join(id).join("subagents/a.jsonl"), "{}\n").unwrap();
    fs::create_dir_all(root.join("file-history").join(id)).unwrap();
    fs::write(root.join("file-history").join(id).join("v1"), "old").unwrap();
    project
}

#[test]
fn sessions_lists_this_folder() {
    let sb = Sandbox::new("sessions");
    sb.run(&["add", "work", "--no-login"]);
    assert!(stderr(&sb.run(&["sessions"])).contains("No sessions"));
    fake_session(&sb, "work", "aaaa-1111", "Fix the login bug");
    let text = stdout(&sb.run(&["sessions"]));
    assert!(text.starts_with("ID"), "{text}");
    assert!(
        text.contains("aaaa-1111") && text.contains("Fix the login bug"),
        "{text}"
    );
    assert!(stdout(&sb.run(&["sessions", "work", "--all"])).contains("FOLDER"));
}

#[test]
fn handoff_copies_the_latest_session_and_resumes_it() {
    let sb = Sandbox::new("handoff");
    sb.run(&["add", "work", "--no-login"]);
    sb.run(&["add", "personal", "--no-login"]);
    let slug = sandbox_slug(&sb);
    fake_session(&sb, "work", "aaaa-1111", "Old one");
    std::thread::sleep(std::time::Duration::from_millis(20));
    fake_session(&sb, "work", "bbbb-2222", "Latest");

    let out = sb.run(&["handoff", "personal", "--", "--model", "opus"]);
    assert_eq!(out.status.code(), Some(7), "{}", stderr(&out));
    assert!(stdout(&out).contains("Copied \"Latest\" (bbbb-2222) from 'work' to 'personal'"));
    assert_eq!(
        sb.called(),
        [
            sb.profile("personal"),
            "--resume".into(),
            "bbbb-2222".into(),
            "--model".into(),
            "opus".into()
        ]
    );
    let to = PathBuf::from(sb.profile("personal"));
    let project = to.join("projects").join(&slug);
    assert!(project.join("bbbb-2222.jsonl").is_file());
    assert!(project.join("bbbb-2222/subagents/a.jsonl").is_file());
    assert_eq!(
        fs::read_to_string(to.join("file-history/bbbb-2222/v1")).unwrap(),
        "old"
    );
    assert!(!project.join("aaaa-1111.jsonl").exists());
    let from = PathBuf::from(sb.profile("work"))
        .join("projects")
        .join(&slug);
    assert!(
        from.join("bbbb-2222.jsonl").is_file(),
        "copy keeps the original"
    );

    let again = sb.run(&["handoff", "personal", "--no-resume"]);
    assert!(
        stderr(&again).contains("already has session bbbb-2222"),
        "{}",
        stderr(&again)
    );
    let forced = sb.run(&["handoff", "personal", "--no-resume", "--force"]);
    assert!(forced.status.success(), "{}", stderr(&forced));
    assert!(stdout(&forced).contains("ccswitch run personal -- --resume bbbb-2222"));
}

#[test]
fn handoff_by_id_with_move() {
    let sb = Sandbox::new("handoff-move");
    sb.run(&["add", "work", "--no-login"]);
    sb.run(&["add", "personal", "--no-login"]);
    let from = fake_session(&sb, "work", "aaaa-1111", "One");
    fake_session(&sb, "work", "aaab-2222", "Two");

    let ambiguous = sb.run(&["handoff", "personal", "--session", "aaa", "--no-resume"]);
    assert!(
        stderr(&ambiguous).contains("matches 2 sessions"),
        "{}",
        stderr(&ambiguous)
    );
    assert!(
        stderr(&sb.run(&["handoff", "personal", "--session", "zz"])).contains("no session 'zz'")
    );
    assert!(stderr(&sb.run(&["handoff", "work"])).contains("already in 'work'"));

    let out = sb.run(&[
        "handoff",
        "personal",
        "--from",
        "work",
        "--session",
        "aaaa",
        "--move",
        "--no-resume",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).starts_with("Moved \"One\""));
    assert!(!from.join("aaaa-1111.jsonl").exists());
    assert!(!from.join("aaaa-1111").exists());
    assert!(!PathBuf::from(sb.profile("work"))
        .join("file-history/aaaa-1111")
        .exists());
    assert!(from.join("aaab-2222.jsonl").exists());
    assert!(stdout(&sb.run(&["sessions", "personal"])).contains("aaaa-1111"));
}

#[test]
fn handoff_without_sessions_is_explained() {
    let sb = Sandbox::new("handoff-none");
    sb.run(&["add", "work", "--no-login"]);
    sb.run(&["add", "personal", "--no-login"]);
    let out = sb.run(&["handoff", "personal"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("no sessions for this folder in 'work'"),
        "{}",
        stderr(&out)
    );
}
