//! Taint analysis engine for security vulnerability detection.
//!
//! Tracks data flow from **sources** (user input) to **sinks** (dangerous
//! operations) within a single function body (intra-procedural).
//!
//! Supported vulnerability classes:
//! - SQL Injection
//! - XSS (Cross-Site Scripting)
//! - SSRF (Server-Side Request Forgery)
//! - Command Injection
//! - Path Traversal
//! - Prototype Pollution
//! - Open Redirect
//! - Log Injection

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use crate::analyzer::parser::{get_language, visit_descendants};
use crate::scanner::language::Language;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A security vulnerability detected by taint analysis.
#[derive(Debug, Clone)]
pub struct TaintVulnerability {
    /// Vulnerability class (e.g. "SQL Injection", "XSS").
    pub vuln_type: String,
    /// Rule ID (e.g. "security/sql-injection").
    pub rule_id: String,
    /// The source that introduced tainted data.
    pub source: String,
    /// The sink where tainted data is consumed unsafely.
    pub sink: String,
    /// Line number of the sink (1-based).
    pub line: u32,
    /// Human-readable message.
    pub message: String,
}

/// Configuration for what to detect.
#[derive(Debug, Clone)]
pub struct TaintConfig {
    /// Which vulnerability classes to check.
    pub checks: Vec<VulnClass>,
}

/// A vulnerability class definition.
#[derive(Debug, Clone)]
pub struct VulnClass {
    pub rule_id: String,
    pub vuln_type: String,
    pub message: String,
    /// Patterns that mark user-controlled data (sources).
    pub sources: Vec<SourcePattern>,
    /// Dangerous operations (sinks).
    pub sinks: Vec<SinkPattern>,
    /// Functions/methods that sanitize data (break the taint chain).
    pub sanitizers: Vec<String>,
}

/// A source pattern — where untrusted data enters.
#[derive(Debug, Clone)]
pub struct SourcePattern {
    /// The object part (e.g. "req", "request", "ctx").
    pub object: String,
    /// The property / accessor (e.g. "body", "query", "params").
    pub property: String,
}

/// A sink pattern — where tainted data is consumed dangerously.
#[derive(Debug, Clone)]
pub struct SinkPattern {
    /// The function/method name (e.g. "query", "exec", "innerHTML").
    pub name: String,
    /// The object part, if this is a method call (e.g. "connection", "db").
    /// Empty string = any object or a standalone function call.
    pub object: String,
}

