//! Source tokenizer used for duplication detection.
//!
//! The tokenizer is intentionally language-agnostic — it works on any
//! C-like syntax. It strips line and block comments, string literals, and
//! template literals, then emits a flat stream of tokens (identifiers,
//! numbers, punctuation, operators). The line number of each token is
//! retained so we can map a duplicate block back to source locations.

use once_cell::sync::Lazy;
use regex::Regex;

/// A token with the line on which it starts. The line is 1-indexed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub text: String,
    pub line: u32,
}

/// Match an identifier (incl. `$` and `_`).
static RE_IDENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z_$][A-Za-z0-9_$]*").unwrap());

/// Tokenize `source` into a flat stream. Comments and string literals are
/// stripped (but their newlines are preserved so line numbers stay aligned).
pub fn tokenize(source: &str) -> Vec<Token> {
    let cleaned = strip_comments_and_strings(source);
    let mut tokens = Vec::new();

    // Walk the cleaned source; track the original line number by counting
    // newlines as we go.
    let mut current_line: u32 = 1;
    let mut idx = 0usize;
    let bytes = cleaned.as_bytes();

    while idx < bytes.len() {
        let c = cleaned[idx..].chars().next().unwrap();

        if c == '\n' {
            current_line += 1;
            idx += c.len_utf8();
            continue;
        }
        if c.is_whitespace() {
            idx += c.len_utf8();
            continue;
        }

        // Identifier?
        if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            let m = RE_IDENT.find(&cleaned[idx..]).unwrap();
            tokens.push(Token {
                text: m.as_str().to_string(),
                line: current_line,
            });
            // Advance idx; the identifier might span multiple lines if the
            // source had a literal newline inside (shouldn't happen after
            // stripping, but be safe).
            for _ in 0..m.end() {
                if cleaned.as_bytes().get(idx) == Some(&b'\n') {
                    current_line += 1;
                }
                idx += 1;
            }
            continue;
        }

        // Number?
        if c.is_ascii_digit() {
            let start = idx;
            while idx < bytes.len()
                && (cleaned.as_bytes()[idx].is_ascii_alphanumeric()
                    || cleaned.as_bytes()[idx] == b'.')
            {
                idx += 1;
            }
            tokens.push(Token {
                text: cleaned[start..idx].to_string(),
                line: current_line,
            });
            continue;
        }

        // Any other character: emit as a single-char token (operators,
        // punctuation, brackets).
        tokens.push(Token {
            text: c.to_string(),
            line: current_line,
        });
        idx += c.len_utf8();
    }

    tokens
}

/// Replace comments and string literals with whitespace (preserving
/// newlines so line numbers don't shift).
fn strip_comments_and_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut out: Vec<u8> = Vec::with_capacity(len);
    let mut i = 0usize;

    while i < len {
        let c = bytes[i];

        // Line comment: // ... \n
        if c == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            // keep the newline (or EOF) — don't consume it
            continue;
        }

        // Block comment: /* ... */
        if c == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] == b'\n' {
                    out.push(b'\n');
                }
                i += 1;
            }
            i = (i + 2).min(len);
            continue;
        }

        // String: "..." or '...'
        if c == b'"' || c == b'\'' {
            let quote = c;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        out.push(b'\n');
                    }
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\n' {
                    out.push(b'\n');
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Template literal: `...`
        if c == b'`' {
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        out.push(b'\n');
                    }
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\n' {
                    out.push(b'\n');
                }
                if bytes[i] == b'`' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        out.push(c);
        i += 1;
    }

    String::from_utf8(out).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_comments() {
        let src = "x = 1; // comment\ny = 2;";
        let toks = tokenize(src);
        // Expect: x, =, 1, ;, y, =, 2, ;
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", "1", ";", "y", "=", "2", ";"]);
    }

    #[test]
    fn strips_block_comments() {
        let src = "x /* foo */ = 1;";
        let toks = tokenize(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", "1", ";"]);
    }

    #[test]
    fn strips_strings() {
        let src = r#"x = "hello"; y = 'world';"#;
        let toks = tokenize(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", ";", "y", "=", ";"]);
    }

    #[test]
    fn preserves_line_numbers_across_comments() {
        let src = "x = 1;\n// comment line 2\ny = 2;";
        let toks = tokenize(src);
        let y_line = toks.iter().find(|t| t.text == "y").unwrap().line;
        assert_eq!(y_line, 3);
    }

    #[test]
    fn preserves_line_numbers_across_multiline_comment() {
        let src = "x = 1;\n/*\nmulti\nline\n*/\ny = 2;";
        let toks = tokenize(src);
        let y_line = toks.iter().find(|t| t.text == "y").unwrap().line;
        assert_eq!(y_line, 6);
    }

    #[test]
    fn handles_escaped_quotes_in_strings() {
        let src = r#"x = "a\"b";"#;
        let toks = tokenize(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", ";"]);
    }
}
