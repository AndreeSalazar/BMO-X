//! C Preprocessor — `#include`, `#define`, `#if`, `#ifdef`, `#endif`.
//!
//! ## Estado actual (v1.8.8)
//!
//! Soporta las directivas más comunes:
//! - `#include <name>` / `#include "name"` (sin resolución real, solo borra)
//! - `#define NAME` (constante vacía)
//! - `#define NAME value` (macro con un solo token)
//! - `#define NAME(a, b) body` (macro con parámetros — implementación básica)
//! - `#undef NAME`
//! - `#if expr` / `#ifdef NAME` / `#ifndef NAME` / `#else` / `#elif` / `#endif`
//! - `#pragma once` (ignorado)
//! - `#error "message"` (error)
//!
//! **No soportado todavía**:
//! - Macros variádicas (`...`, `__VA_ARGS__`)
//! - Stringification (`#x`)
//! - Token pasting (`##`)
//! - `#include` con path real (debería leer del FS)

#![allow(dead_code)]

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use core::fmt;

/// Una macro definida.
#[derive(Clone, Debug)]
pub enum Macro {
    /// `#define NAME` (objeto vacío).
    Empty,
    /// `#define NAME value` (objeto con un valor).
    Object(String),
    /// `#define NAME(params) body` (función-like).
    Function {
        params: Vec<String>,
        body: String,
    },
}

/// Resultado del preprocessing: el texto con directivas resueltas.
pub struct PreprocessResult {
    pub output: String,
    pub macros: BTreeMap<String, Macro>,
    pub errors: Vec<PreprocessError>,
}

/// Error de preprocessor.
#[derive(Clone, Debug)]
pub enum PreprocessError {
    UnknownDirective(String),
    UnterminatedIf,
    UndefinedMacro(String),
    InvalidIfExpression,
    ErrorDirective(String),
}

impl fmt::Display for PreprocessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDirective(d) => write!(f, "unknown directive: #{}", d),
            Self::UnterminatedIf => f.write_str("unterminated #if/#ifdef/#ifndef"),
            Self::UndefinedMacro(n) => write!(f, "undefined macro: {}", n),
            Self::InvalidIfExpression => f.write_str("invalid #if expression"),
            Self::ErrorDirective(m) => write!(f, "#error: {}", m),
        }
    }
}

/// Preprocesa el código C.
pub fn preprocess(source: &[u8], filename: &str) -> Result<PreprocessResult, PreprocessError> {
    let src = core::str::from_utf8(source).map_err(|_| {
        PreprocessError::ErrorDirective("source is not valid UTF-8".into())
    })?;

    let mut macros: BTreeMap<String, Macro> = BTreeMap::new();
    // Macros predefinidas.
    macros.insert("__STDC__".into(), Macro::Object("1".into()));
    macros.insert("__STDC_VERSION__".into(), Macro::Object("201112".into()));
    macros.insert("__FILE__".into(), Macro::Object(format!("\"{}\"", filename)));
    macros.insert("__LINE__".into(), Macro::Object("1".into()));
    // Macros de FastOS.
    macros.insert("__FastOS__".into(), Macro::Object("1".into()));
    macros.insert("__fastos__".into(), Macro::Object("1".into()));

    let mut output = String::new();
    let mut errors = Vec::new();
    let mut if_stack: Vec<bool> = Vec::new(); // true = branch activa
    let mut line_no: u32 = 0;

    for line in src.lines() {
        line_no += 1;
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix('#') {
            // Es una directiva de preprocessor.
            let dir_line = rest.trim_start();
            let mut parts = dir_line.splitn(2, char::is_whitespace);
            let directive = parts.next().unwrap_or("").to_string();
            let arg = parts.next().unwrap_or("").trim().to_string();

            // Si estamos en un branch false, ignorar todo excepto #if/#ifdef/#ifndef/#endif/#else/#elif.
            let in_active = if_stack.iter().all(|&b| b);

            match directive.as_str() {
                "include" if in_active => {
                    output.push_str(&format!("// #include {}\n", arg));
                }
                "define" if in_active => {
                    if let Some((name, rest)) = split_define(&arg) {
                        let m = if rest.is_empty() {
                            Macro::Empty
                        } else if let Some((params, body)) = parse_function_macro(&rest) {
                            Macro::Function { params, body }
                        } else {
                            Macro::Object(rest.to_string())
                        };
                        macros.insert(name.to_string(), m);
                    }
                }
                "undef" if in_active => {
                    macros.remove(&arg);
                }
                "if" if in_active => {
                    let val = eval_if_expr(&arg, &macros);
                    if_stack.push(val);
                }
                "ifdef" => {
                    let defined = macros.contains_key(&arg);
                    if_stack.push(in_active && defined);
                }
                "ifndef" => {
                    let defined = macros.contains_key(&arg);
                    if_stack.push(in_active && !defined);
                }
                "else" => {
                    if let Some(top) = if_stack.last_mut() {
                        *top = !*top;
                    }
                }
                "elif" => {
                    if let Some(top) = if_stack.last_mut() {
                        *top = eval_if_expr(&arg, &macros);
                    }
                }
                "endif" => {
                    if_stack.pop();
                }
                "pragma" => { /* ignorar */ }
                "error" if in_active => {
                    errors.push(PreprocessError::ErrorDirective(arg));
                }
                "" => { /* # vacío */ }
                _ if in_active => {
                    errors.push(PreprocessError::UnknownDirective(directive));
                }
                _ => { /* directiva en branch false: ignorar */ }
            }
        } else {
            // Línea normal: si estamos en branch activo, expandir macros y emitir.
            let in_active = if_stack.iter().all(|&b| b);
            if in_active {
                let expanded = expand_macros(trimmed, &macros, line_no);
                output.push_str(&expanded);
                output.push('\n');
            }
        }
    }

    if !if_stack.is_empty() {
        return Err(PreprocessError::UnterminatedIf);
    }

    Ok(PreprocessResult { output, macros, errors })
}

