//! Rule registry. Phase 0 only ships a stub — no rules are implemented yet.
//! In Phase 2 this module will define the trait, register built-in rules, and
//! provide the `lens rules` command implementation.

use std::process::ExitCode;

use owo_colors::OwoColorize;

/// List all available rules. In Phase 0 this just prints a placeholder.
pub fn list() -> anyhow::Result<ExitCode> {
    println!("{}", "Available rules".bold().cyan());
    println!("{}", "─".repeat(60).dimmed());
    println!(
        "{}",
        "  No rules registered yet. Rule engine arrives in Phase 2.".dimmed()
    );
    println!();
    println!("{}", "Planned categories:".bold());
    println!("  • Generic AST rules (function length, complexity, nesting depth)");
    println!("  • Rust:    unused imports, unsafe usage, panic-prone code");
    println!("  • TS/JS:   no-explicit-any, var-vs-let, console.log in prod");
    println!("  • Python:  bare except, mutable default args, print-debugging");
    println!("  • Go:      error not checked, ignored errors, println-debug");
    println!("  • Java:    System.out.println, raw types, unused locals");
    Ok(ExitCode::SUCCESS)
}
