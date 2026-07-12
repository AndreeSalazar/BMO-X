//! C Preprocessor — handles #define, #include, #ifdef, #if, #endif.
//!
//! Runs before the tokenizer. Expands macros, resolves includes,
//! evaluates conditional compilation, and outputs a clean source string.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use crate::CError;
use crate::StandardFeatures;

#[derive(Debug, Clone)]
struct MacroDef {
    params: Vec<String>,
    body: String,
}

pub struct Preprocessor {
    defines: HashMap<String, MacroDef>,
    include_paths: Vec<PathBuf>,
    features: StandardFeatures,
    line: usize,
}

impl Preprocessor {
    pub fn new(features: &StandardFeatures, include_paths: Vec<PathBuf>) -> Self {
        let mut pp = Self {
            defines: HashMap::new(),
            include_paths,
            features: features.clone(),
            line: 0,
        };
        pp.defines.insert("__BMO__".into(), MacroDef { params: vec![], body: "1".into() });
        pp.defines.insert("__STDC__".into(), MacroDef { params: vec![], body: "1".into() });
        pp
    }

    /// Helper: get the current skip status (should we emit this line?)
    fn is_active(&self, skip_active: &[bool]) -> bool {
        skip_active.last().copied().unwrap_or(true)
    }

    pub fn preprocess(&mut self, source: &str, file_path: &Path) -> Result<String, CError> {
        if let Some(parent) = file_path.parent() {
            if !self.include_paths.contains(&parent.to_path_buf()) {
                self.include_paths.insert(0, parent.to_path_buf());
            }
        }

        let lines: Vec<&str> = source.lines().collect();
        let mut output = String::with_capacity(source.len());
        let mut skip_active: Vec<bool> = vec![true];
        let mut i = 0usize;

        while i < lines.len() {
            self.line = i + 1;
            let raw = lines[i].trim();

            if raw.starts_with('#') {
                let directive = raw[1..].trim_start();
                let (cmd, rest) = split_first_word(directive);

                match cmd {
                    "define" => {
                        if self.is_active(&skip_active) {
                            self.handle_define(rest)?;
                        }
                    }
                    "undef" => {
                        if self.is_active(&skip_active) {
                            self.defines.remove(rest.trim());
                        }
                    }
                    "include" => {
                        if self.is_active(&skip_active) {
                            let included = self.handle_include(rest, file_path)?;
                            output.push_str(&included);
                            output.push('\n');
                        }
                    }
                    "ifdef" => {
                        let name = rest.trim();
                        let is_def = self.defines.contains_key(name);
                        let parent = self.is_active(&skip_active);
                        skip_active.push(parent && is_def);
                    }
                    "ifndef" => {
                        let name = rest.trim();
                        let is_def = self.defines.contains_key(name);
                        let parent = self.is_active(&skip_active);
                        skip_active.push(parent && !is_def);
                    }
                    "if" => {
                        let val = self.eval_if_expr(rest)?;
                        let parent = self.is_active(&skip_active);
                        skip_active.push(parent && val != 0);
                    }
                    "else" => {
                        if let Some(prev) = skip_active.pop() {
                            let parent = self.is_active(&skip_active);
                            skip_active.push(parent && !prev);
                        }
                    }
                    "elif" => {
                        skip_active.pop();
                        let val = self.eval_if_expr(rest)?;
                        let parent = self.is_active(&skip_active);
                        skip_active.push(parent && val != 0);
                    }
                    "endif" => {
                        skip_active.pop();
                    }
                    "error" => {
                        if self.is_active(&skip_active) {
                            return Err(CError::new(self.line, format!("#error: {}", rest)));
                        }
                    }
                    "pragma" => {
                        // skip (handled by include guard)
                    }
                    _ => {}
                }
            } else {
                if self.is_active(&skip_active) {
                    let expanded = self.expand_line(raw, true);
                    output.push_str(&expanded);
                    output.push('\n');
                }
            }
            i += 1;
        }

        if skip_active.len() != 1 {
            return Err(CError::new(self.line, "unterminated #if/#ifdef (missing #endif)"));
        }

        Ok(output)
    }

    fn handle_define(&mut self, rest: &str) -> Result<(), CError> {
        let rest = rest.trim();
        let (name, params, body) = if let Some(pos) = rest.find('(') {
            let name = rest[..pos].to_string();
            let end = rest[pos..].find(')')
                .ok_or_else(|| CError::new(self.line, "#define: missing )"))?;
            let end_pos = pos + end;
            let params: Vec<String> = rest[pos+1..end_pos]
                .split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let body = rest[end_pos+1..].trim().to_string();
            (name, params, body)
        } else {
            let (name_part, rest_part) = split_first_word(rest);
            (name_part.to_string(), vec![], rest_part.trim().to_string())
        };
        if name.is_empty() {
            return Err(CError::new(self.line, "#define: empty name"));
        }
        self.defines.insert(name, MacroDef { params, body });
        Ok(())
    }

