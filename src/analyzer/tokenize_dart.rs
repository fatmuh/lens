//! Dart source tokenizer for duplication detection (CPD).
//!
//! Matches SonarQube's CPD tokenizer behavior for Dart:
//! - Strips line comments (// and ///)
//! - Strips block comments (/* ... */)
//! - Strips string literals (including multiline, raw, and interpolation)
//! - Strips string interpolation expressions (${...})
//! - Keeps identifiers, keywords, numbers, operators, punctuation

use once_cell::sync::Lazy;
use regex::Regex;

use super::tokenize::Token;

static RE_IDENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z_$][A-Za-z0-9_$]*").unwrap());

/// Dart keywords that SonarQube CPD treats as significant tokens.
const DART_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "assert",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "covariant",
    "default",
    "deferred",
    "do",
    "dynamic",
    "else",
    "enum",
    "export",
    "extends",
    "extension",
    "external",
    "factory",
    "false",
    "final",
    "finally",
    "for",
    "Function",
    "get",
    "hide",
    "if",
    "implements",
    "import",
    "in",
    "interface",
    "is",
    "late",
    "library",
    "mixin",
    "new",
    "null",
    "on",
    "operator",
    "part",
    "required",
    "rethrow",
    "return",
    "sealed",
    "set",
    "show",
    "static",
    "super",
    "switch",
    "sync",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// Tokenize Dart source for CPD duplication detection.
