//! ÑEXO Module Resolver — Resolución de módulos, dependencias, compilation order.
//!
//! Diseño:
//! - Cada archivo `.nexo` es una unidad de compilación
//! - `usa nexo::io` → busca archivo `nexo/io.nexo`
//! - Grafo de dependencias con topological sort
//! - Detección de ciclos
//! - Symbol table por módulo
//!
//! ## Convenciones de archivos
//!
//! ```text
//!   mi_proyecto/
//!     main.nexo          → módulo raíz
//!     nexo/
//!       io.nexo          → nexo::io
//!       mem.nexo         → nexo::mem
//!       gfx.nexo         → nexo::gfx
//! ```

#![allow(dead_code)]

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::vec;

use crate::barex::{BxError, BxResult};
use super::parser::{Ast, Stmt, Path, TypeAnnotation};

/// Identificador único de módulo (ruta punteada: "nexo::io").
pub type ModuleId = String;

/// Información sobre un módulo compilado.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub id: ModuleId,
    pub source: Vec<u8>,
    pub ast: Option<Ast>,
    pub dependencies: Vec<ModuleId>,
    pub compilation_order: Option<usize>,
}

/// Tabla de símbolos exportados por un módulo.
#[derive(Debug, Clone)]
pub struct SymbolEntry {
    pub module_id: ModuleId,
    pub name: String,
    pub kind: SymbolKind,
    pub is_pub: bool,
    pub ty: Option<TypeAnnotation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Constant,
    Module,
    Type,
}

/// Resolved symbol with full path.
#[derive(Debug, Clone)]
pub struct ResolvedSymbol {
    pub path: Path,
    pub kind: SymbolKind,
    pub module_id: ModuleId,
}

