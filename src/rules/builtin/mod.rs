//! Built-in rules. Returns the full list used by [`super::RuleRegistry`].

use crate::rules::Rule;

pub mod max_function_complexity;
pub mod max_function_lines;
pub mod max_params;
pub mod no_async_promise_executor;
pub mod no_await_in_loop;
pub mod no_console;
pub mod no_duplicate_imports;
pub mod no_dupe_keys;
pub mod no_else_return;
pub mod no_empty_function;
pub mod no_eqeqeq;
pub mod no_eval;
pub mod no_explicit_any;
pub mod no_fallthrough;
pub mod no_html_link;
pub mod no_implicit_any;
pub mod no_implied_eval;
pub mod no_import_assign;
pub mod no_lonely_if;
pub mod no_magic_numbers;
pub mod no_negated_condition;
pub mod no_nested_ternary;
pub mod no_new_buffer;
pub mod no_new_func;
pub mod no_non_null_assertion;
pub mod no_param_reassign;
pub mod no_promise_all_in_loop;
pub mod no_prototype_builtins;
pub mod no_redeclare;
pub mod no_return_await;
pub mod no_script_url;
pub mod no_self_compare;
pub mod no_throw_literal;
pub mod no_unneeded_ternary;
pub mod no_unreachable;
pub mod no_unsafe_finally;
pub mod no_unused_vars;
pub mod no_useless_concat;
pub mod no_useless_rename;
pub mod no_useless_return;
pub mod no_var;
pub mod prefer_arrow_callback;
pub mod prefer_const;
pub mod prefer_nullish_coalescing;
pub mod prefer_optional_chain;
pub mod prefer_template;
pub mod require_await;
pub mod consistent_type_imports;
pub mod default_case;

/// All built-in rules, in the order they should be listed in `lens rules`.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        // Security
        Box::new(no_eval::NoEval),
        Box::new(no_new_func::NoNewFunc),
        Box::new(no_implied_eval::NoImpliedEval),
        Box::new(no_script_url::NoScriptUrl),
        Box::new(no_html_link::NoHtmlLink),
        Box::new(no_prototype_builtins::NoPrototypeBuiltins),
        // Correctness
        Box::new(no_async_promise_executor::NoAsyncPromiseExecutor),
        Box::new(no_unsafe_finally::NoUnsafeFinally),
        Box::new(no_fallthrough::NoFallthrough),
        Box::new(no_unreachable::NoUnreachable),
        Box::new(no_dupe_keys::NoDupeKeys),
        Box::new(no_redeclare::NoRedeclare),
        // Type safety
        Box::new(no_explicit_any::NoExplicitAny),
        Box::new(no_implicit_any::NoImplicitAny),
        Box::new(no_non_null_assertion::NoNonNullAssertion),
        Box::new(consistent_type_imports::ConsistentTypeImports),
        Box::new(prefer_nullish_coalescing::PreferNullishCoalescing),
        Box::new(prefer_optional_chain::PreferOptionalChain),
        // Best practices
        Box::new(require_await::RequireAwait),
        Box::new(no_duplicate_imports::NoDuplicateImports),
        Box::new(no_import_assign::NoImportAssign),
        Box::new(no_param_reassign::NoParamReassign),
        Box::new(no_promise_all_in_loop::NoPromiseAllInLoop),
        Box::new(no_return_await::NoReturnAwait),
        Box::new(no_await_in_loop::NoAwaitInLoop),
        Box::new(default_case::DefaultCase),
        // Metrics-based
        Box::new(max_function_lines::MaxFunctionLines),
        Box::new(max_function_complexity::MaxFunctionComplexity),
        Box::new(max_params::MaxParams),
        // Style
        Box::new(no_var::NoVar),
        Box::new(no_eqeqeq::NoEqeqeq),
        Box::new(prefer_const::PreferConst),
        Box::new(prefer_template::PreferTemplate),
        Box::new(prefer_arrow_callback::PreferArrowCallback),
        Box::new(no_throw_literal::NoThrowLiteral),
        Box::new(no_empty_function::NoEmptyFunction),
        Box::new(no_useless_concat::NoUselessConcat),
        Box::new(no_useless_rename::NoUselessRename),
        Box::new(no_useless_return::NoUselessReturn),
        Box::new(no_negated_condition::NoNegatedCondition),
        Box::new(no_lonely_if::NoLonelyIf),
        Box::new(no_nested_ternary::NoNestedTernary),
        Box::new(no_unneeded_ternary::NoUnneededTernary),
        Box::new(no_else_return::NoElseReturn),
        Box::new(no_unused_vars::NoUnusedVars),
        // Tooling
        Box::new(no_magic_numbers::NoMagicNumbers),
        Box::new(no_console::NoConsole),
        Box::new(no_new_buffer::NoNewBuffer),
    ]
}
