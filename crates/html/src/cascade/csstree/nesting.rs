/// Maximum simultaneous CSS and selector delimiter depth accepted by the
/// recursive parsers. The guard counts `(`, `{`, and `[` together, ignores
/// escaped delimiters, and does not count delimiters inside strings/comments.
/// A 64-level cap accepts ordinary generated CSS while bounding recursive
/// parser and AST-drop depth on platforms with small thread stacks.
const MAX_NESTING_DEPTH: usize = 64;

/// Whether source contains more than [`MAX_NESTING_DEPTH`] nested CSS
/// delimiters. This shared lexical check runs before both recursive parsers.
pub(crate) fn exceeds_nesting_limit(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut closers = [0; MAX_NESTING_DEPTH];
    let mut depth = 0;
    let mut quote = None;
    let mut in_comment = false;
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                in_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if let Some(delimiter) = quote {
            if byte == b'\\' {
                index = skip_escape(bytes, index);
            } else if is_newline(byte) {
                // css-tree's tokenizer ends a bad string at an unescaped
                // newline and resumes tokenizing after it.
                quote = None;
                index = skip_newline(bytes, index);
            } else if byte == delimiter {
                quote = None;
                index += 1;
            } else {
                index += 1;
            }
            continue;
        }

        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            in_comment = true;
            index += 2;
            continue;
        }
        if byte == b'\'' || byte == b'"' {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'\\' {
            index = skip_escape(bytes, index);
            continue;
        }

        let closer = match byte {
            b'(' => Some(b')'),
            b'{' => Some(b'}'),
            b'[' => Some(b']'),
            _ => None,
        };
        if let Some(closer) = closer {
            if depth == MAX_NESTING_DEPTH {
                return true;
            }
            closers[depth] = closer;
            depth += 1;
        } else if matches!(byte, b')' | b'}' | b']') && depth > 0 && closers[depth - 1] == byte {
            depth -= 1;
        }
        index += 1;
    }

    false
}

/// Skip one CSS escape, including the optional whitespace after a hex escape.
fn skip_escape(bytes: &[u8], index: usize) -> usize {
    let mut next = (index + 1).min(bytes.len());
    if next == bytes.len() {
        return next;
    }

    if bytes[next].is_ascii_hexdigit() {
        let mut digits = 0;
        while next < bytes.len() && bytes[next].is_ascii_hexdigit() && digits < 6 {
            next += 1;
            digits += 1;
        }
        if next < bytes.len() && is_css_whitespace(bytes[next]) {
            // CSS treats CRLF as one escape terminator.
            if bytes[next] == b'\r' && bytes.get(next + 1) == Some(&b'\n') {
                next += 2;
            } else {
                next += 1;
            }
        }
        next
    } else if is_newline(bytes[next]) {
        skip_newline(bytes, next)
    } else {
        next + 1
    }
}

fn is_newline(byte: u8) -> bool {
    matches!(byte, b'\n' | b'\r' | b'\x0c')
}

fn is_css_whitespace(byte: u8) -> bool {
    is_newline(byte) || matches!(byte, b' ' | b'\t')
}

fn skip_newline(bytes: &[u8], index: usize) -> usize {
    if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
        index + 2
    } else {
        index + 1
    }
}
