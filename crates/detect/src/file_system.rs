//! Port of `cli/engine/node/file-system.mjs`: the directory walker, the
//! import graph, framework dev-server config detection, and the port probe.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use regex::Regex;

use crate::jsp;
use crate::util::{re, read_text, read_text_with_error, ANY, D, WS};

/// JS `SKIP_DIRS`.
pub const SKIP_DIRS: &[&str] = &["node_modules", "dist", "build", "__pycache__"];
/// JS `HIDDEN_SOURCE_DIRS`.
pub const HIDDEN_SOURCE_DIRS: &[&str] = &[".vitepress", ".vuepress", ".storybook"];
/// JS `SCANNABLE_EXTENSIONS` (insertion order matters for `resolveImport`).
pub const SCANNABLE_EXTENSIONS: &[&str] = &[
    ".html",
    ".htm",
    ".css",
    ".scss",
    ".sass",
    ".less",
    ".jsx",
    ".tsx",
    ".js",
    ".ts",
    ".vue",
    ".svelte",
    ".astro",
    ".blade.php",
];
/// JS `HTML_EXTENSIONS`.
pub const HTML_EXTENSIONS: &[&str] = &[".html", ".htm"];

/// JS: file-system.mjs#hasScannableExtension
pub fn has_scannable_extension(filename: &str) -> bool {
    let lower = impeccino_core::js::to_lower_case(filename);
    if SCANNABLE_EXTENSIONS.contains(&jsp::extname(&lower).as_str()) {
        return true;
    }
    for ext in SCANNABLE_EXTENSIONS {
        if ext[1..].contains('.') && lower.ends_with(ext) {
            return true;
        }
    }
    false
}

/// `HTML_EXTENSIONS.has(path.extname(filePath).toLowerCase())`.
pub fn is_html_path(file_path: &str) -> bool {
    HTML_EXTENSIONS.contains(&impeccino_core::js::to_lower_case(&jsp::extname(file_path)).as_str())
}

/// JS: file-system.mjs#walkDir(dir, onReadError). An unreadable directory is
/// reported and skipped rather than silently yielding nothing (#711).
pub fn walk_dir_reporting(
    dir: &str,
    on_read_error: &mut dyn FnMut(&str, &std::io::Error),
) -> Vec<String> {
    walk_dir_skipping(dir, on_read_error, &|_, _| false)
}

/// `walk_dir_reporting` that also leaves out every entry `skip(path, is_dir)`
/// accepts; a skipped directory is not descended into. The detector passes
/// the project's own ignore rules here (`project_ignores`).
pub fn walk_dir_skipping(
    dir: &str,
    on_read_error: &mut dyn FnMut(&str, &std::io::Error),
    skip: &dyn Fn(&str, bool) -> bool,
) -> Vec<String> {
    let mut files = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            on_read_error(dir, &e);
            return files;
        }
    };
    let mut entries: Vec<(String, bool)> = Vec::new();
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // `withFileTypes` reports the entry's own type: a symlink is neither a
        // directory nor a file, so Node skips it in the directory branch and
        // treats it as a candidate file when its name has a scannable ext.
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push((name, is_dir));
    }
    // Node's readdir returns entries in the order libuv's scandir yields,
    // which on macOS/Linux is sorted by name for the common filesystems.
    entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    for (name, is_dir) in entries {
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        if is_dir && name.starts_with('.') && !HIDDEN_SOURCE_DIRS.contains(&name.as_str()) {
            continue;
        }
        let full = jsp::join(&[dir, &name]);
        if is_dir {
            if !skip(&full, true) {
                files.extend(walk_dir_skipping(&full, on_read_error, skip));
            }
        } else if has_scannable_extension(&name) && !skip(&full, false) {
            files.push(full);
        }
    }
    files
}

// ─── Import graph ────────────────────────────────────────────────────────────

