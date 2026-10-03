//! String-level guards over the two launchers (`skill/scripts/impeccino`,
//! `skill/scripts/impeccino.cmd`). The .cmd cannot be executed here (no Windows),
//! so this pins the shapes a dry parse depends on: asset/URL construction in
//! both launchers matches the release asset naming of
//! `.github/workflows/release-engine.yml`, the .cmd contains
//! no multi-line parenthesized blocks (the parse-time `%var%` expansion bug
//! that made its download path dead code), both launchers carry the
//! engine-probe handshake, and the .cmd verifies downloads via certutil.

/// Where `.github/workflows/release-engine.yml` publishes engine binaries.
const DEFAULT_DOWNLOAD_BASE: &str = "https://github.com/sebastian-software/impeccino/releases/download";

/// The release asset name for one platform, as the launchers compose it.
fn asset_url(base: &str, version: &str, os: &str, arch: &str) -> String {
    let ext = if os == "windows" { ".exe" } else { "" };
    format!("{}/engine-v{version}/impeccino-{os}-{arch}{ext}", base.trim_end_matches('/'))
}

/// The launchers ship next to the skill they power.
fn launcher_dir() -> String {
    format!("{}/../../skill/scripts", env!("CARGO_MANIFEST_DIR"))
}

fn launcher_file(name: &str) -> String {
    let path = format!("{}/{name}", launcher_dir());
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn sh_launcher_asset_naming_matches_engine() {
    let sh = launcher_file("impeccino");
    // The composed URL is <base>/engine-v<version>/impeccino-<os>-<arch>[.exe].
    assert!(sh.contains(&format!("IMPECCINO_DOWNLOAD_BASE:-{DEFAULT_DOWNLOAD_BASE}")));
    assert!(sh.contains(r#"asset="impeccino-$os-$arch""#));
    assert!(sh.contains(r#"url="$base/engine-v$version/$asset""#));
    // Windows-on-ARM fallback, same naming the engine computes.
    let win_x64 = asset_url(DEFAULT_DOWNLOAD_BASE, "V", "windows", "x64");
    assert!(win_x64.ends_with("/engine-vV/impeccino-windows-x64.exe"));
    assert!(sh.contains(r#"url="$base/engine-v$version/impeccino-windows-x64.exe""#));
    // Only automatic, version-pinned candidates are probed; PATH and the
    // unversioned user cache are not fallback candidates.
    assert!(sh.contains("engine-probe"));
    assert!(sh.contains(r#"probe_ok "$bin" "$version""#));
    assert!(sh.contains(r#"probe_ok "$cached" "$version""#));
    assert!(sh.contains("cmp -s - "));
    assert!(!sh.contains("home_bin"));
    assert!(!sh.contains("command -v impeccino"));
    assert!(sh.contains("IMPECCINO_BIN points to a missing or non-executable file"));
    assert!(sh.contains("--connect-timeout 5 --max-time 60"));
    assert!(sh.contains("record_failure || true"));
    // The final error must not recommend the npm package (it still serves 3.x).
    assert!(!sh.contains("npm i -g"));
    assert!(!sh.contains("npm install"));
}

#[test]
fn cmd_launcher_asset_naming_matches_engine() {
    let cmd = launcher_file("impeccino.cmd");
    assert!(cmd.contains(DEFAULT_DOWNLOAD_BASE));
    // URL construction: expanded on straight-line statements, no blocks.
    assert!(cmd.contains(r#"set "asset=impeccino-windows-%arch%.exe""#));
    assert!(cmd.contains(r#"set "url=%IMPECCINO_DOWNLOAD_BASE%/engine-v%version%/%asset%""#));
    // arm64 falls back to the x64 asset (Windows on ARM runs x64 binaries).
    assert!(cmd.contains(r#"set "asset=impeccino-windows-x64.exe""#));
    assert!(cmd.contains(r#"if /I "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "arch=arm64""#));
    // sha256 verification via certutil against the pinned digest.
    assert!(cmd.contains("certutil -hashfile"));
    assert!(cmd.contains("engine.sha256"));
    // Exact, successful engine/version handshake for sibling/cache candidates.
    assert!(cmd.contains("engine-probe"));
    assert!(cmd.contains(r#"findstr /x /c:"impeccino-engine %~2""#));
    assert!(cmd.contains(r#"if not "%probe_status%"=="0""#));
    assert!(cmd.contains(r#"if not "%probe_lines%"=="1""#));
    assert!(!cmd.contains("home_bin"));
    assert!(!cmd.contains("where impeccino"));
    assert!(cmd.contains("IMPECCINO_BIN points to a missing or unusable executable"));
    assert!(cmd.contains("--connect-timeout 5 --max-time 60"));
    assert!(cmd.contains(r#"set "stage=%cache_dir%\impeccino-%RANDOM%%RANDOM%.part""#));
    assert!(cmd.contains(".impeccino-download-failed"));
    assert!(cmd.contains(r#"System32\WindowsPowerShell\v1.0\powershell.exe"#));
    assert_eq!(cmd.matches(r#"<nul >nul 2>nul"#).count(), 2);
    assert!(cmd.contains("[IO.File]::ReadAllText($p)"));
    assert!(cmd.contains("[IO.File]::Delete($p)"));
    assert!(!cmd.contains("Get-Content -Raw"));
    assert!(!cmd.contains("npm i -g"));
}

#[test]
fn launchers_fail_explicit_overrides_and_bound_downloads() {
    let sh = launcher_file("impeccino");
    let cmd = launcher_file("impeccino.cmd");
    assert!(sh.contains("IMPECCINO_BIN points to a missing or non-executable file"));
    assert!(cmd.contains("IMPECCINO_BIN points to a missing or unusable executable"));
    assert!(sh.contains("--connect-timeout 5 --max-time 60"));
    assert!(cmd.contains("--connect-timeout 5 --max-time 60"));
    assert!(sh.contains(".impeccino-download-failed"));
    assert!(cmd.contains(".impeccino-download-failed"));
}

#[test]
fn cmd_launcher_has_no_multiline_parenthesized_blocks() {
    // cmd.exe expands %var% inside a parenthesized block at parse time, so a
    // `set` + read-back inside one block silently reads the pre-block value
    // (rollout review S3: the download path could never fire). The launcher
    // is written as straight-line goto flow; keep it that way.
    let cmd = launcher_file("impeccino.cmd");
    for (i, line) in cmd.lines().enumerate() {
        assert!(
            !line.trim_end().ends_with('('),
            "impeccino.cmd line {}: opens a multi-line parenthesized block: {line}",
            i + 1
        );
    }
}

#[test]
fn launchers_verify_only_against_the_digests_pinned_in_the_skill() {
    // engine.sha256 next to the launchers pins each release asset as
    // `<sha256>  engine-v<version>/<asset>`. A version without pins is
    // refused before any download; a missing hash tool or a mismatch is
    // fatal. There is no fallback to a checksum file from the release.
    let sh = launcher_file("impeccino");
    let cmd = launcher_file("impeccino.cmd");
    assert!(sh.contains(r#"key="engine-v$version/${url##*/}""#));
    assert!(sh.contains(r#"grep -qF "  engine-v$version/" "$dir/engine.sha256""#));
    assert!(cmd.contains(r#"if "%%b"=="engine-v%version%/%asset%""#));
    assert!(cmd.contains(r#"findstr /c:"  engine-v%version%/" "%~dp0engine.sha256""#));
    assert!(cmd.contains("if not defined expected goto no_pin_downloaded"));
    for text in [&sh, &cmd] {
        assert!(text.contains("refusing the unverified download") || text.contains("refusing to download an unverified engine"));
        assert!(text.contains("checksum mismatch downloading"));
        assert!(!text.contains("$url.sha256") && !text.contains("%url%.sha256"), "no sidecar fallback");
    }
}

#[test]
fn launchers_reference_the_same_release_channel() {
    let sh = launcher_file("impeccino");
    let cmd = launcher_file("impeccino.cmd");
    for text in [&sh, &cmd] {
        assert!(text.contains("https://github.com/sebastian-software/impeccino/releases"));
    }
    // Spot-check the release naming the launcher template composes.
    assert_eq!(
        asset_url(DEFAULT_DOWNLOAD_BASE, "1.2.3", "darwin", "arm64"),
        format!("{DEFAULT_DOWNLOAD_BASE}/engine-v1.2.3/impeccino-darwin-arm64")
    );
}

#[test]
fn sh_launcher_exports_skill_dir_before_the_env_bin_exec() {
    // Regression: the launcher used to `exec "$IMPECCINO_BIN"` BEFORE it
    // exported IMPECCINO_SKILL_DIR/SELF, so a binary reached via IMPECCINO_BIN
    // ran with no skill dir and could not inline reference/*.md or read its own
    // version (native platform refs and UPDATE_AVAILABLE silently vanished).
    // The skill-behavior suite caught it; the oracle could not (it sets the env
    // directly). Guard the ordering at the string level and functionally.
    let sh = launcher_file("impeccino");
    let export_pos = sh
        .find("export IMPECCINO_SKILL_DIR IMPECCINO_SELF")
        .expect("launcher exports the skill-dir env");
    let env_bin_exec = sh
        .find(r#"exec "$IMPECCINO_BIN""#)
        .expect("launcher execs IMPECCINO_BIN");
    assert!(
        export_pos < env_bin_exec,
        "IMPECCINO_SKILL_DIR must be exported before the IMPECCINO_BIN exec"
    );
}

#[cfg(unix)]
#[test]
fn sh_launcher_passes_skill_dir_to_the_env_bin() {
    use std::os::unix::fs::PermissionsExt;
    let launcher = format!("{}/impeccino", launcher_dir());
    let dir = std::env::temp_dir().join(format!("impeccino-launcher-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A stub "engine binary" that just prints the skill dir it was handed.
    let stub = dir.join("stub");
    std::fs::write(&stub, "#!/bin/sh\nprintf 'SKILL_DIR=%s\\n' \"${IMPECCINO_SKILL_DIR:-UNSET}\"\n").unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = std::process::Command::new("sh")
        .arg(&launcher)
        .arg("context")
        .env("IMPECCINO_BIN", &stub)
        .env_remove("IMPECCINO_SKILL_DIR")
        .output()
        .expect("run launcher");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("SKILL_DIR=") && !stdout.contains("SKILL_DIR=UNSET") && !stdout.contains("SKILL_DIR=\n"),
        "launcher must export a non-empty IMPECCINO_SKILL_DIR to the IMPECCINO_BIN binary; got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn detect_dev_server_tip_quotes_the_actual_launcher_path() {
    use std::collections::HashMap;
    use std::net::TcpListener;

    let root = std::env::temp_dir().join(format!("impeccino-detect-tip-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("next.config.js"), "module.exports = {};\n").unwrap();
    let port = TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    std::fs::write(
        root.join("next.config.js"),
        format!("module.exports = {{ port: {port} }};\n"),
    )
    .unwrap();

    let self_path = "/tmp/impeccino space '$(touch marker)/scripts/impeccino";
    let env = HashMap::from([("IMPECCINO_SELF".to_string(), self_path.to_string())]);
    let (mut io, captured) = impeccino_common::Io::captured("", root.clone(), env);
    let html = impeccino_detect::MissingHtmlEngine;
    let engines = impeccino_detect::Engines {
        html: &html,
        url: None,
    };
    let status = impeccino_detect::run_detect(&[".".to_string()], &mut io, &engines);
    let stderr = String::from_utf8(captured.stderr.borrow().clone()).unwrap();
    assert_eq!(status, 0, "{stderr}");
    assert!(
        stderr.contains(
            "'/tmp/impeccino space '\\''$(touch marker)/scripts/impeccino' detect http://localhost:"
        ),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn detect_dev_server_tip_documents_powershell_invocation() {
    let path = format!("{}/../detect/src/cli.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    assert!(source.contains("In PowerShell, prefix the quoted launcher path with `&`."));
    assert!(source.contains("quote_executable_path(self_cmd, cfg!(windows))"));
}
