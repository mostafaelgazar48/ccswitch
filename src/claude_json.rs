//! Reads and edits a profile's `.claude.json`, where Claude Code keeps the login's account
//! details and the user-scope MCP servers.

use std::{fs, io, path::Path};

use serde_json::{Map, Value};

use crate::{
    error::{Error, Result},
    settings,
    store::write_atomic,
};

pub const FILE: &str = ".claude.json";

/// Who a profile is logged in as.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Account {
    pub email: String,
    pub org: String,
    /// `organizationType` without its `claude_` prefix, such as `pro`, `max` or `team`.
    pub plan: String,
}

impl Account {
    /// "Acme (team)"; a personal organization is shown as "personal".
    pub fn org_and_plan(&self) -> String {
        let org = if self.org.ends_with("'s Organization") || self.org.is_empty() {
            "personal"
        } else {
            &self.org
        };
        if self.plan.is_empty() {
            org.to_owned()
        } else {
            format!("{org} ({})", self.plan)
        }
    }
}

/// The whole file as a JSON object; empty when the profile has none yet.
pub fn read(profile: &Path) -> Result<Map<String, Value>> {
    let file = profile.join(FILE);
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(Error::io(format!("reading {}", file.display()), e)),
    };
    settings::parse(&text).map_err(|message| Error::Settings {
        path: file.display().to_string(),
        message,
    })
}

/// Replaces the file, keeping it readable by the owner only, as Claude does.
pub fn write(profile: &Path, json: &Map<String, Value>) -> Result<()> {
    let file = profile.join(FILE);
    write_atomic(&file, &settings::to_string(json))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600))
            .map_err(|e| Error::io(format!("protecting {}", file.display()), e))?;
    }
    Ok(())
}

pub fn account(json: &Map<String, Value>) -> Option<Account> {
    let oauth = json.get("oauthAccount")?.as_object()?;
    let field = |key: &str| {
        oauth
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let email = field("emailAddress");
    if email.is_empty() {
        return None;
    }
    let plan = field("organizationType");
    Some(Account {
        email,
        org: field("organizationName"),
        plan: plan.strip_prefix("claude_").unwrap_or(&plan).to_owned(),
    })
}

/// User-scope MCP servers, by name.
pub fn mcp_servers(json: &Map<String, Value>) -> Map<String, Value> {
    json.get("mcpServers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub fn set_mcp_servers(json: &mut Map<String, Value>, servers: Map<String, Value>) {
    json.insert("mcpServers".to_owned(), Value::Object(servers));
}

/// One line about a server: its transport and what it runs or connects to.
pub fn describe_server(config: &Value) -> String {
    let field = |key: &str| config.get(key).and_then(Value::as_str);
    let kind = field("type").unwrap_or(if field("url").is_some() {
        "http"
    } else {
        "stdio"
    });
    let target = match field("url") {
        Some(url) => url.to_owned(),
        None => std::iter::once(field("command").unwrap_or("?").to_owned())
            .chain(
                config
                    .get("args")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(String::from),
            )
            .collect::<Vec<_>>()
            .join(" "),
    };
    format!("{kind}  {target}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Map<String, Value> {
        settings::parse(text).unwrap()
    }

    #[test]
    fn reads_account_and_plan() {
        let json = parse(
            r#"{"oauthAccount":{"emailAddress":"a@b.c","organizationName":"Acme","organizationType":"claude_team"}}"#,
        );
        let account = account(&json).unwrap();
        assert_eq!(account.email, "a@b.c");
        assert_eq!(account.org_and_plan(), "Acme (team)");

        let personal = Account {
            org: "a@b.c's Organization".into(),
            plan: "pro".into(),
            ..Account::default()
        };
        assert_eq!(personal.org_and_plan(), "personal (pro)");
    }

    #[test]
    fn describes_servers() {
        let stdio = serde_json::json!({"type":"stdio","command":"npx","args":["-y","pkg"],"env":{"K":"secret"}});
        assert_eq!(describe_server(&stdio), "stdio  npx -y pkg");
        let http = serde_json::json!({"type":"http","url":"https://x.dev/mcp"});
        assert_eq!(describe_server(&http), "http  https://x.dev/mcp");
        assert_eq!(
            describe_server(&serde_json::json!({"command":"run"})),
            "stdio  run"
        );
    }

    #[test]
    fn missing_login_and_servers() {
        assert_eq!(account(&parse("{}")), None);
        assert_eq!(account(&parse(r#"{"oauthAccount":{}}"#)), None);
        assert!(mcp_servers(&parse("{}")).is_empty());
        assert_eq!(mcp_servers(&parse(r#"{"mcpServers":{"x":{}}}"#)).len(), 1);
    }
}
