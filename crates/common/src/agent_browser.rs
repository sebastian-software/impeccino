//! Resolve and launch the external agent-browser CLI consistently.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentBrowser {
    path: PathBuf,
}

impl AgentBrowser {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Build a command for this executable. `std::process::Command` handles
    /// explicit Windows `.cmd` and `.bat` paths with its batch-file escaping.
    pub fn command(&self, args: &[OsString]) -> Command {
        let mut command = Command::new(&self.path);
        command.args(args);
        command
    }
}

/// Resolve an explicit executable or search the supplied PATH entries.
/// Environment inputs are passed by the caller so context generation can
/// remain deterministic and side-effect free.
pub fn resolve_agent_browser(
    explicit: Option<&OsStr>,
    search_paths: &[PathBuf],
    path_ext: Option<&OsStr>,
) -> Option<AgentBrowser> {
    resolve_agent_browser_for(explicit, search_paths, path_ext, cfg!(windows))
}

fn resolve_agent_browser_for(
    explicit: Option<&OsStr>,
    search_paths: &[PathBuf],
    path_ext: Option<&OsStr>,
    windows: bool,
) -> Option<AgentBrowser> {
    if let Some(value) = explicit.filter(|value| !value.is_empty()) {
        let candidate = PathBuf::from(value);
        let has_path = candidate.is_absolute()
            || candidate.components().count() > 1
            || (windows && value.to_string_lossy().contains('\\'));
        if has_path {
            return find_candidate(&candidate, windows, path_ext);
        }
        if let Some(found) = find_in_paths(&candidate, search_paths, windows, path_ext) {
            return Some(found);
        }
        return None;
    }

    find_in_paths(Path::new("agent-browser"), search_paths, windows, path_ext)
}

fn find_in_paths(
    name: &Path,
    search_paths: &[PathBuf],
    windows: bool,
    path_ext: Option<&OsStr>,
) -> Option<AgentBrowser> {
    for directory in search_paths {
        let candidate = directory.join(name);
        if let Some(found) = find_candidate(&candidate, windows, path_ext) {
            return Some(found);
        }
    }
    None
}

fn find_candidate(
    candidate: &Path,
    windows: bool,
    path_ext: Option<&OsStr>,
) -> Option<AgentBrowser> {
    if windows && candidate.extension().is_none() {
        // npm places a POSIX shell shim (`agent-browser`) beside its Windows
        // command shim (`agent-browser.cmd`). Match Windows PATH lookup by
        // trying executable extensions before accepting an extensionless
        // file that cannot be launched directly by CreateProcess.
        for extension in windows_extensions(path_ext) {
            let mut with_extension = candidate.as_os_str().to_os_string();
            with_extension.push(".");
            with_extension.push(extension);
            let with_extension = PathBuf::from(with_extension);
            if with_extension.is_file() {
                return Some(AgentBrowser {
                    path: with_extension,
                });
            }
        }
    }
    candidate.is_file().then(|| AgentBrowser {
        path: candidate.to_path_buf(),
    })
}