impl Default for TaintConfig {
    fn default() -> Self {
        Self {
            checks: vec![
                sql_injection(),
                xss(),
                ssrf(),
                command_injection(),
                path_traversal(),
                prototype_pollution(),
                open_redirect(),
                log_injection(),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Vulnerability class definitions
// ---------------------------------------------------------------------------

pub fn sql_injection() -> VulnClass {
    VulnClass {
        rule_id: "security/sql-injection".into(),
        vuln_type: "SQL Injection".into(),
        message: "User input flows into SQL query without parameterization. Use parameterized queries instead.".into(),
        sources: vec![
            SourcePattern { object: "req".into(), property: "body".into() },
            SourcePattern { object: "req".into(), property: "query".into() },
            SourcePattern { object: "req".into(), property: "params".into() },
            SourcePattern { object: "request".into(), property: "body".into() },
            SourcePattern { object: "request".into(), property: "query".into() },
            SourcePattern { object: "request".into(), property: "params".into() },
            SourcePattern { object: "ctx".into(), property: "request".into() },
            SourcePattern { object: "ctx".into(), property: "query".into() },
            SourcePattern { object: "event".into(), property: "body".into() },
            SourcePattern { object: "event".into(), property: "queryStringParameters".into() },
            SourcePattern { object: "input".into(), property: "*".into() },
        ],
        sinks: vec![
            SinkPattern { name: "query".into(), object: "".into() },
            SinkPattern { name: "query".into(), object: "db".into() },
            SinkPattern { name: "query".into(), object: "connection".into() },
            SinkPattern { name: "query".into(), object: "pool".into() },
            SinkPattern { name: "query".into(), object: "client".into() },
            SinkPattern { name: "raw".into(), object: "knex".into() },
            SinkPattern { name: "raw".into(), object: "".into() },
            SinkPattern { name: "$queryRaw".into(), object: "".into() },
            SinkPattern { name: "$queryRawUnsafe".into(), object: "".into() },
            SinkPattern { name: "$executeRaw".into(), object: "".into() },
            SinkPattern { name: "$executeRawUnsafe".into(), object: "".into() },
            SinkPattern { name: "sql".into(), object: "".into() },
            SinkPattern { name: "execute".into(), object: "".into() },
            SinkPattern { name: "all".into(), object: "db".into() },
            SinkPattern { name: "run".into(), object: "db".into() },
        ],
        sanitizers: vec![
            "escape".into(),
            "parameterize".into(),
            "sanitize".into(),
            "format".into(),
            "mysql.escape".into(),
            "pg.escapeLiteral".into(),
            "escapeLiteral".into(),
            "escapeIdentifier".into(),
        ],
    }
}

pub fn xss() -> VulnClass {
    VulnClass {
        rule_id: "security/xss".into(),
        vuln_type: "XSS".into(),
        message:
            "User input is rendered as raw HTML without escaping. Use textContent or sanitize."
                .into(),
        sources: vec![
            SourcePattern {
                object: "req".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "query".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "params".into(),
            },
            SourcePattern {
                object: "request".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "request".into(),
                property: "query".into(),
            },
            SourcePattern {
                object: "ctx".into(),
                property: "request".into(),
            },
            SourcePattern {
                object: "ctx".into(),
                property: "query".into(),
            },
            SourcePattern {
                object: "props".into(),
                property: "*".into(),
            },
            SourcePattern {
                object: "this".into(),
                property: "props".into(),
            },
        ],
        sinks: vec![
            SinkPattern {
                name: "innerHTML".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "outerHTML".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "write".into(),
                object: "document".into(),
            },
            SinkPattern {
                name: "writeln".into(),
                object: "document".into(),
            },
            SinkPattern {
                name: "dangerouslySetInnerHTML".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "insertAdjacentHTML".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "html".into(),
                object: "$".into(),
            },
            SinkPattern {
                name: "append".into(),
                object: "jQuery".into(),
            },
            SinkPattern {
                name: "send".into(),
                object: "res".into(),
            },
            SinkPattern {
                name: "end".into(),
                object: "res".into(),
            },
            SinkPattern {
                name: "json".into(),
                object: "res".into(),
            },
        ],
        sanitizers: vec![
            "escapeHtml".into(),
            "encodeURI".into(),
            "encodeURIComponent".into(),
            "sanitize".into(),
            "DOMPurify.sanitize".into(),
            "xss".into(),
            "he.encode".into(),
            "escape".into(),
        ],
    }
}

pub fn ssrf() -> VulnClass {
    VulnClass {
        rule_id: "security/ssrf".into(),
        vuln_type: "SSRF".into(),
        message: "User input flows into a network request. Validate and restrict URLs.".into(),
        sources: vec![
            SourcePattern {
                object: "req".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "query".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "params".into(),
            },
            SourcePattern {
                object: "request".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "ctx".into(),
                property: "request".into(),
            },
            SourcePattern {
                object: "event".into(),
                property: "body".into(),
            },
        ],
        sinks: vec![
            SinkPattern {
                name: "fetch".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "get".into(),
                object: "axios".into(),
            },
            SinkPattern {
                name: "post".into(),
                object: "axios".into(),
            },
            SinkPattern {
                name: "put".into(),
                object: "axios".into(),
            },
            SinkPattern {
                name: "request".into(),
                object: "axios".into(),
            },
            SinkPattern {
                name: "get".into(),
                object: "http".into(),
            },
            SinkPattern {
                name: "get".into(),
                object: "https".into(),
            },
            SinkPattern {
                name: "request".into(),
                object: "http".into(),
            },
            SinkPattern {
                name: "request".into(),
                object: "https".into(),
            },
            SinkPattern {
                name: "get".into(),
                object: "request".into(),
            },
            SinkPattern {
                name: "post".into(),
                object: "request".into(),
            },
            SinkPattern {
                name: "get".into(),
                object: "rp".into(),
            },
            SinkPattern {
                name: "get".into(),
                object: "got".into(),
            },
            SinkPattern {
                name: "post".into(),
                object: "got".into(),
            },
            SinkPattern {
                name: "send".into(),
                object: "needle".into(),
            },
            SinkPattern {
                name: "get".into(),
                object: "superagent".into(),
            },
        ],
        sanitizers: vec![
            "isValidUrl".into(),
            "validateUrl".into(),
            "isAllowedHost".into(),
            "normalizeUrl".into(),
        ],
    }
}

pub fn command_injection() -> VulnClass {
    VulnClass {
        rule_id: "security/command-injection".into(),
        vuln_type: "Command Injection".into(),
        message:
            "User input flows into a system command. Use execFile with arguments array instead."
                .into(),
        sources: vec![
            SourcePattern {
                object: "req".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "query".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "params".into(),
            },
            SourcePattern {
                object: "request".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "ctx".into(),
                property: "request".into(),
            },
            SourcePattern {
                object: "input".into(),
                property: "*".into(),
            },
        ],
        sinks: vec![
            SinkPattern {
                name: "exec".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "exec".into(),
                object: "child_process".into(),
            },
            SinkPattern {
                name: "execSync".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "execSync".into(),
                object: "child_process".into(),
            },
            SinkPattern {
                name: "spawn".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "spawnSync".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "execFile".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "system".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "exec".into(),
                object: "shelljs".into(),
            },
        ],
        sanitizers: vec![
            "escape".into(),
            "sanitize".into(),
            "shellEscape".into(),
            "quote".into(),
        ],
    }
}

pub fn path_traversal() -> VulnClass {
    VulnClass {
        rule_id: "security/path-traversal".into(),
        vuln_type: "Path Traversal".into(),
        message: "User input flows into a file path. Validate and normalize paths.".into(),
        sources: vec![
            SourcePattern {
                object: "req".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "query".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "params".into(),
            },
            SourcePattern {
                object: "request".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "ctx".into(),
                property: "request".into(),
            },
        ],
        sinks: vec![
            SinkPattern {
                name: "readFile".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "readFileSync".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "writeFile".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "writeFileSync".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "appendFile".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "unlink".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "unlinkSync".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "mkdir".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "rmdir".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "createReadStream".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "createWriteStream".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "stat".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "lstat".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "readdir".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "access".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "open".into(),
                object: "fs".into(),
            },
            SinkPattern {
                name: "readFileSync".into(),
                object: "".into(),
            },
            SinkPattern {
                name: "writeFileSync".into(),
                object: "".into(),
            },
        ],
        sanitizers: vec![
            "normalize".into(),
            "resolve".into(),
            "basename".into(),
            "dirname".into(),
            "sanitize".into(),
        ],
    }
}

pub fn prototype_pollution() -> VulnClass {
    VulnClass {
        rule_id: "security/prototype-pollution".into(),
        vuln_type: "Prototype Pollution".into(),
        message: "User input flows into an object merge/copy operation. Validate keys to prevent __proto__ pollution.".into(),
        sources: vec![
            SourcePattern { object: "req".into(), property: "body".into() },
            SourcePattern { object: "req".into(), property: "query".into() },
            SourcePattern { object: "JSON".into(), property: "parse".into() },
            SourcePattern { object: "request".into(), property: "body".into() },
            SourcePattern { object: "ctx".into(), property: "request".into() },
        ],
        sinks: vec![
            SinkPattern { name: "merge".into(), object: "".into() },
            SinkPattern { name: "defaultsDeep".into(), object: "".into() },
            SinkPattern { name: "extend".into(), object: "jQuery".into() },
            SinkPattern { name: "assign".into(), object: "Object".into() },
            SinkPattern { name: "defineProperty".into(), object: "Object".into() },
            SinkPattern { name: "set".into(), object: "lodash".into() },
            SinkPattern { name: "setWith".into(), object: "lodash".into() },
            SinkPattern { name: "zipObjectDeep".into(), object: "lodash".into() },
            SinkPattern { name: "deepExtend".into(), object: "".into() },
            SinkPattern { name: "deepMerge".into(), object: "".into() },
            SinkPattern { name: "cloneDeep".into(), object: "".into() },
        ],
        sanitizers: vec![
            "isSafeKey".into(),
            "hasOwnProperty".into(),
            "sanitizeKeys".into(),
        ],
    }
}

pub fn open_redirect() -> VulnClass {
    VulnClass {
        rule_id: "security/open-redirect".into(),
        vuln_type: "Open Redirect".into(),
        message: "User input flows into a redirect. Validate and whitelist redirect URLs.".into(),
        sources: vec![
            SourcePattern {
                object: "req".into(),
                property: "query".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "body".into(),
            },
            SourcePattern {
                object: "req".into(),
                property: "params".into(),
            },
            SourcePattern {
                object: "request".into(),
                property: "query".into(),
            },
            SourcePattern {
                object: "ctx".into(),
                property: "query".into(),
            },
        ],
        sinks: vec![
            SinkPattern {
                name: "redirect".into(),
                object: "res".into(),
            },
            SinkPattern {
                name: "redirect".into(),
                object: "response".into(),
            },
            SinkPattern {
                name: "redirect".into(),
                object: "ctx".into(),
            },
            SinkPattern {
                name: "writeHead".into(),
                object: "res".into(),
            },
            SinkPattern {
                name: "set".into(),
                object: "location".into(),
            },
            SinkPattern {
                name: "replace".into(),
                object: "window.location".into(),
            },
            SinkPattern {
                name: "assign".into(),
                object: "window.location".into(),
            },
            SinkPattern {
                name: "href".into(),
                object: "location".into(),
            },
        ],
        sanitizers: vec![
            "isSafeUrl".into(),
            "isAllowedOrigin".into(),
            "normalizeUrl".into(),
            "isValidRedirect".into(),
        ],
    }
}

pub fn log_injection() -> VulnClass {
    VulnClass {
        rule_id: "security/log-injection".into(),
        vuln_type: "Log Injection".into(),
        message: "User input flows into log output without sanitization. Sanitize newlines and control characters.".into(),
        sources: vec![
            SourcePattern { object: "req".into(), property: "body".into() },
            SourcePattern { object: "req".into(), property: "query".into() },
            SourcePattern { object: "req".into(), property: "params".into() },
            SourcePattern { object: "ctx".into(), property: "request".into() },
            SourcePattern { object: "input".into(), property: "*".into() },
        ],
        sinks: vec![
            SinkPattern { name: "info".into(), object: "logger".into() },
            SinkPattern { name: "warn".into(), object: "logger".into() },
            SinkPattern { name: "error".into(), object: "logger".into() },
            SinkPattern { name: "debug".into(), object: "logger".into() },
            SinkPattern { name: "log".into(), object: "console".into() },
            SinkPattern { name: "info".into(), object: "console".into() },
            SinkPattern { name: "warn".into(), object: "console".into() },
            SinkPattern { name: "error".into(), object: "console".into() },
            SinkPattern { name: "info".into(), object: "winston".into() },
            SinkPattern { name: "log".into(), object: "log4js".into() },
            SinkPattern { name: "info".into(), object: "pino".into() },
        ],
        sanitizers: vec![
            "replace".into(),
            "trim".into(),
            "normalize".into(),
            "stripControlChars".into(),
        ],
    }
}

// ---------------------------------------------------------------------------
// Core analysis engine
// ---------------------------------------------------------------------------

/// Run taint analysis on a source file. Returns all detected vulnerabilities.
pub fn analyze(source: &str, lang: Language) -> Vec<TaintVulnerability> {
    let Some(tslang) = get_language(lang) else {
        return vec![];
    };

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tslang).ok();
    let Some(tree) = parser.parse(source, None) else {
        return vec![];
    };

    let config = TaintConfig::default();
    let root = tree.root_node();
    let source_bytes = source.as_bytes();

    // Collect all function/method/arrow bodies
    let mut bodies: Vec<Node> = vec![];
    collect_function_bodies(root, &mut bodies);

    let mut vulns = Vec::new();

    for body in &bodies {
        for check in &config.checks {
            let found = analyze_body(*body, check, source, source_bytes);
            vulns.extend(found);
        }
    }

    // De-duplicate by (rule_id, line)
    let mut seen = HashSet::new();
    vulns.retain(|v| seen.insert((v.rule_id.clone(), v.line)));

    vulns
}

/// Collect all function body nodes in the AST.
fn collect_function_bodies<'a>(node: Node<'a>, bodies: &mut Vec<Node<'a>>) {
    let kind = node.kind();
    if kind == "function"
        || kind == "function_declaration"
        || kind == "method_definition"
        || kind == "arrow_function"
        || kind == "generator_function"
        || kind == "generator_function_declaration"
        || kind == "function_expression"
    {
        // Get the body node
        if let Some(body) = node.child_by_field_name("body") {
            bodies.push(body);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_function_bodies(child, bodies);
    }
}

/// Analyze a single function body for one vulnerability class.
fn analyze_body<'a>(
    body: Node<'a>,
    check: &VulnClass,
    source: &str,
    source_bytes: &[u8],
) -> Vec<TaintVulnerability> {
    let mut vulns = Vec::new();

    // Step 1: Find all tainted variables (from sources)
    let tainted = find_tainted_vars(
        body,
        &check.sources,
        &check.sanitizers,
        source,
        source_bytes,
    );

    // Step 2: Track taint through assignments
    let tainted = propagate_taint(body, tainted, &check.sanitizers, source, source_bytes);

    // Step 3: Check if any tainted variable reaches a sink
    let sink_calls = find_sink_calls(body, &check.sinks, source, source_bytes);

    for (sink_var, sink_name, sink_line, sink_obj) in &sink_calls {
        // Direct match: tainted variable is used directly in sink
        if tainted.contains(sink_var) {
            vulns.push(TaintVulnerability {
                vuln_type: check.vuln_type.clone(),
                rule_id: check.rule_id.clone(),
                source: format!("{}.{{property}}", sink_var),
                sink: sink_name.clone(),
                line: *sink_line,
                message: check.message.clone(),
            });
            continue;
        }

        // Check if any tainted variable appears in the arguments of the sink call
        // by looking at call_expression arguments
        if is_tainted_in_args(*sink_line, &tainted, source) {
            vulns.push(TaintVulnerability {
                vuln_type: check.vuln_type.clone(),
                rule_id: check.rule_id.clone(),
                source: "user input".into(),
                sink: sink_name.clone(),
                line: *sink_line,
                message: check.message.clone(),
            });
        }
    }

    vulns
}

/// Find variables that receive data from taint sources.
fn find_tainted_vars<'a>(
    body: Node<'a>,
    sources: &[SourcePattern],
    sanitizers: &[String],
    source: &str,
    source_bytes: &[u8],
) -> HashSet<String> {
    let mut tainted = HashSet::new();

    visit_descendants(body, |node| {
        // Look for member_expression: req.body, req.query, etc.
        if node.kind() == "member_expression" {
            if let Some(text) = node.utf8_text(source_bytes).ok() {
                let text = text.trim();
                if is_source_match(text, sources) {
                    // Check if this source is wrapped in a sanitizer call
                    if is_in_sanitizer(node, sanitizers, source_bytes) {
                        return;
                    }
                    // Find the enclosing assignment to get the variable name
                    if let Some(parent) = node.parent() {
                        if let Some(var_name) = extract_assigned_var(parent, source_bytes) {
                            tainted.insert(var_name);
                        }
                    }
                }
            }
        }

        // Look for destructuring: const { body } = req;
        if node.kind() == "variable_declarator" || node.kind() == "assignment_expression" {
            if let Some(value_node) = node.child_by_field_name("value") {
                if let Ok(text) = value_node.utf8_text(source_bytes) {
                    let text = text.trim();
                    // Skip if the value is a sanitizer call
                    let is_sanitized = sanitizers.iter().any(|s| text.contains(s));
                    if is_sanitized {
                        return;
                    }
                    if is_source_match(text, sources) {
                        if let Some(name_node) = node.child_by_field_name("name") {
                            if let Ok(name) = name_node.utf8_text(source_bytes) {
                                tainted.insert(name.trim().to_string());
                            }
                        }
                    }
                }
            }
        }

        // Look for object destructuring patterns: const { body } = req
        if node.kind() == "variable_declarator" {
            if let Some(name_node) = node.child_by_field_name("name") {
                // Check for object_pattern: { body, query }
                if name_node.kind() == "object_pattern" {
                    if let Some(value_node) = node.child_by_field_name("value") {
                        if let Ok(val_text) = value_node.utf8_text(source_bytes) {
                            let val_text = val_text.trim();
                            // Check if the RHS is a known source object
                            let source_objects: Vec<&str> =
                                sources.iter().map(|s| s.object.as_str()).collect();
                            if source_objects.contains(&val_text) {
                                // All destructured properties are tainted
                                extract_destructured_names(&name_node, source_bytes, &mut tainted);
                            }
                        }
                    }
                }
            }
        }
    });

    tainted
}

/// Check if a text matches any source pattern.
fn is_source_match(text: &str, sources: &[SourcePattern]) -> bool {
    for src in sources {
        if src.property == "*" {
            // Wildcard: match object.anything
            if text.starts_with(&src.object) && text.contains('.') {
                return true;
            }
        } else {
            let pattern = format!("{}.{}", src.object, src.property);
            if text == pattern || text.starts_with(&format!("{}.", pattern)) {
                return true;
            }
        }
    }
    false
}

/// Check if a member_expression node is inside a sanitizer function call.
fn is_in_sanitizer<'a>(node: Node<'a>, sanitizers: &[String], source_bytes: &[u8]) -> bool {
    let mut current = node;
    for _ in 0..3 {
        if let Some(parent) = current.parent() {
            if parent.kind() == "call_expression" {
                if let Some(func) = parent.child_by_field_name("function") {
                    if let Ok(func_text) = func.utf8_text(source_bytes) {
                        let func_text = func_text.trim();
                        for s in sanitizers {
                            if func_text.contains(s.as_str()) {
                                return true;
                            }
                        }
                    }
                }
            }
            current = parent;
        } else {
            break;
        }
    }
    false
}
fn extract_assigned_var(node: Node, source_bytes: &[u8]) -> Option<String> {
    // Walk up to find variable_declarator or assignment_expression
    let mut current = node;
    for _ in 0..3 {
        let kind = current.kind();
        if kind == "variable_declarator" || kind == "assignment_expression" {
            if let Some(name_node) = current.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    return Some(name.trim().to_string());
                }
            }
        }
        current = current.parent()?;
    }
    None
}

