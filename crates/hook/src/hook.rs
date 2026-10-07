//! Post-edit and Stop hooks, session tracking and bounded rescans.

use impeccino_core::findings::Finding;
use impeccino_core::js;
use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::hook_lib::*;
use crate::stop_baseline;
use crate::util::{
    exists, iso_now, jsp, node_read_error, now_ms, str_field, truthy_value, utf16_len,
};

/// JS `runHook` / `runStopHook` result: `{ exitCode: 0, stdout, audit }`.
pub struct RunResult {
    pub stdout: String,
    pub audit: Map<String, Value>,
}

fn is_rendered_finding_line(line: &str) -> bool {
    line.starts_with("- L") || line.starts_with("- [")
}

fn rendered_finding_count(text: &str) -> usize {
    text.lines()
        .filter(|line| is_rendered_finding_line(line))
        .count()
}

fn limit_rendered_findings(text: &str, max_findings: usize) -> (String, usize) {
    if max_findings == 0 && rendered_finding_count(text) > 0 {
        return (String::new(), 0);
    }
    let mut kept = Vec::new();
    let mut shown = 0;
    for line in text.lines() {
        if is_rendered_finding_line(line) {
            if shown >= max_findings {
                continue;
            }
            shown += 1;
        }
        kept.push(line);
    }
    (kept.join("\n"), shown)
}

fn join_messages_with_limit(messages: &[String], max_chars: usize) -> String {
    join_messages_with_limit_and_count(messages, max_chars).0
}

fn join_messages_with_limit_and_count(messages: &[String], max_chars: usize) -> (String, usize) {
    let mut output = String::new();
    let mut joined = 0usize;
    for message in messages {
        let separator = if output.is_empty() { "" } else { "\n\n" };
        let next_len = utf16_len(&output) + utf16_len(separator) + utf16_len(message);
        if next_len > max_chars {
            break;
        }
        output.push_str(separator);
        output.push_str(message);
        joined += 1;
    }
    (output, joined)
}

const INVOCATION_OMISSION_NOTE: &str =
    "[impeccino@1] More touched-project findings were omitted; audit all projects.";

fn invocation_omission_reserve() -> usize {
    utf16_len(INVOCATION_OMISSION_NOTE) + 2
}

fn append_invocation_omission_note(text: &str) -> String {
    let separator = if text.is_empty() { "" } else { "\n\n" };
    format!("{text}{separator}{INVOCATION_OMISSION_NOTE}")
}

fn ms_since(started: f64) -> Value {
    Value::from((now_ms() - started).max(0.0) as u64)
}

fn with(audit: &Map<String, Value>, extra: Vec<(&str, Value)>) -> Map<String, Value> {
    let mut out = audit.clone();
    for (k, v) in extra {
        out.insert(k.to_string(), v);
    }
    out
}

fn result(audit: &Map<String, Value>, extra: Vec<(&str, Value)>) -> RunResult {
    RunResult {
        stdout: String::new(),
        audit: with(audit, extra),
    }
}

