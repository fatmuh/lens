//! Built-in rules. Returns the full list used by [`super::RuleRegistry`].

use crate::rules::Rule;

pub mod max_function_complexity;
pub mod max_function_lines;
pub mod max_params;
pub mod no_console;
pub mod no_empty_function;
pub mod no_eqeqeq;
pub mod no_explicit_any;
pub mod no_implicit_any;
pub mod no_magic_numbers;
pub mod no_throw_literal;
pub mod no_unreachable;
pub mod no_unused_vars;
pub mod no_var;
pub mod prefer_const;

/// All built-in rules, in the order they should be listed in `lens rules`.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(no_explicit_any::NoExplicitAny),
        Box::new(no_implicit_any::NoImplicitAny),
        Box::new(no_console::NoConsole),
        Box::new(no_var::NoVar),
        Box::new(no_eqeqeq::NoEqeqeq),
        Box::new(prefer_const::PreferConst),
        Box::new(no_unused_vars::NoUnusedVars),
        Box::new(no_magic_numbers::NoMagicNumbers),
        Box::new(no_throw_literal::NoThrowLiteral),
        Box::new(no_empty_function::NoEmptyFunction),
        Box::new(no_unreachable::NoUnreachable),
        Box::new(max_function_lines::MaxFunctionLines),
        Box::new(max_function_complexity::MaxFunctionComplexity),
        Box::new(max_params::MaxParams),
    ]
}
