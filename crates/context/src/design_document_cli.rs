//! Explicit draft validation, preview, and guarded DESIGN.md updates.
use crate::{context::resolve_context, target_args::TargetOptions};
use impeccino_common::{
    design_document::validate,
    design_update::{metadata_preview, preview, unified_diff, Removals},
    Io,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const USAGE: &str = "Usage: impeccino design-document <validate|preview|write> --draft <file|-> [--target <path>]\n\nPreview never writes. Write requires --expected <previewRevision> from the reviewed preview.\n  --metadata-only  Refresh the marked JSON block, preserving all other Markdown bytes.\nExplicit removals: --remove-token <JSON-pointer>, --remove-waiver <rule-id>,\n                  --remove-metadata <JSON-pointer-within-metadata>.\nCreative prose and permission to remove existing decisions remain with the caller.\n";

struct Args {
    command: String,
    draft: String,
    target: TargetOptions,
    expected: Option<String>,
    metadata_only: bool,
    removals: Removals,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let Some(command) = args
        .first()
        .filter(|command| ["validate", "preview", "write"].contains(&command.as_str()))
    else {
        return Err(USAGE.into());
    };
    let mut parsed = Args {
        command: command.clone(),
        draft: String::new(),
        target: TargetOptions::default(),
        expected: None,
        metadata_only: false,
        removals: Removals::default(),
    };
    let mut index = 1;
    while index < args.len() {
        let flag = &args[index];
        if flag == "--metadata-only" {
            parsed.metadata_only = true;
            index += 1;
            continue;
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.is_empty() && (!value.starts_with('-') || *value == "-"))
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag.as_str() {
            "--draft" => parsed.draft = value.clone(),
            "--target" | "-t" => parsed.target.target_path = Some(value.clone()),
            "--expected" => parsed.expected = Some(value.clone()),
            "--remove-token" => parsed.removals.tokens.push(value.clone()),
            "--remove-waiver" => parsed.removals.waivers.push(value.to_lowercase()),
            "--remove-metadata" => parsed.removals.metadata.push(value.clone()),
            _ => return Err(format!("Unknown design-document option: {flag}")),
        }
        index += 2;
    }
    if parsed.draft.is_empty() {
        return Err("--draft requires a file path or - for stdin".into());
    }
    if parsed.command == "write" && parsed.expected.is_none() {
        return Err("Write requires --expected <previewRevision>; review a preview first".into());
    }
    if parsed.metadata_only
        && (!parsed.removals.tokens.is_empty() || !parsed.removals.waivers.is_empty())
    {
        return Err("Metadata-only updates can remove metadata values only".into());
    }
    for paths in [
        &mut parsed.removals.tokens,
        &mut parsed.removals.waivers,
        &mut parsed.removals.metadata,
    ] {
        paths.sort();
        paths.dedup();
    }
    Ok(parsed)
}

fn regular(path: &Path) -> Result<Option<String>, String> {
    match fs::symlink_metadata(path) {
        Ok(stat) if stat.file_type().is_file() => fs::read_to_string(path)
            .map(Some)
            .map_err(|error| format!("Cannot read {}: {error}", path.display())),
        Ok(_) => Err(format!(
            "{} must be a regular file; symlinks are not updated",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Cannot inspect {}: {error}", path.display())),
    }
}

fn destination(io: &Io, options: &TargetOptions) -> Result<PathBuf, String> {
    let resolved = resolve_context(&io.cwd.to_string_lossy(), options, &io.env);
    let root = PathBuf::from(&resolved.project_root);
    let path = resolved
        .design_path
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("DESIGN.md"));
    let relative = path.strip_prefix(&root).map_err(|_| "The resolved DESIGN.md is outside the selected project/app; target its owner explicitly".to_string())?;
    let mut parent = root.clone();
    for component in relative
        .components()
        .take(relative.components().count().saturating_sub(1))
    {
        parent.push(component);
        let stat = fs::symlink_metadata(&parent)
            .map_err(|error| format!("Cannot inspect {}: {error}", parent.display()))?;
        if !stat.is_dir() || stat.file_type().is_symlink() {
            return Err(format!(
                "Refusing an unsafe design directory: {}",
                parent.display()
            ));
        }
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Cannot resolve project root: {error}"))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| format!("Cannot resolve design directory: {error}"))?;
    if !canonical_parent.starts_with(canonical_root) {
        return Err("Design destination escapes the selected project/app".into());
    }
    let path = canonical_parent.join(
        path.file_name()
            .ok_or("Design destination has no file name")?,
    );
    let sidecar = path.with_file_name("DESIGN.json");
    match fs::symlink_metadata(&sidecar) {
        Ok(_) => return Err("Migrate or resolve the legacy DESIGN.json with doctor before preparing a single-file update".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(format!("Cannot inspect legacy metadata: {error}")),
    }
    Ok(path)
}

fn revision(
    path: &Path,
    original: Option<&str>,
    draft: &str,
    removals: &Removals,
    updated: &str,
    metadata_only: bool,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"impeccino-design-preview-v1");
    hash.update([u8::from(original.is_some()), u8::from(metadata_only)]);
    let policy = serde_json::to_string(removals).expect("Removal policy contains only strings");
    for part in [
        path.to_string_lossy().as_ref(),
        original.unwrap_or(""),
        draft,
        &policy,
        updated,
    ] {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn report(io: &mut Io, value: &Value) {
    io.out(&format!(
        "{}\n",
        serde_json::to_string_pretty(value)
            .expect("Document reports contain only serializable values")
    ));
}

pub fn run(args: &[String], io: &mut Io) -> i32 {
    if args
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        io.out(USAGE);
        return 0;
    }
    match execute(args, io) {
        Ok(code) => code,
        Err(error) => {
            io.err(&format!("{error}\n"));
            1
        }
    }
}

fn execute(args: &[String], io: &mut Io) -> Result<i32, String> {
    let args = parse_args(args)?;
    let draft = if args.draft == "-" {
        io.stdin().to_string()
    } else {
        fs::read_to_string(io.cwd.join(&args.draft))
            .map_err(|error| format!("Cannot read draft {}: {error}", args.draft))?
    };
    if args.command == "validate" {
        let diagnostics = validate(&draft);
        let valid = !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error");
        report(io, &json!({"valid":valid,"diagnostics":diagnostics}));
        return Ok(if valid { 0 } else { 2 });
    }
    let path = destination(io, &args.target)?;
    let original = regular(&path)?;
    if args.metadata_only && original.is_none() {
        return Err("Metadata-only refresh requires an existing DESIGN.md record".into());
    }
    let update = if args.metadata_only {
        metadata_preview(original.as_deref().unwrap_or(""), &draft, &args.removals)
    } else {
        preview(original.as_deref().unwrap_or(""), &draft, &args.removals)
    };
    let preview_revision = revision(
        &path,
        original.as_deref(),
        &draft,
        &args.removals,
        &update.content,
        args.metadata_only,
    );
    let canonical_cwd = io
        .cwd
        .canonicalize()
        .map_err(|error| format!("Cannot resolve working directory: {error}"))?;
    let display_path = crate::jsp::to_posix(&crate::jsp::relative(
        &canonical_cwd.to_string_lossy(),
        &canonical_cwd.to_string_lossy(),
        &path.to_string_lossy(),
    ));
    let mut result = json!({
        "path":display_path, "valid":update.valid(), "written":false,
        "previewRevision":preview_revision, "metadataOnly":args.metadata_only, "diagnostics":update.diagnostics,
        "changes":update.changes, "preservedSections":update.preserved_sections,
        "diff":unified_diff(original.as_deref().unwrap_or(""), &update.content),
        "content":update.content,
    });
    if args.command == "write" {
        if args.expected.as_deref() != Some(&preview_revision) {
            return Err("The preview is stale or targets different content/removals; review a new preview before writing".into());
        }
        if !update.valid() {
            report(io, &result);
            return Ok(2);
        }
        // Re-resolve the owning boundary and re-read before the atomic replacement.
        if destination(io, &args.target)? != path || regular(&path)? != original {
            return Err("DESIGN.md or its owning boundary changed while preparing the update; review a new preview".into());
        }
        if original.as_deref() != Some(&update.content) {
            impeccino_common::atomic_file::write(&path, update.content.as_bytes())
                .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;
            result["written"] = Value::Bool(true);
        }
    }
    let code = if update.valid() { 0 } else { 2 };
    report(io, &result);
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Project(PathBuf);
    impl Project {
        fn new() -> Self {
            static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "impeccino-design-document-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            fs::create_dir(path.join(".git")).unwrap();
            fs::write(path.join("package.json"), "{}").unwrap();
            Self(path.canonicalize().unwrap())
        }
        fn call(&self, args: &[&str], stdin: &str) -> (i32, String, String) {
            let (mut io, capture) = Io::captured(stdin, self.0.clone(), HashMap::new());
            let code = run(
                &args
                    .iter()
                    .map(|argument| argument.to_string())
                    .collect::<Vec<_>>(),
                &mut io,
            );
            let stdout = String::from_utf8(capture.stdout.borrow().clone()).unwrap();
            let stderr = String::from_utf8(capture.stderr.borrow().clone()).unwrap();
            (code, stdout, stderr)
        }
    }
    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const ORIGINAL: &str = "---\nname: Current\ncolors:\n  primary: \"#123456\"\n---\n## Overview\nOld direction.\n## Colors\nCurrent palette.\n## Iconography\nKeep this foreign guidance.\n";
    const DRAFT: &str = "---\nname: Updated\ncolors:\n  primary: \"#654321\"\n---\n## Overview\nUpdated direction.\n## Colors\nObserved palette.\n";

    #[test]
    fn validate_and_preview_are_read_only_and_write_uses_the_reviewed_record() {
        let project = Project::new();
        let path = project.0.join("DESIGN.md");
        fs::write(&path, ORIGINAL).unwrap();
        let (code, _, _) = project.call(&["validate", "--draft", "-"], DRAFT);
        assert_eq!(code, 0);
        let (code, output, error) = project.call(&["preview", "--draft", "-"], DRAFT);
        assert_eq!(code, 0, "{error}");
        assert_eq!(fs::read_to_string(&path).unwrap(), ORIGINAL);
        let record: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(record["path"], "DESIGN.md");
        assert!(record["diff"].as_str().unwrap().contains("-Old direction."));
        let expected = record["previewRevision"].as_str().unwrap();
        let (code, output, error) =
            project.call(&["write", "--draft", "-", "--expected", expected], DRAFT);
        assert_eq!(code, 0, "{error}");
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap()["written"],
            true
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            record["content"].as_str().unwrap()
        );
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("Keep this foreign guidance."));
    }

    #[test]
    fn canonical_windows_paths_have_the_same_relative_report_path() {
        assert_eq!(
            impeccino_common::jsp::win32::relative(
                r"\\?\C:\project",
                r"\\?\C:\project",
                r"\\?\C:\project\DESIGN.md",
            ),
            "DESIGN.md"
        );
    }

    #[test]
    fn stale_documents_drafts_and_removal_policies_cannot_use_an_old_preview() {
        for mutation in ["document", "draft", "policy"] {
            let project = Project::new();
            let path = project.0.join("DESIGN.md");
            fs::write(&path, ORIGINAL).unwrap();
            let (_, output, _) = project.call(&["preview", "--draft", "-"], DRAFT);
            let record: Value = serde_json::from_str(&output).unwrap();
            let expected = record["previewRevision"].as_str().unwrap();
            let changed_draft = DRAFT.replace("Observed palette.", "Different draft.");
            if mutation == "document" {
                fs::write(
                    &path,
                    ORIGINAL.replace("Old direction.", "Concurrent decision."),
                )
                .unwrap();
            }
            let current = fs::read_to_string(&path).unwrap();
            let mut args = vec!["write", "--draft", "-", "--expected", expected];
            if mutation == "policy" {
                args.extend(["--remove-token", "/colors/primary"]);
            }
            let (code, _, error) = project.call(
                &args,
                if mutation == "draft" {
                    &changed_draft
                } else {
                    DRAFT
                },
            );
            assert_eq!(code, 1);
            assert!(error.contains("preview is stale"), "{error}");
            assert_eq!(fs::read_to_string(&path).unwrap(), current);
        }
    }

    #[test]
    fn missing_expected_revision_and_invalid_drafts_do_not_write() {
        let project = Project::new();
        let path = project.0.join("DESIGN.md");
        fs::write(&path, ORIGINAL).unwrap();
        assert_eq!(project.call(&["write", "--draft", "-"], DRAFT).0, 1);
        let invalid = DRAFT.replace("colors:\n  primary: \"#654321\"", "colors: true");
        let (code, output, _) = project.call(&["preview", "--draft", "-"], &invalid);
        assert_eq!(code, 2);
        let record: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(
            project
                .call(
                    &[
                        "write",
                        "--draft",
                        "-",
                        "--expected",
                        record["previewRevision"].as_str().unwrap()
                    ],
                    &invalid
                )
                .0,
            2
        );
        assert_eq!(fs::read_to_string(path).unwrap(), ORIGINAL);
    }

    #[test]
    fn controlled_updates_never_modify_an_inherited_workspace_record() {
        let project = Project::new();
        fs::write(
            project.0.join("package.json"),
            r#"{"workspaces":["apps/*"]}"#,
        )
        .unwrap();
        fs::create_dir_all(project.0.join("apps/web")).unwrap();
        fs::write(project.0.join("apps/web/package.json"), "{}").unwrap();
        fs::write(project.0.join("DESIGN.md"), ORIGINAL).unwrap();
        let (code, _, error) =
            project.call(&["preview", "--draft", "-", "--target", "apps/web"], DRAFT);
        assert_eq!(code, 1);
        assert!(
            error.contains("outside the selected project/app"),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(project.0.join("DESIGN.md")).unwrap(),
            ORIGINAL
        );
    }

    #[test]
    fn existing_legacy_metadata_must_be_migrated_before_a_single_file_update() {
        let project = Project::new();
        fs::write(project.0.join("DESIGN.md"), ORIGINAL).unwrap();
        fs::write(project.0.join("DESIGN.json"), "{\"unknown\":true}").unwrap();
        let (code, _, error) = project.call(&["preview", "--draft", "-"], DRAFT);
        assert_eq!(code, 1);
        assert!(error.contains("legacy DESIGN.json"));
        assert_eq!(
            fs::read_to_string(project.0.join("DESIGN.json")).unwrap(),
            "{\"unknown\":true}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_files_and_parent_directories_are_not_write_destinations() {
        for parent_link in [false, true] {
            let project = Project::new();
            fs::create_dir(project.0.join("external")).unwrap();
            fs::write(project.0.join("external/record.md"), ORIGINAL).unwrap();
            if parent_link {
                std::os::unix::fs::symlink(project.0.join("external"), project.0.join("docs"))
                    .unwrap();
                fs::write(project.0.join("external/DESIGN.md"), ORIGINAL).unwrap();
            } else {
                std::os::unix::fs::symlink(
                    project.0.join("external/record.md"),
                    project.0.join("DESIGN.md"),
                )
                .unwrap();
            }
            let (code, _, _) = project.call(&["preview", "--draft", "-"], DRAFT);
            assert_eq!(code, 1);
            assert_eq!(
                fs::read_to_string(project.0.join("external/record.md")).unwrap(),
                ORIGINAL
            );
        }
    }

    #[test]
    fn metadata_only_refresh_preserves_every_byte_outside_the_marked_block() {
        let project = Project::new();
        let path = project.0.join("DESIGN.md");
        fs::write(&path, ORIGINAL).unwrap();
        let metadata = "<!-- impeccino:design-metadata -->\n```json\n{\"schemaVersion\":2,\"extensions\":{\"roundedMeta\":{\"pill\":\"999px\"}}}\n```\n";
        let (code, output, error) =
            project.call(&["preview", "--draft", "-", "--metadata-only"], metadata);
        assert_eq!(code, 0, "{error}");
        let record: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(
            record["content"].as_str().unwrap(),
            format!("{ORIGINAL}{metadata}")
        );
        assert_eq!(
            project
                .call(
                    &[
                        "write",
                        "--draft",
                        "-",
                        "--metadata-only",
                        "--expected",
                        record["previewRevision"].as_str().unwrap()
                    ],
                    metadata
                )
                .0,
            0
        );
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            format!("{ORIGINAL}{metadata}")
        );
    }

    #[test]
    fn update_guard_preserves_the_waivers_the_detector_actually_consumes() {
        for source in [
            "# Design\n<!-- impeccino-disable side-tab, GRADIENT-TEXT -- ink rule -->\n",
            "---\ndescription: \"<!-- impeccino-disable side-tab: chosen layout -->\"\n---\n",
            "    ```md\n<!-- impeccino-disable side-tab -->\n    ```\n",
            "````md\n```html\n<!-- impeccino-disable side-tab -->\n```\n````\n",
            "# Design\n<!-- impeccino-disable-line side-tab -->\n<!-- impeccino-disable -->\n",
        ] {
            let actual = impeccino_core::inline_ignores::parse_design_waivers(source);
            let update = preview(source, "# Design\n", &Removals::default());
            assert_eq!(
                update
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "waiver-removed"),
                !actual.is_empty(),
                "{source}"
            );
            let removals = Removals {
                waivers: actual,
                ..Default::default()
            };
            assert!(preview(source, "# Design\n", &removals).valid(), "{source}");
        }
    }

    #[test]
    fn new_records_need_a_review_and_repeated_writes_are_noops() {
        let project = Project::new();
        let (_, output, _) = project.call(&["preview", "--draft", "-"], DRAFT);
        assert!(!project.0.join("DESIGN.md").exists());
        let record: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(
            project
                .call(
                    &[
                        "write",
                        "--draft",
                        "-",
                        "--expected",
                        record["previewRevision"].as_str().unwrap()
                    ],
                    DRAFT
                )
                .0,
            0
        );
        let (_, output, _) = project.call(&["preview", "--draft", "-"], DRAFT);
        let record: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(record["diff"], "");
        let (_, output, _) = project.call(
            &[
                "write",
                "--draft",
                "-",
                "--expected",
                record["previewRevision"].as_str().unwrap(),
            ],
            DRAFT,
        );
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap()["written"],
            false
        );
    }
}