    fn handle_include(&mut self, rest: &str, current_file: &Path) -> Result<String, CError> {
        let path_str = rest.trim().trim_matches('"').trim_matches('<').trim_matches('>').trim_matches('"');
        let mut found: Option<PathBuf> = None;

        // Search relative to current file for "..." includes
        if rest.trim().starts_with('"') {
            if let Some(parent) = current_file.parent() {
                let p = parent.join(path_str);
                if p.exists() { found = Some(p); }
            }
        }

        // Search include paths
        if found.is_none() {
            for base in &self.include_paths {
                let p = base.join(path_str);
                if p.exists() { found = Some(p); break; }
                if let Some(fname) = Path::new(path_str).file_name() {
                    let p2 = base.join(fname);
                    if p2.exists() { found = Some(p2); break; }
                }
            }
        }

        match found {
            Some(p) => {
                let content = fs::read_to_string(&p)
                    .map_err(|e| CError::new(self.line,
                        format!("#include cannot read {}: {}", p.display(), e)))?;
                let mut sub = Preprocessor {
                    defines: self.defines.clone(),
                    include_paths: self.include_paths.clone(),
                    features: self.features.clone(),
                    line: 0,
                };
                sub.preprocess(&content, &p)
            }
            None => Err(CError::new(self.line,
                format!("#include: file not found: {}", path_str))),
        }
    }

    fn eval_if_expr(&mut self, expr: &str) -> Result<i64, CError> {
        let expr = expr.trim();
        if expr.is_empty() { return Ok(0); }

        let mut expanded = expr.to_string();
        while let Some(pos) = expanded.find("defined(") {
            let start = pos + 8;
            if let Some(end) = expanded[start..].find(')') {
                let name = expanded[start..start+end].trim();
                let is_def = if self.defines.contains_key(name) { "1" } else { "0" };
                expanded.replace_range(pos..start+end+1, is_def);
            } else { break; }
        }
        for (name, _) in self.defines.clone().iter() {
            let pattern = format!("defined {}", name);
            if expanded.contains(&pattern) { expanded = expanded.replace(&pattern, "1"); }
        }
        expanded = self.expand_line(&expanded, false);
        Self::eval_simple(&expanded).ok_or_else(||
            CError::new(self.line, format!("#if: cannot evaluate '{}'", expanded)))
    }

    fn eval_simple(expr: &str) -> Option<i64> {
        let expr = expr.trim();
        if expr.is_empty() { return Some(0); }
        if let Ok(n) = expr.parse::<i64>() { return Some(n); }
        if let Some(inner) = expr.strip_prefix('!') {
            let v = Self::eval_simple(inner)?;
            return Some(if v == 0 { 1 } else { 0 });
        }
        let ops = ["==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^"];
        for op in &ops {
            if let Some(pos) = expr.find(op) {
                let a = Self::eval_simple(expr[..pos].trim())?;
                let b = Self::eval_simple(expr[pos+op.len()..].trim())?;
                return match *op {
                    "==" => Some(if a == b { 1 } else { 0 }),
                    "!=" => Some(if a != b { 1 } else { 0 }),
                    "<=" | ">=" => Some(if (op.as_bytes()[0] == b'<' && a <= b) || (op.as_bytes()[0] == b'>' && a >= b) { 1 } else { 0 }),
                    "<"  => Some(if a < b  { 1 } else { 0 }),
                    ">"  => Some(if a > b  { 1 } else { 0 }),
                    "&&" => Some(if a != 0 && b != 0 { 1 } else { 0 }),
                    "||" => Some(if a != 0 || b != 0 { 1 } else { 0 }),
                    "+" => Some(a.wrapping_add(b)),
                    "-" => Some(a.wrapping_sub(b)),
                    "*" => Some(a.wrapping_mul(b)),
                    "/" => if b != 0 { Some(a / b) } else { None },
                    "%" => if b != 0 { Some(a % b) } else { None },
                    "&" => Some(a & b), "|" => Some(a | b), "^" => Some(a ^ b),
                    "<<" => Some(a.wrapping_shl(b as u32)),
                    ">>" => Some(a.wrapping_shr(b as u32)),
                    _ => None,
                };
            }
        }
        None
    }

    fn expand_line(&self, line: &str, _report_errors: bool) -> String {
        let mut result = line.to_string();
        let mut names: Vec<&String> = self.defines.keys().collect();
        names.sort_by_key(|n| -(n.len() as i32));
        for _ in 0..10 {
            let mut expanded = false;
            for name in &names {
                let m = &self.defines[*name];
                if m.params.is_empty() && result.contains(*name) {
                    let mut new = String::new();
                    let bytes = result.as_bytes();
                    let name_bytes = name.as_bytes();
                    let mut pos = 0usize;
                    while pos < bytes.len() {
                        if pos + name_bytes.len() <= bytes.len()
                            && &bytes[pos..pos + name_bytes.len()] == name_bytes
                            && (pos == 0 || !is_ident_char(bytes[pos-1]))
                            && (pos + name_bytes.len() == bytes.len() || !is_ident_char(bytes[pos + name_bytes.len()]))
                        {
                            new.push_str(&m.body);
                            pos += name_bytes.len();
                            expanded = true;
                        } else {
                            new.push(bytes[pos] as char);
                            pos += 1;
                        }
                    }
                    result = new;
                }
            }
            if !expanded { break; }
        }
        result
    }
}

fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    if let Some(pos) = s.find(|c: char| c.is_whitespace()) {
        (&s[..pos], s[pos..].trim_start())
    } else {
        (s, "")
    }
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