static IMPORT_SPECIFIER_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(&format!(
            r#"import{WS}+(?:{ANY}*?from{WS}+)?['"]([^'"]+)['"]"#
        ))
        .unwrap(),
        Regex::new(&format!(
            r#"@import{WS}+(?:url\({WS}*)?['"]?([^'");{WS_CHARS}]+)['"]?{WS}*\)?"#,
            WS_CHARS = impeccino_core::js::WS_CHARS
        ))
        .unwrap(),
        Regex::new(&format!(r#"@(?:use|forward){WS}+['"]([^'"]+)['"]"#)).unwrap(),
    ]
});

/// JS: file-system.mjs#resolveImport
pub fn resolve_import(specifier: &str, from_dir: &str, file_set: &[String]) -> Option<String> {
    if !(specifier.starts_with('.') || specifier.starts_with('/')) {
        return None;
    }
    let base = jsp::resolve(from_dir, &[specifier]);
    if file_set.iter().any(|f| *f == base) {
        return Some(base);
    }
    for ext in SCANNABLE_EXTENSIONS {
        let with_ext = format!("{base}{ext}");
        if file_set.iter().any(|f| *f == with_ext) {
            return Some(with_ext);
        }
    }
    for ext in SCANNABLE_EXTENSIONS {
        let index_file = jsp::join(&[&base, &format!("index{ext}")]);
        if file_set.iter().any(|f| *f == index_file) {
            return Some(index_file);
        }
    }
    None
}

/// JS: file-system.mjs#buildImportGraph(files, onReadError). A file that
/// cannot be read is reported and left out of the graph; the caller skips it
/// for the scan too (#711).
pub fn build_import_graph_reporting(
    files: &[String],
    on_read_error: &mut dyn FnMut(&str, &std::io::Error),
) -> Vec<(String, Vec<String>)> {
    let mut graph = Vec::new();
    for file in files {
        let content = match read_text_with_error(file) {
            Ok(c) => c,
            Err(e) => {
                on_read_error(file, &e);
                continue;
            }
        };
        let dir = jsp::dirname(file);
        let mut imports: Vec<String> = Vec::new();
        for pattern in IMPORT_SPECIFIER_PATTERNS.iter() {
            for m in pattern.captures_iter(&content) {
                if let Some(resolved) = resolve_import(&m[1], &dir, files) {
                    if !imports.contains(&resolved) {
                        imports.push(resolved);
                    }
                }
            }
        }
        graph.push((file.clone(), imports));
    }
    graph
}

// ─── Framework dev server detection ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fingerprint {
    /// `{ header, value }`: header must be present; `value` (case-insensitive
    /// substring keyword) must match when given.
    Header {
        header: &'static str,
        value: Option<&'static str>,
    },
    /// `{ body }`: the response body must contain the (case-insensitive when
    /// `ci`) keyword.
    Body { keyword: &'static str, ci: bool },
}

#[derive(Debug, Clone, Copy)]
pub struct FrameworkConfig {
    pub name: &'static str,
    pub files: &'static [&'static str],
    pub default_port: u32,
    /// `portRe` capture group 1 is the port.
    pub port_re: &'static str,
    pub fingerprint: Fingerprint,
}

/// JS `FRAMEWORK_CONFIGS` (first match wins).
pub const FRAMEWORK_CONFIGS: &[FrameworkConfig] = &[
    FrameworkConfig {
        name: "Next.js",
        files: &["next.config.js", "next.config.mjs", "next.config.ts"],
        default_port: 3000,
        port_re: "port",
        fingerprint: Fingerprint::Header {
            header: "x-powered-by",
            value: Some("next"),
        },
    },
    FrameworkConfig {
        name: "SvelteKit",
        files: &["svelte.config.js", "svelte.config.ts"],
        default_port: 5173,
        port_re: "port",
        fingerprint: Fingerprint::Header {
            header: "x-sveltekit-page",
            value: None,
        },
    },
    FrameworkConfig {
        name: "Nuxt",
        files: &["nuxt.config.js", "nuxt.config.ts"],
        default_port: 3000,
        port_re: "port",
        fingerprint: Fingerprint::Header {
            header: "x-powered-by",
            value: Some("nuxt"),
        },
    },
    FrameworkConfig {
        name: "Vite",
        files: &["vite.config.js", "vite.config.ts", "vite.config.mjs"],
        default_port: 5173,
        port_re: "port",
        fingerprint: Fingerprint::Body {
            keyword: "@vite/client",
            ci: false,
        },
    },
    FrameworkConfig {
        name: "Astro",
        files: &["astro.config.js", "astro.config.ts", "astro.config.mjs"],
        default_port: 4321,
        port_re: "port",
        fingerprint: Fingerprint::Body {
            keyword: "astro",
            ci: true,
        },
    },
    FrameworkConfig {
        name: "Angular",
        files: &["angular.json"],
        default_port: 4200,
        port_re: "json",
        fingerprint: Fingerprint::Body {
            keyword: "ng-version",
            ci: true,
        },
    },
    FrameworkConfig {
        name: "Remix",
        files: &["remix.config.js", "remix.config.ts"],
        default_port: 3000,
        port_re: "port",
        fingerprint: Fingerprint::Header {
            header: "x-powered-by",
            value: Some("remix"),
        },
    },
];

