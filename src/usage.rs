//! Token usage read from the conversation logs Claude Code keeps in each profile.

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::{self, BufRead, BufReader},
    path::Path,
    time::SystemTime,
};

use serde_json::Value;

use crate::{
    error::{Context, Error, Result},
    pricing,
};

/// Token counts and estimated cost, for one request or summed over many.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Totals {
    pub requests: u64,
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub cost: f64,
    /// Requests from models missing from the price table, so not in `cost`.
    pub unpriced: u64,
}

impl Totals {
    pub fn tokens(&self) -> u64 {
        self.input + self.output + self.cache_write + self.cache_read
    }

    pub fn add(&mut self, other: &Totals) {
        self.requests += other.requests;
        self.input += other.input;
        self.output += other.output;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
        self.cost += other.cost;
        self.unpriced += other.unpriced;
    }
}

/// One API request, as logged.
pub struct Request {
    pub day: String,
    pub model: String,
    pub session: String,
    pub totals: Totals,
}

/// Every request logged in `profile` on or after `since_day` (`YYYY-MM-DD`, UTC). Files last
/// changed before `since` are skipped without being read. Requests already in `seen` are left
/// out: a streamed reply can be logged more than once, and `handoff` copies conversations
/// between profiles.
pub fn requests(
    profile: &Path,
    since_day: &str,
    since: SystemTime,
    seen: &mut HashSet<String>,
) -> Result<Vec<Request>> {
    let mut files = Vec::new();
    find_logs(&profile.join("projects"), since, &mut files)?;
    let mut out = Vec::new();
    for file in files {
        let reader = BufReader::new(
            fs::File::open(&file).with_context(|| format!("reading {}", file.display()))?,
        );
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if !line.contains("\"usage\"") {
                continue;
            }
            let Some((key, request)) = parse(&line) else {
                continue;
            };
            if request.day.as_str() >= since_day && seen.insert(key) {
                out.push(request);
            }
        }
    }
    Ok(out)
}

fn find_logs(dir: &Path, since: SystemTime, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(Error::io(format!("reading {}", dir.display()), e)),
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let path = entry.path();
        if meta.is_dir() {
            find_logs(&path, since, out)?;
        } else if path.extension().is_some_and(|e| e == "jsonl")
            && meta.modified().map_or(true, |m| m >= since)
        {
            out.push(path);
        }
    }
    Ok(())
}

/// A logged assistant reply with token usage, and the key that identifies the API request.
fn parse(line: &str) -> Option<(String, Request)> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "assistant" {
        return None;
    }
    let message = v.get("message")?;
    let usage = message.get("usage")?;
    let model = message.get("model")?.as_str()?.to_owned();
    // Claude Code's own placeholder replies (such as errors); no API request was made.
    if model == "<synthetic>" {
        return None;
    }
    let count = |obj: &Value, key: &str| obj.get(key).and_then(Value::as_u64).unwrap_or(0);
    let input = count(usage, "input_tokens");
    let output = count(usage, "output_tokens");
    let read = count(usage, "cache_read_input_tokens");
    let write = count(usage, "cache_creation_input_tokens");
    // Older logs don't split writes by cache lifetime; count those as the 5-minute cache.
    let write_1h = usage
        .get("cache_creation")
        .map_or(0, |c| count(c, "ephemeral_1h_input_tokens"))
        .min(write);
    let fast = usage.get("speed").and_then(Value::as_str) == Some("fast");
    let (cost, unpriced) = match pricing::lookup(&model) {
        Some(p) => (
            p.cost(input, output, write - write_1h, write_1h, read, fast),
            0,
        ),
        None => (0.0, 1),
    };
    let id = message
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let request_id = v
        .get("requestId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let key = if id.is_empty() && request_id.is_empty() {
        // Nothing to dedupe on; keep every line.
        format!(
            "line:{}",
            v.get("uuid").and_then(Value::as_str).unwrap_or(line)
        )
    } else {
        format!("{id}:{request_id}")
    };
    let request = Request {
        day: v.get("timestamp")?.as_str()?.get(..10)?.to_owned(),
        model,
        session: v
            .get("sessionId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        totals: Totals {
            requests: 1,
            input,
            output,
            cache_write: write,
            cache_read: read,
            cost,
            unpriced,
        },
    };
    Some((key, request))
}

/// Sums totals by key, in key order.
pub fn group<'a>(rows: impl IntoIterator<Item = (String, &'a Totals)>) -> BTreeMap<String, Totals> {
    let mut groups: BTreeMap<String, Totals> = BTreeMap::new();
    for (key, totals) in rows {
        groups.entry(key).or_default().add(totals);
    }
    groups
}

/// 1234567 → "1,234,567".
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"type":"assistant","timestamp":"2026-10-18T21:29:40.043Z","sessionId":"s1","requestId":"req_1","message":{"id":"msg_1","model":"claude-opus-5-5","usage":{"input_tokens":1000000,"output_tokens":0,"cache_creation_input_tokens":1000000,"cache_read_input_tokens":0,"cache_creation":{"ephemeral_1h_input_tokens":1000000,"ephemeral_5m_input_tokens":0}}}}"#;

    #[test]
    fn parses_usage_and_prices_it() {
        let (key, r) = parse(LINE).unwrap();
        assert_eq!(key, "msg_1:req_1");
        assert_eq!(r.day, "2026-10-18");
        assert_eq!(r.session, "s1");
        assert_eq!(r.totals.input, 1_000_000);
        // $4 input + 1h cache write at 2x input.
        assert!((r.totals.cost - 12.0).abs() < 1e-9, "{}", r.totals.cost);
        assert!(parse(r#"{"type":"user","message":{"usage":{}}}"#).is_none());
    }

    #[test]
    fn unknown_models_are_counted_not_priced() {
        let line = LINE.replace("claude-opus-5-5", "gpt-5");
        let (_, r) = parse(&line).unwrap();
        assert_eq!((r.totals.cost, r.totals.unpriced), (0.0, 1));
        assert!(parse(&LINE.replace("claude-opus-5-5", "<synthetic>")).is_none());
    }

    #[test]
    fn formats_thousands() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
