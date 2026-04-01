//! Helpers for collecting identifier/name references from JS expressions.

use crate::types::{FunctionUsedName, MemberAssignment, PropertyValue};

// ─── regex literal preprocessing ────────────────────────────────────────────

/// Preprocess JS regex literals: replace `/pattern/flags` with a space.
///
/// This prevents the content of regex character classes (e.g. `/[0-9A-Za-z]+/`) from
/// being tokenized as identifiers, which would cause false "undefined name" errors.
///
/// Heuristic: a `/` starts a regex literal when the preceding non-whitespace character
/// is NOT an identifier character, `)`, or `]` (those positions indicate division).
pub fn preprocess_regex_literals(expr: &str) -> String {
    let chars: Vec<char> = expr.chars().collect();
    let mut result = String::with_capacity(expr.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        // Pass quoted strings through unchanged (they are handled by tokenize_idents).
        if ch == '"' || ch == '\'' {
            result.push(ch);
            let delim = ch;
            i += 1;
            while i < chars.len() {
                let c = chars[i];
                result.push(c);
                if c == '\\' && i + 1 < chars.len() {
                    i += 1;
                    result.push(chars[i]);
                } else if c == delim {
                    break;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if ch == '/' {
            // Determine regex vs division: `/` is a regex start unless preceded by
            // an identifier char, digit, `)`, or `]` (those mean it's division).
            let prev = result.trim_end().chars().last();
            let is_division = match prev {
                Some(c) => c.is_alphanumeric() || c == '_' || c == ')' || c == ']',
                None => false,
            };
            if !is_division {
                // Skip the regex literal body.
                i += 1;
                let mut in_class = false;
                while i < chars.len() {
                    match chars[i] {
                        '\\' => {
                            i += 1; // skip escaped char (the next char is consumed below)
                        }
                        '[' if !in_class => {
                            in_class = true;
                        }
                        ']' if in_class => {
                            in_class = false;
                        }
                        '/' if !in_class => {
                            i += 1; // consume closing /
                            break;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                // Skip regex flags (g, i, m, s, u, y, d, v …)
                while i < chars.len() && chars[i].is_alphabetic() {
                    i += 1;
                }
                result.push(' '); // replace the whole literal with a separator
                continue;
            }
        }
        result.push(ch);
        i += 1;
    }
    result
}

// ─── template literal preprocessing ────────────────────────────────────────

/// Preprocess template literals: keep only the `${…}` interpolation content, strip plain text.
///
/// Example: `` `hello ${name} world` `` → `" name "` (only the interpolated part remains).
/// This prevents plain text tokens inside backtick strings (e.g. `{word}`) from being
/// misidentified as identifiers.
pub fn preprocess_template_literals(expr: &str) -> String {
    let mut result = String::with_capacity(expr.len());
    let mut in_template = false;
    let mut in_interp = false;
    let mut interp_depth = 0u32;
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if in_interp {
            match ch {
                '{' => {
                    interp_depth += 1;
                    result.push(ch);
                }
                '}' => {
                    if interp_depth == 0 {
                        in_interp = false;
                        result.push(' '); // separator so next token is separate
                    } else {
                        interp_depth -= 1;
                        result.push(ch);
                    }
                }
                _ => result.push(ch),
            }
        } else if in_template {
            match ch {
                '`' => in_template = false,
                '$' if i + 1 < chars.len() && chars[i + 1] == '{' => {
                    in_interp = true;
                    interp_depth = 0;
                    result.push(' '); // separator before interpolation
                    i += 2; // skip '$' and '{'
                    continue;
                }
                _ => {} // plain text inside template — skip
            }
        } else {
            match ch {
                '`' => in_template = true,
                _ => result.push(ch),
            }
        }
        i += 1;
    }
    result
}

// ─── name collection ────────────────────────────────────────────────────────

/// Extract all `name` and `name.member` pairs referenced in a JS expression.
pub fn collect_names_from_expression(expr: &str) -> Vec<FunctionUsedName> {
    let preprocessed = preprocess_template_literals(expr);
    let mut result = Vec::new();
    let tokens = tokenize_idents(&preprocessed);

    let mut i = 0;
    // Track the previous significant (non-identifier) token so that we can
    // recognise object literal keys: `{key: value}` and `{k1: v, k2: v}`.
    // After `{` or `,` an identifier immediately followed by `:` is a key, not
    // a variable reference.  This correctly handles ternary `a ? b : c` because
    // `b` is preceded by `?`, not `{` or `,`.
    let mut prev_tok: &str = "";
    while i < tokens.len() {
        let tok = &tokens[i];
        // Only real identifiers (start with letter or _)
        if !is_identifier(tok) {
            prev_tok = tok.as_str();
            i += 1;
            // After a closing paren/bracket, skip any chain tail (.member, ?.member, etc.)
            // so that `(expr).length` does not emit `length` as a standalone name.
            if tok == ")" || tok == "]" {
                i = skip_chain_tokens(&tokens, i);
            }
            continue;
        }
        // instanceof: skip the keyword AND the type name that follows it
        if tok == "instanceof" {
            prev_tok = "instanceof";
            i += 1; // skip "instanceof"
            if i < tokens.len() && is_identifier(&tokens[i]) {
                i += 1; // skip the type name (e.g. `Item`)
            }
            continue;
        }
        if is_js_keyword(tok) {
            prev_tok = tok.as_str();
            i += 1;
            continue;
        }
        // Object literal key: `{key: value}` or `{k1: v, k2: v}`.
        // Only skip when the previous significant token was `{` or `,`.
        if (prev_tok == "{" || prev_tok == ",") && i + 1 < tokens.len() && tokens[i + 1] == ":" {
            prev_tok = ":";
            i += 2; // skip key + ":"
            continue;
        }
        // Skip function-call identifiers: name(
        if i + 1 < tokens.len() && tokens[i + 1] == "(" {
            prev_tok = "id";
            i += 2;
            continue;
        }
        // Check if it's identifier.identifier or identifier?.identifier
        let is_dot_chain = i + 2 < tokens.len() && tokens[i + 1] == "." && is_identifier(&tokens[i + 2]);
        let is_opt_chain =
            i + 3 < tokens.len() && tokens[i + 1] == "?" && tokens[i + 2] == "." && is_identifier(&tokens[i + 3]);
        if is_dot_chain || is_opt_chain {
            let member = if is_dot_chain {
                tokens[i + 2].clone()
            } else {
                tokens[i + 3].clone()
            };
            result.push(FunctionUsedName {
                name: tok.clone(),
                accessed_item: Some(member),
                line: 0,
            });
            prev_tok = "id";
            i += if is_dot_chain { 3 } else { 4 };
            // Skip the rest of the chain: .c.d, ?.d, [idx], (), and combinations.
            // None of these introduce new base names that need scope-checking.
            'chain: loop {
                // .ident or ?.ident chains
                while (i + 1 < tokens.len() && tokens[i] == "." && is_identifier(&tokens[i + 1]))
                    || (i + 2 < tokens.len()
                        && tokens[i] == "?"
                        && tokens[i + 1] == "."
                        && is_identifier(&tokens[i + 2]))
                {
                    i += if tokens[i] == "." { 2 } else { 3 };
                }
                // [anything] bracket access or () call — skip to matching closer
                if i < tokens.len() && (tokens[i] == "[" || tokens[i] == "(") {
                    let (opener, closer) = if tokens[i] == "[" { ("[", "]") } else { ("(", ")") };
                    let mut depth = 1usize;
                    i += 1;
                    while i < tokens.len() && depth > 0 {
                        if tokens[i] == opener {
                            depth += 1;
                        } else if tokens[i] == closer {
                            depth -= 1;
                        }
                        i += 1;
                    }
                } else {
                    break 'chain;
                }
            }
        } else {
            result.push(FunctionUsedName {
                name: tok.clone(),
                accessed_item: None,
                line: 0,
            });
            prev_tok = "id";
            i += 1;
            // Skip any following bracket/call/dot chain so chained members
            // (e.g. arr[0].member) don't get flagged as standalone names.
            i = skip_chain_tokens(&tokens, i);
        }
    }

    result
}

/// Skip/process remaining chain tokens after a dot-chain base was pushed.
///
/// Like `skip_chain_tokens` but also collects names from inside `(args)` calls,
/// so that arguments like `BaseFunctions.method(undefinedVar)` cause `undefinedVar`
/// to be checked against scope.  Arrow-function parameters inside the args are
/// detected and excluded to prevent false positives.
fn skip_chain_collect_args(tokens: &[String], mut i: usize, result: &mut Vec<(String, Option<String>)>) -> usize {
    loop {
        if i + 1 < tokens.len() && tokens[i] == "." && is_identifier(&tokens[i + 1]) {
            i += 2; // skip ".member"
        } else if i + 2 < tokens.len() && tokens[i] == "?" && tokens[i + 1] == "." && is_identifier(&tokens[i + 2]) {
            i += 3; // skip "?.member"
        } else if i < tokens.len() && tokens[i] == "(" {
            i += 1; // skip "("
            let arg_start = i;
            let mut depth = 1usize;
            while i < tokens.len() && depth > 0 {
                if tokens[i] == "(" {
                    depth += 1;
                } else if tokens[i] == ")" {
                    depth -= 1;
                }
                i += 1;
            }
            // Process arg content: tokens[arg_start..i-1] (the closing `)` is at i-1).
            if i > arg_start + 1 {
                let arg_tokens = &tokens[arg_start..i - 1];
                // Reconstruct a string; replace split `= >` tokens back into `=>` so
                // that `collect_arrow_params` can detect arrow-function params.
                let arg_expr: String = arg_tokens.join(" ").replace("= >", "=>");
                let arrow_params: std::collections::HashSet<String> =
                    collect_arrow_params(&arg_expr).into_iter().collect();
                for n in collect_names_from_expression(&arg_expr) {
                    if !arrow_params.contains(n.name.as_str()) {
                        result.push((n.name, n.accessed_item));
                    }
                }
            }
        } else if i < tokens.len() && tokens[i] == "[" {
            let mut depth = 1usize;
            i += 1;
            while i < tokens.len() && depth > 0 {
                if tokens[i] == "[" {
                    depth += 1;
                } else if tokens[i] == "]" {
                    depth -= 1;
                }
                i += 1;
            }
        } else {
            break;
        }
    }
    i
}

/// Like `collect_names_from_expression` but only returns the base names.
///
/// Handles chained member access and function calls by skipping their tails:
/// - `BaseFunctions.calculate().INTERNAL_ENUM.ITEM` → only `["BaseFunctions"]`
/// - `BaseFunctions.getCachedFieldList().map(field => field.name)` → `["BaseFunctions"]`
/// - `core.uptime() && core2.value` → `["core", "core2"]`
pub fn collect_base_names_from_expression(expr: &str) -> Vec<String> {
    let preprocessed = preprocess_template_literals(expr);
    let tokens = tokenize_idents(&preprocessed);
    let mut result = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let tok = &tokens[i];
        if !is_identifier(tok) {
            i += 1;
            // After a closing paren/bracket, skip any chain tail (.member, ?.member, etc.)
            // so that `(expr).length` does not emit `length` as a standalone base name.
            if tok == ")" || tok == "]" {
                i = skip_chain_tokens(&tokens, i);
            }
            continue;
        }
        // instanceof: skip the keyword AND the type name that follows it
        if tok == "instanceof" {
            i += 1;
            if i < tokens.len() && is_identifier(&tokens[i]) {
                i += 1;
            }
            continue;
        }
        if is_js_keyword(tok) {
            i += 1;
            continue;
        }
        // `base.member` or `base?.member` — push base, then skip the entire remaining chain
        // (.member, (args), .member, …) so inner tokens aren't wrongly flagged.
        if i + 2 < tokens.len() && tokens[i + 1] == "." && is_identifier(&tokens[i + 2]) {
            result.push(tok.clone());
            i += 3; // consumed base + "." + member
            i = skip_chain_tokens(&tokens, i);
            continue;
        }
        if i + 3 < tokens.len() && tokens[i + 1] == "?" && tokens[i + 2] == "." && is_identifier(&tokens[i + 3]) {
            result.push(tok.clone());
            i += 4; // consumed base + "?" + "." + member
            i = skip_chain_tokens(&tokens, i);
            continue;
        }
        // Standalone function call `func(…)` — skip entirely (don't push name).
        if i + 1 < tokens.len() && tokens[i + 1] == "(" {
            i += 2; // skip identifier and "("
            i = skip_to_matching_close_paren(&tokens, i);
            i = skip_chain_tokens(&tokens, i);
            continue;
        }
        // Plain identifier — also skip any following bracket/call/dot chain
        result.push(tok.clone());
        i += 1;
        i = skip_chain_tokens(&tokens, i);
    }
    result
}

/// Like `collect_base_names_from_expression` but also captures the first member access.
///
/// For `a.b.c` → `("a", Some("b"))` — the deeper chain is discarded.
/// For `foo`   → `("foo", None)`.
/// Used to validate C++ object member access in property value expressions.
///
/// Also processes function call arguments inside chains so that
/// `BaseFunctions.method(undefinedVar)` causes `undefinedVar` to be returned.
/// Object-literal keys (`{key: value}`) are skipped via prev_tok tracking.
pub fn collect_dotted_accesses_from_expression(expr: &str) -> Vec<(String, Option<String>)> {
    let preprocessed = preprocess_template_literals(expr);
    let tokens = tokenize_idents(&preprocessed);
    let mut result = Vec::new();
    let mut i = 0;
    // Track the previous significant token for object-literal key detection:
    // `{key: val}` and `{k1: v, k2: v}` — an identifier followed by `:` after
    // `{` or `,` is a key, not a variable reference.
    let mut prev_tok: &str = "";
    while i < tokens.len() {
        let tok = &tokens[i];
        if !is_identifier(tok) {
            let s = tok.as_str();
            match s {
                "{" | "," => prev_tok = s,
                _ => {}
            }
            i += 1;
            if tok == ")" || tok == "]" {
                i = skip_chain_tokens(&tokens, i);
            }
            continue;
        }
        if tok == "instanceof" {
            prev_tok = "id";
            i += 1;
            if i < tokens.len() && is_identifier(&tokens[i]) {
                i += 1;
            }
            continue;
        }
        if is_js_keyword(tok) {
            prev_tok = tok.as_str();
            i += 1;
            continue;
        }
        // Object literal key: `{key: value}` or `{k1: v, k2: v}`.
        if (prev_tok == "{" || prev_tok == ",") && i + 1 < tokens.len() && tokens[i + 1] == ":" {
            prev_tok = ":";
            i += 2; // skip key + ":"
            continue;
        }
        // `base.member` — capture both; also collect names from any call args in chain.
        if i + 2 < tokens.len() && tokens[i + 1] == "." && is_identifier(&tokens[i + 2]) {
            result.push((tok.clone(), Some(tokens[i + 2].clone())));
            prev_tok = "id";
            i += 3;
            i = skip_chain_collect_args(&tokens, i, &mut result);
            continue;
        }
        // `base?.member`
        if i + 3 < tokens.len() && tokens[i + 1] == "?" && tokens[i + 2] == "." && is_identifier(&tokens[i + 3]) {
            result.push((tok.clone(), Some(tokens[i + 3].clone())));
            prev_tok = "id";
            i += 4;
            i = skip_chain_collect_args(&tokens, i, &mut result);
            continue;
        }
        // Standalone function call — skip (don't collect name, don't process args).
        if i + 1 < tokens.len() && tokens[i + 1] == "(" {
            prev_tok = "id";
            i += 2;
            i = skip_to_matching_close_paren(&tokens, i);
            i = skip_chain_tokens(&tokens, i);
            continue;
        }
        // Plain identifier
        result.push((tok.clone(), None));
        prev_tok = "id";
        i += 1;
        i = skip_chain_tokens(&tokens, i);
    }
    result
}

// ─── token utilities ────────────────────────────────────────────────────────

/// Skip past the closing `)` that matches an already-open paren.
/// `i` starts just after the opening `(` (depth = 1).
/// Returns the position after the `)`.
pub fn skip_to_matching_close_paren(tokens: &[String], mut i: usize) -> usize {
    let mut depth = 1usize;
    while i < tokens.len() && depth > 0 {
        match tokens[i].as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    i
}

/// Skip any further chain suffixes: `.member`, `?.member`, `(args)`, and `[idx]` in any combination.
/// E.g. after consuming `base.member`, skips `.c?.d(e)[0].f` etc.
pub fn skip_chain_tokens(tokens: &[String], mut i: usize) -> usize {
    loop {
        if i + 1 < tokens.len() && tokens[i] == "." && is_identifier(&tokens[i + 1]) {
            i += 2; // skip ".member"
        } else if i + 2 < tokens.len() && tokens[i] == "?" && tokens[i + 1] == "." && is_identifier(&tokens[i + 2]) {
            i += 3; // skip "?.member"
        } else if i < tokens.len() && (tokens[i] == "(" || tokens[i] == "[") {
            let (opener, closer) = if tokens[i] == "(" { ("(", ")") } else { ("[", "]") };
            let mut depth = 1usize;
            i += 1;
            while i < tokens.len() && depth > 0 {
                if tokens[i] == opener {
                    depth += 1;
                } else if tokens[i] == closer {
                    depth -= 1;
                }
                i += 1;
            }
        } else {
            break;
        }
    }
    i
}

pub fn is_identifier(s: &str) -> bool {
    s.starts_with(|c: char| c.is_alphabetic() || c == '_')
}

/// Split expression into identifier tokens and punctuation.
pub fn tokenize_idents(expr: &str) -> Vec<String> {
    let preprocessed = preprocess_regex_literals(expr);
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_char = ' ';
    let mut escape_next = false;

    for ch in preprocessed.chars() {
        if in_string {
            if escape_next {
                escape_next = false;
                continue;
            }
            if ch == '\\' {
                escape_next = true;
                continue;
            }
            if ch == string_char {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                in_string = true;
                string_char = ch;
            }
            '.' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                tokens.push(".".to_string());
            }
            '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | ':' | '=' | '+' | '-' | '*' | '/' | '%' | '!' | '&'
            | '|' | '<' | '>' | '?' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                tokens.push(ch.to_string());
            }
            ' ' | '\t' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Find the index of the `(` matching the `)` at `close_pos` in `s`.
/// Walks backwards, tracking brace depth.
pub fn find_matching_open_paren(s: &str, close_pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 1i32;
    let mut i = close_pos as isize - 1;
    while i >= 0 {
        match bytes[i as usize] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i as usize);
                }
            }
            _ => {}
        }
        i -= 1;
    }
    None
}

/// Extract parameter names from `function(params)` or `function name(params)` patterns in `line`.
///
/// Used inside function bodies to register callback parameters as declared locals so they
/// aren't flagged as undefined variable references.
///
/// Handles:
/// - `.then(function (points) {`  → `["points"]`
/// - `arr.forEach(function(element) {` → `["element"]`
/// - `(function named(a, b) {` → `["a", "b"]`
pub fn collect_function_keyword_params(line: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut rest = line;
    while let Some(kw_pos) = rest.find("function") {
        // Make sure "function" is a whole word (not part of another identifier like "functionName")
        let before_ok = kw_pos == 0 || {
            let c = rest.as_bytes()[kw_pos - 1];
            !c.is_ascii_alphanumeric() && c != b'_'
        };
        let after_kw = &rest[kw_pos + 8..];
        let after_kw_ok = after_kw
            .chars()
            .next()
            .map_or(true, |c| !c.is_alphanumeric() && c != '_');
        rest = after_kw;
        if !before_ok || !after_kw_ok {
            continue;
        }
        let trimmed = after_kw.trim_start();
        // Skip optional function name (named function expression)
        let trimmed = if trimmed.starts_with(|c: char| c.is_alphabetic() || c == '_') {
            let end = trimmed
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(trimmed.len());
            trimmed[end..].trim_start()
        } else {
            trimmed
        };
        if let Some(after_open) = trimmed.strip_prefix('(') {
            if let Some(close) = after_open.find(')') {
                for p in after_open[..close].split(',') {
                    let name = p.trim();
                    if !name.is_empty() && is_identifier(name) && !is_js_keyword(name) {
                        params.push(name.to_string());
                    }
                }
            }
        }
    }
    params
}

/// Extract parameter names from `(params) =>` patterns in `line`.
///
/// Handles all forms:
/// - `onSelectPoint: (id) => { ... }`
/// - `.filter((item) => item.x)`
/// - `save: (a, b, c) => { ... }`
pub fn collect_arrow_params(line: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut search_from = 0;
    while let Some(rel_pos) = line[search_from..].find("=>") {
        let arrow_pos = search_from + rel_pos;
        let before = line[..arrow_pos].trim_end();
        if before.ends_with(')') {
            // `(params) =>` form
            let close = before.len() - 1;
            if let Some(open) = find_matching_open_paren(before, close) {
                let params_str = &before[open + 1..close];
                for p in params_str.split(',') {
                    let name = p.trim();
                    if !name.is_empty() && is_identifier(name) {
                        params.push(name.to_string());
                    }
                }
            }
        } else {
            // `ident =>` form (single param without parens), e.g. `element => expr` or `textChanged => ...`
            // Walk backwards from end of `before` to extract the trailing identifier.
            let bytes = before.as_bytes();
            let mut end = bytes.len();
            while end > 0
                && (bytes[end - 1].is_ascii_alphanumeric() || bytes[end - 1] == b'_' || bytes[end - 1] == b'$')
            {
                end -= 1;
            }
            let ident = &before[end..];
            if !ident.is_empty()
                && ident.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && !is_js_keyword(ident)
            {
                params.push(ident.to_string());
            }
        }
        search_from = arrow_pos + 2;
    }
    params
}

/// If the line contains `catch (name)` or `catch(name)`, returns the catch variable name.
/// Used to add the catch variable to declared_locals so it is not flagged as undefined.
pub fn try_parse_catch_param(line: &str) -> Option<String> {
    let pos = line.find("catch")?;
    let after = line[pos + 5..].trim_start();
    let after = after.strip_prefix('(')?;
    let close = after.find(')')?;
    let name = after[..close].trim();
    if name.is_empty() || is_js_keyword(name) {
        return None;
    }
    if !is_identifier(name) {
        return None;
    }
    Some(name.to_string())
}

/// If the line looks like `for (let/const/var name ...)` or
/// `for (let/const/var [a, b] ...)`, returns the loop variable name(s).
/// Used to add the loop variable(s) to declared_locals.
pub fn try_parse_for_vars(line: &str) -> Vec<String> {
    let Some(rest) = line
        .trim()
        .strip_prefix("for")
        .map(str::trim)
        .and_then(|s| s.strip_prefix('('))
    else {
        return vec![];
    };
    let Some(rest) = rest
        .strip_prefix("let ")
        .or_else(|| rest.strip_prefix("const "))
        .or_else(|| rest.strip_prefix("var "))
        .map(str::trim)
    else {
        return vec![];
    };
    // Array destructuring: `for (const [a, b] of ...)`
    if rest.starts_with('[') {
        let Some(close) = rest.find(']') else {
            return vec![];
        };
        return rest[1..close]
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty() && is_identifier(s))
            .map(str::to_string)
            .collect();
    }
    // Regular single variable
    let name_end = rest
        .find(|c: char| c.is_whitespace() || c == ')' || c == '=')
        .unwrap_or(rest.len());
    let name = &rest[..name_end];
    if name.is_empty() || !is_identifier(name) {
        return vec![];
    }
    vec![name.to_string()]
}