/// Extract names from object_pattern destructuring.
fn extract_destructured_names<'a>(
    node: &Node<'a>,
    source_bytes: &[u8],
    tainted: &mut HashSet<String>,
) {
    visit_descendants(*node, |child| {
        if child.kind() == "shorthand_property_identifier" || child.kind() == "property_identifier"
        {
            if let Ok(name) = child.utf8_text(source_bytes) {
                tainted.insert(name.trim().to_string());
            }
        }
    });
}

/// Propagate taint through variable assignments within the body.
/// e.g., if `body` is tainted, then `const name = body.name` makes `name` tainted.
fn propagate_taint<'a>(
    body: Node<'a>,
    mut tainted: HashSet<String>,
    sanitizers: &[String],
    source: &str,
    source_bytes: &[u8],
) -> HashSet<String> {
    // Do 3 passes to handle chains: a → b → c
    for _ in 0..3 {
        let mut new_tainted = HashSet::new();
        visit_descendants(body, |node| {
            if node.kind() == "variable_declarator" || node.kind() == "assignment_expression" {
                if let Some(value_node) = node.child_by_field_name("value") {
                    if let Ok(val_text) = value_node.utf8_text(source_bytes) {
                        let val_text = val_text.trim();

                        // Check if any sanitizer is applied
                        let is_sanitized = sanitizers.iter().any(|s| val_text.contains(s));
                        if is_sanitized {
                            return;
                        }

                        // Check if any tainted variable appears in the RHS
                        let rhs_tainted = tainted.iter().any(|t| {
                            val_text.contains(t.as_str()) && is_whole_var_match(val_text, t)
                        });

                        if rhs_tainted {
                            if let Some(name_node) = node.child_by_field_name("name") {
                                if let Ok(name) = name_node.utf8_text(source_bytes) {
                                    new_tainted.insert(name.trim().to_string());
                                }
                            }
                        }
                    }
                }
            }

            // Also track: member access on tainted objects
            // e.g., const x = req.body.name → x is tainted
            if node.kind() == "variable_declarator" || node.kind() == "assignment_expression" {
                if let Some(value_node) = node.child_by_field_name("value") {
                    if let Ok(val_text) = value_node.utf8_text(source_bytes) {
                        let val_text = val_text.trim();
                        // Check for tainted_var.something pattern
                        for t in &tainted {
                            let prefix = format!("{}.", t);
                            let prefix2 = format!("{}[", t);
                            if val_text.starts_with(&prefix)
                                || val_text.starts_with(&prefix2)
                                || val_text.contains(&prefix)
                                || val_text.contains(&prefix2)
                            {
                                let is_sanitized = sanitizers.iter().any(|s| val_text.contains(s));
                                if !is_sanitized {
                                    if let Some(name_node) = node.child_by_field_name("name") {
                                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                                            new_tainted.insert(name.trim().to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        tainted.extend(new_tainted);
    }
    tainted
}

/// Check if a variable name appears as a whole word in text.
fn is_whole_var_match(text: &str, var_name: &str) -> bool {
    // Simple check: var_name appears and is bounded by non-identifier chars
    if let Some(idx) = text.find(var_name) {
        let before_ok = idx == 0
            || !text.as_bytes()[idx - 1].is_ascii_alphanumeric()
            || text.as_bytes()[idx - 1] == b'.'
            || text.as_bytes()[idx - 1] == b'[';
        let after = idx + var_name.len();
        let after_ok = after >= text.len()
            || !text.as_bytes()[after].is_ascii_alphanumeric()
            || text.as_bytes()[after] == b'.'
            || text.as_bytes()[after] == b'[';
        before_ok || after_ok
    } else {
        false
    }
}

/// Find calls to sink functions and extract the first argument variable name.
fn find_sink_calls<'a>(
    body: Node<'a>,
    sinks: &[SinkPattern],
    source: &str,
    source_bytes: &[u8],
) -> Vec<(String, String, u32, String)> {
    let mut results = Vec::new();

    visit_descendants(body, |node| {
        // --- Call expressions: db.query(x), fetch(x), etc. ---
        if node.kind() == "call_expression" {
            let func_node = match node.child_by_field_name("function") {
                Some(n) => n,
                None => return,
            };

            let func_text = match func_node.utf8_text(source_bytes) {
                Ok(t) => t.trim().to_string(),
                Err(_) => return,
            };

            let line = node.start_position().row as u32 + 1;

            let func_name = extract_call_name(&func_text);
            let obj_name = extract_call_object(&func_text);

            for sink in sinks {
                let name_matches = func_name == sink.name;
                let obj_matches = sink.object.is_empty() || obj_name == sink.object;

                if name_matches && obj_matches {
                    let arg_var = extract_first_arg_var(node, source_bytes)
                        .unwrap_or_else(|| func_text.clone());
                    results.push((arg_var, sink.name.clone(), line, obj_name.to_string()));
                }
            }
        }

        // --- Property assignments: el.innerHTML = x, res.redirect = x, etc. ---
        if node.kind() == "assignment_expression" {
            let left_node = match node.child_by_field_name("left") {
                Some(n) => n,
                None => return,
            };
            let right_node = match node.child_by_field_name("right") {
                Some(n) => n,
                None => return,
            };

            if let Ok(left_text) = left_node.utf8_text(source_bytes) {
                let left_text = left_text.trim();
                let prop_name = extract_call_name(left_text);
                let obj_name = extract_call_object(left_text);
                let line = node.start_position().row as u32 + 1;

                for sink in sinks {
                    let name_matches = prop_name == sink.name;
                    let obj_matches = sink.object.is_empty() || obj_name == sink.object;

                    if name_matches && obj_matches {
                        let arg_var = right_node
                            .utf8_text(source_bytes)
                            .map(|t| t.trim().to_string())
                            .unwrap_or_default();
                        results.push((arg_var, sink.name.clone(), line, obj_name.to_string()));
                    }
                }
            }
        }
    });

    results
}

/// Extract function name from a call expression text.
/// e.g., "db.query" → "query", "fetch" → "fetch"
fn extract_call_name(text: &str) -> &str {
    if let Some(dot) = text.rfind('.') {
        &text[dot + 1..]
    } else if let Some(dot) = text.rfind("?.") {
        &text[dot + 2..]
    } else {
        text
    }
}

/// Extract object name from a method call.
/// e.g., "db.query" → "db", "fetch" → ""
fn extract_call_object(text: &str) -> &str {
    if let Some(dot) = text.rfind('.') {
        &text[..dot]
    } else if let Some(dot) = text.rfind("?.") {
        &text[..dot]
    } else {
        ""
    }
}

/// Extract the first argument variable name from a call expression.
fn extract_first_arg_var<'a>(call_node: Node<'a>, source_bytes: &[u8]) -> Option<String> {
    let args_node = call_node.child_by_field_name("arguments")?;
    for child in args_node.children(&mut args_node.walk()) {
        if child.is_named() {
            if let Ok(text) = child.utf8_text(source_bytes) {
                return Some(text.trim().to_string());
            }
        }
    }
    None
}

/// Check if any tainted variable appears in the source line at a given line number.
fn is_tainted_in_args(line: u32, tainted: &HashSet<String>, source: &str) -> bool {
    let line_text = match source.lines().nth((line - 1) as usize) {
        Some(l) => l,
        None => return false,
    };

    tainted
        .iter()
        .any(|t| line_text.contains(t.as_str()) && is_whole_var_match(line_text, t))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::language::Language;

    #[test]
    fn test_sql_injection_basic() {
        let code = r#"
import { db } from './db';
app.post('/users', (req, res) => {
    const name = req.body.name;
    db.query('SELECT * FROM users WHERE name = ' + name);
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        let sql = vulns.iter().find(|v| v.rule_id == "security/sql-injection");
        assert!(
            sql.is_some(),
            "Should detect SQL injection, got: {:?}",
            vulns
        );
    }

    #[test]
    fn test_xss_innerhtml() {
        let code = r#"
app.get('/greet', (req, res) => {
    const name = req.query.name;
    el.innerHTML = '<h1>Hello ' + name + '</h1>';
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        let xss = vulns.iter().find(|v| v.rule_id == "security/xss");
        assert!(xss.is_some(), "Should detect XSS, got: {:?}", vulns);
    }

    #[test]
    fn test_command_injection() {
        let code = r#"
app.post('/run', (req, res) => {
    const cmd = req.body.command;
    exec(cmd);
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        let ci = vulns
            .iter()
            .find(|v| v.rule_id == "security/command-injection");
        assert!(
            ci.is_some(),
            "Should detect command injection, got: {:?}",
            vulns
        );
    }

    #[test]
    fn test_path_traversal() {
        let code = r#"
app.get('/files', (req, res) => {
    const filename = req.query.file;
    fs.readFileSync(filename);
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        let pt = vulns
            .iter()
            .find(|v| v.rule_id == "security/path-traversal");
        assert!(
            pt.is_some(),
            "Should detect path traversal, got: {:?}",
            vulns
        );
    }

    #[test]
    fn test_ssrf_fetch() {
        let code = r#"
app.post('/proxy', (req, res) => {
    const url = req.body.url;
    fetch(url);
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        let ssrf = vulns.iter().find(|v| v.rule_id == "security/ssrf");
        assert!(ssrf.is_some(), "Should detect SSRF, got: {:?}", vulns);
    }

    #[test]
    fn test_sanitized_no_false_positive() {
        let code = r#"
app.post('/users', (req, res) => {
    const name = escape(req.body.name);
    db.query('SELECT * FROM users WHERE name = ' + name);
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        let sql = vulns.iter().find(|v| v.rule_id == "security/sql-injection");
        assert!(
            sql.is_none(),
            "Should NOT flag sanitized input, got: {:?}",
            vulns
        );
    }

    #[test]
    fn test_taint_propagation_chain() {
        let code = r#"
app.post('/search', (req, res) => {
    const input = req.body.q;
    const query = input;
    db.query('SELECT * FROM items WHERE name = ' + query);
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        let sql = vulns.iter().find(|v| v.rule_id == "security/sql-injection");
        assert!(
            sql.is_some(),
            "Should detect through propagation chain, got: {:?}",
            vulns
        );
    }

    #[test]
    fn test_open_redirect() {
        let code = r#"
app.get('/go', (req, res) => {
    const url = req.query.url;
    res.redirect(url);
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        let redir = vulns.iter().find(|v| v.rule_id == "security/open-redirect");
        assert!(
            redir.is_some(),
            "Should detect open redirect, got: {:?}",
            vulns
        );
    }

    #[test]
    fn test_no_vuln_in_safe_code() {
        let code = r#"
app.get('/users', (req, res) => {
    db.query('SELECT * FROM users WHERE id = $1', [req.params.id]);
    res.json({ users: [] });
});
"#;
        let vulns = analyze(code, Language::TypeScript);
        assert!(
            vulns.is_empty(),
            "Safe code should have no vulns, got: {:?}",
            vulns
        );
    }
}