fn payload_message(stdout: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(stdout).ok()?;
    let object = value.as_object()?;
    object
        .get("hookSpecificOutput")
        .and_then(Value::as_object)
        .and_then(|inner| inner.get("additionalContext"))
        .or_else(|| object.get("additionalContext"))
        .or_else(|| object.get("additional_context"))
        .or_else(|| object.get("reason"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Parse stdin the way `runHook` does: `Err` for malformed JSON, `Ok(None)`
/// for a parsed non-object.
fn parse_event(stdin: &str) -> Result<Option<Map<String, Value>>, ()> {
    match serde_json::from_str::<Value>(stdin) {
        Ok(Value::Object(o)) => Ok(Some(o)),
        Ok(_) => Ok(None),
        Err(_) => Err(()),
    }
}

/// `event.session_id || 'unknown'` as the cache key; `None` when absent.
fn session_id_of(event: &Map<String, Value>) -> Option<Value> {
    event
        .get("session_id")
        .filter(|v| truthy_value(Some(v)))
        .cloned()
}

fn session_key(v: &Option<Value>) -> String {
    match v {
        Some(v) => crate::util::js_string(v),
        None => "unknown".to_string(),
    }
}

/// JS: runHook({ stdinJson, env, cwd })
pub fn run_hook(rt: &Runtime, stdin: &str) -> RunResult {
    run_hook_inner(rt, stdin, None, true, None, None, None)
}

fn run_hook_inner(
    rt: &Runtime,
    stdin: &str,
    primary_override: Option<&[String]>,
    design_note_allowed: bool,
    target_override: Option<&[String]>,
    finding_limit: Option<usize>,
    output_char_limit: Option<usize>,
) -> RunResult {
    let mut audit: Map<String, Value> = Map::new();
    audit.insert("ts".into(), Value::String(iso_now()));
    audit.insert("event".into(), Value::String("PostToolUse".into()));

    if truthy(rt.env("IMPECCINO_HOOK_DISABLED")) {
        return result(
            &audit,
            vec![
                ("skipped", Value::from("env-disabled")),
                ("durationMs", Value::from(0)),
            ],
        );
    }
    let started = now_ms();

    let event = match parse_event(stdin) {
        Err(()) => {
            return result(
                &audit,
                vec![
                    ("skipped", Value::from("stdin-malformed")),
                    ("durationMs", ms_since(started)),
                ],
            )
        }
        Ok(None) => {
            return result(
                &audit,
                vec![
                    ("skipped", Value::from("stdin-empty")),
                    ("durationMs", ms_since(started)),
                ],
            )
        }
        Ok(Some(e)) => e,
    };

    let harness = resolve_harness(rt, Some(&event));
    let event = normalize_hook_event(rt, &event, &rt.proc_cwd, harness);
    audit.insert("harness".into(), Value::from(harness));

    let session_cwd = str_field(&event, "cwd").unwrap_or(&rt.proc_cwd).to_string();
    let primary_files = primary_override.map_or_else(
        || {
            normalize_scan_targets(
                rt,
                &resolve_target_files(rt, &event, &session_cwd),
                &session_cwd,
            )
        },
        |files| files.to_vec(),
    );
    if primary_override.is_none() && primary_files.len() > 1 {
        let mut project_groups: Vec<(String, Vec<String>)> = Vec::new();
        for file in &primary_files {
            let root = resolve_cache_cwd(rt, Some(file), &session_cwd);
            if let Some((_, files)) = project_groups
                .iter_mut()
                .find(|(existing, _)| *existing == root)
            {
                files.push(file.clone());
            } else {
                project_groups.push((root, vec![file.clone()]));
            }
        }
        if project_groups.len() > 1 {
            let mut results = Vec::new();
            let mut messages = Vec::new();
            let mut message_findings = Vec::new();
            let mut omitted_findings = 0usize;
            let aggregate_char_limit =
                (DEFAULT_MAX_CHARS as usize).saturating_sub(invocation_omission_reserve());
            let total_primary_targets: usize =
                project_groups.iter().map(|(_, files)| files.len()).sum();
            let mut extra_targets = MAX_SCAN_TARGETS.saturating_sub(total_primary_targets);
            let mut remaining_findings = cap_of(&HookConfig::default());
            for (index, (root, files)) in project_groups.into_iter().enumerate() {
                let target_limit = files.len() + extra_targets;
                let targets = expand_scan_targets_with_limit(rt, &files, &root, target_limit);
                extra_targets =
                    extra_targets.saturating_sub(targets.len().saturating_sub(files.len()));
                let child = run_hook_inner(
                    rt,
                    stdin,
                    Some(&files),
                    index == 0,
                    Some(&targets),
                    Some(remaining_findings),
                    Some(aggregate_char_limit),
                );
                if let Some(message) = payload_message(&child.stdout) {
                    if !message.is_empty() {
                        let (message, shown) =
                            limit_rendered_findings(&message, remaining_findings);
                        let child_findings = child
                            .audit
                            .get("freshFindings")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as usize;
                        omitted_findings += child_findings.saturating_sub(shown);
                        if !message.is_empty() {
                            remaining_findings = remaining_findings.saturating_sub(shown);
                            messages.push(message);
                            message_findings.push(shown);
                        }
                    }
                }
                results.push(child);
            }
            let audit_source = results
                .iter()
                .find(|result| !result.stdout.is_empty())
                .or_else(|| results.first());
            let mut merged = audit_source
                .map(|result| result.audit.clone())
                .unwrap_or_else(Map::new);
            for key in ["findings", "freshFindings", "freshFiles", "deferred"] {
                let total: u64 = results
                    .iter()
                    .filter_map(|r| r.audit.get(key).and_then(Value::as_u64))
                    .sum();
                if total > 0 || results.iter().any(|r| r.audit.contains_key(key)) {
                    merged.insert(key.to_string(), Value::from(total));
                }
            }
            let duration: u64 = results
                .iter()
                .filter_map(|r| r.audit.get("durationMs").and_then(Value::as_u64))
                .sum();
            merged.insert("durationMs".into(), Value::from(duration));
            let emitted = results
                .iter()
                .any(|r| r.audit.get("emitted") == Some(&Value::Bool(true)));
            if emitted {
                merged.insert("emitted".into(), Value::Bool(true));
                merged.remove("skipped");
                merged.remove("platform");
            }
            let (mut text, joined) =
                join_messages_with_limit_and_count(&messages, aggregate_char_limit);
            omitted_findings += message_findings.iter().skip(joined).sum::<usize>();
            if omitted_findings > 0 {
                text = append_invocation_omission_note(&text);
            }
            let stdout = if text.is_empty() {
                String::new()
            } else {
                payload(&text, "PostToolUse", harness)
            };
            return RunResult {
                stdout,
                audit: merged,
            };
        }
    }
    let project_cwd =
        resolve_cache_cwd(rt, primary_files.first().map(String::as_str), &session_cwd);
    let render_cwd = if primary_override.is_some() {
        session_cwd.clone()
    } else {
        project_cwd.clone()
    };
    audit.insert("cwd".into(), Value::String(project_cwd.clone()));
    let target_files = target_override.map_or_else(
        || expand_scan_targets(rt, &primary_files, &project_cwd),
        |targets| targets.to_vec(),
    );
    let session_value = session_id_of(&event);
    audit.insert(
        "session".into(),
        session_value.clone().unwrap_or(Value::Null),
    );
    if let Some(tool) = event.get("tool_name").filter(|v| truthy_value(Some(v))) {
        audit.insert("tool".into(), tool.clone());
    }

    if target_files.is_empty() {
        return result(
            &audit,
            vec![
                ("skipped", Value::from("no-file-path")),
                ("durationMs", ms_since(started)),
            ],
        );
    }

    let mut config = read_config(&project_cwd);
    if let Some(limit) = finding_limit.filter(|limit| *limit > 0) {
        config.limits.max_findings = limit as f64;
    }
    if let Some(limit) = output_char_limit {
        let configured = if config.limits.max_chars == 0.0 || config.limits.max_chars.is_nan() {
            DEFAULT_MAX_CHARS
        } else {
            config.limits.max_chars
        };
        config.limits.max_chars = configured.min(limit as f64);
    }

    let mut cache = read_cache(&project_cwd);
    let session_id = session_key(&session_value);
    let mut scans = HashMap::new();
    let tiered = per_edit_tiering_active(&config, harness);

    struct Pending {
        file_path: String,
        known: Vec<String>,
    }
    let mut pending_winner: Option<Pending> = None;
    let mut clean_winner: Option<String> = None;
    let mut fresh_groups: Vec<Group> = Vec::new();
    let mut suppression_winner: Option<String> = None;
    let mut clean_ack_deduped = false;
    let mut skipped_bytes: u64 = 0;
    let quiet_mode = truthy(rt.env("IMPECCINO_HOOK_QUIET"));
    let mut detector_threw_any = false;
    let mut last_skip = "no-scannable-file";
    let mut suppressed_hit = false;
    let mut cache_dirty = false;
    let mut deferred_total: usize = 0;
    let mut native_platform_seen: Option<String> = None;
    let mut non_native_target_seen = false;

    for file_path in &target_files {
        audit.insert("file".into(), Value::String(file_path.clone()));

        if has_path_traversal(file_path) || is_sensitive_path(file_path) {
            last_skip = "sensitive";
            continue;
        }
        if is_generated_path(file_path) {
            last_skip = "generated";
            continue;
        }
        let ext = js::to_lower_case(&jsp::extname(file_path));
        let configured = match_configured_extension(file_path, &config.extensions);
        audit.insert(
            "ext".into(),
            Value::String(
                configured
                    .map(|c| c.ext.clone())
                    .unwrap_or_else(|| ext.clone()),
            ),
        );
        if !ALLOWED_EXTS.contains(&ext.as_str()) && configured.is_none() {
            last_skip = "extension";
            continue;
        }
        if !exists(file_path) {
            last_skip = "file-missing";
            continue;
        }
        if is_project_skipped(rt, file_path) {
            last_skip = "git-ignored";
            continue;
        }
        if !is_scan_target_inside_project(rt, file_path, &project_cwd) {
            last_skip = "outside-project";
            continue;
        }
        let platform = resolve_project_platform_for_target(rt, &project_cwd, file_path);
        if is_native_platform(platform.as_deref()) {
            native_platform_seen.get_or_insert_with(|| platform.unwrap_or_default());
            last_skip = "native-platform";
            continue;
        }
        non_native_target_seen = true;
        let max_file_bytes = config.limits.max_file_bytes;
        if max_file_bytes > 0.0 {
            let size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
            if size as f64 > max_file_bytes {
                skipped_bytes = size;
                last_skip = "too-large";
                continue;
            }
        }

        let use_html_engine = match configured {
            Some(c) => c.engine == "html",
            None => ext == ".html" || ext == ".htm",
        };
        if primary_files.contains(file_path) {
            if harness == "claude" {
                stop_baseline::capture(
                    rt,
                    &event,
                    &mut cache,
                    &session_id,
                    file_path,
                    use_html_engine,
                );
            }
            let edit_count = bump_edit_count(&mut cache, &session_id, file_path);
            cache_dirty = true;
            audit.insert("editCount".into(), Value::from(edit_count as u64));
            if edit_count > EDIT_COUNT_THRESHOLD as f64 {
                stop_baseline::invalidate(&mut cache, &session_id, file_path);
                let just_crossed = edit_count == (EDIT_COUNT_THRESHOLD + 1) as f64;
                if just_crossed && suppression_winner.is_none() {
                    suppression_winner = Some(file_path.clone());
                }
                last_skip = "suppressed";
                suppressed_hit = true;
                continue;
            }
        }

        let content = match std::fs::read(file_path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                // JS: fs.readFileSync throws out of runHook's try; the catch
                // records the error and emits nothing.
                return RunResult {
                    stdout: String::new(),
                    audit: with(
                        &audit,
                        vec![("error", Value::String(node_read_error(file_path, &e)))],
                    ),
                };
            }
        };
        let scan = scans.entry(file_path.clone()).or_insert_with(|| {
            design_system_options_for_file(rt, &config, &project_cwd, file_path)
        });
        let mut detector_threw = false;
        let findings: Vec<Finding> = if use_html_engine {
            match detector_detect_html(rt, file_path, scan) {
                Ok(f) => f,
                Err(_) => {
                    detector_threw = true;
                    vec![]
                }
            }
        } else {
            detector_detect_text(&content, file_path, scan)
        };
        if !detector_threw && !use_html_engine {
            stop_baseline::reconcile(&mut cache, &session_id, file_path, &findings);
        }
        let raw_count = findings.len();
        let filtered = filter_findings_for(findings, &config, scan);
        let (immediate, deferred) = if tiered {
            split_findings_by_tier(filtered)
        } else {
            (filtered, vec![])
        };
        if !deferred.is_empty() {
            touch_file(&mut cache, &session_id, file_path);
            cache_dirty = true;
            deferred_total += deferred.len();
        }
        let fresh = dedupe_against_cache(&immediate, &mut cache, &session_id, file_path);
        audit.insert("findings".into(), Value::from(raw_count));
        audit.insert("freshFindings".into(), Value::from(fresh.len()));
        if deferred_total > 0 {
            audit.insert("deferred".into(), Value::from(deferred_total));
        }
        if detector_threw {
            detector_threw_any = true;
            continue;
        }
        // JS: Grok ignores PostToolUse stdout, so Stop is the user-visible
        // pass. Remembering here would dedupe those findings out of Stop.
        // Touch the file so Stop has it, and leave the finding list empty
        // (pbakaus/impeccable#646).
        if harness == "grok" {
            touch_file(&mut cache, &session_id, file_path);
        } else {
            remember_findings(&mut cache, &session_id, file_path, &immediate);
        }
        cache_dirty = true;

        if !fresh.is_empty() {
            fresh_groups.push(Group {
                file_path: file_path.clone(),
                findings: fresh,
            });
            continue;
        }
        if !immediate.is_empty() && pending_winner.is_none() {
            pending_winner = Some(Pending {
                file_path: file_path.clone(),
                known: immediate.iter().map(finding_cache_key).collect(),
            });
        } else if immediate.is_empty() && clean_winner.is_none() {
            if quiet_mode || !should_emit_ack_for_file(file_path, &config) {
                clean_winner = Some(file_path.clone());
            } else if truthy_value(
                ensure_file(&mut cache, &session_id, file_path).get("cleanAcked"),
            ) {
                clean_ack_deduped = true;
            } else {
                ensure_file(&mut cache, &session_id, file_path)
                    .insert("cleanAcked".into(), Value::Bool(true));
                clean_winner = Some(file_path.clone());
                clean_ack_deduped = false;
            }
        }
    }
    if !fresh_groups.is_empty() {
        let scan = &scans[&fresh_groups[0].file_path];
        let short = footer_mode_short(&mut cache, &session_id);
        let reserve = if design_note_allowed {
            design_note_reserve(rt, scan, &mut cache, &session_id)
        } else {
            0.0
        };
        let rendered = render_grouped_template(
            rt,
            &fresh_groups,
            &config,
            &RenderOpts {
                cwd: Some(render_cwd.clone()),
                short_footer: short,
                reserve_chars: reserve,
            },
        );
        let text = if design_note_allowed {
            append_design_system_note_once(rt, &rendered, scan, &mut cache, &session_id, &config)
        } else {
            rendered
        };
        commit_footer_shown(rt, &mut cache, &session_id, &text);
        persist_project_cache(rt, &project_cwd, &session_cwd, &mut cache, &session_id);
        let all: usize = fresh_groups.iter().map(|g| g.findings.len()).sum();
        return RunResult {
            stdout: payload(&text, "PostToolUse", harness),
            audit: with(
                &audit,
                vec![
                    ("file", Value::String(fresh_groups[0].file_path.clone())),
                    ("emitted", Value::Bool(true)),
                    ("freshFiles", Value::from(fresh_groups.len())),
                    ("freshFindings", Value::from(all)),
                    ("chars", Value::from(utf16_len(&text))),
                    ("durationMs", ms_since(started)),
                ],
            ),
        };
    }

    enum Ack {
        Pending(String),
        Clean(String),
    }
    let mut ack: Option<Ack> = None;
    if !quiet_mode {
        if let Some(p) = pending_winner
            .as_ref()
            .filter(|p| should_emit_ack_for_file(&p.file_path, &config))
        {
            let base = render_pending_ack(rt, &p.file_path, &p.known, &render_cwd);
            let scan = &scans[&p.file_path];
            let text = if design_note_allowed {
                append_design_system_note_once(rt, &base, scan, &mut cache, &session_id, &config)
            } else {
                base
            };
            ack = Some(Ack::Pending(text));
        } else if suppression_winner.is_none() && !clean_ack_deduped {
            if let Some(c) = clean_winner
                .as_ref()
                .filter(|c| should_emit_ack_for_file(c, &config))
            {
                let base = render_clean_ack(rt, c, &render_cwd);
                let scan = &scans[c];
                let text = if design_note_allowed {
                    append_design_system_note_once(
                        rt,
                        &base,
                        scan,
                        &mut cache,
                        &session_id,
                        &config,
                    )
                } else {
                    base
                };
                ack = Some(Ack::Clean(text));
            }
        }
    }

    // Issues pbakaus/impeccable#344 and pbakaus/impeccable#305 kept a clean edit from creating state in a
    // project that never used Impeccino. State lives in the user cache now
    // (docs/adr/0020), never in the project, so a clean edit persists once a
    // session cache exists for the project; the first finding creates it.
    if deferred_total > 0 || (cache_dirty && exists(&get_cache_path(&project_cwd))) {
        persist_project_cache(rt, &project_cwd, &session_cwd, &mut cache, &session_id);
    }

    if detector_threw_any && pending_winner.is_none() && clean_winner.is_none() {
        return result(
            &audit,
            vec![
                ("emitted", Value::Bool(false)),
                ("error", Value::from("detector-threw")),
                ("durationMs", ms_since(started)),
            ],
        );
    }
    if quiet_mode {
        return result(
            &audit,
            vec![
                ("emitted", Value::Bool(false)),
                ("quiet", Value::Bool(true)),
                ("durationMs", ms_since(started)),
            ],
        );
    }
    if let Some(Ack::Pending(text)) = &ack {
        let p = pending_winner.as_ref().unwrap();
        return RunResult {
            stdout: payload(text, "PostToolUse", harness),
            audit: with(
                &audit,
                vec![
                    ("file", Value::String(p.file_path.clone())),
                    ("emitted", Value::Bool(true)),
                    ("kind", Value::from("pending")),
                    ("pending", Value::from(p.known.len())),
                    ("chars", Value::from(utf16_len(text))),
                    ("durationMs", ms_since(started)),
                ],
            ),
        };
    }
    if let Some(sw) = &suppression_winner {
        let text = suppression_notice(rt, &relativize(rt, sw, &render_cwd));
        return RunResult {
            stdout: payload(&text, "PostToolUse", harness),
            audit: with(
                &audit,
                vec![
                    ("file", Value::String(sw.clone())),
                    ("suppressed", Value::Bool(true)),
                    ("emitted", Value::Bool(true)),
                    ("durationMs", ms_since(started)),
                ],
            ),
        };
    }
    if let Some(Ack::Clean(text)) = &ack {
        let c = clean_winner.as_ref().unwrap();
        return RunResult {
            stdout: payload(text, "PostToolUse", harness),
            audit: with(
                &audit,
                vec![
                    ("file", Value::String(c.clone())),
                    ("emitted", Value::Bool(true)),
                    ("kind", Value::from("clean")),
                    ("chars", Value::from(utf16_len(text))),
                    ("durationMs", ms_since(started)),
                ],
            ),
        };
    }
    if pending_winner.is_some() || clean_winner.is_some() {
        return result(
            &audit,
            vec![
                ("emitted", Value::Bool(false)),
                ("skipped", Value::from("non-ui-ack")),
                ("durationMs", ms_since(started)),
            ],
        );
    }
    if clean_ack_deduped {
        return result(
            &audit,
            vec![
                ("emitted", Value::Bool(false)),
                ("skipped", Value::from("clean-ack-deduped")),
                ("durationMs", ms_since(started)),
            ],
        );
    }
    if suppressed_hit {
        return result(
            &audit,
            vec![
                ("suppressed", Value::Bool(true)),
                ("emitted", Value::Bool(false)),
                ("durationMs", ms_since(started)),
            ],
        );
    }
    if native_platform_seen.is_some() && !non_native_target_seen {
        return result(
            &audit,
            vec![
                ("skipped", Value::from("native-platform")),
                (
                    "platform",
                    Value::String(native_platform_seen.unwrap_or_default()),
                ),
                ("durationMs", ms_since(started)),
            ],
        );
    }
    let mut extra = vec![("skipped", Value::from(last_skip))];
    if last_skip == "too-large" {
        extra.push(("bytes", Value::from(skipped_bytes)));
    }
    extra.push(("durationMs", ms_since(started)));
    result(&audit, extra)
}

/// JS: runStopHook({ stdinJson, env, cwd })
pub fn run_stop_hook(rt: &Runtime, stdin: &str) -> RunResult {
    let mut audit: Map<String, Value> = Map::new();
    audit.insert("ts".into(), Value::String(iso_now()));
    audit.insert("event".into(), Value::String("Stop".into()));

    if truthy(rt.env("IMPECCINO_HOOK_DISABLED")) {
        return result(
            &audit,
            vec![
                ("skipped", Value::from("env-disabled")),
                ("durationMs", Value::from(0)),
            ],
        );
    }
    let started = now_ms();
    let event = match parse_event(stdin) {
        Err(()) => {
            return result(
                &audit,
                vec![
                    ("skipped", Value::from("stdin-malformed")),
                    ("durationMs", ms_since(started)),
                ],
            )
        }
        Ok(None) => {
            return result(
                &audit,
                vec![
                    ("skipped", Value::from("stdin-empty")),
                    ("durationMs", ms_since(started)),
                ],
            )
        }
        Ok(Some(e)) => e,
    };
    let harness = resolve_harness(rt, Some(&event));
    audit.insert("harness".into(), Value::from(harness));
    let event = normalize_hook_event(rt, &event, &rt.proc_cwd, harness);
    // Stop-hook re-entry guard (pbakaus/impeccable#400): Claude Code and Codex send
    // `stop_hook_active`; Grok sends `stopHookActive`, copied onto the
    // snake_case field by the normalizer. Cursor and GitHub Copilot omit
    // the field, so the strict `=== true` is a no-op for them.
    let stop_hook_active = event.get("stop_hook_active") == Some(&Value::Bool(true));
    // JS: Grok fires Stop twice: `end_turn` (the gate that can inject
    // additionalContext) then an observe-only `shutdown`. A second deep
    // pass would re-emit the same findings. Claude omits `reason`; only
    // skip when Grok named a reason that is not end_turn (pbakaus/impeccable#646).
    if harness == "grok" {
        if let Some(Value::String(reason)) = event.get("reason") {
            if reason != "end_turn" {
                return result(
                    &audit,
                    vec![
                        ("skipped", Value::from("stop-reason")),
                        ("reason", Value::String(reason.clone())),
                        ("durationMs", ms_since(started)),
                    ],
                );
            }
        }
    }
    let session_cwd = rt.resolve(&[str_field(&event, "cwd").unwrap_or(&rt.proc_cwd)]);
    audit.insert("cwd".into(), Value::String(session_cwd.clone()));
    let session_value = session_id_of(&event);
    let session_id = session_key(&session_value);
    audit.insert(
        "session".into(),
        session_value.unwrap_or_else(|| Value::from("unknown")),
    );
    if stop_hook_active {
        return result(
            &audit,
            vec![
                ("skipped", Value::from("stop-hook-active")),
                ("durationMs", ms_since(started)),
            ],
        );
    }

    // Edits are cached at the owning project root. The session-cwd cache is a
    // bounded index to those roots, so Stop can find child projects without a
    // machine-wide cache search. Include the session cwd itself for older
    // caches and sessions whose owning project is the cwd.
    let index = read_cache(&session_cwd);
    let mut roots = Vec::new();
    for root in registered_project_roots(&index, &session_id) {
        let root = rt.resolve(&[&root]);
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    if !touched_files(&index, &session_id).is_empty() && !roots.contains(&session_cwd) {
        roots.push(session_cwd.clone());
    }
    if roots.is_empty() {
        return result(
            &audit,
            vec![
                ("skipped", Value::from("no-touched-files")),
                ("durationMs", ms_since(started)),
            ],
        );
    }
    if roots.len() > 1 {
        let offset = advance_stop_project_offset(rt, &session_cwd, &session_id, roots.len());
        roots.rotate_left(offset);
    }
    let multi_root_session = roots.len() > 1;
    let output_char_limit = if multi_root_session {
        (DEFAULT_MAX_CHARS as usize).saturating_sub(invocation_omission_reserve())
    } else {
        DEFAULT_MAX_CHARS as usize
    };

    let mut rendered_projects = Vec::new();
    let mut scanned_total = 0usize;
    let mut pre_existing_total = 0usize;
    let mut new_findings_total = 0usize;
    let mut unknown_total = 0usize;
    let mut fresh_files_total = 0usize;
    let mut fresh_findings_total = 0usize;
    let mut projects_with_files = 0usize;
    let mut non_native_projects = 0usize;
    let mut native_platform_seen: Option<String> = None;
    let mut output_findings_remaining = cap_of(&HookConfig::default());
    let mut output_chars_used = 0usize;
    let mut omitted_findings_total = 0usize;

    for project_cwd in roots {
        if scanned_total >= STOP_MAX_FILES {
            break;
        }
        let mut cache = read_cache(&project_cwd);
        let candidate_files = touched_files(&cache, &session_id);
        // The index only locates caches. It cannot grant a stale or malformed
        // file permission to escape that file's actual project, or to bypass
        // sensitive/generated-file filtering. Re-derive the nearest project
        // from disk and require it to match the registered cache root.
        let mut touched: Vec<String> = candidate_files
            .into_iter()
            .filter(|file| {
                !has_path_traversal(file)
                    && !is_sensitive_path(file)
                    && !is_generated_path(file)
                    && is_scan_target_inside_project(rt, file, &project_cwd)
                    && resolve_cache_cwd(rt, Some(file), &session_cwd) == project_cwd
            })
            .collect();
        if touched.is_empty() {
            continue;
        }
        projects_with_files += 1;

        let partial_batch = touched.len() > STOP_MAX_FILES - scanned_total;
        let offset = if partial_batch {
            ensure_session(&mut cache, &session_id)
                .get("stopScanOffset")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize
                % touched.len()
        } else {
            0
        };
        touched.rotate_left(offset);
        let project_config = read_config(&project_cwd);
        let mut scans = HashMap::new();
        let mut fresh_groups: Vec<Group> = Vec::new();
        let mut cache_dirty = false;
        let mut project_has_non_native_target = false;
        for (position, file_path) in touched.iter().enumerate() {
            if scanned_total >= STOP_MAX_FILES {
                break;
            }
            let platform = resolve_project_platform_for_target(rt, &project_cwd, file_path);
            if is_native_platform(platform.as_deref()) {
                native_platform_seen.get_or_insert_with(|| platform.unwrap_or_default());
                continue;
            }
            project_has_non_native_target = true;
            let ext = js::to_lower_case(&jsp::extname(file_path));
            let configured = match_configured_extension(file_path, &project_config.extensions);
            if !ALLOWED_EXTS.contains(&ext.as_str()) && configured.is_none() {
                continue;
            }
            if !exists(file_path) || is_project_skipped(rt, file_path) {
                continue;
            }
            let max_file_bytes = project_config.limits.max_file_bytes;
            if max_file_bytes > 0.0
                && std::fs::metadata(file_path)
                    .map(|metadata| metadata.len() as f64 > max_file_bytes)
                    .unwrap_or(false)
            {
                continue;
            }
            scanned_total += 1;
            if partial_batch {
                ensure_session(&mut cache, &session_id).insert(
                    "stopScanOffset".into(),
                    Value::from((offset + position + 1) % touched.len()),
                );
                cache_dirty = true;
            }
            let content = match std::fs::read(file_path) {
                Ok(b) => String::from_utf8_lossy(&b).into_owned(),
                Err(_) => continue,
            };
            let use_html_engine = match configured {
                Some(c) => c.engine == "html",
                None => ext == ".html" || ext == ".htm",
            };
            let scan = scans.entry(file_path.clone()).or_insert_with(|| {
                design_system_options_for_file(rt, &project_config, &project_cwd, file_path)
            });
            // A detector failure tells us nothing about the file. Leave its
            // remembered state alone rather than recording an empty scan.
            let findings = if use_html_engine {
                match detector_detect_html(rt, file_path, scan) {
                    Ok(f) => f,
                    Err(_) => continue,
                }
            } else {
                detector_detect_text(&content, file_path, scan)
            };
            if !use_html_engine {
                stop_baseline::reconcile(&mut cache, &session_id, file_path, &findings);
            }
            let filtered = filter_findings_for(findings, &project_config, scan);
            let classified = stop_baseline::classify(
                &cache,
                &session_id,
                file_path,
                use_html_engine,
                filtered.clone(),
            );
            pre_existing_total += classified.pre_existing;
            new_findings_total += classified.new;
            unknown_total += classified.unknown;
            let fresh =
                dedupe_against_cache(&classified.findings, &mut cache, &session_id, file_path);
            // Sync the cache to the live scan, including an empty result, so
            // a later reintroduction can be reported again.
            remember_findings(&mut cache, &session_id, file_path, &filtered);
            cache_dirty = true;
            if !fresh.is_empty() {
                fresh_groups.push(Group {
                    file_path: file_path.clone(),
                    findings: fresh,
                });
            }
        }

        if project_has_non_native_target {
            non_native_projects += 1;
        }

        if fresh_groups.is_empty() {
            if cache_dirty {
                persist_project_cache(rt, &project_cwd, &session_cwd, &mut cache, &session_id);
            }
            continue;
        }

        fresh_files_total += fresh_groups.len();
        let project_fresh_findings = fresh_groups
            .iter()
            .map(|group| group.findings.len())
            .sum::<usize>();
        fresh_findings_total += project_fresh_findings;
        let separator_chars = if rendered_projects.is_empty() { 0 } else { 2 };
        let remaining_chars = output_char_limit
            .saturating_sub(output_chars_used)
            .saturating_sub(separator_chars);
        if output_findings_remaining == 0 || remaining_chars == 0 {
            omitted_findings_total += project_fresh_findings;
            persist_project_cache(rt, &project_cwd, &session_cwd, &mut cache, &session_id);
            continue;
        }

        let mut output_config = project_config.clone();
        output_config.limits.max_findings = output_findings_remaining as f64;
        output_config.limits.max_chars = remaining_chars as f64;

        let scan = &scans[&fresh_groups[0].file_path];
        let short = footer_mode_short(&mut cache, &session_id);
        let first_unknown = fresh_groups
            .iter()
            .flat_map(|group| &group.findings)
            .position(|f| f.name.starts_with("[attribution unknown]"));
        let mut attribution_note = if first_unknown.is_some() {
            format!("{ENVELOPE_PREFIX} {}", stop_baseline::UNKNOWN_NOTE)
        } else {
            String::new()
        };
        // Findings and attribution take priority. Append the lower-priority
        // stale DESIGN.md notice only if it fits.
        let render = |note: &str, render_config: &HookConfig| {
            render_grouped_template(
                rt,
                &fresh_groups,
                render_config,
                &RenderOpts {
                    cwd: Some(session_cwd.clone()),
                    short_footer: short,
                    reserve_chars: if note.is_empty() {
                        0.0
                    } else {
                        (utf16_len(note) + 2) as f64
                    },
                },
            )
        };
        let mut rendered = render(&attribution_note, &output_config);
        if !attribution_note.is_empty()
            && !rendered.lines().any(|line| {
                line.starts_with("- ")
                    && (line.contains("[attribution unknown]") || line.contains("[new]"))
            })
        {
            attribution_note = format!("{ENVELOPE_PREFIX} {}", stop_baseline::COMPACT_UNKNOWN_NOTE);
            rendered = render(&attribution_note, &output_config);
        }
        let shows_unknown = rendered
            .lines()
            .any(|line| line.starts_with("- ") && line.contains("[attribution unknown]"));
        if !shows_unknown {
            if let Some(prefix @ 1..) = first_unknown {
                let mut visible_config = output_config.clone();
                visible_config.limits.max_findings = cap_of(&output_config).min(prefix) as f64;
                rendered = render("", &visible_config);
            }
        }
        let text = if shows_unknown {
            format!("{attribution_note}\n\n{rendered}")
        } else {
            rendered
        };
        let mut display_cache = cache.clone();
        let text = append_design_system_note_once(
            rt,
            &text,
            scan,
            &mut display_cache,
            &session_id,
            &output_config,
        );
        let shown_findings = rendered_finding_count(&text);
        let next_chars = output_chars_used + separator_chars + utf16_len(&text);
        if shown_findings <= output_findings_remaining && next_chars <= output_char_limit {
            cache = display_cache;
            commit_footer_shown(rt, &mut cache, &session_id, &text);
            output_findings_remaining -= shown_findings;
            output_chars_used = next_chars;
            omitted_findings_total += project_fresh_findings.saturating_sub(shown_findings);
            rendered_projects.push(text);
        } else {
            omitted_findings_total += project_fresh_findings;
        }
        persist_project_cache(rt, &project_cwd, &session_cwd, &mut cache, &session_id);
    }

    audit.insert("scannedFiles".into(), Value::from(scanned_total));
    audit.insert(
        "preExistingFindings".into(),
        Value::from(pre_existing_total),
    );
    audit.insert("newFindings".into(), Value::from(new_findings_total));
    audit.insert("unknownFindings".into(), Value::from(unknown_total));
    if rendered_projects.is_empty() {
        if projects_with_files > 0 && non_native_projects == 0 && native_platform_seen.is_some() {
            return result(
                &audit,
                vec![
                    ("skipped", Value::from("native-platform")),
                    (
                        "platform",
                        Value::String(native_platform_seen.unwrap_or_default()),
                    ),
                    ("durationMs", ms_since(started)),
                ],
            );
        }
        let reason = if projects_with_files == 0 {
            "no-touched-files"
        } else if fresh_findings_total > 0 {
            "output-budget"
        } else {
            "stop-clean"
        };
        return result(
            &audit,
            vec![
                ("emitted", Value::Bool(false)),
                ("skipped", Value::from(reason)),
                ("durationMs", ms_since(started)),
            ],
        );
    }
    audit.insert("freshFiles".into(), Value::from(fresh_files_total));
    let mut text = join_messages_with_limit(&rendered_projects, output_char_limit);
    if multi_root_session && omitted_findings_total > 0 {
        text = append_invocation_omission_note(&text);
    }
    let output = payload(&text, "Stop", harness);
    RunResult {
        stdout: output,
        audit: with(
            &audit,
            vec![
                ("emitted", Value::Bool(true)),
                ("freshFindings", Value::from(fresh_findings_total)),
                ("chars", Value::from(utf16_len(&text))),
                ("durationMs", ms_since(started)),
            ],
        ),
    }
}

fn is_stop_event(stdin: &str) -> bool {
    // Route on raw stdin through hook-lib's
    // isStopEvent, which matches Claude's `hook_event_name: "Stop"` and
    // Grok Build's `hookEventName: "stop"`.
    match serde_json::from_str::<Value>(stdin) {
        Ok(Value::Object(o)) => crate::hook_lib::is_stop_event(&o),
        _ => false,
    }
}

/// `impeccino hook` (hook.mjs main). Returns the exit code (always 0).
pub fn run(rt: &Runtime, stdin: &str, io: &mut impeccino_common::Io) -> i32 {
    let result = if is_stop_event(stdin) {
        run_stop_hook(rt, stdin)
    } else {
        run_hook(rt, stdin)
    };
    write_audit_log(rt, &result.audit, &rt.proc_cwd);
    if !result.stdout.is_empty() {
        io.out(&result.stdout);
    }
    0
}

#[cfg(test)]
mod event_budget_tests {
    use super::{
        join_messages_with_limit, limit_rendered_findings, rendered_finding_count, utf16_len,
    };

    #[test]
    fn renderer_finding_budget_does_not_count_footer_bullets() {
        let rendered = "[impeccino@1] findings\n- L1 [gradient-text] Gradient text.\n- [side-tab] Side-tab accent border.\n\nTriage findings:\n- Real design problem: fix it.\n- Confident false positive: waive it.\n- Unsure: ask in one line.";
        let (limited, shown) = limit_rendered_findings(rendered, 1);
        assert_eq!(shown, 1);
        assert_eq!(rendered_finding_count(&limited), 1);
        assert!(limited.contains("- L1 [gradient-text]"));
        assert!(!limited.contains("[side-tab]"));
        assert!(limited.contains("- Real design problem:"));
        assert!(limited.contains("- Unsure:"));
    }

    #[test]
    fn multi_project_join_respects_the_event_wide_utf16_char_limit() {
        let first = format!(
            "[impeccino@1] app one\n- L1 [long] {}\n\nFooter",
            "😀".repeat(3_970)
        );
        let second = "[impeccino@1] app two\n- L1 [gradient-text] detail\n\nFooter".to_string();
        let joined = join_messages_with_limit(&[first, second], 8_000);
        assert!(utf16_len(&joined) <= 8_000);
        assert!(joined.contains("app one"));
        assert!(!joined.contains("app two"));
    }
}
