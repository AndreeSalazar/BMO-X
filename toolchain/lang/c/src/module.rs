use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use crate::ast::*;
use crate::CError;

/// Manifest parsed from BMO.toml
#[derive(Debug, Clone)]
pub struct ModuleManifest {
    pub name: String,
    pub version: String,
    pub exports: Vec<String>,
    pub source_files: Vec<String>,
}

/// Resolves modules from filesystem
pub struct ModuleResolver {
    pub base_paths: Vec<PathBuf>,
}

/// Descubre las tablas sem-asm (forge/sem-asm/tables): stdlib/ + standards/C.
/// NOTA: antes buscaba "Semantic_ASM/" — directorio muerto tras la
/// reorganización; el descubrimiento fallaba en silencio.
pub fn discover_include_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let candidates = &[
        "toolchain/forge/sem-asm/tables",
        "forge/sem-asm/tables",
        "../forge/sem-asm/tables",
        "../../forge/sem-asm/tables",
        "../toolchain/forge/sem-asm/tables",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.join("stdlib").is_dir() {
            // Add standards/C too
            let std_path = p.join("standards").join("C");
            if std_path.is_dir() { paths.push(std_path); }
            paths.push(p);
            break;
        }
    }
    // relativo a la crate (toolchain/lang/c → ../../forge/sem-asm/tables)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let asm_path = PathBuf::from(&manifest_dir).join("../../forge/sem-asm/tables");
        if asm_path.join("stdlib").is_dir() {
            let std_path = asm_path.join("standards").join("C");
            if std_path.is_dir() { paths.push(std_path); }
            paths.push(asm_path);
        }
    }
    paths
}

impl ModuleResolver {
    pub fn new(base_paths: Vec<PathBuf>) -> Self {
        Self { base_paths }
    }

    /// Auto-descubre las tablas sem-asm (forge/sem-asm/tables) como base paths.
    pub fn with_semantic_asm(mut self) -> Self {
        self.base_paths.extend(discover_include_paths());
        self
    }

    /// Find a module by name (e.g. "bmo/pci") across all base paths
    pub fn find_manifest(&self, name: &str) -> Result<ModuleManifest, CError> {
        for base in &self.base_paths {
            let mod_dir = base.join(name);
            if mod_dir.is_dir() {
                return Self::parse_manifest(&mod_dir.join("BMO.toml"));
            }
        }
        Err(CError::new(0, format!("module not found: {name}")))
    }

    /// Get the base directory for a module (the folder containing BMO.toml + sources)
    pub fn find_base_dir(&self, name: &str) -> std::path::PathBuf {
        for base in &self.base_paths {
            let mod_dir = base.join(name);
            if mod_dir.is_dir() {
                return mod_dir;
            }
        }
        std::path::PathBuf::from(name)
    }