/// If the line looks like `identifier: expr` (a JS object literal property key),
/// returns the part after the colon so only the value is checked for undefined names.
/// The key itself is not a variable reference.
///
/// Requires the key to be a plain single-word identifier (only `[A-Za-z0-9_$]`) to
/// avoid matching things like URLs inside strings (`'qrc:/foo/bar'`) or chained calls.
pub fn try_parse_object_key(line: &str) -> Option<&str> {
    let colon_pos = line.find(':')?;
    let key = line[..colon_pos].trim();
    // Must be a plain single identifier — every character alphanumeric, _ or $
    if key.is_empty() {
        return None;
    }
    if !is_identifier(key) {
        return None;
    }
    if !key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
        return None;
    }
    if is_js_keyword(key) {
        return None;
    }
    Some(&line[colon_pos + 1..])
}

/// If the line looks like a JS object method shorthand: `methodName(params) {`,
/// returns the list of parameter names so they can be added to `declared_locals`
/// instead of being flagged as undefined variable references.
///
/// Returns `None` for anything that doesn't match: JS keywords (`if`, `while`, …),
/// dotted names, etc.
pub fn try_parse_method_shorthand_params(line: &str) -> Option<Vec<String>> {
    let before_brace = line.trim().strip_suffix('{')?.trim();
    let before_paren = before_brace.strip_suffix(')')?;
    let open_paren = before_paren.rfind('(')?;
    let name_part = before_paren[..open_paren].trim();
    let params_str = &before_paren[open_paren + 1..];
    // Name must be a single, non-keyword identifier (no spaces, no dots)
    if name_part.is_empty() || is_js_keyword(name_part) || !is_identifier(name_part) {
        return None;
    }
    if !name_part.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
        return None;
    }
    let params: Vec<String> = params_str
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty() && is_identifier(s))
        .map(str::to_string)
        .collect();
    Some(params)
}

