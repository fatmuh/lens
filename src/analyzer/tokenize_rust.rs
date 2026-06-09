//! Rust source tokenizer for duplication detection (CPD).
//!
//! Matches SonarQube's CPD tokenizer behavior for Rust:
//! - Strips line comments (//, ///, //!)
//! - Strips nested block comments (/* ... */)
//! - Strips string literals ("...", r"...", r#"..."#)
//! - Strips byte strings (b"...", br"...")
//! - Strips char and byte char literals ('x', b'x')
//! - Strips raw identifiers (r#keyword → keyword)
//! - Keeps identifiers, keywords, numbers, operators, punctuation
//! - Attributes are preserved ([derive(...)], [test], etc.)

use once_cell::sync::Lazy;
use regex::Regex;

use super::tokenize::Token;

static RE_IDENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z_$][A-Za-z0-9_$]*").unwrap());

const SQ: u8 = b'\''; // single quote
const BS: u8 = b'\\'; // backslash

/// Tokenize Rust source for CPD duplication detection.
pub fn tokenize_rust(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut current_line: u32 = 1;
    let mut i = 0usize;

    while i < len {
        let c = bytes[i];

        if c == b'\n' {
            current_line += 1;
            i += 1;
            continue;
        }
        if c == b'\r' {
            i += 1;
            continue;
        }
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }

        // Line comment: //, ///, //!
        if c == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // Block comment: /* ... */ with nesting
        if c == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            let mut depth = 1u32;
            while i + 1 < len && depth > 0 {
                if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\n' {
                    current_line += 1;
                }
                i += 1;
            }
            continue;
        }

        // Byte char: b'x' or byte string: b"..." or raw byte string: br"..."
        if c == b'b' && i + 1 < len {
            if bytes[i + 1] == SQ {
                // b'x' — byte char
                i += 2;
                while i < len && bytes[i] != SQ {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                continue;
            }
            if bytes[i + 1] == b'"' {
                // b"..." — byte string
                i += 2;
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == BS && i + 1 < len {
                        i += 2;
                        continue;
                    }
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                continue;
            }
            if bytes[i + 1] == b'r' {
                // br"..." or br#"..."# — raw byte string
                let hashes = count_hashes(bytes, i + 2);
                if let Some(n) = hashes {
                    i += 3 + n; // skip b, r, hashes, opening "
                    let closing = make_closing(n);
                    skip_raw_string(bytes, len, &mut i, &closing, &mut current_line);
                    continue;
                }
            }
        }

        // Raw string: r"...", r#"..."#, r##"..."##
        if c == b'r' && i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') {
            let hashes = count_hashes(bytes, i + 1);
            if let Some(n) = hashes {
                i += 2 + n; // skip r, hashes, opening "
                let closing = make_closing(n);
                skip_raw_string(bytes, len, &mut i, &closing, &mut current_line);
                continue;
            }
        }

        // Interpreted string: "..."
        if c == b'"' {
            i += 1;
            while i < len && bytes[i] != b'"' {
                if bytes[i] == BS && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\n' {
                    current_line += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            continue;
        }

        // Char literal: 'x' or '\n' — distinguish from lifetime 'a
        if c == SQ {
            if is_char_literal(bytes, len, i) {
                i += 1;
                while i < len && bytes[i] != SQ {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                continue;
            }
            // Lifetime — skip the ' and let the identifier be picked up
            i += 1;
            continue;
        }

        // Identifier / keyword / raw identifier
        let ch = source[i..].chars().next().unwrap();
        if ch.is_ascii_alphabetic() || ch == '_' {
            // Raw identifier: r#keyword
            if c == b'r' && i + 1 < len && bytes[i + 1] == b'#' {
                let ident_start = i + 2;
                if ident_start < len
                    && (bytes[ident_start].is_ascii_alphabetic() || bytes[ident_start] == b'_')
                {
                    let m = RE_IDENT.find(&source[ident_start..]).unwrap();
                    tokens.push(Token {
                        text: m.as_str().to_string(),
                        line: current_line,
                    });
                    i = ident_start + m.end();
                    continue;
                }
            }
            let m = RE_IDENT.find(&source[i..]).unwrap();
            tokens.push(Token {
                text: m.as_str().to_string(),
                line: current_line,
            });
            i += m.end();
            continue;
        }

        // Number literal
        if ch.is_ascii_digit() {
            let start = i;
            if bytes[i] == b'0' && i + 1 < len {
                match bytes[i + 1] {
                    b'x' | b'X' => {
                        i += 2;
                        while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                            i += 1;
                        }
                    }
                    b'b' | b'B' => {
                        i += 2;
                        while i < len && (bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_')
                        {
                            i += 1;
                        }
                    }
                    b'o' | b'O' => {
                        i += 2;
                        while i < len
                            && ((bytes[i] >= b'0' && bytes[i] <= b'7') || bytes[i] == b'_')
                        {
                            i += 1;
                        }
                    }
                    _ => {}
                }
                if i > start + 2 {
                    // Consumed prefix — skip suffix
                    while i < len && bytes[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    tokens.push(Token {
                        text: source[start..i].to_string(),
                        line: current_line,
                    });
                    continue;
                }
            }
            while i < len {
                let b = bytes[i];
                if b.is_ascii_alphanumeric() || b == b'.' || b == b'_' {
                    i += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: source[start..i].to_string(),
                line: current_line,
            });
            continue;
        }

        // Multi-char operators
        let remaining = &source[i..];
        let next3: String = remaining.chars().take(3).collect();
        let next2: String = remaining.chars().take(2).collect();

        if next3 == "...="
            || next3 == "..="
            || next3 == "<<="
            || next3 == ">>="
            || next3 == "&&="
            || next3 == "||="
            || next3 == "..."
        {
            tokens.push(Token {
                text: next3.clone(),
                line: current_line,
            });
            i += next3.chars().map(|c| c.len_utf8()).sum::<usize>();
            continue;
        }

        if next2 == "=="
            || next2 == "!="
            || next2 == "<="
            || next2 == ">="
            || next2 == "&&"
            || next2 == "||"
            || next2 == "<<"
            || next2 == ">>"
            || next2 == "+="
            || next2 == "-="
            || next2 == "*="
            || next2 == "/="
            || next2 == "%="
            || next2 == "&="
            || next2 == "|="
            || next2 == "^="
            || next2 == "->"
            || next2 == "=>"
            || next2 == ".."
            || next2 == "::"
        {
            tokens.push(Token {
                text: next2.clone(),
                line: current_line,
            });
            i += next2.chars().map(|c| c.len_utf8()).sum::<usize>();
            continue;
        }

        // Single char
        tokens.push(Token {
            text: ch.to_string(),
            line: current_line,
        });
        i += ch.len_utf8();
    }

    tokens
}

/// Count `#` chars then check for `"`.
fn count_hashes(bytes: &[u8], start: usize) -> Option<usize> {
    let mut n = 0;
    let mut i = start;
    while i < bytes.len() && bytes[i] == b'#' {
        n += 1;
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'"' {
        Some(n)
    } else {
        None
    }
}

/// Build closing sequence for raw string with `n` hashes.
fn make_closing(n: usize) -> Vec<u8> {
    let mut v = vec![b'"'];
    for _ in 0..n {
        v.push(b'#');
    }
    v
}

/// Skip raw string content until closing sequence.
fn skip_raw_string(bytes: &[u8], len: usize, i: &mut usize, closing: &[u8], line: &mut u32) {
    while *i + closing.len() <= len {
        if &bytes[*i..*i + closing.len()] == closing {
            *i += closing.len();
            return;
        }
        if bytes[*i] == b'\n' {
            *line += 1;
        }
        *i += 1;
    }
}

/// Check if position `i` starts a char literal (not a lifetime).
fn is_char_literal(bytes: &[u8], len: usize, i: usize) -> bool {
    // 'x' → SQ, non-SQ/BS, SQ
    if i + 2 < len && bytes[i + 1] != SQ && bytes[i + 1] != BS && bytes[i + 2] == SQ {
        return true;
    }
    // '\n' → SQ, BS, any, SQ
    if i + 3 < len && bytes[i + 1] == BS && bytes[i + 3] == SQ {
        return true;
    }
    // '\x41', '\u{1234}' → SQ, BS, ..., SQ
    if i + 1 < len && bytes[i + 1] == BS {
        let mut j = i + 2;
        while j < len && bytes[j] != SQ {
            j += 1;
        }
        return j < len;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_comments() {
        let src = "let x = 1; // comment\nlet y = 2;";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["let", "x", "=", "1", ";", "let", "y", "=", "2", ";"]
        );
    }

    #[test]
    fn strips_doc_comments() {
        let src = "/// Doc\n//! Module\nlet x = 1;";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["let", "x", "=", "1", ";"]);
    }

    #[test]
    fn strips_block_comments() {
        let src = "let x /* comment */ = 1;";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["let", "x", "=", "1", ";"]);
    }

    #[test]
    fn strips_nested_block_comments() {
        let src = "let x /* outer /* inner */ still outer */ = 1;";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["let", "x", "=", "1", ";"]);
    }

    #[test]
    fn strips_strings() {
        let src = "let s = \"hello world\";";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["let", "s", "=", ";"]);
    }

    #[test]
    fn strips_raw_strings() {
        let src = "let s = r\"hello\"; let t = r#\"world\"#;";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["let", "s", "=", ";", "let", "t", "=", ";"]);
    }

    #[test]
    fn strips_byte_strings() {
        let src = "let b = b\"hello\";";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["let", "b", "=", ";"]);
    }

    #[test]
    fn strips_char_literals() {
        let src = "let c = 'x'; let n = 'a';";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["let", "c", "=", ";", "let", "n", "=", ";"]);
    }

    #[test]
    fn preserves_lifetimes() {
        let src = "fn foo<'a>(x: &'a str) -> &'a str { x }";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert!(texts.contains(&"a"));
        assert!(texts.contains(&"fn"));
        assert!(texts.contains(&"foo"));
        assert!(texts.contains(&"str"));
    }

    #[test]
    fn rust_operators() {
        let src = "let x = a..=b; let y = a..b; if a && b || c {}";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert!(texts.contains(&"..="));
        assert!(texts.contains(&".."));
        assert!(texts.contains(&"&&"));
        assert!(texts.contains(&"||"));
    }

    #[test]
    fn preserves_line_numbers() {
        let src = "let x = 1;\n// comment\nlet y = 2;";
        let toks = tokenize_rust(src);
        let y_line = toks.iter().find(|t| t.text == "y").unwrap().line;
        assert_eq!(y_line, 3);
    }

    #[test]
    fn struct_definition() {
        let src = "#[derive(Debug)]\nstruct Server {\n    name: String,\n    port: u16,\n}";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert!(texts.contains(&"struct"));
        assert!(texts.contains(&"Server"));
        assert!(texts.contains(&"String"));
        assert!(texts.contains(&"u16"));
        assert!(texts.contains(&"derive"));
        assert!(texts.contains(&"Debug"));
    }

    #[test]
    fn complex_rust_code() {
        let src = "use std::collections::HashMap;\npub struct Server {\n    name: String,\n}\nimpl Server {\n    pub fn new(name: &str) -> Self {\n        Self { name: name.to_string() }\n    }\n}";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert!(texts.contains(&"struct"));
        assert!(texts.contains(&"Server"));
        assert!(texts.contains(&"impl"));
        assert!(texts.contains(&"pub"));
        assert!(texts.contains(&"fn"));
    }

    #[test]
    fn path_separators() {
        let src = "std::collections::HashMap::new()";
        let toks = tokenize_rust(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert!(texts.contains(&"::"));
        assert!(texts.contains(&"std"));
        assert!(texts.contains(&"HashMap"));
    }
}
