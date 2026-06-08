//! Cobertura XML format parser.
//!
//! Cobertura is the de-facto standard for Java/.NET projects and is
//! emitted by many tools (Cobertura, JaCoCo with `--xml cobertura`,
//! ReportGenerator, codecov, ...).
//!
//! Schema reference: <https://github.com/cobertura/cobertura/blob/master/cobertura/src/site/markdown/format.md>

use std::path::PathBuf;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::coverage::{CoverageReport, FileCoverage};

pub fn parse(content: &str) -> CoverageReport {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut files: Vec<FileCoverage> = Vec::new();
    let mut current: Option<FileCoverage> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                if name.as_ref() == b"class" {
                    // Read `filename` attribute.
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"filename" {
                            if let Ok(val) = attr.unescape_value() {
                                current = Some(FileCoverage {
                                    path: PathBuf::from(val.into_owned()),
                                    total_lines: 0,
                                    covered_lines: 0,
                                    coverage_percent: 0.0,
                                    uncovered_lines: Vec::new(),
                                    executable_lines: Vec::new(),
                                    covered_lines_set: std::collections::HashSet::new(),
                                });
                            }
                        }
                    }
                } else if name.as_ref() == b"line" {
                    // Cobertura <line> is a leaf, not a container, but we
                    // treat it as start here just in case.
                    if let Some(f) = current.as_mut() {
                        read_line_into(f, &e);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"line" {
                    if let Some(f) = current.as_mut() {
                        read_line_into(f, &e);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"class" {
                    if let Some(f) = current.take() {
                        files.push(finalize(f));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("warning: cobertura parse error: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    if let Some(f) = current.take() {
        files.push(finalize(f));
    }

    let mut report = CoverageReport {
        format: "cobertura".into(),
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

fn read_line_into(f: &mut FileCoverage, e: &quick_xml::events::BytesStart<'_>) {
    let mut line_num: u32 = 0;
    let mut hits: u64 = 0;
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"number" => {
                if let Ok(v) = attr.unescape_value() {
                    line_num = v.parse().unwrap_or(0);
                }
            }
            b"hits" => {
                if let Ok(v) = attr.unescape_value() {
                    hits = v.parse().unwrap_or(0);
                }
            }
            _ => {}
        }
    }
    if line_num > 0 {
        f.total_lines += 1;
        f.executable_lines.push(line_num);
        if hits > 0 {
            f.covered_lines += 1;
            f.covered_lines_set.insert(line_num);
        } else {
            f.uncovered_lines.push(line_num);
        }
    }
}

fn finalize(mut f: FileCoverage) -> FileCoverage {
    f.uncovered_lines.sort_unstable();
    f.uncovered_lines.dedup();
    f.coverage_percent = if f.total_lines > 0 {
        (f.covered_lines as f64 / f.total_lines as f64) * 100.0
    } else {
        0.0
    };
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_cobertura() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<coverage line-rate="0.5" lines-covered="2" lines-valid="4" version="0.1">
  <packages>
    <package name="com.example">
      <classes>
        <class name="Foo" filename="src/Foo.java">
          <lines>
            <line number="1" hits="1"/>
            <line number="2" hits="1"/>
            <line number="3" hits="0"/>
            <line number="4" hits="0"/>
          </lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>
"#;
        let r = parse(xml);
        assert_eq!(r.file_count, 1);
        assert_eq!(r.total_lines, 4);
        assert_eq!(r.covered_lines, 2);
        let foo = &r.files[0];
        assert_eq!(foo.path, PathBuf::from("src/Foo.java"));
        assert_eq!(foo.uncovered_lines, vec![3, 4]);
    }
}