/// Module resolver: manages module loading, dependency resolution, compilation order.
pub struct ModuleResolver {
    modules: BTreeMap<ModuleId, ModuleInfo>,
    symbols: Vec<SymbolEntry>,
    root_module: Option<ModuleId>,
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self {
            modules: BTreeMap::new(),
            symbols: Vec::new(),
            root_module: None,
        }
    }

    /// Register a source file as a module.
    pub fn register_source(&mut self, module_id: &str, source: Vec<u8>) {
        let info = ModuleInfo {
            id: module_id.to_string(),
            source,
            ast: None,
            dependencies: Vec::new(),
            compilation_order: None,
        };
        self.modules.insert(module_id.to_string(), info);
    }

    /// Set the root module (entry point).
    pub fn set_root(&mut self, module_id: &str) {
        self.root_module = Some(module_id.to_string());
    }

    /// Phase 1: Parse all registered modules and extract `use` declarations.
    pub fn parse_all(&mut self) -> BxResult<()> {
        let module_ids: Vec<ModuleId> = self.modules.keys().cloned().collect();
        for id in &module_ids {
            let source = self.modules[id].source.clone();
            let mut lex = super::lexer::Lexer::new(&source);
            let tokens = lex.tokenize()?;
            let mut parser = super::parser::Parser::new(&tokens);
            let ast = parser.parse()?;

            // Extract dependencies from `use` statements
            let deps = self.extract_dependencies(&ast);

            if let Some(module) = self.modules.get_mut(id) {
                module.ast = Some(ast);
                module.dependencies = deps;
            }
        }
        Ok(())
    }

    /// Extract module dependencies from AST `use` statements.
    fn extract_dependencies(&self, ast: &Ast) -> Vec<ModuleId> {
        let mut deps = Vec::new();
        for item in &ast.items {
            self.extract_deps_stmt(item, &mut deps);
        }
        deps.sort();
        deps.dedup();
        deps
    }

    fn extract_deps_stmt(&self, stmt: &Stmt, deps: &mut Vec<ModuleId>) {
        match stmt {
            Stmt::Use { path, .. } => {
                if !path.is_empty() {
                    deps.push(path.join("::"));
                }
            }
            Stmt::UseGlob { path } => {
                if !path.is_empty() {
                    deps.push(path.join("::"));
                }
            }
            Stmt::Pub { inner } => self.extract_deps_stmt(inner, deps),
            Stmt::Module { items, .. } => {
                for item in items {
                    self.extract_deps_stmt(item, deps);
                }
            }
            Stmt::FnDecl { body, .. } => {
                for s in body {
                    self.extract_deps_stmt(s, deps);
                }
            }
            Stmt::If { then_body, else_body, .. } => {
                for s in then_body { self.extract_deps_stmt(s, deps); }
                if let Some(eb) = else_body {
                    for s in eb { self.extract_deps_stmt(s, deps); }
                }
            }
            Stmt::While { body, .. } => {
                for s in body { self.extract_deps_stmt(s, deps); }
            }
            Stmt::For { body, .. } => {
                for s in body { self.extract_deps_stmt(s, deps); }
            }
            Stmt::Block(stmts) => {
                for s in stmts { self.extract_deps_stmt(s, deps); }
            }
            Stmt::ImplDecl { methods, .. } => {
                for m in methods { self.extract_deps_stmt(m, deps); }
            }
            _ => {}
        }
    }

    /// Phase 2: Build dependency graph and detect cycles.
    pub fn resolve_dependencies(&self) -> BxResult<()> {
        // Check for missing modules
        for (_id, info) in &self.modules {
            for dep in &info.dependencies {
                if !self.modules.contains_key(dep) {
                    crate::diag::info("nexo_mod", "Module dependency not registered");
                    // Don't error — might be a stdlib module we haven't loaded yet
                }
            }
        }

        // Detect cycles using DFS
        let mut visited = BTreeMap::new();
        for id in self.modules.keys() {
            if !visited.contains_key(id) {
                self.detect_cycle_dfs(id, &mut visited, &mut Vec::new())?;
            }
        }
        Ok(())
    }

    fn detect_cycle_dfs(&self, id: &str, visited: &mut BTreeMap<String, bool>, stack: &mut Vec<String>) -> BxResult<()> {
            if let Some(&in_stack) = visited.get(id) {
            if in_stack {
                crate::diag::warn("nexo_mod", "Circular dependency detected");
                return Err(BxError::InvalidArgument);
            }
            return Ok(()); // Already fully visited
        }

        visited.insert(id.to_string(), true);
        stack.push(id.to_string());

        if let Some(info) = self.modules.get(id) {
            for dep in &info.dependencies {
                self.detect_cycle_dfs(dep, visited, stack)?;
            }
        }

        stack.pop();
        visited.insert(id.to_string(), false);
        Ok(())
    }

    /// Phase 3: Topological sort — determine compilation order.
    pub fn compute_compilation_order(&mut self) -> BxResult<Vec<ModuleId>> {
        let mut in_degree: BTreeMap<ModuleId, usize> = BTreeMap::new();
        let mut dependents: BTreeMap<ModuleId, Vec<ModuleId>> = BTreeMap::new();

        // Initialize
        for id in self.modules.keys() {
            in_degree.entry(id.clone()).or_insert(0);
        }

        // Build reverse dependency graph
        for (id, info) in &self.modules {
            for dep in &info.dependencies {
                if self.modules.contains_key(dep) {
                    dependents.entry(dep.clone()).or_default().push(id.clone());
                    *in_degree.entry(id.clone()).or_insert(0) += 1;
                }
            }
        }

        // Kahn's algorithm
        let mut queue: Vec<ModuleId> = Vec::new();
        for (id, &degree) in &in_degree {
            if degree == 0 {
                queue.push(id.clone());
            }
        }

        let mut order = Vec::new();
        while let Some(current) = queue.pop() {
            order.push(current.clone());
            if let Some(deps) = dependents.get(&current) {
                for dep in deps {
                    let d = in_degree.get_mut(dep).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push(dep.clone());
                    }
                }
            }
        }

        // Check if all modules were included (no cycles)
        if order.len() != self.modules.len() {
            crate::diag::warn("nexo_mod", "Some modules could not be ordered (circular dependency)");
        }

        // Update compilation order
        for (i, id) in order.iter().enumerate() {
            if let Some(info) = self.modules.get_mut(id) {
                info.compilation_order = Some(i);
            }
        }

        Ok(order)
    }

    /// Register symbols from a parsed module.
    pub fn register_symbols(&mut self, module_id: &str) {
        let ast = match self.modules.get(module_id) {
            Some(info) => match &info.ast {
                Some(ast) => ast.clone(),
                None => return,
            },
            None => return,
        };

        for item in &ast.items {
            self.extract_symbols(module_id, item, false);
        }
    }

    fn extract_symbols(&mut self, module_id: &str, stmt: &Stmt, is_pub: bool) {
        match stmt {
            Stmt::FnDecl { name, params: _, ret, .. } => {
                self.symbols.push(SymbolEntry {
                    module_id: module_id.to_string(),
                    name: name.clone(),
                    kind: SymbolKind::Function,
                    is_pub,
                    ty: ret.clone(),
                });
            }
            Stmt::StructDecl { name, .. } => {
                self.symbols.push(SymbolEntry {
                    module_id: module_id.to_string(),
                    name: name.clone(),
                    kind: SymbolKind::Struct,
                    is_pub,
                    ty: None,
                });
            }
            Stmt::EnumDecl { name, .. } => {
                self.symbols.push(SymbolEntry {
                    module_id: module_id.to_string(),
                    name: name.clone(),
                    kind: SymbolKind::Enum,
                    is_pub,
                    ty: None,
                });
            }
            Stmt::Let { name, ty, .. } if is_pub => {
                self.symbols.push(SymbolEntry {
                    module_id: module_id.to_string(),
                    name: name.clone(),
                    kind: SymbolKind::Constant,
                    is_pub: true,
                    ty: ty.clone(),
                });
            }
            Stmt::Pub { inner } => {
                self.extract_symbols(module_id, inner, true);
            }
            Stmt::Module { name, items } => {
                self.symbols.push(SymbolEntry {
                    module_id: module_id.to_string(),
                    name: name.clone(),
                    kind: SymbolKind::Module,
                    is_pub,
                    ty: None,
                });
                for item in items {
                    self.extract_symbols(module_id, item, is_pub);
                }
            }
            _ => {}
        }
    }

    /// Resolve a qualified path to a symbol.
    pub fn resolve_symbol(&self, path: &[String]) -> Option<ResolvedSymbol> {
        if path.is_empty() { return None; }

        // Strategy 1: Exact match — `module::symbol`
        // Strategy 2: Module member — search all modules for matching symbol name
        // Strategy 3: Prelude search — check current module + common modules

        // Try direct module::symbol resolution
        if path.len() >= 2 {
            let module_path = &path[..path.len()-1];
            let symbol_name = &path[path.len()-1];
            let module_id = module_path.join("::");

            for sym in &self.symbols {
                if sym.module_id == module_id && sym.name == *symbol_name && sym.is_pub {
                    return Some(ResolvedSymbol {
                        path: path.to_vec(),
                        kind: sym.kind,
                        module_id: sym.module_id.clone(),
                    });
                }
            }
        }

        // Try symbol-only resolution (from imported modules)
        if path.len() == 1 {
            let name = &path[0];
            // Search all modules for this symbol name
            let mut found = None;
            for sym in &self.symbols {
                if sym.name == *name && sym.is_pub {
                    if found.is_some() {
                        crate::diag::warn("nexo_mod", "Ambiguous symbol");
                        return None;
                    }
                    found = Some(ResolvedSymbol {
                        path: vec![sym.module_id.clone(), name.clone()],
                        kind: sym.kind,
                        module_id: sym.module_id.clone(),
                    });
                }
            }
            return found;
        }

        None
    }

    /// Get a list of all registered module IDs.
    pub fn module_ids(&self) -> Vec<&ModuleId> {
        self.modules.keys().collect()
    }

    /// Get module info by ID.
    pub fn get_module(&self, id: &str) -> Option<&ModuleInfo> {
        self.modules.get(id)
    }

    /// Get the root module.
    pub fn root_module(&self) -> Option<&str> {
        self.root_module.as_deref()
    }
}

impl Default for ModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}
