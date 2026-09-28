//! Edits to a profile's Claude Code `settings.json`, keeping every other key as it was.

use serde_json::{Map, Value};

pub type Settings = Map<String, Value>;

const BYPASS: &str = "bypassPermissions";

pub fn parse(text: &str) -> Result<Settings, String> {
    if text.trim().is_empty() {
        return Ok(Settings::new());
    }
    match serde_json::from_str(text).map_err(|e| e.to_string())? {
        Value::Object(map) => Ok(map),
        _ => Err("expected a JSON object".to_owned()),
    }
}

pub fn to_string(settings: &Settings) -> String {
    let mut text = serde_json::to_string_pretty(settings).expect("a JSON map always serializes");
    text.push('\n');
    text
}

pub fn is_bypass(settings: &Settings) -> bool {
    settings
        .get("permissions")
        .and_then(|p| p.get("defaultMode"))
        .and_then(Value::as_str)
        == Some(BYPASS)
}

/// Turning bypass off only removes it; any other default mode the user chose is left alone.
pub fn set_bypass(settings: &mut Settings, on: bool) -> Result<(), String> {
    if on {
        let permissions = settings
            .entry("permissions")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .ok_or("\"permissions\" is not a JSON object")?;
        permissions.insert("defaultMode".to_owned(), BYPASS.into());
    } else if is_bypass(settings) {
        if let Some(permissions) = settings
            .get_mut("permissions")
            .and_then(Value::as_object_mut)
        {
            permissions.shift_remove("defaultMode");
            if permissions.is_empty() {
                settings.shift_remove("permissions");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_bypass_and_keeps_other_keys() {
        let mut s = parse(r#"{"model":"opus","permissions":{"allow":["Bash(ls)"]}}"#).unwrap();
        assert!(!is_bypass(&s));
        set_bypass(&mut s, true).unwrap();
        assert!(is_bypass(&s));
        assert_eq!(s["model"], "opus");
        set_bypass(&mut s, false).unwrap();
        assert_eq!(
            Value::Object(s),
            serde_json::json!({"model":"opus","permissions":{"allow":["Bash(ls)"]}})
        );
    }

    #[test]
    fn off_removes_empty_permissions_and_keeps_other_modes() {
        let mut s = Settings::new();
        set_bypass(&mut s, true).unwrap();
        set_bypass(&mut s, false).unwrap();
        assert!(s.is_empty());

        let mut s = parse(r#"{"permissions":{"defaultMode":"acceptEdits"}}"#).unwrap();
        set_bypass(&mut s, false).unwrap();
        assert_eq!(s["permissions"]["defaultMode"], "acceptEdits");
    }

    #[test]
    fn rejects_unusable_files() {
        assert!(parse("").unwrap().is_empty());
        assert!(parse("[1]").is_err());
        assert!(parse("{not json").is_err());
        let mut s = parse(r#"{"permissions":true}"#).unwrap();
        assert!(set_bypass(&mut s, true).is_err());
    }
}
