//! Built-in rules. Returns the full list used by [`super::RuleRegistry`].

use crate::rules::Rule;

pub mod max_function_complexity;
pub mod max_function_lines;
pub mod max_params;
pub mod no_async_promise_executor;
pub mod no_console;
pub mod no_duplicate_imports;
pub mod no_dupe_keys;
pub mod no_empty_function;
pub mod no_eqeqeq;
pub mod no_eval;
pub mod no_explicit_any;
pub mod no_fallthrough;
pub mod no_html_link;
pub mod no_implicit_any;
pub mod no_lonely_if;
pub mod no_magic_numbers;
pub mod no_negated_condition;
pub mod no_nested_ternary;
pub mod no_new_func;
pub mod no_promise_all_in_loop;
pub mod no_script_url;
pub mod no_self_compare;
pub mod no_throw_literal;
pub mod no_unneeded_ternary;
pub mod no_unreachable;
pub mod no_unsafe_finally;
pub mod no_unused_vars;
pub mod no_useless_concat;
pub mod no_var;
pub mod prefer_const;
pub mod prefer_template;
pub mod require_await;

/// All built-in rules, in the order they should be listed in `lens rules`.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        // Security
        Box::new(no_eval::NoEval),
        Box::new(no_new_func::NoNewFunc),
        Box::new(no_script_url::NoScriptUrl),
        Box::new(no_html_link::NoHtmlLink),
        // Correctness
        Box::new(no_async_promise_executor::NoAsyncPromiseExecutor),
        Box::new(no_unsafe_finally::NoUnsafeFinally),
        Box::new(no_fallthrough::NoFallthrough),
        Box::new(no_unreachable::NoUnreachable),
        Box::new(no_dupe_keys::NoDupeKeys),
        // Type safety
        Box::new(no_explicit_any::NoExplicitAny),
        Box::new(no_implicit_any::NoImplicitAny),
        // Best practices
        Box::new(require_await::RequireAwait),
        Box::new(no_self_compare::NoSelfCompare),
        Box::new(no_duplicate_imports::NoDuplicateImports),
        Box::new(no_promise_all_in_loop::NoPromiseAllInLoop),
        // Metrics-based
        Box::new(max_function_lines::MaxFunctionLines),
        Box::new(max_function_complexity::MaxFunctionComplexity),
        Box::new(max_params::MaxParams),
        // Style
        Box::new(no_var::NoVar),
        Box::new(no_eqeqeq::NoEqeqeq),
        Box::new(prefer_const::PreferConst),
        Box::new(prefer_template::PreferTemplate),
        Box::new(no_throw_literal::NoThrowLiteral),
        Box::new(no_empty_function::NoEmptyFunction),
        Box::new(no_useless_concat::NoUselessConcat),
        Box::new(no_negated_condition::NoNegatedCondition),
        Box::new(no_lonely_if::NoLonelyIf),
        Box::new(no_nested_ternary::NoNestedTernary),
        Box::new(no_unneeded_ternary::NoUnneededTernary),
        Box::new(no_unused_vars::NoUnusedVars),
        // Tooling
        Box::new(no_magic_numbers::NoMagicNumbers),
        Box::new(no_console::NoConsole),
    ]
}
