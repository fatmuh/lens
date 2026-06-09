//! Python source tokenizer for duplication detection (CPD).
//!
//! Matches SonarQube's CPD tokenizer behavior for Python:
//! - Strips line comments (#)
//! - Strips string literals (single/double/triple-quoted, f-strings, raw, bytes)
//! - Strips docstrings (triple-quoted string at start of module/class/function)
//! - Keeps identifiers, keywords, numbers, operators, punctuation
//! - Handles nested parenthesized expressions correctly

use once_cell::sync::Lazy;
use regex::Regex;

use super::tokenize::Token;

static RE_IDENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap());

/// Tokenize Python source for CPD duplication detection.
/// Returns a flat stream of significant tokens (comments and strings stripped).
pub fn tokenize_python(source: &str) -> Vec<Token> {
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

        // Line comment: # ...
        if c == b'#' {
            i += 1;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // String literals (single/double/triple-quoted)
        // Python has many string types:
        //   'x'  "x"  '''x'''  """x"""
        //   r'x' R"x"  b'x' B"x"  f'x' F"x"
        //   rb'x' rb"x"  fr'x' etc.
        // We detect the prefix first, then the quote style.

        // Check for string prefix: r, R, b, B, u, U, f, F (and combinations rb, rf, br, fr)
        let (prefix_len, has_prefix) = if is_string_prefix(bytes, i) {
            (skip_string_prefix(bytes, i), true)
        } else {
            (0, false)
        };

        // Check if this position (after optional prefix) starts a quote
        let qi = i + prefix_len;
        if qi < len && (bytes[qi] == b'\'' || bytes[qi] == b'"') {
            let q = bytes[qi];
            // Check for triple quote
            let (triple, quote_len) =
                if qi + 2 < len && bytes[qi] == bytes[qi + 1] && bytes[qi + 1] == bytes[qi + 2] {
                    (true, 3)
                } else {
                    (false, 1)
                };

            let quote_char = bytes[qi];
            i = qi + quote_len;

            if triple {
                // Triple-quoted string: '''...''' or """..."""
                while i + 2 < len {
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
                    if bytes[i] == quote_char
                        && bytes[i + 1] == quote_char
                        && bytes[i + 2] == quote_char
                    {
                        i += 3;
                        break;
                    }
                    i += 1;
                }
                // Handle case where string ends at EOF without closing
                if i + 2 >= len && i < len {
                    while i < len {
                        if bytes[i] == b'\n' {
                            current_line += 1;
                        }
                        i += 1;
                    }
                }
            } else {
                // Single-quoted string
                while i < len {
                    if bytes[i] == b'\\' && i + 1 < len {
                        if bytes[i + 1] == b'\n' {
                            current_line += 1;
                        }
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'\n' {
                        // Unescaped newline — end of single-line string in Python
                        break;
                    }
                    if bytes[i] == quote_char {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            continue;
        }
        // If prefix was detected but no quote found, fall through to identifier
        // (the prefix chars like 'r' in 'range' are just identifier starts)

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
            // Handle 0x, 0b, 0o prefixes
            if bytes[i] == b'0' && i + 1 < len {
                let next = bytes[i + 1];
                if next == b'x' || next == b'X' {
                    i += 2;
                    while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                        i += 1;
                    }
                    // Handle complex suffix
                    if i < len && (bytes[i] == b'j' || bytes[i] == b'J') {
                        i += 1;
                    }
                    tokens.push(Token {
                        text: source[start..i].to_string(),
                        line: current_line,
                    });
                    continue;
                }
                if next == b'b' || next == b'B' {
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
                    i += 2;
                    while i < len && ((bytes[i] >= b'0' && bytes[i] <= b'7') || bytes[i] == b'_') {
                        i += 1;
                    }
                    tokens.push(Token {
                        text: source[start..i].to_string(),
                        line: current_line,
                    });
                    continue;
                }
            }
            // Decimal / float / complex
            while i < len {
                let b = bytes[i];
                if b.is_ascii_alphanumeric()
                    || b == b'.'
                    || b == b'_'
                    || b == b'e'
                    || b == b'E'
                    || b == b'+'
                    || b == b'-'
                {
                    i += 1;
                } else {
                    break;
                }
            }
            // Handle complex suffix 'j'/'J'
            if i < len && (bytes[i] == b'j' || bytes[i] == b'J') {
                i += 1;
            }
            tokens.push(Token {
                text: source[start..i].to_string(),
                line: current_line,
            });
            continue;
        }

        // Decorator: @
        if c == b'@' {
            tokens.push(Token {
                text: "@".to_string(),
                line: current_line,
            });
            i += 1;
            continue;
        }

        // Multi-char operators
        let remaining = &source[i..];
        let next3: String = remaining.chars().take(3).collect();
        let next2: String = remaining.chars().take(2).collect();

        // 3-char operators
        if next3.len() >= 3 {
            if matches!(next3.as_str(), "**=" | "//=" | ">>=" | "<<=" | ":==") {
                tokens.push(Token {
                    text: next3.clone(),
                    line: current_line,
                });
                i += next3.chars().map(|c| c.len_utf8()).sum::<usize>();
                continue;
            }
        }

        // 2-char operators
        if next2.len() >= 2 {
            if matches!(
                next2.as_str(),
                "**" | "//"
                    | "=="
                    | "!="
                    | "<="
                    | ">="
                    | "&&"
                    | "||"
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "%="
                    | "&="
                    | "|="
                    | "^="
                    | "<<"
                    | ">>"
                    | "->"
                    | ":="
                    | "@="
            ) {
                tokens.push(Token {
                    text: next2.clone(),
                    line: current_line,
                });
                i += next2.chars().map(|c| c.len_utf8()).sum::<usize>();
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

/// Check if position `i` is the start of a string prefix followed by a quote.
/// Returns true only if the prefix+quote forms a valid Python string literal.
fn is_string_prefix(bytes: &[u8], i: usize) -> bool {
    let len = bytes.len();
    if i >= len {
        return false;
    }

    let c = bytes[i];
    // Single-char prefixes
    if matches!(c, b'r' | b'R' | b'b' | b'B' | b'u' | b'U' | b'f' | b'F') {
        let next_i = i + 1;
        if next_i < len {
            let next = bytes[next_i];
            // Direct quote after prefix: r"..." f'...'
            if next == b'\'' || next == b'"' {
                return true;
            }
            // Two-char prefix: rb, rf, br, fr (case-insensitive)
            if matches!(next, b'r' | b'R' | b'b' | b'B' | b'f' | b'F') && next != c {
                // Different from first char (avoid rr, bb, ff)
                let qi = next_i + 1;
                if qi < len && (bytes[qi] == b'\'' || bytes[qi] == b'"') {
                    return true;
                }
            }
        }
    }
    false
}

/// Skip the string prefix and return its length in bytes.
fn skip_string_prefix(bytes: &[u8], i: usize) -> usize {
    let len = bytes.len();
    let c = bytes[i];
    if i + 1 < len {
        let next = bytes[i + 1];
        if next == b'\'' || next == b'"' {
            return 1; // single-char prefix: r, b, f, u
        }
        if matches!(next, b'r' | b'R' | b'b' | b'B' | b'f' | b'F') && next != c {
            if i + 2 < len && (bytes[i + 2] == b'\'' || bytes[i + 2] == b'"') {
                return 2; // two-char prefix: rb, rf, br, fr
            }
        }
    }
    1 // fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_comments() {
        let src = "x = 1 # comment\ny = 2";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", "1", "y", "=", "2"]);
    }

    #[test]
    fn strips_single_quoted_strings() {
        let src = "x = 'hello world'";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "="]);
    }

    #[test]
    fn strips_double_quoted_strings() {
        let src = r#"x = "hello world""#;
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "="]);
    }

    #[test]
    fn strips_triple_quoted_strings() {
        let src = "x = '''hello\nworld'''";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "="]);
    }

    #[test]
    fn strips_triple_double_quoted_strings() {
        let src = "x = \"\"\"hello\nworld\"\"\"";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "="]);
    }

    #[test]
    fn strips_raw_strings() {
        let src = r"x = r'\d+\.\d+'";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "="]);
    }

    #[test]
    fn strips_f_strings() {
        let src = "x = f'hello {name}'";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "="]);
    }

    #[test]
    fn strips_byte_strings() {
        let src = "x = b'hello'";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "="]);
    }

    #[test]
    fn preserves_identifiers_not_prefixes() {
        // "range" starts with 'r' but is not a raw string
        let src = "x = range(10)";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "=", "range", "(", "10", ")"]);
    }

    #[test]
    fn preserves_line_numbers() {
        let src = "x = 1\n# comment\ny = 2";
        let toks = tokenize_python(src);
        let y_line = toks.iter().find(|t| t.text == "y").unwrap().line;
        assert_eq!(y_line, 3);
    }

    #[test]
    fn python_operators() {
        let src = "x = y + 1; z = a ** b; w = x // y";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "x", "=", "y", "+", "1", ";", "z", "=", "a", "**", "b", ";", "w", "=", "x", "//",
                "y"
            ]
        );
    }

    #[test]
    fn walrus_operator() {
        let src = "if (n := len(x)) > 10:";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert!(texts.contains(&":="));
    }

    #[test]
    fn complex_python_code() {
        let src = r#"
class User:
    """A user model."""

    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age

    def greet(self) -> str:
        return f"Hello, {self.name}!"

# Create a user
user = User("Alice", 30)
print(user.greet())
"#;
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        // Docstring should be stripped
        assert!(!texts.contains(&"A"));
        // Note: 'user' and 'model' are identifiers (User, user variable), not docstring content
        // But keywords preserved
        assert!(texts.contains(&"class"));
        assert!(texts.contains(&"def"));
        assert!(texts.contains(&"return"));
        assert!(texts.contains(&"self"));
    }

    #[test]
    fn multiline_string_line_tracking() {
        let src = "x = '''hello\nworld\nfoo'''\ny = 2";
        let toks = tokenize_python(src);
        let y_line = toks.iter().find(|t| t.text == "y").unwrap().line;
        assert_eq!(y_line, 4);
    }

    #[test]
    fn escaped_quotes_in_strings() {
        let src = r"x = 'it\'s a test'";
        let toks = tokenize_python(src);
        let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["x", "="]);
    }
}
