//! NOSONAR support.
//!
//! SonarQube recognizes a comment that tells the analyzer to skip that line.
//! We support the same convention in Lens. The exact comment marker depends
//! on the language:
//!
//!   - `// NOSONAR`     — C, C++, C#, Java, Go, JS, TS, Rust, Kotlin, Swift, Scala
//!   - `# NOSONAR`      — Python, Ruby, Shell, YAML, TOML, PHP
//!   - `-- NOSONAR`     — SQL, Lua, Haskell
//!   - `/* NOSONAR */`  — CSS, SCSS, PHP
//!   - `<!-- NOSONAR -->` — HTML, XML
//!
//! Future versions may also support the `// NOSONAR(n)` variant used to
//! reference a specific rule.

use std::collections::HashSet;

use once_cell::sync::Lazy;
use regex::Regex;

use super::language::{Language, NosonarStyle};

/// Pre-compiled regexes per style. We compile once at startup.
static RE_LINE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:^|\s)(?://[!/]?|--)\s*NOSONAR\b").unwrap());
static RE_HASH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:^|\s)#\s*NOSONAR\b").unwrap());
static RE_BLOCK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:/\*|<!--)\s*NOSONAR\b").unwrap());

/// Returns true if the given line contains a NOSONAR marker for the given
/// language. The match is whitespace-tolerant and case-insensitive.
pub fn is_marked(line: &str, lang: Option<Language>) -> bool {
    let Some(lang) = lang else { return RE_HASH.is_match(line) };
    for style in lang.nosonar_styles() {
        let re = match style {
            NosonarStyle::Line => &*RE_LINE,
            NosonarStyle::Hash => &*RE_HASH,
            NosonarStyle::DashDash => &*RE_LINE, // `//`, `--`, `#` all handled by RE_LINE
            NosonarStyle::Block => &*RE_BLOCK,
        };
        if re.is_match(line) {
            return true;
        }
    }
    false
}

/// A pre-computed set of NOSONAR-marked line numbers for a single file.
#[derive(Debug, Default, Clone)]
pub struct NosonarMap {
    /// 1-indexed line numbers that contain a NOSONAR marker.
    marked: HashSet<u32>,
}

impl NosonarMap {
    pub fn from_source(source: &str, lang: Option<Language>) -> Self {
        let mut m = NosonarMap::default();
        for (idx, line) in source.lines().enumerate() {
            if is_marked(line, lang) {
                m.marked.insert((idx as u32) + 1);
            }
        }
        m
    }

    /// Returns `true` if the given (1-indexed) line is marked NOSONAR.
    #[allow(dead_code)] // used in Phase 2 rule engine
    pub fn is_marked(&self, line: u32) -> bool {
        self.marked.contains(&line)
    }

    pub fn count(&self) -> usize {
        self.marked.len()
    }

    #[allow(dead_code)] // used in Phase 2 for reporting
    pub fn lines(&self) -> impl Iterator<Item = u32> + '_ {
        let mut v: Vec<u32> = self.marked.iter().copied().collect();
        v.sort_unstable();
        v.into_iter()
    }
}

/// Count NOSONAR markers in a file's content (helper for Phase 0 reporting).
pub fn count(source: &str, lang: Option<Language>) -> usize {
    NosonarMap::from_source(source, lang).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_style() {
        assert!(is_marked("    // NOSONAR", Some(Language::Rust)));
        assert!(is_marked("x = 1; // NOSONAR", Some(Language::Java)));
        assert!(!is_marked("x = 1; // regular", Some(Language::Rust)));
    }

    #[test]
    fn hash_style() {
        assert!(is_marked("x = 1  # NOSONAR", Some(Language::Python)));
        assert!(is_marked("#NOSONAR", Some(Language::Ruby)));
        assert!(!is_marked("# todo", Some(Language::Python)));
    }

    #[test]
    fn block_style() {
        assert!(is_marked("<!-- NOSONAR -->", Some(Language::Html)));
        assert!(is_marked("/* NOSONAR */", Some(Language::Css)));
    }

    #[test]
    fn case_insensitive() {
        assert!(is_marked("// nosonar", Some(Language::Rust)));
        assert!(is_marked("// Nosonar", Some(Language::Go)));
    }

    #[test]
    fn map_collects_lines() {
        let src = "a\nb // NOSONAR\nc\n# NOSONAR\n";
        let m = NosonarMap::from_source(src, Some(Language::Rust));
        // Lines 2 (line-style) is marked; line 4 is `# NOSONAR` which is NOT
        // the Rust style, so it shouldn't be counted for Rust.
        assert!(m.is_marked(2));
        assert!(!m.is_marked(4));
        assert_eq!(m.count(), 1);
    }
}
