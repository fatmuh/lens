//! Terminal report rendering. Phase 0 ships a minimal stub.

#![allow(dead_code)]

use anyhow::Result;

pub fn render(_ctx: &super::super::scanner::ScanContext) -> Result<()> {
    // Delegated to scanner::report_terminal for now. Will be refactored
    // once metrics/rules are wired in.
    Ok(())
}