re!(PORT_RE, format!("port{WS}*[:=]{WS}*({D}+)"));
re!(JSON_PORT_RE, format!("\"port\"{WS}*:{WS}*({D}+)"));

#[derive(Debug, Clone)]
pub struct DetectedFramework {
    pub name: &'static str,
    pub port: u32,
    pub config_path: String,
    pub fingerprint: Fingerprint,
}

/// JS: file-system.mjs#detectFrameworkConfig
pub fn detect_framework_config(dir: &str) -> Option<DetectedFramework> {
    let rd = std::fs::read_dir(dir).ok()?;
    let entries: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    for cfg in FRAMEWORK_CONFIGS {
        let Some(matched) = cfg.files.iter().find(|f| entries.iter().any(|e| e == *f)) else {
            continue;
        };
        let config_path = jsp::join(&[dir, matched]);
        let mut port = cfg.default_port;
        if let Some(content) = read_text(&config_path) {
            let re: &Regex = if cfg.port_re == "json" {
                &JSON_PORT_RE
            } else {
                &PORT_RE
            };
            if let Some(m) = re.captures(&content) {
                // parseInt(digits, 10): a run of ASCII digits, so a plain parse;
                // an absurdly long run overflows to Infinity in JS and would never
                // be a usable port anyway.
                port = m[1].parse::<u32>().unwrap_or(u32::MAX);
            }
        }
        return Some(DetectedFramework {
            name: cfg.name,
            port,
            config_path,
            fingerprint: cfg.fingerprint,
        });
    }
    None
}

/// JS `isPortListening` result: `{ listening: true, matched }` or `{ listening: false }`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PortProbe {
    pub listening: bool,
    pub matched: bool,
}

const MAX_HTTP_RESPONSE_BYTES: usize = 1024 * 1024;

/// JS: file-system.mjs#isPortListening. With a fingerprint, an HTTP GET of
/// `http://localhost:${port}/` with a 2 s total deadline. Requests stay on
/// loopback, including redirects. Without one, a loopback TCP connect uses a
/// 500 ms total deadline.
pub fn is_port_listening(port: u32, fingerprint: Option<Fingerprint>) -> PortProbe {
    let Ok(port) = u16::try_from(port) else {
        return PortProbe {
            listening: false,
            matched: false,
        };
    };
    if port == 0 {
        return PortProbe {
            listening: false,
            matched: false,
        };
    }
    let Some(fp) = fingerprint else {
        let listening = tcp_connect_loopback(port, Duration::from_millis(500));
        return PortProbe {
            listening,
            matched: listening,
        };
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    match http_get_localhost(port, deadline) {
        None => PortProbe {
            listening: false,
            matched: false,
        },
        Some((headers, body)) => {
            match fp {
                Fingerprint::Header { header, value } => {
                    if let Some(val) = headers.iter().find(|(k, _)| k == header).map(|(_, v)| v) {
                        let ok = match value {
                            None => true,
                            Some(kw) => val.to_ascii_lowercase().contains(kw),
                        };
                        if ok {
                            return PortProbe {
                                listening: true,
                                matched: true,
                            };
                        }
                    }
                }
                Fingerprint::Body { keyword, ci } => {
                    let hit = if ci {
                        body.to_ascii_lowercase().contains(keyword)
                    } else {
                        body.contains(keyword)
                    };
                    if hit {
                        return PortProbe {
                            listening: true,
                            matched: true,
                        };
                    }
                }
            }
            PortProbe {
                listening: true,
                matched: false,
            }
        }
    }
}

fn loopback_socket_addrs(host: &str, port: u16) -> Option<Vec<SocketAddr>> {
    if port == 0 {
        return None;
    }
    if host.eq_ignore_ascii_case("localhost") {
        return Some(vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
        ]);
    }
    let ip = host.parse::<IpAddr>().ok()?;
    if !ip.is_loopback() {
        return None;
    }
    Some(vec![SocketAddr::new(ip, port)])
}

