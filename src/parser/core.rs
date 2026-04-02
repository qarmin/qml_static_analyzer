//! Core parser state machine: the `Parser` struct and its `impl` block,
//! plus the intermediate result types `ElementBody` and `FunctionBodyData`.

use super::error::ParseError;
use super::expression::{
    collect_arrow_params, collect_function_keyword_params, collect_names_from_expression, is_js_keyword,
    try_parse_catch_param, try_parse_destructure_decl, try_parse_for_vars, try_parse_member_assignment,
    try_parse_method_shorthand_params, try_parse_object_key, try_parse_var_decl,
};
use super::helpers::{
    extract_loader_source_types, is_signal_handler_block, parse_function_header, parse_property_decl,
    parse_signal_decl, parse_type_open, parse_type_open_single_line, strip_comment, try_parse_id,
    try_parse_property_element_open,
};
use crate::types::{FileItem, Function, FunctionUsedName, MemberAssignment, Property, QmlChild, Signal};

// ─── internal parser state ──────────────────────────────────────────────────

pub(super) struct Parser<'src> {
    lines: Vec<(usize, &'src str)>, // (1-based line number, trimmed content)
    pos: usize,
}

impl<'src> Parser<'src> {
    pub(super) fn new(source: &'src str) -> Self {
        let lines: Vec<(usize, &'src str)> = source
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim()))
            .filter(|(_, l)| !l.is_empty())
            .collect();
        Parser { lines, pos: 0 }
    }

    fn peek(&self) -> Option<(usize, &'src str)> {
        self.lines.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<(usize, &'src str)> {
        let v = self.lines.get(self.pos).copied();
        if v.is_some() {
            self.pos += 1;
        }
        v
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        let line = self.peek().map_or(0, |(l, _)| l);
        ParseError {
            line,
            message: msg.into(),
        }
    }

    // ─── file-level ────────────────────────────────────────────────────────

    pub(super) fn parse_file(&mut self, name: &str) -> Result<FileItem, ParseError> {
        let mut imports = Vec::new();

        // Collect import lines (and skip any comments / pragma lines between them)
        while let Some((_, line)) = self.peek() {
            if line.starts_with("import ") {
                imports.push(strip_comment(line).trim().to_string());
                self.advance();
            } else if line.starts_with("//") || line.starts_with("pragma ") {
                self.advance();
            } else {
                break;
            }
        }

        // Next non-empty line must be `TypeName {` (the root element)
        let (lineno, line) = self.advance().ok_or_else(|| self.err("Expected root element"))?;

        let base_type = parse_type_open(line).ok_or_else(|| ParseError {
            line: lineno,
            message: format!("Expected root element declaration, got: `{line}`"),
        })?;

        // Parse the body of the root element
        let body = self.parse_element_body()?;

        Ok(FileItem {
            name: name.to_string(),
            base_type,
            id: body.id,
            imports,
            signals: body.signals,
            properties: body.properties,
            functions: body.functions,
            children: body.children,
            assignments: body.assignments,
        })
    }

    // ─── element body ──────────────────────────────────────────────────────

    /// Parse everything between `{` and its matching `}`.
    /// The opening `{` was already consumed as part of the type-name line.
    fn parse_element_body(&mut self) -> Result<ElementBody, ParseError> {
        let mut body = ElementBody::default();

        loop {
            let Some((lineno, raw_line)) = self.peek() else {
                return Err(self.err("Unexpected end of input inside element body"));
            };

            let line = strip_comment(raw_line).trim();

            // `}` alone, or `},` / `};` (element separator inside `[…]` arrays)
            if line == "}" || line == "}," || line == "};" {
                self.advance(); // consume the `}`
                break;
            }

            // ── id: ────────────────────────────────────────────────────────
            if let Some(id_val) = try_parse_id(line) {
                body.id = Some(id_val);
                self.advance();
                continue;
            }

            // ── signal declaration ─────────────────────────────────────────
            if line.starts_with("signal ") {
                let sig = parse_signal_decl(line).ok_or_else(|| ParseError {
                    line: lineno,
                    message: format!("Malformed signal declaration: `{line}`"),
                })?;
                body.signals.push(sig);
                self.advance();
                continue;
            }

            // ── property declaration ───────────────────────────────────────
            if line.starts_with("property ")
                || line.starts_with("required property ")
                || line.starts_with("readonly property ")
            {
                let prop_line = line
                    .strip_prefix("required ")
                    .or_else(|| line.strip_prefix("readonly "))
                    .unwrap_or(line);
                let mut prop = parse_property_decl(prop_line).ok_or_else(|| ParseError {
                    line: lineno,
                    message: format!("Malformed property declaration: `{line}`"),
                })?;
                prop.line = lineno;
                body.properties.push(prop);
                self.advance();
                // If the declaration's value opens a JS/element block on the same line,
                // consume it so the surrounding element body parses correctly.
                // e.g. `readonly property Foo bar: SomeType {`
                //
                // We do NOT skip `[` arrays here: an array may contain QML elements with
                // `id:` values needed by the parent scope (e.g. `readonly property
                // list<QtObject> __data: [ Component { id: menuComp … }, … ]`).
                // Fix #1 (`},` closes element bodies) already handles the array separators.
                if line.ends_with('{') {
                    self.skip_block()?;
                }
                continue;
            }

            // ── function / signal handler ──────────────────────────────────
            if line.starts_with("function ") {
                self.advance(); // consume the `function …(…) {` line
                let func = self.parse_function(line, lineno)?;
                body.functions.push(func);
                continue;
            }

            // `async function name(…) {` — strip the `async ` prefix before parsing.
            if line.starts_with("async function ") {
                self.advance();
                let func = self.parse_function(&line["async ".len()..], lineno)?;
                body.functions.push(func);
                continue;
            }

            // ── inline signal-handler block  onXxx: { … } ─────────────────
            if is_signal_handler_block(line) {
                self.advance();
                let func = self.parse_inline_handler(line, lineno)?;
                body.functions.push(func);
                continue;
            }

            // ── single-line child element  TypeName { } ────────────────────
            if let Some(type_name) = parse_type_open_single_line(line) {
                self.advance();
                body.children.push(QmlChild {
                    type_name,
                    id: None,
                    properties: vec![],
                    functions: vec![],
                    children: vec![],
                    assignments: vec![],
                    line: lineno,
                });
                continue;
            }

            // ── child element  TypeName { ──────────────────────────────────
            if let Some(type_name) = parse_type_open(line) {
                self.advance();

                // Connections { … } — skip body but extract the `id` if present,
                // so that code referencing the id (e.g. `czoker.x`) doesn't get
                // flagged as undefined.
                if type_name == "Connections" {
                    let connections_id = self.parse_connections_for_id()?;
                    if let Some(id) = connections_id {
                        body.children.push(QmlChild {
                            type_name: "Connections".to_string(),
                            id: Some(id),
                            properties: vec![],
                            functions: vec![],
                            children: vec![],
                            assignments: vec![],
                            line: lineno,
                        });
                    }
                    continue;
                }

                // Loader { … } — extract children from source: "qrc:/…/TypeName.qml"
                if type_name == "Loader" {
                    let loader_children = self.parse_loader_body()?;
                    body.children.extend(loader_children);
                    continue;
                }

                let child_body = self.parse_element_body()?;
                body.children.push(QmlChild {
                    type_name,
                    id: child_body.id,
                    properties: child_body.properties,
                    functions: child_body.functions,
                    children: child_body.children,
                    assignments: child_body.assignments,
                    line: lineno,
                });
                continue;
            }

            // ── property: TypeName { — inline element via property ─────────
            // e.g. `delegate: Rectangle {`  or  `header: ToolBar {`
            if let Some(type_name) = try_parse_property_element_open(line) {
                self.advance();
                let child_body = self.parse_element_body()?;
                body.children.push(QmlChild {
                    type_name,
                    id: child_body.id,
                    properties: child_body.properties,
                    functions: child_body.functions,
                    children: child_body.children,
                    assignments: child_body.assignments,
                    line: lineno,
                });
                continue;
            }

            // ── property: [ — array block assignment, skip entire block ──────
            // e.g. `states: [  State { … }, … ]`
            if line.contains(':') && line.ends_with('[') {
                self.advance();
                self.skip_bracket_block()?;
                continue;
            }

            // ── property: { — JavaScript expression/object block, skip ──────
            // e.g. `sourceComponent: { ternary ? a : b }`
            // Special case: `model: [{` — the value starts with `[` so the `{` is
            // inside an array literal; use skip_bracket_block() to track the outer `[`
            // and avoid exiting prematurely at `}, {` separators.
            if line.contains(':') && line.ends_with('{') {
                self.advance();
                let val_starts_with_bracket = line.find(':').is_some_and(|p| line[p + 1..].trim().starts_with('['));
                if val_starts_with_bracket {
                    self.skip_bracket_block()?;
                } else {
                    self.skip_block()?;
                }
                continue;
            }

            // ── standalone `{` — JS object/block continuation from previous line ──
            // e.g. the second argument object in `showDialog(...,\n    {`)
            if line == "{" {
                self.advance();
                self.skip_block()?;
                continue;
            }

            // ── property assignment (e.g. `width: 400` or `Layout.fillWidth: true`) ──
            if line.contains(':') && !line.ends_with('{') {
                // Record plain (non-dotted) assignments for unknown-property checking
                if let Some(colon_pos) = line.find(':') {
                    let key = line[..colon_pos].trim();
                    // Key must be a plain QML property identifier: starts lowercase,
                    // only alphanumeric/_/$, not a JS keyword. This filters out
                    // continuation lines (e.g. `qsTr("Exit") :`, `true :`) that
                    // are actually parts of multi-line expressions.
                    let value = line[colon_pos + 1..].trim().to_string();
                    // Also require a non-empty value: an empty RHS means this line is a
                    // continuation of a multi-line expression (e.g. a ternary branch
                    // `fixationGoodColor :` on its own line), not a real QML assignment.
                    if !value.is_empty()
                        && !key.is_empty()
                        && !key.contains(' ')
                        && key.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                        && key
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.')
                        && !is_js_keyword(key)
                    {
                        body.assignments.push((key.to_string(), value, lineno));
                    }
                }
                self.advance();
                continue;
            }

            // ── anything else: skip ────────────────────────────────────────
            self.advance();
        }

        Ok(body)
    }

    // ─── function body ─────────────────────────────────────────────────────

    /// Called after advancing past the `function foo(…) {` line.
    pub(super) fn parse_function(&mut self, header: &str, lineno: usize) -> Result<Function, ParseError> {
        let (name, params, has_open_brace) = parse_function_header(header).ok_or_else(|| ParseError {
            line: 0,
            message: format!("Malformed function header: `{header}`"),
        })?;

        let is_signal_handler =
            name.starts_with("on") && name.len() > 2 && name.chars().nth(2).is_some_and(|c| c.is_uppercase());

        if !has_open_brace {
            while let Some((_, l)) = self.advance() {
                if strip_comment(l).trim().contains('{') {
                    break;
                }
            }
        }

        // If the entire body is already closed on the header line
        // (e.g. `function toggle() {}` or `function reset() { x = 0; }`),
        // skip reading further lines to avoid consuming the outer `}`.
        if has_open_brace && net_brace_depth(header) <= 0 {
            return Ok(Function {
                name,
                is_signal_handler,
                parameters: params,
                used_names: vec![],
                declared_locals: vec![],
                member_assignments: vec![],
                line: lineno,
            });
        }

        let body = self.collect_function_body_names()?;

        Ok(Function {
            name,
            is_signal_handler,
            parameters: params,
            used_names: body.used_names,
            declared_locals: body.declared_locals,
            member_assignments: body.member_assignments,
            line: lineno,
        })
    }

    /// Parse an inline handler like `onHeightChanged: { … }` or `onClicked: doSomething()`.
    /// Also handles arrow-function forms: `onSelectPoint: (id) => { … }`.
    fn parse_inline_handler(&mut self, header: &str, lineno: usize) -> Result<Function, ParseError> {
        let colon_pos = header.find(':').expect("inline handler header must contain ':'");
        let name_raw = header[..colon_pos].trim();
        let name = name_raw.strip_suffix("()").unwrap_or(name_raw).to_string();
        let rest = header[colon_pos + 1..].trim();

        let is_signal_handler = !name.contains('.');

        // Arrow params declared on the header line (e.g. `(id) =>`)
        let header_arrow_params = collect_arrow_params(rest);

        // `function (param) {` or `function(param) {` form — extract params.
        // `parse_function_header` requires `"function "` (space after keyword) so it fails
        // for the no-space form `function(event)`. Use `collect_function_keyword_params`
        // which handles both `function(` and `function (` transparently.
        let header_function_params: Vec<String> = if rest.starts_with("function ") || rest.starts_with("function(") {
            collect_function_keyword_params(rest)
        } else {
            vec![]
        };

        if rest.is_empty() {
            // Handler body begins on the next line, e.g.:
            //   onCurrentItemChanged:
            //       if (cond) { ... }
            let Some((_, next_raw)) = self.peek() else {
                return Ok(Function {
                    name,
                    is_signal_handler,
                    parameters: vec![],
                    used_names: vec![],
                    declared_locals: vec![],
                    member_assignments: vec![],
                    line: lineno,
                });
            };
            let next_line = strip_comment(next_raw).trim().to_string();
            self.advance();
            return if next_line.ends_with('{') {
                let paren_depth: i32 = next_line.chars().fold(0i32, |d, c| match c {
                    '(' => d + 1,
                    ')' => d - 1,
                    _ => d,
                });
                if paren_depth == 0 {
                    let body = self.collect_function_body_names()?;
                    Ok(Function {
                        name,
                        is_signal_handler,
                        parameters: vec![],
                        used_names: body.used_names,
                        declared_locals: body.declared_locals,
                        member_assignments: body.member_assignments,
                        line: lineno,
                    })
                } else {
                    // `{` is inside unclosed parens — consume until balanced
                    let mut pdepth = paren_depth;
                    let mut bdepth: i32 = next_line.chars().fold(0i32, |d, c| match c {
                        '{' => d + 1,
                        '}' => d - 1,
                        _ => d,
                    });
                    while pdepth > 0 || bdepth > 0 {
                        let Some((_, raw)) = self.advance() else { break };
                        for ch in strip_comment(raw).chars() {
                            match ch {
                                '(' => pdepth += 1,
                                ')' => pdepth -= 1,
                                '{' => bdepth += 1,
                                '}' => bdepth -= 1,
                                _ => {}
                            }
                        }
                    }
                    let used_names = collect_names_from_expression(&next_line);
                    Ok(Function {
                        name,
                        is_signal_handler,
                        parameters: vec![],
                        used_names,
                        declared_locals: vec![],
                        member_assignments: vec![],
                        line: lineno,
                    })
                }
            } else {
                // Expression body starting on the next line, possibly spanning multiple lines:
                //   onOpened:
                //       if (condition)
                //           doSomething()
                // Collect lines until we hit a blank line, closing brace, or a new QML declaration.
                let mut body_lines = vec![next_line];
                loop {
                    let Some((_, peek_raw)) = self.peek() else { break };
                    let peek = strip_comment(peek_raw).trim().to_string();
                    if peek.is_empty()
                        || peek.starts_with('}')
                        || is_signal_handler_block(&peek)
                        || peek.starts_with("function ")
                        || peek.starts_with("property ")
                        || peek.starts_with("readonly ")
                        || peek.starts_with("signal ")
                        || parse_type_open(&peek).is_some()
                    {
                        break;
                    }
                    body_lines.push(peek);
                    self.advance();
                }
                let mut used_names = Vec::new();
                let mut member_assignments = Vec::new();
                for l in &body_lines {
                    used_names.extend(collect_names_from_expression(l));
                    if let Some(ma) = try_parse_member_assignment(l) {
                        member_assignments.push(ma);
                    }
                }
                Ok(Function {
                    name,
                    is_signal_handler,
                    parameters: vec![],
                    used_names,
                    declared_locals: vec![],
                    member_assignments,
                    line: lineno,
                })
            };
        }

        if rest == "{" || rest.ends_with('{') {
            // Check if `{` is at the top level or inside an unclosed `(`.
            // e.g. `onClicked: showDialog(arg, {` — the `{` is inside a call, not the handler body.
            let paren_depth: i32 = rest.chars().fold(0i32, |d, c| match c {
                '(' => d + 1,
                ')' => d - 1,
                _ => d,
            });

            if paren_depth == 0 {
                // True multi-line block: `{ … }`, `(params) => { … }`, `function (p) { … }`
                let body = self.collect_function_body_names()?;
                let mut declared_locals = body.declared_locals;
                declared_locals.extend(header_arrow_params);
                declared_locals.extend(header_function_params);
                return Ok(Function {
                    name,
                    is_signal_handler,
                    parameters: vec![],
                    used_names: body.used_names,
                    declared_locals,
                    member_assignments: body.member_assignments,
                    line: lineno,
                });
            }

            // The `{` is inside an unclosed `(` — multi-line expression, not a block.
            // Consume continuation lines until parens and braces balance to zero.
            let mut pdepth = paren_depth;
            let mut bdepth: i32 = rest.chars().fold(0i32, |d, c| match c {
                '{' => d + 1,
                '}' => d - 1,
                _ => d,
            });
            while pdepth > 0 || bdepth > 0 {
                let Some((_, raw_line)) = self.advance() else { break };
                let line = strip_comment(raw_line).trim();
                for ch in line.chars() {
                    match ch {
                        '(' => pdepth += 1,
                        ')' => pdepth -= 1,
                        '{' => bdepth += 1,
                        '}' => bdepth -= 1,
                        _ => {}
                    }
                }
            }
            let used_names = collect_names_from_expression(rest);
            return Ok(Function {
                name,
                is_signal_handler,
                parameters: vec![],
                used_names,
                declared_locals: header_arrow_params,
                member_assignments: vec![],
                line: lineno,
            });
        }

        // single-expression: `onClicked: doSomething()` or `onXxx: (id) => expr`
        let used_names = collect_names_from_expression(rest);
        Ok(Function {
            name,
            is_signal_handler,
            parameters: vec![],
            used_names,
            declared_locals: header_arrow_params,
            member_assignments: vec![],
            line: lineno,
        })
    }

    /// Skip an entire `[ … ]` bracket block (e.g. `states: [ … ]`), discarding its contents.
    fn skip_bracket_block(&mut self) -> Result<(), ParseError> {
        let mut depth = 1i32;
        loop {
            let Some((_, raw_line)) = self.advance() else {
                return Err(self.err("Unexpected end of input inside bracket block"));
            };
            let line = strip_comment(raw_line).trim();
            // Count [ and ] while skipping string contents.
            let mut in_str = false;
            let mut str_ch = ' ';
            let mut escape = false;
            for ch in line.chars() {
                if escape {
                    escape = false;
                    continue;
                }
                if in_str {
                    if ch == '\\' {
                        escape = true;
                    } else if ch == str_ch {
                        in_str = false;
                    }
                    continue;
                }
                match ch {
                    '"' | '\'' | '`' => {
                        in_str = true;
                        str_ch = ch;
                    }
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Consume a `Connections { … }` block, returning the `id` value if found.
    /// Everything else inside (target, functions, …) is discarded.
    fn parse_connections_for_id(&mut self) -> Result<Option<String>, ParseError> {
        let mut found_id = None;
        let mut depth = 1usize;
        loop {
            let Some((_, raw_line)) = self.advance() else {
                return Err(self.err("Unexpected end of input inside Connections block"));
            };
            let line = strip_comment(raw_line).trim();
            depth = (depth as i32 + net_brace_depth(line)) as usize;
            if depth == 0 {
                return Ok(found_id);
            }
            if depth > 0
                && let Some(id) = try_parse_id(line)
            {
                found_id = Some(id);
            }
        }
    }

    /// Skip an entire `{ … }` block (e.g. Connections), discarding its contents.
    fn skip_block(&mut self) -> Result<(), ParseError> {
        let mut depth = 1i32;
        loop {
            let Some((_, raw_line)) = self.advance() else {
                return Err(self.err("Unexpected end of input inside skipped block"));
            };
            let line = strip_comment(raw_line).trim();
            depth += net_brace_depth(line);
            if depth <= 0 {
                return Ok(());
            }
        }
    }

    /// Parse a `Loader { … }` body.
    fn parse_loader_body(&mut self) -> Result<Vec<QmlChild>, ParseError> {
        let mut loader_id: Option<String> = None;
        let mut source_types: Vec<(String, usize)> = Vec::new();
        let mut children: Vec<QmlChild> = Vec::new();

        loop {
            let Some((lineno, raw_line)) = self.peek() else {
                return Err(self.err("Unexpected end of input inside Loader body"));
            };
            let line = strip_comment(raw_line).trim();

            if line == "}" {
                self.advance();
                break;
            }

            // id:
            if let Some(id_val) = try_parse_id(line) {
                loader_id = Some(id_val);
                self.advance();
                continue;
            }

            // signal / property declaration — skip
            if line.starts_with("signal ")
                || line.starts_with("property ")
                || line.starts_with("required property ")
                || line.starts_with("readonly property ")
            {
                self.advance();
                if line.ends_with('[') {
                    self.skip_bracket_block()?;
                } else if line.ends_with('{') {
                    self.skip_block()?;
                }
                continue;
            }

            // function — consume properly (in case it spans multiple lines)
            if line.starts_with("function ") {
                self.advance();
                self.parse_function(line, lineno)?;
                continue;
            }

            // inline signal-handler block — consume properly
            if is_signal_handler_block(line) {
                self.advance();
                self.parse_inline_handler(line, lineno)?;
                continue;
            }

            // single-line child element  TypeName { }
            if let Some(type_name) = parse_type_open_single_line(line) {
                self.advance();
                children.push(QmlChild {
                    type_name,
                    id: None,
                    properties: vec![],
                    functions: vec![],
                    children: vec![],
                    assignments: vec![],
                    line: lineno,
                });
                continue;
            }

            // multi-line child element  TypeName {
            if let Some(type_name) = parse_type_open(line) {
                self.advance();
                if type_name == "Connections" {
                    let connections_id = self.parse_connections_for_id()?;
                    if let Some(id) = connections_id {
                        children.push(QmlChild {
                            type_name: "Connections".to_string(),
                            id: Some(id),
                            properties: vec![],
                            functions: vec![],
                            children: vec![],
                            assignments: vec![],
                            line: lineno,
                        });
                    }
                    continue;
                }
                if type_name == "Loader" {
                    let nested = self.parse_loader_body()?;
                    children.extend(nested);
                    continue;
                }
                let child_body = self.parse_element_body()?;
                children.push(QmlChild {
                    type_name,
                    id: child_body.id,
                    properties: child_body.properties,
                    functions: child_body.functions,
                    children: child_body.children,
                    assignments: child_body.assignments,
                    line: lineno,
                });
                continue;
            }

            // property: { — JavaScript expression/object block, skip
            if line.contains(':') && line.ends_with('{') {
                self.advance();
                self.skip_block()?;
                continue;
            }

            // property assignment — also check for source: "qrc:/…/Foo.qml"
            if line.contains(':') && !line.ends_with('{') {
                source_types.extend(extract_loader_source_types(line).into_iter().map(|t| (t, lineno)));
                self.advance();
                continue;
            }

            self.advance();
        }

        // Synthetic children from source: "qrc:/…/TypeName.qml"
        if !source_types.is_empty() {
            for (type_name, lineno) in source_types {
                children.push(QmlChild {
                    type_name,
                    id: loader_id.clone(),
                    properties: vec![],
                    functions: vec![],
                    children: vec![],
                    assignments: vec![],
                    line: lineno,
                });
            }
        } else if let Some(id) = loader_id {
            // No source types but the Loader has an id — create a placeholder so the id
            // is preserved in the parent file's element tree (needed for scope analysis).
            children.push(QmlChild {
                type_name: "Loader".to_string(),
                id: Some(id),
                properties: vec![],
                functions: vec![],
                children: vec![],
                assignments: vec![],
                line: 0,
            });
        }

        Ok(children)
    }

    /// Consume lines until the matching `}`, collecting used names, declared locals,
    /// and simple member assignments.
    fn collect_function_body_names(&mut self) -> Result<FunctionBodyData, ParseError> {
        let mut used_names = Vec::new();
        let mut declared_locals = Vec::new();
        let mut member_assignments = Vec::new();
        let mut depth = 1usize;

        loop {
            let Some((lineno, raw_line)) = self.advance() else {
                return Err(self.err("Unexpected end of input inside function body"));
            };

            let line = strip_comment(raw_line).trim();

            // Count braces while skipping string literal contents.
            let mut in_str = false;
            let mut str_ch = ' ';
            let mut escape = false;
            for ch in line.chars() {
                if escape {
                    escape = false;
                    continue;
                }
                if in_str {
                    if ch == '\\' {
                        escape = true;
                    } else if ch == str_ch {
                        in_str = false;
                    }
                    continue;
                }
                match ch {
                    '"' | '\'' | '`' => {
                        in_str = true;
                        str_ch = ch;
                    }
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(FunctionBodyData {
                                used_names,
                                declared_locals,
                                member_assignments,
                            });
                        }
                    }
                    _ => {}
                }
            }

            // Arrow function params: `(a, b) => ...`
            declared_locals.extend(collect_arrow_params(line));

            // `function(params)` callback params: `.then(function (points) {`
            declared_locals.extend(collect_function_keyword_params(line));

            // Catch clause parameter: `catch (e)` or `} catch(_) {`
            if let Some(catch_var) = try_parse_catch_param(line) {
                declared_locals.push(catch_var);
            }

            // Helper: annotate a slice of FunctionUsedName with the current line number.
            let annotate = |mut names: Vec<FunctionUsedName>| -> Vec<FunctionUsedName> {
                for n in &mut names {
                    n.line = lineno;
                }
                names
            };

            // Destructuring declaration: `const { a, b } = expr` or `const [a, b] = expr`
            if let Some((names, rhs)) = try_parse_destructure_decl(line) {
                declared_locals.extend(names);
                if !rhs.is_empty() {
                    used_names.extend(annotate(collect_names_from_expression(&rhs)));
                }
                continue;
            }

            // Variable declaration: `let`/`const`/`var name = expr`
            if let Some((name, rhs)) = try_parse_var_decl(line) {
                declared_locals.push(name.to_string());
                if !rhs.is_empty() {
                    used_names.extend(annotate(collect_names_from_expression(rhs)));
                }
                continue;
            }

            // For-loop variable: `for (const/let/var name of/in expr)`
            {
                let for_vars = try_parse_for_vars(line);
                if !for_vars.is_empty() {
                    declared_locals.extend(for_vars);
                    used_names.extend(annotate(collect_names_from_expression(line)));
                    continue;
                }
            }

            // Member assignment: `obj.member = literal`
            if let Some(assignment) = try_parse_member_assignment(line) {
                used_names.push(FunctionUsedName {
                    name: assignment.object.clone(),
                    accessed_item: Some(assignment.member.clone()),
                    line: lineno,
                });
                member_assignments.push(assignment);
                continue;
            }

            // JS object literal property: `key: expr`
            if let Some(value_part) = try_parse_object_key(line) {
                used_names.extend(annotate(collect_names_from_expression(value_part)));
                continue;
            }

            // JS object method shorthand: `methodName(params) {`
            // The params are that method's own parameters — don't collect them as
            // free variable references that need to be in scope.
            if line.ends_with('{')
                && let Some(method_params) = try_parse_method_shorthand_params(line)
            {
                declared_locals.extend(method_params);
                continue;
            }

            // Normal line
            used_names.extend(annotate(collect_names_from_expression(line)));
        }
    }
}

// ─── brace depth helper ─────────────────────────────────────────────────────

/// Net change in `{}`-depth for one line, skipping brace characters inside string literals.
fn net_brace_depth(line: &str) -> i32 {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut str_ch = ' ';
    let mut escape = false;
    for ch in line.chars() {
        if escape {
            escape = false;
            continue;
        }
        if in_str {
            if ch == '\\' {
                escape = true;
            } else if ch == str_ch {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' | '\'' | '`' => {
                in_str = true;
                str_ch = ch;
            }
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
    }
    depth
}

// ─── intermediate result types ──────────────────────────────────────────────

pub(super) struct FunctionBodyData {
    pub used_names: Vec<FunctionUsedName>,
    pub declared_locals: Vec<String>,
    pub member_assignments: Vec<MemberAssignment>,
}

#[derive(Default)]
pub(super) struct ElementBody {
    pub id: Option<String>,
    pub signals: Vec<Signal>,
    pub properties: Vec<Property>,
    pub functions: Vec<Function>,
    pub children: Vec<QmlChild>,
    pub assignments: Vec<(String, String, usize)>,
}
