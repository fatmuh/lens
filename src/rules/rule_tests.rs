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
        no_eval::NoEval, no_new_func::NoNewFunc, no_script_url::NoScriptUrl,
        no_unsafe_finally::NoUnsafeFinally, no_dupe_keys::NoDupeKeys,
        no_fallthrough::NoFallthrough, no_self_compare::NoSelfCompare,
        no_duplicate_imports::NoDuplicateImports,
        no_async_promise_executor::NoAsyncPromiseExecutor,
        require_await::RequireAwait, prefer_template::PreferTemplate,
        no_useless_concat::NoUselessConcat, no_negated_condition::NoNegatedCondition,
        no_lonely_if::NoLonelyIf, no_nested_ternary::NoNestedTernary,
        no_unneeded_ternary::NoUnneededTernary, no_html_link::NoHtmlLink,
        no_promise_all_in_loop::NoPromiseAllInLoop,
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
    // -----------------------------------------------------------------
    // Security
    // -----------------------------------------------------------------
    #[test]
    fn no_eval_flags_eval_call() {
        let r = NoEval;
        let (f, s) = ts_file("eval('1+1');");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_new_func_flags_function_ctor() {
        let r = NoNewFunc;
        let (f, s) = ts_file("const f = new Function('return 1');");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_script_url_flags_javascript_url() {
        let r = NoScriptUrl;
        let (f, s) = ts_file(r#"const u = "javascript:alert(1)";"#);
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_html_link_flags_dangerously_set_inner_html() {
        let r = NoHtmlLink;
        let mut f = ts_file(r#"<div dangerouslySetInnerHTML={{__html: x}} />"#).0;
        f.language = Some(Language::Tsx);
        f.path = PathBuf::from("test.tsx");
        assert_eq!(r.check(&f, r#"<div dangerouslySetInnerHTML={{__html: x}} />"#).len(), 1);
    }

    // -----------------------------------------------------------------
    // Correctness
    // -----------------------------------------------------------------
    #[test]
    fn no_async_promise_executor_flags_async() {
        let r = NoAsyncPromiseExecutor;
        let (f, s) = ts_file("new Promise(async (resolve) => { resolve(1); });");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_unsafe_finally_flags_return() {
        let r = NoUnsafeFinally;
        let (f, s) = ts_file("try { x(); } finally { return 1; }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_fallthrough_flags_missing_break() {
        let r = NoFallthrough;
        let (f, s) = ts_file("switch (x) { case 1: foo(); case 2: bar(); break; }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_dupe_keys_flags_duplicate() {
        let r = NoDupeKeys;
        let (f, s) = ts_file("const o = { a: 1, a: 2 };");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_self_compare_flags_x_eq_x() {
        let r = NoSelfCompare;
        let (f, s) = ts_file("if (x === x) { }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_duplicate_imports_flags_same_source() {
        let r = NoDuplicateImports;
        let (f, s) = ts_file(r#"import { a } from "x"; import { b } from "x";"#);
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    // -----------------------------------------------------------------
    // Best practices
    // -----------------------------------------------------------------
    #[test]
    fn require_await_flags_async_without_await() {
        let r = RequireAwait;
        let (f, s) = ts_file("async function f() { return 1; }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn require_await_ignores_async_with_await() {
        let r = RequireAwait;
        let (f, s) = ts_file("async function f() { return await x; }");
        assert!(r.check(&f, &s).is_empty());
    }

    // -----------------------------------------------------------------
    // Style
    // -----------------------------------------------------------------
    #[test]
    fn prefer_template_flags_string_concat() {
        let r = PreferTemplate;
        let (f, s) = ts_file(r#"const s = "Hello " + name;"#);
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_useless_concat_flags_two_literals() {
        let r = NoUselessConcat;
        let (f, s) = ts_file(r#"const s = "a" + "b";"#);
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_negated_condition_flags_if_not_with_else() {
        let r = NoNegatedCondition;
        let (f, s) = ts_file("if (!x) { a(); } else { b(); }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_lonely_if_flags_if_in_else() {
        let r = NoLonelyIf;
        let (f, s) = ts_file("if (x) { a(); } else { if (y) { b(); } }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_nested_ternary_flags_nested() {
        let r = NoNestedTernary;
        let (f, s) = ts_file("const x = a ? b : (c ? d : e);");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_unneeded_ternary_flags_true_false() {
        let r = NoUnneededTernary;
        let (f, s) = ts_file("const x = cond ? true : false;");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_promise_all_in_loop_flags_loop_use() {
        let r = NoPromiseAllInLoop;
        let (f, s) = ts_file("for (const x of items) { await Promise.all([p(x)]); }");
        assert!(r.check(&f, &s).len() >= 1, "expected at least 1 issue, got none");
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
        assert!(reg.rules().len() >= 30, "expected at least 30 built-in rules, got {}", reg.rules().len());
    }
}