fn windows_extensions(path_ext: Option<&OsStr>) -> Vec<OsString> {
    let configured = path_ext
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    let mut extensions: Vec<OsString> = configured
        .split(';')
        .filter(|extension| !extension.is_empty())
        .filter(|extension| {
            ["COM", "EXE", "BAT", "CMD"].iter().any(|allowed| {
                extension
                    .trim_start_matches('.')
                    .eq_ignore_ascii_case(allowed)
            })
        })
        .map(|extension| extension.trim_start_matches('.').into())
        .collect();
    for required in ["EXE", "CMD", "BAT"] {
        if !extensions
            .iter()
            .any(|extension| extension.to_string_lossy().eq_ignore_ascii_case(required))
        {
            extensions.push(required.into());
        }
    }
    extensions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "impeccino-agent-browser-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn explicit_and_path_resolution_use_only_the_supplied_environment() {
        let path = temp_dir("resolve");
        std::fs::write(path.join("agent-browser"), "#!/bin/sh\n").unwrap();
        // Use uppercase to simulate Windows' case-insensitive `.CMD` lookup
        // when this synthetic resolver test runs on a case-sensitive host.
        let shim = path.join("agent-browser.CMD");
        std::fs::write(&shim, "@echo off\r\n").unwrap();
        std::fs::write(
            path.join("agent-browser.PS1"),
            "Write-Output 'unsupported shim'\n",
        )
        .unwrap();

        let resolved = resolve_agent_browser_for(
            None,
            std::slice::from_ref(&path),
            Some(OsStr::new(".PS1;.CMD")),
            true,
        )
        .unwrap();
        assert_eq!(resolved.path(), shim);
        let explicit = resolve_agent_browser_for(
            Some(OsStr::new("agent-browser")),
            std::slice::from_ref(&path),
            Some(OsStr::new(".PS1;.CMD")),
            true,
        )
        .unwrap();
        assert_eq!(explicit.path(), shim);
        assert_eq!(
            resolve_agent_browser_for(Some(OsStr::new("missing-browser")), &[path], None, true),
            None
        );

        let extensionless = temp_dir("resolve-extensionless");
        let native = extensionless.join("agent-browser");
        std::fs::write(&native, "native executable fixture").unwrap();
        let resolved =
            resolve_agent_browser_for(None, std::slice::from_ref(&extensionless), None, true)
                .unwrap();
        assert_eq!(resolved.path(), native);
    }

    #[test]
    fn explicit_path_with_spaces_and_cmd_extension_is_resolved() {
        let path = temp_dir("path with spaces & symbols").join("agent-browser.cmd");
        std::fs::write(&path, "@echo off\r\n").unwrap();
        let resolved = resolve_agent_browser_for(Some(path.as_os_str()), &[], None, true).unwrap();
        assert_eq!(resolved.path(), path);
    }

    #[cfg(windows)]
    #[test]
    fn cmd_shim_round_trips_shell_sensitive_arguments_without_injection() {
        fn unquote(value: &str) -> &str {
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or(value)
        }

        let directory = temp_dir("cmd roundtrip & symbols");
        let shim = directory.join("agent-browser.cmd");
        let capture = directory.join("arguments.txt");
        let marker = directory.join("injected.txt");
        std::fs::write(
            &shim,
            "@echo off\r\nchcp 65001 >nul\r\nsetlocal DisableDelayedExpansion\r\n>\"%IMPECCINO_TEST_ARG_CAPTURE%\" echo %1\r\n>>\"%IMPECCINO_TEST_ARG_CAPTURE%\" echo %2\r\nexit /b 0\r\n",
        ).unwrap();
        let browser = resolve_agent_browser_for(Some(shim.as_os_str()), &[], None, true).unwrap();
        let url = format!(
            "https://localhost/?q=two%20words&literal=%PATH%!&inject=ok& echo INJECTED > {}",
            marker.file_name().unwrap().to_string_lossy()
        );
        let phrase = "O'Brien's quoted phrase with spaces — café";
        let args = vec![OsString::from(&url), OsString::from(phrase)];
        let mut command = browser.command(&args);
        command
            .current_dir(&directory)
            .env("IMPECCINO_TEST_ARG_CAPTURE", &capture);
        crate::proc::hide_window(&mut command);
        let status = command.status().expect("launch the real cmd shim path");
        assert!(status.success(), "cmd shim exited with {status}");
        let lines: Vec<String> = std::fs::read_to_string(&capture)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        assert_eq!(
            lines.len(),
            2,
            "shim received a different argument count: {lines:?}"
        );
        assert_eq!(
            unquote(&lines[0]),
            url,
            "URL argument changed across the .cmd boundary"
        );
        assert_eq!(
            unquote(&lines[1]),
            phrase,
            "spaced apostrophe argument changed across the .cmd boundary"
        );
        assert!(
            !marker.exists(),
            "URL metacharacters executed a second command"
        );
    }
}
