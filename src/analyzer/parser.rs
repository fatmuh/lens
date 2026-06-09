//! Tree-sitter parser helpers.
//!
//! For Phase 1 we only have TypeScript support. The pool is intentionally
//! simple: create a parser per file. Tree-sitter parser construction is
//! cheap (microseconds) and avoids the complexity of a shared pool.

use tree_sitter::{Language, Node, Parser, Tree};

use crate::scanner::language::Language as SrcLanguage;

/// Returns the tree-sitter `Language` for the given source language, or
/// `None` if it isn't supported yet.
pub fn get_language(lang: SrcLanguage) -> Option<Language> {
    match lang {
        SrcLanguage::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        SrcLanguage::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        SrcLanguage::Dart => Some(tree_sitter_dart::LANGUAGE.into()),
        SrcLanguage::Go => Some(tree_sitter_go::LANGUAGE.into()),
        SrcLanguage::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
        _ => None,
    }
}

/// Run `f` with a freshly created parser for the given language, applied to
/// `source`. Returns `None` if the language is unsupported.
pub fn with_parser<F, R>(lang: SrcLanguage, source: &str, f: F) -> Option<R>
where
    F: FnOnce(&Tree) -> R,
{
    let language = get_language(lang)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(source, None)?;
    Some(f(&tree))
}

/// Walk all descendants of a node, invoking `visitor` on each.
///
/// We avoid the higher-level cursor helpers because we want to control
/// recursion explicitly (cheaper for very large files).
pub fn visit_descendants<'a, F>(node: Node<'a>, mut visitor: F)
where
    F: FnMut(Node<'a>),
{
    fn walk<'a, F: FnMut(Node<'a>)>(node: Node<'a>, visitor: &mut F) {
        visitor(node);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk(child, visitor);
        }
    }
    walk(node, &mut visitor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::language::Language;

    #[test]
    fn parses_simple_typescript() {
        let source = "const x: number = 1;\nfunction foo() { return x; }\n";
        let result = with_parser(Language::TypeScript, source, |tree| {
            tree.root_node().kind().to_string()
        });
        assert_eq!(result, Some("program".to_string()));
    }

    #[test]
    fn returns_none_for_unsupported() {
        let result = with_parser(Language::Python, "def main(): pass", |_| ());
        assert!(result.is_none());
    }
}