    /// Parse a BMO.toml file
    fn parse_manifest(path: &Path) -> Result<ModuleManifest, CError> {
        let text = fs::read_to_string(path)
            .map_err(|e| CError::new(0, format!("cannot read {}: {e}", path.display())))?;
        let mut name = String::new();
        let mut version = String::new();
        let mut exports = Vec::new();
        let mut source_files = Vec::new();
        let mut current_section = String::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len()-1].trim().to_lowercase();
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim().trim_matches('"');
                match current_section.as_str() {
                    "module" => {
                        match key { "name" => name = val.to_string(), "version" => version = val.to_string(), _ => {} }
                    }
                    "exports" => {
                        // Support two formats:
                        //   functions = "name1, name2, ..."    (list of names)
                        //   funcname = "signature"              (name is the key itself)
                        if key == "functions" {
                            for f in val.split(',').map(|s| s.trim().trim_matches('"')) {
                                if !f.is_empty() { exports.push(f.to_string()); }
                            }
                        } else {
                            // Treat the key as the exported function name
                            exports.push(key.to_string());
                        }
                    }
                    "sources" => {
                        if key == "files" {
                            for f in val.split(',').map(|s| s.trim().trim_matches('"')) {
                                if !f.is_empty() { source_files.push(f.to_string()); }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            name = path.parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        }
        if source_files.is_empty() {
            // auto-detect .c files in module dir
            if let Some(dir) = path.parent() {
                if let Ok(entries) = fs::read_dir(dir) {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.extension().map_or(false, |ext| ext == "c") {
                            if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                                source_files.push(fname.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(ModuleManifest { name, version, exports, source_files })
    }
}

/// Tracks used functions via reachability analysis from main()
pub fn find_used_functions(program: &Program, exported: &[String]) -> Vec<String> {
    let mut used: HashSet<String> = HashSet::new();
    // Start from exported functions (they are API entry points)
    for e in exported { used.insert(e.clone()); }
    // Add test_* functions (they are called by test framework)
    for f in &program.functions {
        if f.name.starts_with("test_") { used.insert(f.name.clone()); }
    }

    // Build call graph
    let mut callers: HashMap<String, Vec<String>> = HashMap::new();
    for f in &program.functions {
        let mut callees = Vec::new();
        collect_callees(&f.body, &mut callees);
        callers.insert(f.name.clone(), callees);
    }

    // BFS from main and exported/test functions
    let mut queue: Vec<String> = used.iter().cloned().collect();
    while let Some(current) = queue.pop() {
        if let Some(callees) = callers.get(&current) {
            for callee in callees {
                if used.insert(callee.clone()) {
                    queue.push(callee.clone());
                }
            }
        }
    }

    used.into_iter().collect()
}

fn collect_callees(stmts: &[Stmt], callees: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::If(_, t, e) => { collect_callees(&[t.as_ref().clone()], callees); if let Some(el) = e { collect_callees(&[el.as_ref().clone()], callees); } }
            Stmt::While(_, b) => collect_callees(&[b.as_ref().clone()], callees),
            Stmt::DoWhile(b, _) => collect_callees(&[b.as_ref().clone()], callees),
            Stmt::For(_, _, _, b) => collect_callees(&[b.as_ref().clone()], callees),
            Stmt::Switch(_, cases) => { for c in cases { collect_callees(&c.stmts, callees); } }
            Stmt::Block(stmts) => collect_callees(stmts, callees),
            Stmt::Expr(e) | Stmt::Return(Some(e)) => collect_expr_callees(e, callees),
            Stmt::DeclAssign(_, _, Some(e)) => collect_expr_callees(e, callees),
            _ => {}
        }
    }
}

fn collect_expr_callees(expr: &Expr, callees: &mut Vec<String>) {
    match expr {
        Expr::Call(name, args) => {
            callees.push(name.clone());
            for a in args { collect_expr_callees(a, callees); }
        }
        Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a) => collect_expr_callees(a, callees),
        Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
            | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
            | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
            | Expr::Shl(a,b) | Expr::Shr(a,b) => { collect_expr_callees(a, callees); collect_expr_callees(b, callees); }
        Expr::Conditional(c,t,f) => { collect_expr_callees(c, callees); collect_expr_callees(t, callees); collect_expr_callees(f, callees); }
        Expr::Arrow(p,_,_,_) => collect_expr_callees(p, callees),
        Expr::AssignArrow(p,_,_,_,v) => { collect_expr_callees(p, callees); collect_expr_callees(v, callees); }
        Expr::Assign(_, v) | Expr::AssignField(_,_,_,_,v) => collect_expr_callees(v, callees),
        Expr::Field(b,_,_,_) => collect_expr_callees(b, callees),
        Expr::Cast(_, a) => collect_expr_callees(a, callees),
        Expr::Subscript(_, idx, _) => collect_expr_callees(idx, callees),
        Expr::AssignSubscript(_, idx, _, v) => { collect_expr_callees(idx, callees); collect_expr_callees(v, callees); }
        Expr::Comma(v) => { for e in v { collect_expr_callees(e, callees); } }
        _ => {}
    }
}
