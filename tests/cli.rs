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
