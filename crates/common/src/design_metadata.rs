//! Detector metadata belongs to DESIGN.md; DESIGN.json is a legacy input.
use serde_json::Value;
use std::{fs, path::Path};

pub const MARKER: &str = "<!-- impeccino:design-metadata -->";

/// Read the one marked JSON block, ignoring markers inside example fences.
/// An invalid block is an error, never permission to fall back to a sidecar.
pub fn embedded_metadata(markdown: &str) -> Result<Option<Value>, String> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut fence: Option<(char, usize)> = None;
    let mut result = None;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if let Some((ch, count, suffix)) = fence_line(line) {
            match fence {
                Some((open, size)) if ch == open && count >= size && suffix.is_empty() => {
                    fence = None;
                }
                None => fence = Some((ch, count)),
                _ => {}
            }
        } else if fence.is_none() && line == MARKER {
            if result.is_some() {
                return Err("DESIGN.md has more than one design-metadata block".into());
            }
            index += 1;
            while index < lines.len() && lines[index].trim().is_empty() {
                index += 1;
            }
            let Some((ch, size, "json")) = lines.get(index).and_then(|s| fence_line(s.trim()))
            else {
                return Err("The design-metadata marker must precede a JSON fence".into());
            };
            let start = index + 1;
            index = start;
            while index < lines.len() {
                if matches!(fence_line(lines[index].trim()), Some((c, n, "")) if c == ch && n >= size)
                {
                    break;
                }
                index += 1;
            }
            if index == lines.len() {
                return Err("The design-metadata JSON fence is not closed".into());
            }
            result = Some(parse_metadata(&lines[start..index].join("\n"))?);
        }
        index += 1;
    }
    Ok(result)
}

fn fence_line(line: &str) -> Option<(char, usize, &str)> {
    let ch = line.chars().next()?;
    if !matches!(ch, '`' | '~') {
        return None;
    }
    let count = line.chars().take_while(|c| *c == ch).count();
    (count >= 3).then(|| (ch, count, line[count..].trim()))
}

pub fn parse_metadata(text: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if !value.is_object() {
        return Err("Design metadata must be a JSON object".into());
    }
    Ok(value)
}

