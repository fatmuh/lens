//! Unit tests for the built-in rules.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::analyzer::FileAnalysis;
    use crate::rules::builtin::{
        no_explicit_any::NoExplicitAny, no_var::NoVar, no_eqeqeq::NoEqeqeq,
        no_throw_literal::NoThrowLiteral, no_empty_function::NoEmptyFunction,
        no_unreachable::NoUnreachable, no_unused_vars::NoUnusedVars,
        max_params::MaxParams, no_implicit_any::NoImplicitAny,
        no_magic_numbers::NoMagicNumbers, no_console::NoConsole,
    };
    use crate::rules::{Issue, Rule, Severity};
    use crate::scanner::language::Language;

    /// Helper: build a partial `FileAnalysis` for rule tests.
    fn ts_file(source: &str) -> (FileAnalysis, String) {
        let path = PathBuf::from("test.ts");
        let analysis = FileAnalysis {
            path: path.clone(),
            language: Some(Language::TypeScript),
            analyzed: true,
            metrics: None,
            tokens: None,
            nosonar_count: 0,
            issues: Vec::new(),
        };
        (analysis, source.to_string())
    }

    /// Helper: count issues by rule_id.
    fn count_by_rule(issues: &[Issue]) -> std::collections::HashMap<String, usize> {
        let mut m = std::collections::HashMap::new();
        for i in issues {
            *m.entry(i.rule_id.clone()).or_default() += 1;
        }
        m
    }

    // -----------------------------------------------------------------
    // no-explicit-any
    // -----------------------------------------------------------------
    #[test]
    fn no_explicit_any_finds_any() {
        let r = NoExplicitAny;
        let (f, s) = ts_file("function f(x: any) { return x; }");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1, "expected 1 `any` issue, got {}", issues.len());
        assert_eq!(issues[0].rule_id, "no-explicit-any");
        assert_eq!(issues[0].severity, Severity::Major);
    }

    #[test]
    fn no_explicit_any_ignores_typed() {
        let r = NoExplicitAny;
        let (f, s) = ts_file("function f(x: number) { return x; }");
        let issues = r.check(&f, &s);
        assert!(issues.is_empty());
    }

    // -----------------------------------------------------------------
    // no-var
    // -----------------------------------------------------------------
    #[test]
    fn no_var_finds_var() {
        let r = NoVar;
        let (f, s) = ts_file("var x = 5;");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "no-var");
    }

    #[test]
    fn no_var_ignores_let_const() {
        let r = NoVar;
        let (f, s) = ts_file("let x = 5; const y = 6;");
        let issues = r.check(&f, &s);
        assert!(issues.is_empty());
    }

    // -----------------------------------------------------------------
    // no-eqeqeq
    // -----------------------------------------------------------------
    #[test]
    fn no_eqeqeq_finds_double_equals() {
        let r = NoEqeqeq;
        let (f, s) = ts_file("if (a == b) { }");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("==="));
    }

    #[test]
    fn no_eqeqeq_ignores_triple_equals() {
        let r = NoEqeqeq;
        let (f, s) = ts_file("if (a === b) { }");
        let issues = r.check(&f, &s);
        assert!(issues.is_empty());
    }

    // -----------------------------------------------------------------
    // no-throw-literal
    // -----------------------------------------------------------------
    #[test]
    fn no_throw_literal_flags_string() {
        let r = NoThrowLiteral;
        let (f, s) = ts_file("throw 'error';");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn no_throw_literal_ignores_error() {
        let r = NoThrowLiteral;
        let (f, s) = ts_file("throw new Error('boom');");
        let issues = r.check(&f, &s);
        assert!(issues.is_empty());
    }

    // -----------------------------------------------------------------
    // no-empty-function
    // -----------------------------------------------------------------
    #[test]
    fn no_empty_function_flags_empty() {
        let r = NoEmptyFunction;
        let (f, s) = ts_file("function noop() {}");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn no_empty_function_ignores_non_empty() {
        let r = NoEmptyFunction;
        let (f, s) = ts_file("function withBody() { return 1; }");
        let issues = r.check(&f, &s);
        assert!(issues.is_empty());
    }

    // -----------------------------------------------------------------
    // no-unreachable
    // -----------------------------------------------------------------
    #[test]
    fn no_unreachable_flags_dead_code() {
        let r = NoUnreachable;
        let (f, s) = ts_file(
            "function f() { return 1; console.log('never'); }",
        );
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Critical);
    }

    // -----------------------------------------------------------------
    // no-unused-vars
    // -----------------------------------------------------------------
    #[test]
    fn no_unused_vars_flags_unused_param() {
        let r = NoUnusedVars;
        let (f, s) = ts_file("function f(unused: number, used: number) { return used; }");
        let issues = r.check(&f, &s);
        let counts = count_by_rule(&issues);
        assert_eq!(counts.get("no-unused-vars").copied().unwrap_or(0), 1, "got counts {:?}", counts);
    }

    // -----------------------------------------------------------------
    // max-params
    // -----------------------------------------------------------------
    #[test]
    fn max_params_flags_too_many() {
        let r = MaxParams;
        let (f, s) = ts_file("function f(a: number, b: number, c: number, d: number, e: number, f: number, g: number) {}");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1);
    }

    // -----------------------------------------------------------------
    // no-implicit-any
    // -----------------------------------------------------------------
    #[test]
    fn no_implicit_any_flags_untyped_param() {
        let r = NoImplicitAny;
        let (f, s) = ts_file("function f(x) { return x; }");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1);
    }

    // -----------------------------------------------------------------
    // no-magic-numbers
    // -----------------------------------------------------------------
    #[test]
    fn no_magic_numbers_flags_non_allowed() {
        let r = NoMagicNumbers;
        let (f, s) = ts_file("console.log(42);");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1, "got {} issues", issues.len());
    }

    #[test]
    fn no_magic_numbers_allows_named_const() {
        // const x = 42 — the 42 is already named via `x`.
        let r = NoMagicNumbers;
        let (f, s) = ts_file("const x = 42;");
        let issues = r.check(&f, &s);
        assert!(issues.is_empty());
    }

    #[test]
    fn no_magic_numbers_allows_common() {
        let r = NoMagicNumbers;
        let (f, s) = ts_file("if (x > 0) { return 1; }");
        let issues = r.check(&f, &s);
        assert!(issues.is_empty());
    }

    // -----------------------------------------------------------------
    // no-console (skips .spec/.test files; here we pass a non-test name)
    // -----------------------------------------------------------------
    #[test]
    fn no_console_flags_console_call() {
        let r = NoConsole;
        let mut f = ts_file("console.log('hi');").0;
        f.path = PathBuf::from("src/foo.ts"); // not a test file
        let issues = r.check(&f, "console.log('hi');");
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn no_console_skips_test_files() {
        let r = NoConsole;
        let mut f = ts_file("console.log('hi');").0;
        f.path = PathBuf::from("src/foo.spec.ts");
        let issues = r.check(&f, "console.log('hi');");
        assert!(issues.is_empty(), "test files should skip the rule");
    }

    // -----------------------------------------------------------------
    // Registry: every rule has required metadata
    // -----------------------------------------------------------------
    #[test]
    fn all_rules_have_unique_ids() {
        let reg = crate::rules::RuleRegistry::default_registry();
        let mut ids = std::collections::HashSet::new();
        for r in reg.rules() {
            assert!(ids.insert(r.id()), "duplicate rule id: {}", r.id());
            assert!(!r.name().is_empty(), "{} has no name", r.id());
            assert!(!r.description().is_empty(), "{} has no description", r.id());
        }
        assert!(reg.rules().len() >= 10, "expected at least 10 built-in rules");
    }
}
