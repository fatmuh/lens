//! Go source tokenizer for duplication detection (CPD).
//!
//! Matches SonarQube's CPD tokenizer behavior for Go:
//! - Strips line comments (//)
//! - Strips block comments (/* ... */)
//! - Strips string literals (interpreted "", raw ``)
//! - Strips rune literals ('x')
//! - Keeps identifiers, keywords, numbers, operators, punctuation

use once_cell::sync::Lazy;
use regex::Regex;

use super::tokenize::Token;

static RE_IDENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z_$][A-Za-z0-9_$]*").unwrap());

/// Go keywords that SonarQube CPD treats as significant tokens.
const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

/// Tokenize Go source for CPD duplication detection.
/// Returns a flat stream of significant tokens (comments and strings stripped).
pub fn tokenize_go(source: &str) -> Vec<Token> {
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

        // Line comment: // (also ///)
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

        // Raw string literal: `...` (backtick, no escape sequences, can span lines)
        if c == b'`' {
            i += 1;
            while i < len && bytes[i] != b'`' {
                if bytes[i] == b'\n' {
                    current_line += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1; // skip closing backtick
            }
            continue;
        }

        // Interpreted string literal: "..." (with \ escapes)
        if c == b'"' {
            i += 1;
            while i < len && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2; // skip escape sequence
                    continue;
                }
                if bytes[i] == b'\n' {
                    // Unescaped newline in string is illegal in Go, but handle gracefully
                    current_line += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1; // skip closing quote
            }
            continue;
        }

        // Rune literal: 'x' or '\n' etc.
        if c == b'\'' {
            i += 1;
            while i < len && bytes[i] != b'\'' {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\n' {
                    current_line += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1; // skip closing quote
            }
            continue;
        }

        // Identifier / keyword
        let ch = source[i..].chars().next().unwrap();
        if ch.is_ascii_alphabetic() || ch == '_' {
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
                    // Hex: 0xFF
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
                    // Binary: 0b1010
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
                if next == b'o' || next == b'O' {
                    // Octal: 0o755
                    i += 2;
                    while i < len && (bytes[i] >= b'0' && bytes[i] <= b'7' || bytes[i] == b'_') {
                        i += 1;
                    }
                    tokens.push(Token {
                        text: source[start..i].to_string(),
                        line: current_line,
                    });
                    continue;
                }
            }
            // Decimal / float / imaginary
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

        // Multi-char operators (Go-specific)
        let remaining = &source[i..];
        let next3: String = remaining.chars().take(3).collect();
        let next2: String = remaining.chars().take(2).collect();

        // 3-char operators
        if next3.len() >= 3 {
            if matches!(
                next3.as_str(),
                "...=" | "===" // ...= is not real Go, but handle ... spread
            ) || next3.as_str() == "..."
            {
                let op_len = 3;
                tokens.push(Token {
                    text: next3.clone(),
                    line: current_line,
                });
                i += op_len;
                continue;
            }
        }

        // 2-char operators
        if next2.len() >= 2 {
            if matches!(
                next2.as_str(),
                "==" | "!="
                    | "<="
                    | ">="
                    | "&&"
                    | "||"
                    | "<<"
                    | ">>"
                    | "&^"
                    | ":="
                    | "->"
                    | "<-"
                    | "++"
                    | "--"
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "%="
                    | "&="
                    | "|="
                    | "^="
                    | "<<="
                    | ">>="
                    | "&^="
            ) {
                // Check for 3-char compound assignments first
                if next3.len() >= 3 && matches!(next3.as_str(), "<<=" | ">>=" | "&^=") {
                    tokens.push(Token {
                        text: next3.clone(),
                        line: current_line,
                    });
                    i += 3;
                    continue;
                }
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
        let src = "x := 1 // comment\ny := 2";
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", ":=", "1", "y", ":=", "2"]);
    }

    #[test]
    fn strips_block_comments() {
        let src = "x /* foo */ := 1";
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", ":=", "1"]);
    }

    #[test]
    fn strips_interpreted_strings() {
        let src = r#"x := "hello world""#;
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", ":="]);
    }

    #[test]
    fn strips_raw_strings() {
        let src = "x := `hello\nworld`";
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", ":="]);
    }

    #[test]
    fn strips_rune_literals() {
        let src = "x := 'a'";
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", ":="]);
    }

    #[test]
    fn go_keywords() {
        let src = "func main() { }";
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["func", "main", "(", ")", "{", "}"]);
    }

    #[test]
    fn go_operators() {
        let src = "x := y + 1; a := b * 2; ch <- val";
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["x", ":=", "y", "+", "1", ";", "a", ":=", "b", "*", "2", ";", "ch", "<-", "val"]
        );
    }

    #[test]
    fn preserves_line_numbers() {
        let src = "x := 1\n// comment\ny := 2";
        let toks = tokenize_go(src);
        let y_line = toks.iter().find(|t| t.text == "y").unwrap().line;
        assert_eq!(y_line, 3);
    }

    #[test]
    fn hex_numbers() {
        let src = "x := 0xFF; y := 0xABCD";
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", ":=", "0xFF", ";", "y", ":=", "0xABCD"]);
    }

    #[test]
    fn struct_definition() {
        let src = r#"
type Server struct {
    Name string
    Port int
}
"#;
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert!(texts.contains(&"type"));
        assert!(texts.contains(&"Server"));
        assert!(texts.contains(&"struct"));
        assert!(texts.contains(&"string"));
        assert!(texts.contains(&"int"));
    }

    #[test]
    fn channel_operators() {
        let src = "ch <- x; y := <-ch";
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["ch", "<-", "x", ";", "y", ":=", "<-", "ch"]);
    }

    #[test]
    fn complex_go_code() {
        let src = r#"
package main

import "fmt"

// User represents a user
type User struct {
    Name string
    Age  int
}

func NewUser(name string, age int) *User {
    return &User{Name: name, Age: age}
}

func (u *User) Greet() string {
    return fmt.Sprintf("Hello, %s!", u.Name)
}
"#;
        let toks = tokenize_go(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        // Comment should be stripped
        assert!(!texts.contains(&"represents"));
        assert!(!texts.contains(&"user"));
        // But keywords and identifiers preserved
        assert!(texts.contains(&"package"));
        assert!(texts.contains(&"func"));
        assert!(texts.contains(&"type"));
        assert!(texts.contains(&"struct"));
        assert!(texts.contains(&"return"));
    }
}