/// Split de `#define NAME value` o `#define NAME(params) value`.
fn split_define(arg: &str) -> Option<(&str, &str)> {
    let mut chars = arg.char_indices();
    let start = chars.next()?.0;
    loop {
        match chars.next() {
            Some((i, c)) if c == '(' || c.is_whitespace() => {
                let name = &arg[start..i];
                let rest = &arg[i..];
                return Some((name.trim(), rest.trim()));
            }
            None => return Some((arg.trim(), "")),
            _ => {}
        }
    }
}

/// Parse `#define NAME(p1, p2) body`.
fn parse_function_macro(rest: &str) -> Option<(Vec<String>, String)> {
    if !rest.starts_with('(') { return None; }
    let close = rest.find(')')?;
    let params_str = &rest[1..close];
    let body = rest[close+1..].trim().to_string();
    let params: Vec<String> = if params_str.trim().is_empty() {
        Vec::new()
    } else {
        params_str.split(',').map(|p| p.trim().to_string()).collect()
    };
    Some((params, body))
}

/// Evalúa una expresión constante para `#if`.
fn eval_if_expr(expr: &str, macros: &BTreeMap<String, Macro>) -> bool {
    let trimmed = expr.trim();
    if trimmed == "0" || trimmed.is_empty() { return false; }
    if trimmed == "1" { return true; }
    if let Some(m) = macros.get(trimmed) {
        return match m {
            Macro::Empty => false,
            Macro::Object(v) => v != "0",
            Macro::Function { .. } => true,
        };
    }
    // Heurística simple: defined(NAME)
    if let Some(name) = trimmed.strip_prefix("defined ") {
        return macros.contains_key(name.trim());
    }
    // Por defecto: si parece un número distinto de 0, true.
    trimmed.parse::<i64>().map(|n| n != 0).unwrap_or(true)
}

/// Expande macros en una línea (implementación básica, no recursiva).
fn expand_macros(line: &str, macros: &BTreeMap<String, Macro>, _line_no: u32) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_alphabetic() || c == '_' {
            let mut name = String::new();
            name.push(c);
            while let Some(&nc) = chars.peek() {
                if nc.is_ascii_alphanumeric() || nc == '_' { name.push(chars.next().unwrap()); }
                else { break; }
            }
            if let Some(m) = macros.get(&name) {
                match m {
                    Macro::Empty => { /* no emite nada */ }
                    Macro::Object(v) => out.push_str(v),
                    Macro::Function { params, body } => {
                        // Buscar (
                        if chars.peek() == Some(&'(') {
                            chars.next(); // consume (
                            let mut args = Vec::new();
                            let mut cur = String::new();
                            let mut depth = 1;
                            while let Some(&nc) = chars.peek() {
                                if nc == '(' { depth += 1; cur.push(chars.next().unwrap()); }
                                else if nc == ')' {
                                    depth -= 1;
                                    if depth == 0 { chars.next(); break; }
                                    cur.push(')');
                                }
                                else if nc == ',' && depth == 1 { chars.next(); args.push(cur.clone()); cur.clear(); }
                                else { cur.push(chars.next().unwrap()); }
                            }
                            args.push(cur);
                            // Sustituir params en body
                            let mut expanded = body.clone();
                            for (i, p) in params.iter().enumerate() {
                                let val = args.get(i).cloned().unwrap_or_default();
                                expanded = expanded.replace(p, &val);
                            }
                            out.push_str(&expanded);
                        } else {
                            out.push_str(name.as_str());
                        }
                    }
                }
            } else {
                out.push_str(&name);
            }
        } else {
            out.push(c);
        }
    }
    out
}
