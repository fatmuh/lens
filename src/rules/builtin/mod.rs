//! Built-in rules. Returns the full list used by [`super::RuleRegistry`].

use crate::rules::Rule;

pub mod camelcase;
pub mod consistent_type_imports;
pub mod curly;
pub mod default_case;
pub mod max_function_complexity;
pub mod max_function_lines;
pub mod max_params;
pub mod no_array_constructor;
pub mod no_async_promise_executor;
pub mod no_await_in_loop;
pub mod no_bitwise;
pub mod no_buffer_constructor;
pub mod no_class_assign;
pub mod no_compare_neg_zero;
pub mod no_console;
pub mod no_constant_condition;
pub mod no_control_regex;
pub mod no_delete_var;
pub mod no_div_regex;
pub mod no_dupe_args;
pub mod no_dupe_class_members;
pub mod no_dupe_keys;
pub mod no_duplicate_case;
pub mod no_duplicate_imports;
pub mod no_else_return;
pub mod no_empty_function;
pub mod no_empty_interface;
pub mod no_empty_pattern;
pub mod no_eqeqeq;
pub mod no_eval;
pub mod no_ex_assign;
pub mod no_explicit_any;
pub mod no_extra_bind;
pub mod no_extra_boolean_cast;
pub mod no_fallthrough;
pub mod no_func_assign;
pub mod no_html_link;
pub mod no_implicit_any;
pub mod no_implicit_globals;
pub mod no_implied_eval;
pub mod no_import_assign;
pub mod no_inner_declaration;
pub mod no_label_var;
pub mod no_lonely_if;
pub mod no_magic_numbers;
pub mod no_misused_new;
pub mod no_mixed_operators;
pub mod no_multi_str;
pub mod no_negated_condition;
pub mod no_nested_ternary;
pub mod no_new;
pub mod no_new_buffer;
pub mod no_new_func;
pub mod no_new_require;
pub mod no_new_symbol;
pub mod no_non_null_assertion;
pub mod no_obj_calls;
pub mod no_octal;
pub mod no_octal_escape;
pub mod no_param_reassign;
pub mod no_path_concat;
pub mod no_promise_all_in_loop;
pub mod no_proto;
pub mod no_prototype_builtins;
pub mod no_redeclare;
pub mod no_regex_spaces;
pub mod no_return_await;
pub mod no_script_url;
pub mod no_self_assign;
pub mod no_self_compare;
pub mod no_sparse_arrays;
pub mod no_template_curly_in_string;
pub mod no_throw_literal;
pub mod no_underscore_dangle;
pub mod no_unneeded_ternary;
pub mod no_unreachable;
pub mod no_unsafe_finally;
pub mod no_unsafe_negation;
pub mod no_unused_vars;
pub mod no_useless_concat;
pub mod no_useless_escape;
pub mod no_useless_rename;
pub mod no_useless_return;
pub mod no_var;
pub mod no_void;
pub mod no_warning_comments;
pub mod no_with;
pub mod one_var;
pub mod operator_assignment;
pub mod prefer_arrow_callback;
pub mod prefer_const;
pub mod prefer_nullish_coalescing;
pub mod prefer_optional_chain;
pub mod prefer_promise_reject_errors;
pub mod prefer_spread;
pub mod prefer_template;
pub mod quote_props;
pub mod require_await;

/// All built-in rules, in the order they should be listed in `lens rules`.
/// Uses default thresholds.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    all_rules_with(&Default::default())
}