fn tcp_connect_loopback(port: u16, timeout: Duration) -> bool {
    let Some(addrs) = loopback_socket_addrs("localhost", port) else {
        return false;
    };
    let deadline = Instant::now() + timeout;
    for addr in addrs {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        if remaining.is_zero() {
            break;
        }
        if TcpStream::connect_timeout(&addr, remaining).is_ok() {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpTarget {
    host: String,
    port: u16,
    path: String,
}

fn parse_http_authority(authority: &str) -> Option<(String, u16)> {
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let close = bracketed.find(']')?;
        let host = bracketed[..close].parse::<Ipv6Addr>().ok()?.to_string();
        let suffix = &bracketed[close + 1..];
        let port = if suffix.is_empty() {
            80
        } else {
            suffix.strip_prefix(':')?.parse::<u16>().ok()?
        };
        (host, port)
    } else {
        if authority.matches(':').count() > 1 {
            return None;
        }
        match authority.rsplit_once(':') {
            Some((host, port)) => (host.to_string(), port.parse::<u16>().ok()?),
            None => (authority.to_string(), 80),
        }
    };
    (port != 0).then_some((host, port))
}

fn request_path(path: &str) -> Option<String> {
    let path = path.split('#').next()?;
    if !path.starts_with('/') || path.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
        return None;
    }
    Some(path.to_string())
}

/// Resolve redirects without DNS. Hostnames are limited to exact `localhost`;
/// numeric destinations must parse as loopback IPs.
fn parse_redirect_target(location: &str, current: &HttpTarget) -> Option<HttpTarget> {
    if location.starts_with("//") {
        return None;
    }
    if location.starts_with('/') {
        return Some(HttpTarget {
            host: current.host.clone(),
            port: current.port,
            path: request_path(location)?,
        });
    }

    let rest = location.strip_prefix("http://")?;
    let path_start = rest
        .find(|ch| matches!(ch, '/' | '?' | '#'))
        .unwrap_or(rest.len());
    let (host, port) = parse_http_authority(&rest[..path_start])?;
    loopback_socket_addrs(&host, port)?;

    let suffix = &rest[path_start..];
    let path = if suffix.is_empty() || suffix.starts_with('#') {
        "/".to_string()
    } else if suffix.starts_with('?') {
        format!("/{suffix}")
    } else {
        suffix.to_string()
    };
    Some(HttpTarget {
        host,
        port,
        path: request_path(&path)?,
    })
}

/// A bounded HTTP/1.1 client for the local dev-server probe. It follows at
/// most 20 HTTP redirects to loopback. A rejected redirect leaves the source
/// response available, so matching uses only the response received locally.
fn http_get_localhost(port: u16, deadline: Instant) -> Option<(Vec<(String, String)>, String)> {
    let mut target = HttpTarget {
        host: "localhost".to_string(),
        port,
        path: "/".to_string(),
    };
    for _ in 0..21 {
        let (status, headers, body) =
            http_get_once(&target.host, target.port, &target.path, deadline)?;
        if (301..=303).contains(&status) || status == 307 || status == 308 {
            let Some(loc) = headers
                .iter()
                .find(|(k, _)| k == "location")
                .map(|(_, v)| v.clone())
            else {
                return Some((headers, body));
            };
            if let Some(next) = parse_redirect_target(&loc, &target) {
                target = next;
                continue;
            }
            return Some((headers, body));
        }
        return Some((headers, body));
    }
    None
}

fn http_get_once(
    host: &str,
    port: u16,
    path: &str,
    deadline: Instant,
) -> Option<(u16, Vec<(String, String)>, String)> {
    if port == 0 || path.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
        return None;
    }
    let addrs = loopback_socket_addrs(host, port)?;
    let mut stream = None;
    for addr in addrs {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        if remaining.is_zero() {
            return None;
        }
        if let Ok(s) = TcpStream::connect_timeout(&addr, remaining) {
            stream = Some(s);
            break;
        }
    }
    let mut stream = stream?;
    let host_header = if host.parse::<Ipv6Addr>().is_ok() {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_header}\r\nUser-Agent: impeccino\r\nAccept: */*\r\nConnection: close\r\n\r\n"
    );
    let mut written = 0;
    while written < req.len() {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        if remaining.is_zero() {
            return None;
        }
        stream.set_write_timeout(Some(remaining)).ok()?;
        match stream.write(&req.as_bytes()[written..]) {
            Ok(0) => return None,
            Ok(n) => written += n,
            Err(_) => return None,
        }
    }

    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        if remaining.is_zero() {
            return None;
        }
        stream.set_read_timeout(Some(remaining)).ok()?;
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let new_len = buf.len().checked_add(n)?;
                if new_len > MAX_HTTP_RESPONSE_BYTES {
                    return None;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(_) => return None,
        }
    }
    if Instant::now() >= deadline {
        return None;
    }
    let split = find_header_end(&buf)?;
    let head = String::from_utf8_lossy(&buf[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status_line = lines.next()?;
    let status: u16 = status_line.split_whitespace().nth(1)?.parse().ok()?;
    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let raw_body = &buf[split + 4..];
    let chunked = headers
        .iter()
        .any(|(k, v)| k == "transfer-encoding" && v.to_ascii_lowercase().contains("chunked"));
    let body_bytes = if chunked {
        dechunk(raw_body)
    } else {
        raw_body.to_vec()
    };
    Some((
        status,
        headers,
        String::from_utf8_lossy(&body_bytes).into_owned(),
    ))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn dechunk(body: &[u8]) -> Vec<u8> {
    decode_chunked_body(body).unwrap_or_default()
}

fn decode_chunked_body(body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut cursor = 0;
    loop {
        let remaining = body.get(cursor..)?;
        let line_end = remaining.windows(2).position(|w| w == b"\r\n")?;
        let size_line = std::str::from_utf8(&remaining[..line_end]).ok()?;
        let size_text = size_line.split(';').next()?.trim();
        if size_text.is_empty() || !size_text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let size = usize::from_str_radix(size_text, 16).ok()?;
        let data_start = cursor.checked_add(line_end)?.checked_add(2)?;
        if size == 0 {
            let trailers = body.get(data_start..)?;
            if trailers == b"\r\n" {
                return Some(out);
            }
            let trailer_end = find_header_end(trailers)?;
            if trailer_end.checked_add(4)? != trailers.len() {
                return None;
            }
            let trailer_fields = std::str::from_utf8(&trailers[..trailer_end]).ok()?;
            if trailer_fields.split("\r\n").any(|line| !line.contains(':')) {
                return None;
            }
            return Some(out);
        }

        let data_end = data_start.checked_add(size)?;
        let chunk_end = data_end.checked_add(2)?;
        if body.get(data_end..chunk_end)? != b"\r\n" {
            return None;
        }
        out.extend_from_slice(body.get(data_start..data_end)?);
        cursor = chunk_end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn serve_response(response: Vec<u8>) -> (u16, thread::JoinHandle<bool>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(250);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => return false,
                }
            };
            if stream.set_nonblocking(false).is_err() {
                return false;
            }
            let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(&response);
            true
        });
        (port, server)
    }

    #[test]
    fn extensions() {
        assert!(has_scannable_extension("a.blade.php"));
        assert!(has_scannable_extension("A.HTML"));
        assert!(!has_scannable_extension("a.php"));
        assert!(is_html_path("/x/y.HTM"));
    }

    #[test]
    fn imports() {
        // Paths in the platform's own form: the resolver joins with Node's
        // `path` semantics for the host, so a POSIX root would never match the
        // file set on Windows.
        let root = if cfg!(windows) { "C:\\p" } else { "/p" };
        let a = jsp::join(&[root, "a.tsx"]);
        let b_index = jsp::join(&[root, "b", "index.css"]);
        let files = vec![a.clone(), b_index.clone()];
        assert_eq!(resolve_import("./a", root, &files).as_deref(), Some(a.as_str()));
        assert_eq!(resolve_import("./b", root, &files).as_deref(), Some(b_index.as_str()));
        assert_eq!(resolve_import("react", root, &files), None);
    }

    #[test]
    fn import_graph_keeps_files_with_invalid_utf8() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "impeccino-latin1-import-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("main.css");
        let dependency = dir.join("dep.css");
        std::fs::write(&source, b"@import \"./dep.css\"; /* caf\xe9 */\n").unwrap();
        std::fs::write(&dependency, b".dep { color: red; }\n").unwrap();
        let source = source.to_string_lossy().into_owned();
        let dependency = dependency.to_string_lossy().into_owned();
        let files = vec![source.clone(), dependency.clone()];
        let mut errors = Vec::new();

        let graph = build_import_graph_reporting(&files, &mut |file, error| {
            errors.push((file.to_string(), error.kind()));
        });

        assert!(errors.is_empty(), "unexpected read errors: {errors:?}");
        assert_eq!(
            graph,
            vec![
                (source, vec![dependency.clone()]),
                (dependency, Vec::new()),
            ]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_ports_do_not_wrap_to_an_open_port() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = u32::from(listener.local_addr().unwrap().port());
        assert_eq!(
            is_port_listening(port + 65_536, None),
            PortProbe {
                listening: false,
                matched: false,
            }
        );
        drop(listener);

        let (port, server) = serve_response(
            b"HTTP/1.1 200 OK\r\nX-Powered-By: Next.js\r\nContent-Length: 0\r\n\r\n".to_vec(),
        );
        let probe = is_port_listening(
            u32::from(port) + 65_536,
            Some(Fingerprint::Header {
                header: "x-powered-by",
                value: Some("next"),
            }),
        );
        assert!(!server.join().unwrap());
        assert_eq!(
            probe,
            PortProbe {
                listening: false,
                matched: false,
            }
        );
    }

    #[test]
    fn redirect_targets_are_restricted_to_loopback() {
        let current = HttpTarget {
            host: "localhost".to_string(),
            port: 3000,
            path: "/".to_string(),
        };
        assert_eq!(
            parse_redirect_target("http://LOCALHOST:5173/app?q=1#section", &current),
            Some(HttpTarget {
                host: "LOCALHOST".to_string(),
                port: 5173,
                path: "/app?q=1".to_string(),
            })
        );
        assert_eq!(
            parse_redirect_target("http://127.0.0.2:3000/", &current).map(|target| target.path),
            Some("/".to_string())
        );
        assert_eq!(
            parse_redirect_target("http://[::1]:4321?mode=dev#top", &current),
            Some(HttpTarget {
                host: "::1".to_string(),
                port: 4321,
                path: "/?mode=dev".to_string(),
            })
        );
        assert_eq!(
            parse_redirect_target("/next?mode=dev#top", &current),
            Some(HttpTarget {
                host: "localhost".to_string(),
                port: 3000,
                path: "/next?mode=dev".to_string(),
            })
        );

        for location in [
            "http://example.invalid/",
            "http://192.0.2.1/",
            "http://user@127.0.0.1/",
            "http://localhost:65536/",
            "http://localhost:0/",
            "//example.invalid/path",
            "https://localhost/",
            "next",
        ] {
            assert_eq!(
                parse_redirect_target(location, &current),
                None,
                "unexpectedly accepted redirect {location}"
            );
        }
    }

    #[test]
    fn loopback_redirects_are_followed_to_the_fingerprint_response() {
        let (target_port, target_server) =
            serve_response(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nnext".to_vec());
        let redirect = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{target_port}/app\r\nContent-Length: 0\r\n\r\n"
        );
        let (port, redirect_server) = serve_response(redirect.into_bytes());
        let probe = is_port_listening(
            u32::from(port),
            Some(Fingerprint::Body {
                keyword: "next",
                ci: false,
            }),
        );
        assert!(redirect_server.join().unwrap());
        assert!(target_server.join().unwrap());
        assert_eq!(
            probe,
            PortProbe {
                listening: true,
                matched: true,
            }
        );
    }

    #[test]
    fn non_loopback_redirect_keeps_the_local_response_only() {
        let (port, server) = serve_response(
            b"HTTP/1.1 302 Found\r\nLocation: http://example.invalid/\r\nContent-Length: 0\r\n\r\n"
                .to_vec(),
        );
        let probe = is_port_listening(
            u32::from(port),
            Some(Fingerprint::Header {
                header: "x-remote-fingerprint",
                value: Some("present"),
            }),
        );
        assert!(server.join().unwrap());
        assert_eq!(
            probe,
            PortProbe {
                listening: true,
                matched: false,
            }
        );
    }

    #[test]
    fn dechunk_rejects_incomplete_payloads_and_missing_terminators() {
        assert!(dechunk(b"4\r\nabc").is_empty());
        assert!(dechunk(b"4\r\nWiki\r\n").is_empty());
        assert_eq!(dechunk(b"4\r\nWiki\r\n0\r\n\r\n"), b"Wiki");
        assert_eq!(
            dechunk(b"4;ext=value\r\nWiki\r\n0\r\nX-Checksum: ok\r\n\r\n"),
            b"Wiki"
        );
        assert!(dechunk(b"z\r\ninvalid\r\n0\r\n\r\n").is_empty());
        assert!(dechunk(b"1\r\naXX0\r\n\r\n").is_empty());
        assert!(dechunk(b"1\r\na\r\n0\r\n\r\nextra").is_empty());
        assert!(dechunk(b"1\r\na\r\n0\r\nmissing-colon\r\n\r\n").is_empty());
    }

    #[test]
    fn dechunk_size_arithmetic_cannot_panic() {
        let result = std::panic::catch_unwind(|| dechunk(b"ffffffffffffffff\r\n"));
        assert!(result.is_ok(), "chunk size arithmetic overflowed");
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn truncated_chunk_cannot_supply_a_fingerprint() {
        let response =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n18\r\nx-powered-by: next";
        let (port, server) = serve_response(response.to_vec());
        let probe = is_port_listening(
            u32::from(port),
            Some(Fingerprint::Body {
                keyword: "next",
                ci: false,
            }),
        );
        assert!(server.join().unwrap());
        assert_eq!(
            probe,
            PortProbe {
                listening: true,
                matched: false,
            }
        );
    }

    #[test]
    fn response_reads_obey_the_shared_total_deadline() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let accept_deadline = Instant::now() + Duration::from_secs(2);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < accept_deadline =>
                    {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => return,
                }
            };
            if stream.set_nonblocking(false).is_err() {
                return;
            }
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            thread::sleep(Duration::from_millis(260));
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n");
            thread::sleep(Duration::from_millis(260));
            let _ = stream.write_all(b"\r\n");
        });

        let started = Instant::now();
        let response = http_get_once("127.0.0.1", port, "/", started + Duration::from_millis(300));
        let elapsed = started.elapsed();
        server.join().unwrap();

        assert!(response.is_none());
        assert!(
            elapsed < Duration::from_millis(420),
            "read exceeded the total deadline: {elapsed:?}"
        );
    }

    #[test]
    fn response_size_is_bounded() {
        let mut response = b"HTTP/1.1 200 OK\r\nContent-Length: 1048577\r\n\r\n".to_vec();
        response.extend(std::iter::repeat_n(b'a', 1_048_577));
        let (port, server) = serve_response(response);
        let result = http_get_once(
            "127.0.0.1",
            port,
            "/",
            Instant::now() + Duration::from_secs(2),
        );
        server.join().unwrap();
        assert!(
            result.is_none(),
            "accepted response body length {:?}",
            result.as_ref().map(|(_, _, body)| body.len())
        );
    }
}
