//! JaCoCo XML format parser.
//!
//! JaCoCo is the default Java code-coverage library. Its XML output uses
//! `<counter type="LINE" missed="..." covered="..."/>` to summarize coverage
//! at package, class, and method levels. We aggregate per-class line
//! coverage into per-file coverage. The Java class name is converted to
//! a path like `com/example/Foo` -> `com/example/Foo.java`.

use std::path::PathBuf;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::coverage::{CoverageReport, FileCoverage};

pub fn parse(content: &str) -> CoverageReport {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    // JaCoCo reports use a stack-based structure: package -> class -> counters.
    // The class element carries `sourcefilename` and the counters we need.
    // The class is a leaf in terms of nested data we care about, so we can
    // process each <class> in a single pass and emit a FileCoverage.

    let mut files: Vec<FileCoverage> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"class" {
                    let class = read_class(&mut reader, &e);
                    if let Some(c) = class {
                        files.push(finalize(c));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("warning: jacoco parse error: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    let mut report = CoverageReport {
        format: "jacoco".into(),
        total_lines: 0,
        covered_lines: 0,
        coverage_percent: 0.0,
        file_count: 0,
        files,
        ut_lines: 0,
        ut_covered_lines: 0,
        ut_coverage_percent: 0.0,
        it_lines: 0,
        it_covered_lines: 0,
        it_coverage_percent: 0.0,
        new_total_lines: 0,
        new_covered_lines: 0,
        new_coverage_percent: 0.0,
    };
    report.recompute_totals();
    report
}

struct ParsedClass {
    path: PathBuf,
    total_lines: u64,
    covered_lines: u64,
    uncovered_lines: Vec<u32>,
}

fn read_class<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    start: &quick_xml::events::BytesStart<'_>,
) -> Option<ParsedClass> {
    let mut class_name = String::new();
    let mut source_filename = String::new();
    let mut missed: u64 = 0;
    let mut covered: u64 = 0;
    let mut uncovered_lines: Vec<u32> = Vec::new();

    for attr in start.attributes().flatten() {
        match attr.key.as_ref() {
            b"name" => {
                if let Ok(v) = attr.unescape_value() {
                    class_name = v.into_owned();
                }
            }
            b"sourcefilename" => {
                if let Ok(v) = attr.unescape_value() {
                    source_filename = v.into_owned();
                }
            }
            _ => {}
        }
    }

    // Walk until the matching </class>.
    let mut buf = Vec::new();
    let mut depth: i32 = 1;
    while depth > 0 {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                if e.name().as_ref() == b"counter" {
                    read_counter(&e, &mut missed, &mut covered, &mut uncovered_lines);
                }
            }
            Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"counter" {
                    read_counter(&e, &mut missed, &mut covered, &mut uncovered_lines);
                }
            }
            Ok(Event::End(e)) => {
                depth -= 1;
                if depth == 0 && e.name().as_ref() != b"class" {
                    // Mismatched nesting; bail.
                    return None;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("warning: jacoco parse error in <class>: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    if source_filename.is_empty() && class_name.is_empty() {
        return None;
    }

    // JaCoCo <counter type="LINE"> gives us totals but not per-line numbers.
    // When we can compute per-line uncovered, we do; otherwise leave it empty.
    let path = if !source_filename.is_empty() {
        PathBuf::from(source_filename.replace('\\', "/"))
    } else {
        // Fallback: derive a path from the FQCN.
        PathBuf::from(class_name.replace('.', "/") + ".java")
    };

    Some(ParsedClass {
        path,
        total_lines: missed + covered,
        covered_lines: covered,
        uncovered_lines,
    })
}

fn read_counter(
    e: &quick_xml::events::BytesStart<'_>,
    missed: &mut u64,
    covered: &mut u64,
    uncovered_lines: &mut Vec<u32>,
) {
    let mut counter_type = String::new();
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"type" {
            if let Ok(v) = attr.unescape_value() {
                counter_type = v.into_owned();
            }
        }
    }
    if counter_type != "LINE" {
        return;
    }
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"missed" => {
                if let Ok(v) = attr.unescape_value() {
                    *missed = v.parse().unwrap_or(0);
                }
            }
            b"covered" => {
                if let Ok(v) = attr.unescape_value() {
                    *covered = v.parse().unwrap_or(0);
                }
            }
            _ => {}
        }
    }
    // JaCoCo's per-counter summary doesn't include the individual line
    // numbers; per-line uncovered lines are only available in the
    // <line> child elements of <method>. We skip that level of detail
    // for Phase 3 — the totals are still useful.
    let _ = uncovered_lines;
}

fn finalize(c: ParsedClass) -> FileCoverage {
    let coverage_percent = if c.total_lines > 0 {
        (c.covered_lines as f64 / c.total_lines as f64) * 100.0
    } else {
        0.0
    };
    let mut uncovered = c.uncovered_lines;
    uncovered.sort_unstable();
    uncovered.dedup();
    FileCoverage {
        path: c.path,
        total_lines: c.total_lines,
        covered_lines: c.covered_lines,
        coverage_percent,
        uncovered_lines: uncovered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jacoco_class_with_line_counter() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<report>
  <package name="com/example">
    <class name="com/example/Foo" sourcefilename="com/example/Foo.java">
      <counter type="INSTRUCTION" missed="0" covered="5"/>
      <counter type="LINE" missed="2" covered="8"/>
      <counter type="METHOD" missed="0" covered="1"/>
    </class>
  </package>
  <counter type="LINE" missed="2" covered="8"/>
</report>
"#;
        let r = parse(xml);
        assert_eq!(r.file_count, 1);
        assert_eq!(r.total_lines, 10);
        assert_eq!(r.covered_lines, 8);
        assert!(r.coverage_percent > 79.0 && r.coverage_percent < 81.0);
        assert_eq!(r.files[0].path, PathBuf::from("com/example/Foo.java"));
    }
}
