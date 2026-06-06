//! JSON report rendering. Phase 0 ships a minimal stub.

#![allow(dead_code)]

use anyhow::Result;
use serde::Serialize;

use super::super::scanner::ScanContext;

#[derive(Debug, Serialize)]
pub struct JsonReport<'a> {
    pub lens_version: &'a str,
    pub root: &'a std::path::Path,
    pub total_files: usize,
}

pub fn render(ctx: &ScanContext) -> Result<String> {
    let r = JsonReport {
        lens_version: env!("CARGO_PKG_VERSION"),
        root: &ctx.root,
        total_files: ctx.files.len(),
    };
    Ok(serde_json::to_string_pretty(&r)?)
}
