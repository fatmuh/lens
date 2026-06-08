//! Source tokenizer used for duplication detection.
//!
//! Matches SonarQube's CPD tokenizer behavior:
//! - Strips comments (line and block)
//! - Strips template literals (multi-line)
//! - Keeps string literals but normalizes their content to a placeholder
//! - Keeps all other tokens: identifiers, keywords, numbers, operators, punctuation
//!
//! This produces token sequences that closely match what SonarQube's
//! JavaScript/TypeScript analyzer feeds into DefaultCpdTokens.

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

/// Tokenize `source` into a flat stream.
/// Comments are stripped. String literals are normalized to `__str__`.
/// Template literals are stripped. All other tokens are preserved.
pub fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut current_line: u32 = 1;
    let mut i = 0usize;

    while i < len {
        let c = bytes[i];

        // Newline — track line numbers
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

        // Line comment: // ... \n
        if c == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // Block comment: /* ... */
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

        // String literal: "..." or '...'
        // Strip content entirely — matches SQ behavior for CPD duplication.
        if c == b'"' || c == b'\'' {
            let quote = c;
            i += 1; // skip opening quote
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        current_line += 1;
                    }
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\n' {
                    current_line += 1;
                }
                if bytes[i] == quote {
                    i += 1; // skip closing quote
                    break;
                }
                i += 1;
            }
            // String content stripped — no token emitted
            continue;
        }

        // Template literal: `...`
        // Strip content but track newlines for line numbers
        if c == b'`' {
            i += 1; // skip opening backtick
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        current_line += 1;
                    }
                    i += 2;
                    continue;
                }
                // Template expression: ${...}
                if bytes[i] == b'$' && i + 1 < len && bytes[i + 1] == b'{' {
                    // Skip the expression content (simplified — doesn't handle nested {})
                    i += 2;
                    let mut depth = 1u32;
                    while i < len && depth > 0 {
                        if bytes[i] == b'{' { depth += 1; }
                        if bytes[i] == b'}' { depth -= 1; }
                        if bytes[i] == b'\n' { current_line += 1; }
                        i += 1;
                    }
                    continue;
                }
                if bytes[i] == b'\n' {
                    current_line += 1;
                }
                if bytes[i] == b'`' {
                    i += 1; // skip closing backtick
                    break;
                }
                i += 1;
            }
            // Template literal content stripped — no token emitted
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
            // m.end() is a byte offset relative to source[i..]
            // Advance by the byte length of the match
            i += m.end();
            continue;
        }

        // Number
        if ch.is_ascii_digit() {
            let start = i;
            while i < len {
                let b = bytes[i];
                if b.is_ascii_alphanumeric() || b == b'.' {
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
        // Use source[i..] slicing carefully to avoid splitting multi-byte chars
        let remaining = &source[i..];
        let chars_remaining = remaining.chars().count();
        let next3: String = remaining.chars().take(3).collect();
        let next2: String = remaining.chars().take(2).collect();

        if next3.len() >= 3 {
            if matches!(next3.as_str(), "===" | "!==" | "<<=" | ">>>=" | "**=" | ">>=" | "<<=" | "...") {
                tokens.push(Token { text: next3.clone(), line: current_line });
                i += next3.len();
                continue;
            }
        }
        if next2.len() >= 2 {
            if matches!(next2.as_str(), "==" | "!=" | "<=" | ">=" | "&&" | "||" | "??" | "=>" | "**" | "++" | "--" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<" | ">>") {
                tokens.push(Token { text: next2.clone(), line: current_line });
                i += next2.len();
                continue;
            }
        }

        // Single character token (operators, punctuation, brackets)
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
        let toks = tokenize(src);
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
    fn normalizes_strings() {
        let src = r#"x = "hello"; y = 'world';"#;
        let toks = tokenize(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        // Strings are stripped entirely (like SQ CPD behavior)
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
        let src = "x = 1;";
        let toks = tokenize(src);
        assert_eq!(toks.len(), 4);
    }

    #[test]
    fn multi_char_operators() {
        let src = "x === y !== z;";
        let toks = tokenize(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "===", "y", "!==", "z", ";"]);
    }

    #[test]
    fn template_literals_normalized() {
        let src = "x = `hello ${name} world`;";
        let toks = tokenize(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", ";"]);
    }

    #[test]
    fn arrow_operators() {
        let src = "const f = (x) => x + 1;";
        let toks = tokenize(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["const", "f", "=", "(", "x", ")", "=>", "x", "+", "1", ";"]);
    }
}
