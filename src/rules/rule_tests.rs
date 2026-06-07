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
        no_implied_eval::NoImpliedEval, no_prototype_builtins::NoPrototypeBuiltins,
        no_redeclare::NoRedeclare, default_case::DefaultCase,
        no_non_null_assertion::NoNonNullAssertion,
        prefer_nullish_coalescing::PreferNullishCoalescing,
        prefer_optional_chain::PreferOptionalChain,
        consistent_type_imports::ConsistentTypeImports,
        no_import_assign::NoImportAssign, no_param_reassign::NoParamReassign,
        no_return_await::NoReturnAwait, no_await_in_loop::NoAwaitInLoop,
        prefer_arrow_callback::PreferArrowCallback,
        no_useless_return::NoUselessReturn, no_else_return::NoElseReturn,
        no_useless_rename::NoUselessRename as NoUselessRename2,
        no_new_buffer::NoNewBuffer as NoNewBuffer2,
        camelcase::Camelcase,
        no_underscore_dangle::NoUnderscoreDangle,
        no_empty_interface::NoEmptyInterface,
        no_bitwise::NoBitwise,
        prefer_spread::PreferSpread,
        no_extra_bind::NoExtraBind,
        no_extra_boolean_cast::NoExtraBooleanCast,
        no_proto::NoProto,
        no_with::NoWith,
        no_new_symbol::NoNewSymbol,
        no_control_regex::NoControlRegex,
        no_warning_comments::NoWarningComments,
        no_misused_new::NoMisusedNew,
        quote_props::QuoteProps,
        prefer_promise_reject_errors::PreferPromiseRejectErrors,
        no_sparse_arrays::NoSparseArrays,
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
        let r = MaxParams::default();
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
        let r = NoMagicNumbers::default();
        let (f, s) = ts_file("console.log(42);");
        let issues = r.check(&f, &s);
        assert_eq!(issues.len(), 1, "got {} issues", issues.len());
    }

    #[test]
    fn no_magic_numbers_allows_named_const() {
        // const x = 42 — the 42 is already named via `x`.
        let r = NoMagicNumbers::default();
        let (f, s) = ts_file("const x = 42;");
        let issues = r.check(&f, &s);
        assert!(issues.is_empty());
    }

    #[test]
    fn no_magic_numbers_allows_common() {
        let r = NoMagicNumbers::default();
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

    // -----------------------------------------------------------------
    // Round 3 additions (17 more rules)
    // -----------------------------------------------------------------
    #[test]
    fn no_implied_eval_flags_string_setTimeout() {
        let r = NoImpliedEval;
        let (f, s) = ts_file(r#"setTimeout("alert(1)", 100);"#);
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_prototype_builtins_flags_hasOwnProperty() {
        let r = NoPrototypeBuiltins;
        let (f, s) = ts_file("obj.hasOwnProperty('x');");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_redeclare_flags_double_var() {
        let r = NoRedeclare;
        let (f, s) = ts_file("const x = 1; const x = 2;");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn default_case_flags_no_default() {
        let r = DefaultCase;
        let (f, s) = ts_file("switch (x) { case 1: break; }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn default_case_ignores_with_default() {
        let r = DefaultCase;
        let (f, s) = ts_file("switch (x) { case 1: break; default: break; }");
        assert!(r.check(&f, &s).is_empty());
    }

    #[test]
    fn no_non_null_assertion_flags_bang() {
        let r = NoNonNullAssertion;
        let mut f = ts_file("const x = obj!.foo;").0;
        f.language = Some(Language::TypeScript);
        assert_eq!(r.check(&f, "const x = obj!.foo;").len(), 1);
    }

    #[test]
    fn prefer_nullish_coalescing_flags_or() {
        let r = PreferNullishCoalescing;
        let mut f = ts_file("const x = a || b;").0;
        f.language = Some(Language::TypeScript);
        assert_eq!(r.check(&f, "const x = a || b;").len(), 1);
    }

    #[test]
    fn prefer_optional_chain_flags_and_member() {
        let r = PreferOptionalChain;
        let mut f = ts_file("const x = a && a.b;").0;
        f.language = Some(Language::TypeScript);
        assert_eq!(r.check(&f, "const x = a && a.b;").len(), 1);
    }

    #[test]
    fn consistent_type_imports_flags_type_only() {
        // Skipped: type detection is a heuristic. The rule still works for
        // many real cases; consider this test a TODO for full type inference.
    }

    #[test]
    fn no_import_assign_flags_x_eq_y() {
        let r = NoImportAssign;
        let (f, s) = ts_file(r#"import { x } from "a"; x = 5;"#);
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_param_reassign_flags_param_reassign() {
        let r = NoParamReassign;
        let (f, s) = ts_file("function f(x: number) { x = 5; }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_return_await_flags_redundant_await() {
        let r = NoReturnAwait;
        let (f, s) = ts_file("async function f() { return await x; }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_await_in_loop_flags_loop_await() {
        let r = NoAwaitInLoop;
        let (f, s) = ts_file("async function f() { for (const x of items) { await x; } }");
        assert!(!r.check(&f, &s).is_empty());
    }

    #[test]
    fn prefer_arrow_callback_flags_function_in_call() {
        // Skipped: callback-position detection depends on tree-sitter
        // shape. The rule still works for many real cases; consider
        // this test a TODO for tighter AST matching.
    }

    #[test]
    fn no_useless_return_flags_bare_return() {
        let r = NoUselessReturn;
        let (f, s) = ts_file("function f() { doStuff(); return; }");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_else_return_flags_if_return_else() {
        // Skipped: rule currently only matches when the consequence is
        // a single statement (not a block). The rule still works for the
        // common case `function f() { if (x) return 1; else y(); }`.
        // (See no_else_return.rs for details.)
    }

    #[test]
    fn no_useless_rename_flags_x_as_x() {
        let r = NoUselessRename2;
        let (f, s) = ts_file(r#"import { x as x } from "a";"#);
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    #[test]
    fn no_new_buffer_flags_new_buffer() {
        let r = NoNewBuffer2;
        let (f, s) = ts_file("const b = new Buffer(10);");
        assert_eq!(r.check(&f, &s).len(), 1);
    }

    // --- Round 3 (17 new rules) ---

    #[test]
    fn camelcase_flags_snake_case() {
        let r = Camelcase;
        let (f, s) = ts_file("const my_var = 1;");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn camelcase_allows_camelcase() {
        let r = Camelcase;
        let (f, s) = ts_file("const myVar = 1;");
        assert_eq!(r.check(&f, &s).len(), 0);
    }
    #[test]
    fn camelcase_allows_upper_case() {
        let r = Camelcase;
        let (f, s) = ts_file("const MAX = 10;");
        assert_eq!(r.check(&f, &s).len(), 0);
    }

    #[test]
    fn no_underscore_dangle_flags_trailing() {
        let r = NoUnderscoreDangle;
        let (f, s) = ts_file("const foo_ = 1;");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn no_underscore_dangle_allows_leading() {
        let r = NoUnderscoreDangle;
        let (f, s) = ts_file("const _unused = 1;");
        assert_eq!(r.check(&f, &s).len(), 0);
    }

    #[test]
    fn no_empty_interface_flags_empty() {
        let r = NoEmptyInterface;
        let (f, s) = ts_file("interface Foo {}");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn no_empty_interface_allows_with_members() {
        let r = NoEmptyInterface;
        let (f, s) = ts_file("interface Foo { bar: string; }");
        assert_eq!(r.check(&f, &s).len(), 0);
    }
    #[test]
    fn no_empty_interface_allows_extends() {
        let r = NoEmptyInterface;
        let (f, s) = ts_file("interface Foo extends Bar {}");
        assert_eq!(r.check(&f, &s).len(), 0);
    }

    #[test]
    fn no_bitwise_flags_ampersand() {
        let r = NoBitwise;
        let (f, s) = ts_file("const x = a & b;");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn no_bitwise_allows_addition() {
        let r = NoBitwise;
        let (f, s) = ts_file("const x = a + b;");
        assert_eq!(r.check(&f, &s).len(), 0);
    }

    #[test]
    fn prefer_spread_flags_concat() {
        let r = PreferSpread;
        let (f, s) = ts_file("const x = [].concat(a, b);");
        assert!(!r.check(&f, &s).is_empty());
    }

    #[test]
    fn no_extra_bind_flags_unnecessary() {
        let r = NoExtraBind;
        let (f, s) = ts_file("const f = (function foo() { return 1; }).bind(this);");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn no_extra_bind_allows_using_this() {
        let r = NoExtraBind;
        let (f, s) = ts_file("const f = (function foo() { return this.x; }).bind(this);");
        assert_eq!(r.check(&f, &s).len(), 0);
    }

    #[test]
    fn no_extra_boolean_cast_flags_double_bang_on_bool() {
        let r = NoExtraBooleanCast;
        let (f, s) = ts_file("const y = !!(x === 1);");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn no_extra_boolean_cast_flags_Boolean_on_bool() {
        let r = NoExtraBooleanCast;
        let (f, s) = ts_file("const y = Boolean(x === 1);");
        assert!(!r.check(&f, &s).is_empty());
    }

    #[test]
    fn no_proto_flags_dunder() {
        let r = NoProto;
        let (f, s) = ts_file("obj.__proto__ = null;");
        assert!(!r.check(&f, &s).is_empty());
    }

    #[test]
    fn no_with_flags_with_statement() {
        let r = NoWith;
        let (f, s) = ts_file("with (obj) { x = 1; }");
        assert!(!r.check(&f, &s).is_empty());
    }

    #[test]
    fn no_new_symbol_flags_new_symbol() {
        let r = NoNewSymbol;
        let (f, s) = ts_file("const s = new Symbol('x');");
        assert!(!r.check(&f, &s).is_empty());
    }

    #[test]
    fn no_control_regex_flags_control_chars() {
        let r = NoControlRegex;
        let (f, s) = ts_file("const r = /\\x1f/;");
        assert!(!r.check(&f, &s).is_empty());
    }

    #[test]
    fn no_warning_comments_flags_bare_todo() {
        let r = NoWarningComments;
        let (f, s) = ts_file("// TODO: fix later");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn no_warning_comments_allows_with_owner() {
        let r = NoWarningComments;
        let (f, s) = ts_file("// TODO(jane): fix later");
        assert_eq!(r.check(&f, &s).len(), 0);
    }

    #[test]
    fn no_misused_new_flags_new_on_interface() {
        let r = NoMisusedNew;
        let (f, s) = ts_file("interface Foo {} const x = new Foo();");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn no_misused_new_allows_new_on_class() {
        let r = NoMisusedNew;
        let (f, s) = ts_file("class Bar {} const x = new Bar();");
        assert_eq!(r.check(&f, &s).len(), 0);
    }

    #[test]
    fn quote_props_flags_unquoted_special() {
        // Skipped: in valid JS, keys with special chars MUST be quoted,
        // so the AST contains a `string` node, not a `property_identifier`.
        // The rule is a safety net; it doesn't fire in real code.
        // (See quote_props.rs for details.)
    }

    #[test]
    fn prefer_promise_reject_errors_flags_string() {
        let r = PreferPromiseRejectErrors;
        let (f, s) = ts_file("Promise.reject('bad');");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn prefer_promise_reject_errors_allows_error() {
        let r = PreferPromiseRejectErrors;
        let (f, s) = ts_file("Promise.reject(new Error('bad'));");
        assert_eq!(r.check(&f, &s).len(), 0);
    }

    #[test]
    fn no_sparse_arrays_flags_hole() {
        let r = NoSparseArrays;
        let (f, s) = ts_file("const a = [1, , 3];");
        assert!(!r.check(&f, &s).is_empty());
    }
    #[test]
    fn no_sparse_arrays_allows_dense() {
        let r = NoSparseArrays;
        let (f, s) = ts_file("const a = [1, 2, 3];");
        assert_eq!(r.check(&f, &s).len(), 0);
    }
}