/// All built-in rules, configured with the user's thresholds from
/// `[rules]` in `quality-gate.toml`.
pub fn all_rules_with(cfg: &crate::config::RulesConfig) -> Vec<Box<dyn Rule>> {
    let disabled: std::collections::HashSet<&str> =
        cfg.disabled.iter().map(|s| s.as_str()).collect();
    let mut rules: Vec<Box<dyn Rule>> = vec![
        Box::new(no_eval::NoEval),
        Box::new(security_taint::SecurityTaint),
        Box::new(dart_rules::DartAvoidPrint),
        Box::new(dart_rules::DartAvoidEmptyCatch),
        Box::new(dart_rules::DartAvoidUnnecessaryContainers),
        Box::new(dart_rules::DartPreferConstConstructors),
        Box::new(dart_rules::DartAvoidWebLibraries),
        Box::new(dart_rules::DartPreferAsyncAwait),
        Box::new(no_new_func::NoNewFunc),
        Box::new(no_implied_eval::NoImpliedEval),
        Box::new(no_script_url::NoScriptUrl),
        Box::new(no_html_link::NoHtmlLink),
        Box::new(no_prototype_builtins::NoPrototypeBuiltins),
        Box::new(no_with::NoWith),
        Box::new(no_proto::NoProto),
        Box::new(no_new_symbol::NoNewSymbol),
        Box::new(no_control_regex::NoControlRegex),
        Box::new(no_buffer_constructor::NoBufferConstructor),
        Box::new(no_unsafe_negation::NoUnsafeNegation),
        Box::new(no_delete_var::NoDeleteVar),
        Box::new(no_async_promise_executor::NoAsyncPromiseExecutor),
        Box::new(no_unsafe_finally::NoUnsafeFinally),
        Box::new(no_fallthrough::NoFallthrough),
        Box::new(no_unreachable::NoUnreachable),
        Box::new(no_dupe_keys::NoDupeKeys),
        Box::new(no_redeclare::NoRedeclare),
        Box::new(no_extra_bind::NoExtraBind),
        Box::new(no_extra_boolean_cast::NoExtraBooleanCast),
        Box::new(no_misused_new::NoMisusedNew),
        Box::new(no_sparse_arrays::NoSparseArrays),
        Box::new(prefer_promise_reject_errors::PreferPromiseRejectErrors),
        Box::new(no_empty_interface::NoEmptyInterface),
        Box::new(no_compare_neg_zero::NoCompareNegZero),
        Box::new(no_constant_condition::NoConstantCondition),
        Box::new(no_dupe_class_members::NoDupeClassMembers),
        Box::new(no_duplicate_case::NoDuplicateCase),
        Box::new(no_empty_pattern::NoEmptyPattern),
        Box::new(no_self_assign::NoSelfAssign),
        Box::new(no_obj_calls::NoObjCalls),
        Box::new(no_template_curly_in_string::NoTemplateCurlyInString),
        Box::new(no_inner_declaration::NoInnerDeclaration),
        Box::new(no_dupe_args::NoDupeArgs),
        Box::new(no_ex_assign::NoExAssign),
        Box::new(no_func_assign::NoFuncAssign),
        Box::new(no_class_assign::NoClassAssign),
        Box::new(no_octal::NoOctal),
        Box::new(no_div_regex::NoDivRegex),
        Box::new(no_octal_escape::NoOctalEscape),
        Box::new(no_explicit_any::NoExplicitAny),
        Box::new(no_implicit_any::NoImplicitAny),
        Box::new(no_non_null_assertion::NoNonNullAssertion),
        Box::new(consistent_type_imports::ConsistentTypeImports),
        Box::new(prefer_nullish_coalescing::PreferNullishCoalescing),
        Box::new(prefer_optional_chain::PreferOptionalChain),
        Box::new(require_await::RequireAwait),
        Box::new(no_duplicate_imports::NoDuplicateImports),
        Box::new(no_import_assign::NoImportAssign),
        Box::new(no_param_reassign::NoParamReassign),
        Box::new(no_promise_all_in_loop::NoPromiseAllInLoop),
        Box::new(no_return_await::NoReturnAwait),
        Box::new(no_await_in_loop::NoAwaitInLoop),
        Box::new(default_case::DefaultCase),
        Box::new(no_array_constructor::NoArrayConstructor),
        Box::new(no_new_require::NoNewRequire),
        Box::new(no_path_concat::NoPathConcat),
        Box::new(no_void::NoVoid),
        Box::new(no_label_var::NoLabelVar),
        Box::new(no_multi_str::NoMultiStr),
        Box::new(no_new::NoNew),
        Box::new(no_implicit_globals::NoImplicitGlobals),
        Box::new(no_bitwise::NoBitwise),
        Box::new(no_self_compare::NoSelfCompare),
        Box::new(no_var::NoVar),
        Box::new(no_eqeqeq::NoEqeqeq),
        Box::new(prefer_const::PreferConst),
        Box::new(prefer_template::PreferTemplate),
        Box::new(prefer_arrow_callback::PreferArrowCallback),
        Box::new(prefer_spread::PreferSpread),
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
        Box::new(camelcase::Camelcase),
        Box::new(no_underscore_dangle::NoUnderscoreDangle),
        Box::new(quote_props::QuoteProps),
        Box::new(no_warning_comments::NoWarningComments),
        Box::new(no_mixed_operators::NoMixedOperators),
        Box::new(one_var::OneVar),
        Box::new(operator_assignment::OperatorAssignment),
        Box::new(curly::Curly),
        Box::new(no_regex_spaces::NoRegexSpaces),
        Box::new(no_useless_escape::NoUselessEscape),
        Box::new(no_console::NoConsole),
        Box::new(no_new_buffer::NoNewBuffer),
        Box::new(max_function_lines::MaxFunctionLines::with_threshold(
            cfg.max_function_lines,
        )),
        Box::new(
            max_function_complexity::MaxFunctionComplexity::with_threshold(
                cfg.max_function_complexity,
            ),
        ),
        Box::new(max_params::MaxParams::with_threshold(cfg.max_params)),
        Box::new(no_magic_numbers::NoMagicNumbers::with_min_value(
            cfg.no_magic_numbers_min,
        )),
    ];
    // Filter out disabled rules.
    rules.retain(|r| !disabled.contains(r.id()));
    rules
}
pub mod dart_rules;
pub mod security_taint;
