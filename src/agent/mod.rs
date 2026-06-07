//! AI-powered auto-fix agent.
//!
//! Uses an OpenAI-compatible API (BYOK) to:
//!   1. **Coverage agent**: write tests for uncovered lines
//!   2. **Dedup agent**: refactor duplicated code into shared functions
//!
//! Both agents respect the constraint: **no behavior change** to existing code.

pub mod client;
pub mod coverage;
pub mod dedup;
pub mod test_runner;
pub mod watch;
