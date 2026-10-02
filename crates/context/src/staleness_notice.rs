//! JS: lib/staleness-notice.mjs

use crate::jsp;
use crate::staleness::Finding;
use crate::util::{json_compact, json_pretty, safe_read, user_cache_dir, Env};
use serde_json::{Map, Value};

const RENOTIFY_INTERVAL_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;

fn cache_path(env: &Env) -> String {
    match env.get("IMPECCINO_STALENESS_CACHE").filter(|v| !v.is_empty()) {
        Some(p) => p.clone(),
        None => jsp::join(&[&user_cache_dir(env), "staleness-check.json"]),
    }
}

fn read_cache(env: &Env) -> Map<String, Value> {
    // returns the `projects` map (JS keeps whole object but only uses .projects)
    if let Some(text) = safe_read(&cache_path(env)) {
        if let Ok(Value::Object(raw)) = serde_json::from_str::<Value>(&text) {
            if let Some(p) = raw.get("projects") {
                if crate::staleness::js_truthy(p) {
                    // raw.projects truthy: return the whole raw; we only need projects entries
                    return raw;
                }
            }
        }
    }
    let mut m = Map::new();
    m.insert("projects".into(), Value::Object(Map::new()));
    m
}

fn as_number(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn prune_cache(cache: &Map<String, Value>, now: f64) -> Map<String, Value> {
    let mut projects = Map::new();
    if let Some(Value::Object(ps)) = cache.get("projects") {
        for (key, entries) in ps {
            let Some(obj) = entries.as_object() else { continue };
            let stamps: Vec<f64> = obj.values().filter_map(as_number).collect();
            if !stamps.is_empty() {
                let max = stamps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if now - max < RENOTIFY_INTERVAL_MS {
                    projects.insert(key.clone(), entries.clone());
                }
            }
        }
    }
    let mut out = Map::new();
    out.insert("projects".into(), Value::Object(projects));
    out
}

fn write_cache(env: &Env, cache: &Map<String, Value>) {
    let fp = cache_path(env);
    let _ = std::fs::create_dir_all(jsp::dirname(&fp));
    let _ = std::fs::write(&fp, json_compact(&Value::Object(cache.clone())));
}

/// The boot staleness check is off for this session. There is no config
/// file (docs/adr/0020), so the env var is the only switch.
pub fn staleness_check_disabled(env: &Env) -> bool {
    env.get("IMPECCINO_NO_STALENESS_CHECK").map(|v| !v.is_empty()).unwrap_or(false)
}

/// JS: filterFreshFindings(findings, { projectRoot, now })
pub fn filter_fresh_findings(env: &Env, findings: Vec<Finding>, project_root: &str, now: f64) -> Vec<Finding> {
    if findings.is_empty() {
        return vec![];
    }
    let auto: Vec<Finding> = findings.iter().filter(|f| f.severity == "auto").cloned().collect();
    let notifiable: Vec<Finding> = findings.iter().filter(|f| f.severity != "auto").cloned().collect();
    if notifiable.is_empty() {
        return auto;
    }
    let key = jsp::resolve(project_root, &[]);
    let cache = read_cache(env);
    let seen: Map<String, Value> = cache
        .get("projects")
        .and_then(|p| p.as_object())
        .and_then(|p| p.get(&key))
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let fresh: Vec<Finding> = notifiable
        .iter()
        .filter(|f| match seen.get(&f.id).and_then(as_number) {
            Some(last) => !(now - last < RENOTIFY_INTERVAL_MS),
            None => true,
        })
        .cloned()
        .collect();
    let live: Vec<&str> = notifiable.iter().map(|f| f.id.as_str()).collect();
    let mut next = Map::new();
    for (id, v) in &seen {
        if live.contains(&id.as_str()) {
            next.insert(id.clone(), v.clone());
        }
    }
    for f in &fresh {
        next.insert(f.id.clone(), Value::from(now as i64));
    }
    let changed = json_compact(&Value::Object(next.clone())) != json_compact(&Value::Object(seen.clone()));
    if changed {
        let mut pruned = prune_cache(&cache, now);
        if let Some(Value::Object(ps)) = pruned.get_mut("projects") {
            ps.insert(key.clone(), Value::Object(next));
        }
        write_cache(env, &pruned);
    }
    let mut out = auto;
    out.extend(fresh);
    out
}

/// JS: buildStalenessDirective(findings)
pub fn build_staleness_directive(findings: &[Finding]) -> Option<String> {
    if findings.is_empty() {
        return None;
    }
    let payload: Vec<Value> = findings.iter().map(|f| f.to_value()).collect();
    let has_reportable = findings.iter().any(|f| f.severity != "auto");
    let mut lines = vec![
        format!("CONTEXT_STALE:\n{}", json_pretty(&Value::Array(payload))),
        "Impeccino's own project files have drifted from what this version reads. Do not stop, reorder, or expand the requested task for any of this.".to_string(),
        "By severity: `auto` is a migration the next write to that file performs anyway, so apply it then and do not raise it with the user. `mention` gets one short line in your reply with the offered fix. `route` names the command that owns the repair; offer it, and run it only if the user asks.".to_string(),
        "A finding that reports a deprecated field is binding: treat that field as absent for every decision in this session, whatever value it holds.".to_string(),
    ];
    if has_reportable {
        lines.push("Surface the reportable findings once, after the task response, in at most two sentences. They are already throttled, so say them plainly rather than hedging about whether they matter.".to_string());
    }
    Some(lines.join(" "))
}
