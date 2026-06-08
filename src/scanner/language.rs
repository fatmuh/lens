//! Language detection by file extension.
//!
//! Used to (a) pick the right tree-sitter grammar in later phases and (b) group
//! files in reports. This is intentionally lightweight — we don't do
//! content-based detection in Phase 0.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Tsx,
    Jsx,
    Python,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Kotlin,
    Swift,
    Scala,
    Bash,
    Sql,
    Html,
    Css,
    Scss,
    Json,
    Yaml,
    Toml,
    Markdown,
    Other,
}

impl Language {
    /// Returns the canonical extension(s) that map to this language.
    pub fn extensions(self) -> &'static [&'static str] {
        use Language::*;
        match self {
            Rust => &["rs"],
            TypeScript => &["ts", "mts", "cts"],
            JavaScript => &["js", "mjs", "cjs"],
            Tsx => &["tsx"],
            Jsx => &["jsx"],
            Python => &["py", "pyi"],
            Go => &["go"],
            Java => &["java"],
            C => &["c", "h"],
            Cpp => &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
            CSharp => &["cs"],
            Ruby => &["rb"],
            Php => &["php"],
            Kotlin => &["kt", "kts"],
            Swift => &["swift"],
            Scala => &["scala", "sc"],
            Bash => &["sh", "bash", "zsh"],
            Sql => &["sql"],
            Html => &["html", "htm"],
            Css => &["css"],
            Scss => &["scss", "sass"],
            Json => &["json"],
            Yaml => &["yaml", "yml"],
            Toml => &["toml"],
            Markdown => &["md", "markdown"],
            Other => &[],
        }
    }

    /// What NOSONAR comment prefix to look for. `None` means this language
    /// has no NOSONAR convention (use Python `#` style as a sensible default).
    pub fn nosonar_styles(self) -> &'static [NosonarStyle] {
        use Language::*;
        use NosonarStyle::*;
        match self {
            Rust | TypeScript | Tsx | JavaScript | Jsx | Go | Java | C | Cpp | CSharp | Kotlin
            | Swift | Scala => &[Line],
            Python | Ruby | Bash | Yaml | Toml => &[Hash],
            Sql => &[DashDash],
            Html => &[Block],
            Css | Scss => &[Block],
            Php => &[Hash, Block],
            Json | Markdown => &[],
            Other => &[Hash],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NosonarStyle {
    /// `// NOSONAR`
    Line,
    /// `# NOSONAR`
    Hash,
    /// `-- NOSONAR`
    DashDash,
    /// `/* NOSONAR */` or `<!-- NOSONAR -->`
    Block,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Language::Rust => "Rust",
            Language::TypeScript => "TypeScript",
            Language::JavaScript => "JavaScript",
            Language::Tsx => "TSX",
            Language::Jsx => "JSX",
            Language::Python => "Python",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::CSharp => "C#",
            Language::Ruby => "Ruby",
            Language::Php => "PHP",
            Language::Kotlin => "Kotlin",
            Language::Swift => "Swift",
            Language::Scala => "Scala",
            Language::Bash => "Shell",
            Language::Sql => "SQL",
            Language::Html => "HTML",
            Language::Css => "CSS",
            Language::Scss => "SCSS",
            Language::Json => "JSON",
            Language::Yaml => "YAML",
            Language::Toml => "TOML",
            Language::Markdown => "Markdown",
            Language::Other => "Other",
        };
        f.write_str(s)
    }
}

impl FromStr for Language {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "rust" | "rs" => Language::Rust,
            "typescript" | "ts" => Language::TypeScript,
            "javascript" | "js" => Language::JavaScript,
            "tsx" => Language::Tsx,
            "jsx" => Language::Jsx,
            "python" | "py" => Language::Python,
            "go" | "golang" => Language::Go,
            "java" => Language::Java,
            "c" => Language::C,
            "cpp" | "c++" | "cxx" => Language::Cpp,
            "csharp" | "cs" | "c#" => Language::CSharp,
            "ruby" | "rb" => Language::Ruby,
            "php" => Language::Php,
            "kotlin" | "kt" => Language::Kotlin,
            "swift" => Language::Swift,
            "scala" => Language::Scala,
            "bash" | "sh" | "shell" => Language::Bash,
            "sql" => Language::Sql,
            "html" => Language::Html,
            "css" => Language::Css,
            "scss" | "sass" => Language::Scss,
            "json" => Language::Json,
            "yaml" | "yml" => Language::Yaml,
            "toml" => Language::Toml,
            "markdown" | "md" => Language::Markdown,
            _ => Language::Other,
        })
    }
}

/// Detect the language of a file by extension. Returns `None` for files
/// without a recognized extension.
pub fn detect(path: &Path) -> Option<Language> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    for lang in ALL_LANGUAGES {
        if lang.extensions().iter().any(|e| *e == ext) {
            return Some(*lang);
        }
    }
    None
}

/// All known languages in stable order.
pub const ALL_LANGUAGES: &[Language] = &[
    Language::Rust,
    Language::TypeScript,
    Language::JavaScript,
    Language::Tsx,
    Language::Jsx,
    Language::Python,
    Language::Go,
    Language::Java,
    Language::C,
    Language::Cpp,
    Language::CSharp,
    Language::Ruby,
    Language::Php,
    Language::Kotlin,
    Language::Swift,
    Language::Scala,
    Language::Bash,
    Language::Sql,
    Language::Html,
    Language::Css,
    Language::Scss,
    Language::Json,
    Language::Yaml,
    Language::Toml,
    Language::Markdown,
    Language::Other,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rust() {
        assert_eq!(detect(Path::new("src/main.rs")), Some(Language::Rust));
    }

    #[test]
    fn detect_tsx() {
        assert_eq!(detect(Path::new("ui/Button.tsx")), Some(Language::Tsx));
    }

    #[test]
    fn unknown_extension_is_none() {
        assert_eq!(detect(Path::new("README")), None);
    }
}