fn read_regular(path: &Path) -> Result<String, String> {
    let stat = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !stat.file_type().is_file() {
        return Err(format!("{} must be a regular file", path.display()));
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Preserve the entire legacy object, including unknown fields, and every
/// byte of the existing Markdown prefix. Remove the old file only after
/// rereading and verifying the replacement. A conflict leaves both alone.
pub fn migrate_legacy_metadata(design: &Path, sidecar: &Path) -> Result<bool, String> {
    if !sidecar.try_exists().map_err(|e| e.to_string())? {
        return Ok(false);
    }
    let original = read_regular(design)?;
    let legacy_text = read_regular(sidecar)?;
    let legacy = parse_metadata(&legacy_text)?;
    match embedded_metadata(&original)? {
        Some(current) if current != legacy => {
            return Err("DESIGN.md and DESIGN.json contain conflicting metadata".into());
        }
        Some(_) => {}
        None => {
            let newline = if original.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            // Preserve the original JSON text too, including number spelling
            // and fields the current engine does not interpret.
            let json = legacy_text
                .trim()
                .replace("\r\n", "\n")
                .replace('\n', newline);
            let updated = format!(
                "{original}{newline}{MARKER}{newline}```json{newline}{json}{newline}```{newline}"
            );
            if embedded_metadata(&updated)? != Some(legacy.clone()) {
                return Err("Cannot append metadata after an unclosed example fence".into());
            }
            if read_regular(design)? != original || read_regular(sidecar)? != legacy_text {
                return Err("Design artifacts changed while migration was being prepared".into());
            }
            crate::atomic_file::write(design, updated.as_bytes()).map_err(|e| e.to_string())?;
        }
    }
    let written = read_regular(design)?;
    if embedded_metadata(&written)? != Some(legacy) || read_regular(sidecar)? != legacy_text {
        return Err("Could not verify migrated metadata; DESIGN.json was retained".into());
    }
    fs::remove_file(sidecar).map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Project(std::path::PathBuf);
    impl Project {
        fn new() -> Self {
            static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "impeccino-metadata-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn migration_preserves_crlf_waivers_unknown_fields_and_json_number_spelling() {
        let p = Project::new();
        let design = p.0.join("DESIGN.md");
        let sidecar = p.0.join("DESIGN.json");
        let original = "# Design\r\n<!-- impeccino-disable glow: brand signal -->\r\n";
        let legacy =
            "{\"schemaVersion\":1,\"unknown\":9007199254740993,\"ratio\":1.2300,\"extra\":[true]}";
        fs::write(&design, original).unwrap();
        fs::write(&sidecar, legacy).unwrap();
        assert!(migrate_legacy_metadata(&design, &sidecar).unwrap());
        let migrated = fs::read_to_string(&design).unwrap();
        assert!(migrated.starts_with(original));
        assert!(migrated.contains(legacy));
        assert!(!migrated.replace("\r\n", "").contains('\n'));
        assert!(!sidecar.exists());
        assert!(!migrate_legacy_metadata(&design, &sidecar).unwrap());
    }

    #[test]
    fn equal_embedded_metadata_only_removes_the_redundant_legacy_file() {
        let p = Project::new();
        let design = p.0.join("DESIGN.md");
        let sidecar = p.0.join("DESIGN.json");
        let original = format!("# Design\n{MARKER}\n```json\n{{\"extra\":true}}\n```\n");
        fs::write(&design, &original).unwrap();
        fs::write(&sidecar, "{ \"extra\": true }").unwrap();
        assert!(migrate_legacy_metadata(&design, &sidecar).unwrap());
        assert_eq!(fs::read_to_string(design).unwrap(), original);
        assert!(!sidecar.exists());
    }

    #[test]
    fn invalid_json_or_unclosed_markdown_fence_leaves_both_files_untouched() {
        for (original, legacy) in [
            ("# Design", "[]"),
            ("# Design\n```markdown\n", "{}"),
            ("# Design", "not json"),
        ] {
            let p = Project::new();
            let design = p.0.join("DESIGN.md");
            let sidecar = p.0.join("DESIGN.json");
            fs::write(&design, original).unwrap();
            fs::write(&sidecar, legacy).unwrap();
            assert!(migrate_legacy_metadata(&design, &sidecar).is_err());
            assert_eq!(fs::read_to_string(design).unwrap(), original);
            assert_eq!(fs::read_to_string(sidecar).unwrap(), legacy);
        }
    }

    #[cfg(unix)]
    #[test]
    fn migration_does_not_follow_symlinked_design_artifacts() {
        for link_design in [false, true] {
            let p = Project::new();
            let design = p.0.join("DESIGN.md");
            let sidecar = p.0.join("DESIGN.json");
            let external = p.0.join("external");
            fs::write(&external, if link_design { "# Design" } else { "{}" }).unwrap();
            let link = if link_design { &design } else { &sidecar };
            std::os::unix::fs::symlink(&external, link).unwrap();
            fs::write(
                if link_design { &sidecar } else { &design },
                if link_design { "{}" } else { "# Design" },
            )
            .unwrap();
            assert!(migrate_legacy_metadata(&design, &sidecar).is_err());
            assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
            assert_eq!(fs::read_to_string(&design).unwrap(), "# Design");
            assert_eq!(fs::read_to_string(&sidecar).unwrap(), "{}");
        }
    }

    #[test]
    fn examples_are_not_metadata_and_actual_blocks_support_crlf_and_tilde_fences() {
        let example = format!("````markdown\n{MARKER}\n```json\n{{}}\n```\n````\n");
        assert_eq!(embedded_metadata(&example).unwrap(), None);
        let actual =
            format!("{example}{MARKER}\r\n\r\n~~~json\r\n{{\"extra\":[1,true]}}\r\n~~~\r\n");
        assert_eq!(
            embedded_metadata(&actual).unwrap(),
            Some(json!({"extra":[1,true]}))
        );
    }

    #[test]
    fn broken_or_duplicate_blocks_cannot_be_mistaken_for_missing_metadata() {
        for content in [
            "not a fence",
            "```json\n{}",
            "```json\n[]\n```",
            "```json\nbroken\n```",
        ] {
            assert!(embedded_metadata(&format!("{MARKER}\n{content}")).is_err());
        }
        let block = format!("{MARKER}\n```json\n{{}}\n```\n");
        assert!(embedded_metadata(&block.repeat(2)).is_err());
    }
}