/// If `line` starts with `let`/`const`/`var name`, returns `(name, rhs_expr)`.
pub fn try_parse_var_decl(line: &str) -> Option<(&str, &str)> {
    let rest = line
        .strip_prefix("let ")
        .or_else(|| line.strip_prefix("const "))
        .or_else(|| line.strip_prefix("var "))
        .map(str::trim)?;

    // name ends at first whitespace, `=`, or `;`
    let name_end = rest
        .find(|c: char| c == '=' || c == ';' || c.is_whitespace())
        .unwrap_or(rest.len());
    let name = &rest[..name_end];

    if name.is_empty() || !is_identifier(name) {
        return None;
    }

    let after_name = rest[name_end..].trim_start();
    let rhs = after_name.strip_prefix('=').map_or("", str::trim);

    Some((name, rhs))
}

/// If `line` looks like `obj.member = value`, returns a `MemberAssignment`.
/// Handles bool/int/float literals precisely; uses TooComplex for strings and complex RHS.
/// String literals are consumed by the tokenizer (skipped entirely), so lines like
/// `obj.member = "str"` produce only 4 tokens — those are captured with TooComplex too.
pub fn try_parse_member_assignment(line: &str) -> Option<MemberAssignment> {
    let tokens = tokenize_idents(line);
    // Minimum pattern: [ident, ".", ident, "="]  (RHS may have been consumed as a string)
    if tokens.len() < 4 {
        return None;
    }
    if !is_identifier(&tokens[0])
        || is_js_keyword(&tokens[0])
        || tokens[1] != "."
        || !is_identifier(&tokens[2])
        || tokens[3] != "="
        || tokens.get(4) == Some(&"=".to_string())
    // reject `==`
    {
        return None;
    }
    let object = tokens[0].clone();
    let member = tokens[2].clone();

    let value = if let Some(rhs) = tokens.get(4) {
        let rhs = rhs.as_str();
        match rhs {
            "true" => PropertyValue::Bool(true),
            "false" => PropertyValue::Bool(false),
            _ if rhs.parse::<i64>().is_ok() => PropertyValue::Int(rhs.parse().expect("TODO")),
            _ if rhs.parse::<f64>().is_ok() => PropertyValue::Double(rhs.parse().expect("TODO")),
            _ => PropertyValue::TooComplex, // complex expression
        }
    } else {
        // RHS was entirely consumed by tokenizer — string literal like `"…"` or backtick string
        PropertyValue::TooComplex
    };

    Some(MemberAssignment { object, member, value })
}

pub fn is_js_keyword(s: &str) -> bool {
    matches!(
        s,
        "let"
            | "const"
            | "var"
            | "function"
            | "return"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "break"
            | "continue"
            | "new"
            | "delete"
            | "typeof"
            | "instanceof"
            | "in"
            | "of"
            | "null"
            | "undefined"
            | "true"
            | "false"
            | "this"
            | "super"
            | "class"
            | "import"
            | "export"
            | "from"
            | "try"
            | "catch"
            | "finally"
            | "throw"
            | "switch"
            | "case"
            | "default"
            | "void"
            | "async"
            | "await"
            | "yield"
            // QML type-cast operator: `value as Type`
            | "as"
            // JS built-in objects used as method receivers (console.log(x), JSON.parse(x)):
            // keeping them as "keywords" here ensures that arguments inside the method call
            // (e.g. `msg` in `console.log(msg)`) continue to be collected by the parser,
            // rather than being silently consumed by the chain-tail skip loop.
            | "console"
            | "JSON"
    )
}