/// Returns a flat stream of significant tokens (comments and strings stripped).
pub fn tokenize_dart(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut current_line: u32 = 1;
    let mut i = 0usize;

    while i < len {
        let c = bytes[i];

        // Newline
        if c == b'\n' {
            current_line += 1;
            i += 1;
            continue;
        }

        // Carriage return
        if c == b'\r' {
            i += 1;
            continue;
        }

        // Whitespace
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }

        // Line comment: // or ///
        if c == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // Block comment: /* ... */ (can be /** ... */ doc comment)
        if c == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] == b'\n' {
                    current_line += 1;
                }
                i += 1;
            }
            i = (i + 2).min(len);
            continue;
        }

        // Raw string: r"...", r'...', r"""...""", r'''...'''
        // Also r"..." multi-line
        if c == b'r' && i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'\'') {
            let quote = bytes[i + 1];
            // Check for triple-quoted
            let triple = i + 4 <= len
                && bytes[i + 1] == quote
                && bytes[i + 2] == quote
                && bytes[i + 3] == quote;
            i += 2; // skip r and opening quote
            if triple {
                i += 2; // skip two more quotes
                let mut seq = 0u8;
                while i < len {
                    if bytes[i] == b'\n' {
                        current_line += 1;
                    }
                    if bytes[i] == quote {
                        seq += 1;
                        if seq == 3 {
                            i += 1;
                            break;
                        }
                    } else {
                        seq = 0;
                    }
                    i += 1;
                }
            } else {
                while i < len {
                    if bytes[i] == b'\n' {
                        current_line += 1;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            continue;
        }

        // String literal: "..." or '...' or """...""" or '''...'''
        if c == b'"' || c == b'\'' {
            let quote = c;
            // Check for triple-quoted string
            let triple = i + 3 <= len
                && bytes[i] == quote
                && i + 1 < len
                && bytes[i + 1] == quote
                && i + 2 < len
                && bytes[i + 2] == quote;

            if triple {
                i += 3; // skip opening triple quote
                let mut seq = 0u8;
                while i < len {
                    if bytes[i] == b'\\' && i + 1 < len {
                        if bytes[i + 1] == b'\n' {
                            current_line += 1;
                        }
                        i += 2;
                        continue;
                    }
                    // String interpolation: ${...}
                    if bytes[i] == b'$' && i + 1 < len && bytes[i + 1] == b'{' {
                        i += 2;
                        let mut depth = 1u32;
                        while i < len && depth > 0 {
                            if bytes[i] == b'{' {
                                depth += 1;
                            }
                            if bytes[i] == b'}' {
                                depth -= 1;
                            }
                            if bytes[i] == b'\n' {
                                current_line += 1;
                            }
                            i += 1;
                        }
                        continue;
                    }
                    if bytes[i] == b'\n' {
                        current_line += 1;
                    }
                    if bytes[i] == quote {
                        seq += 1;
                        if seq == 3 {
                            i += 1;
                            break;
                        }
                    } else {
                        seq = 0;
                    }
                    i += 1;
                }
            } else {
                // Single-quoted string (can be multiline in Dart)
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' && i + 1 < len {
                        if bytes[i + 1] == b'\n' {
                            current_line += 1;
                        }
                        i += 2;
                        continue;
                    }
                    // String interpolation: ${...}
                    if bytes[i] == b'$' && i + 1 < len && bytes[i + 1] == b'{' {
                        i += 2;
                        let mut depth = 1u32;
                        while i < len && depth > 0 {
                            if bytes[i] == b'{' {
                                depth += 1;
                            }
                            if bytes[i] == b'}' {
                                depth -= 1;
                            }
                            if bytes[i] == b'\n' {
                                current_line += 1;
                            }
                            i += 1;
                        }
                        continue;
                    }
                    // $variable interpolation (no braces)
                    if bytes[i] == b'$' && i + 1 < len {
                        i += 1; // skip $
                                // Skip identifier
                        while i < len
                            && (bytes[i].is_ascii_alphanumeric()
                                || bytes[i] == b'_'
                                || bytes[i] == b'$')
                        {
                            i += 1;
                        }
                        continue;
                    }
                    if bytes[i] == b'\n' {
                        current_line += 1;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            // String content stripped — no token emitted
            continue;
        }

        // Identifier / keyword
        let ch = source[i..].chars().next().unwrap();
        if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
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
            // Handle 0x hex, 0b binary, 0o octal prefixes
            if bytes[i] == b'0' && i + 1 < len {
                let next = bytes[i + 1];
                if next == b'x' || next == b'X' {
                    // Hex
                    i += 2;
                    while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                        i += 1;
                    }
                    tokens.push(Token {
                        text: source[start..i].to_string(),
                        line: current_line,
                    });
                    continue;
                }
                if next == b'b' || next == b'B' {
                    // Binary
                    i += 2;
                    while i < len && (bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_') {
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
                if b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'e' || b == b'E' {
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

        // Multi-char operators (Dart-specific)
        let remaining = &source[i..];
        let next3: String = remaining.chars().take(3).collect();
        let next2: String = remaining.chars().take(2).collect();

        if next3.len() >= 3 {
            if matches!(
                next3.as_str(),
                "===" | "!==" | ">>=" | "<<=" | "??=" | ">>>=" | "... "
            )
                // Also handle ... when followed by non-. (spread operator)
                || (next3.starts_with("...") && next3.len() == 3)
            {
                let op_len = if next3.starts_with("...") {
                    3
                } else {
                    next3.len()
                };
                tokens.push(Token {
                    text: next3.clone(),
                    line: current_line,
                });
                i += op_len;
                continue;
            }
        }
        if next2.len() >= 2 {
            if matches!(
                next2.as_str(),
                "==" | "!="
                    | "<="
                    | ">="
                    | "&&"
                    | "||"
                    | "??"
                    | "=>"
                    | "++"
                    | "--"
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "~/"
                    | "~/="
                    | "%="
                    | "&="
                    | "|="
                    | "^="
                    | "<<"
                    | ">>"
                    | "?."
                    | "!."
                    | ".."
            ) {
                tokens.push(Token {
                    text: next2.clone(),
                    line: current_line,
                });
                i += next2.len();
                continue;
            }
        }

        // Single char token
        tokens.push(Token {
            text: ch.to_string(),
            line: current_line,
        });
        i += ch.len_utf8();
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_comments() {
        let src = "x = 1; // comment\ny = 2;";
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", "1", ";", "y", "=", "2", ";"]);
    }

    #[test]
    fn strips_doc_comments() {
        let src = "/// Doc comment\nx = 1;";
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", "1", ";"]);
    }

    #[test]
    fn strips_block_comments() {
        let src = "x /* foo */ = 1;";
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", "1", ";"]);
    }

    #[test]
    fn strips_strings() {
        let src = r#"x = "hello"; y = 'world';"#;
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", ";", "y", "=", ";"]);
    }

    #[test]
    fn strips_raw_strings() {
        let src = r#"x = r"hello"; y = r'single';"#;
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", ";", "y", "=", ";"]);
    }

    #[test]
    fn strips_triple_quoted_strings() {
        let src = r#"x = """hello"""; y = '''world''';"#;
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", ";", "y", "=", ";"]);
    }

    #[test]
    fn strips_string_interpolation() {
        let src = r#"x = "hello ${name} world";"#;
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", ";"]);
    }

    #[test]
    fn preserves_line_numbers() {
        let src = "x = 1;\n// comment\ny = 2;";
        let toks = tokenize_dart(src);
        let y_line = toks.iter().find(|t| t.text == "y").unwrap().line;
        assert_eq!(y_line, 3);
    }

    #[test]
    fn dart_keywords() {
        let src = "class Foo extends Bar { }";
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["class", "Foo", "extends", "Bar", "{", "}"]);
    }

    #[test]
    fn dart_operators() {
        let src = "x ??= y; a?.b; c..d;";
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["x", "??=", "y", ";", "a", "?.", "b", ";", "c", "..", "d", ";"]
        );
    }

    #[test]
    fn null_aware_operators() {
        let src = "x ?? y; a?.b?.c;";
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["x", "??", "y", ";", "a", "?.", "b", "?.", "c", ";"]
        );
    }

    #[test]
    fn hex_numbers() {
        let src = "x = 0xFF; y = 0xABCD;";
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", "0xFF", ";", "y", "=", "0xABCD", ";"]);
    }

    #[test]
    fn flutter_widget() {
        let src = r#"
class MyWidget extends StatelessWidget {
  const MyWidget({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      child: Text('Hello'),
    );
  }
}
"#;
        let toks = tokenize_dart(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        // Should have class, identifiers, braces — no strings
        assert!(texts.contains(&"class"));
        assert!(texts.contains(&"MyWidget"));
        assert!(texts.contains(&"extends"));
        assert!(texts.contains(&"StatelessWidget"));
        assert!(texts.contains(&"Widget"));
        assert!(texts.contains(&"build"));
        // 'Hello' string should be stripped
        assert!(!texts.contains(&"Hello"));
    }
}
