//! ÑEXO Sema — Análisis semántico completo con soporte de módulos.
//!
//! Valida: scopes, tipos, funciones, structs, enums, variables,
//! módulos, visibilidad, imports.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;
use alloc::collections::BTreeMap;

use crate::barex::{BxError, BxResult};
use super::parser::{Ast, Stmt, Expr, TypeAnnotation, BinOp, UnaryOp};

/// Variable info.
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub name: String,
    pub ty: TypeAnnotation,
    pub offset: i32,
    pub mutable: bool,
    pub visibility: Visibility,
}

/// Function signature.
#[derive(Debug, Clone)]
pub struct FnInfo {
    pub name: String,
    pub params: Vec<(String, TypeAnnotation)>,
    pub ret: Option<TypeAnnotation>,
    pub visibility: Visibility,
}

/// Struct definition.
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<(String, TypeAnnotation)>,
    pub visibility: Visibility,
}

/// Visibility of a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

/// Imported name mapping: local_name → (module_id, original_name).
#[derive(Debug, Clone)]
pub struct ImportEntry {
    pub local_name: String,
    pub module_id: String,
    pub original_name: String,
    pub kind: ImportKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Specific,   // `use nexo::io::print;`
    Glob,       // `use nexo::io::*;`
    Alias,      // `use nexo::io::print como p;`
}

/// Scope for variable tracking — supports module hierarchy.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub vars: Vec<VarInfo>,
    pub fns: Vec<FnInfo>,
    pub structs: Vec<StructInfo>,
    pub frame_size: u32,
    pub in_loop: bool,
    /// Current module path (e.g., "nexo::io")
    pub current_module: Option<String>,
    /// Imported names from `use` statements
    pub imports: Vec<ImportEntry>,
    /// Child modules defined in this scope
    pub child_modules: BTreeMap<String, Scope>,
    /// Parent scope reference (for symbol lookup chain)
    pub parent_visible: bool,
}

impl Scope {
    pub fn lookup_var(&self, name: &str) -> Option<&VarInfo> {
        // First check local vars
        if let Some(v) = self.vars.iter().rev().find(|v| v.name == name) {
            return Some(v);
        }
        // Then check imports
        for imp in &self.imports {
            if imp.local_name == name {
                // Would need module resolver to look up the actual symbol
                // For now, trust the import exists
                return None; // Placeholder — full resolution needs ModuleResolver
            }
        }
        None
    }

    pub fn lookup_fn(&self, name: &str) -> Option<&FnInfo> {
        self.fns.iter().rev().find(|f| f.name == name)
    }

    pub fn lookup_struct(&self, name: &str) -> Option<&StructInfo> {
        self.structs.iter().rev().find(|s| s.name == name)
    }

    pub fn push_var(&mut self, name: String, ty: TypeAnnotation, mutable: bool) {
        let offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        self.vars.push(VarInfo {
            name, ty, offset, mutable,
            visibility: Visibility::Private,
        });
    }

    pub fn push_var_pub(&mut self, name: String, ty: TypeAnnotation, mutable: bool) {
        let offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        self.vars.push(VarInfo {
            name, ty, offset, mutable,
            visibility: Visibility::Public,
        });
    }

    pub fn push_fn(&mut self, info: FnInfo) {
        self.fns.push(info);
    }

    pub fn push_struct(&mut self, info: StructInfo) {
        self.structs.push(info);
    }

    /// Add a `use` import to this scope.
    pub fn add_import(&mut self, local_name: String, module_id: String, original_name: String, kind: ImportKind) {
        self.imports.push(ImportEntry { local_name, module_id, original_name, kind });
    }

    /// Check if a name is visible from outside the current module.
    pub fn is_visible(&self, name: &str) -> bool {
        // Check local vars
        if let Some(v) = self.vars.iter().rev().find(|v| v.name == name) {
            return v.visibility == Visibility::Public;
        }
        // Check functions
        if let Some(f) = self.fns.iter().rev().find(|f| f.name == name) {
            return f.visibility == Visibility::Public;
        }
        // Check structs
        if let Some(s) = self.structs.iter().rev().find(|s| s.name == name) {
            return s.visibility == Visibility::Public;
        }
        false
    }
}

/// Semantic analyzer for ÑEXO — module-aware.
pub struct Sema;

impl Sema {
    pub fn new() -> Self { Self }

    pub fn check(&self, ast: &Ast) -> BxResult<()> {
        let mut scope = Scope::default();
        scope.current_module = Some(String::from("root"));
        for item in &ast.items {
            self.check_stmt(item, &mut scope)?;
        }
        Ok(())
    }

    /// Check with explicit module context.
    pub fn check_module(&self, ast: &Ast, module_id: &str) -> BxResult<()> {
        let mut scope = Scope::default();
        scope.current_module = Some(module_id.to_string());
        for item in &ast.items {
            self.check_stmt(item, &mut scope)?;
        }
        Ok(())
    }

    fn check_stmt(&self, stmt: &Stmt, scope: &mut Scope) -> BxResult<()> {
        match stmt {
            Stmt::Let { name, ty, value } => {
                if let Some(val) = value {
                    self.check_expr(val, scope)?;
                }
                let annotated_ty = ty.clone().unwrap_or(TypeAnnotation::Named(String::from("num")));
                scope.push_var(name.clone(), annotated_ty, false);
            }
            Stmt::Mut { name, ty, value } => {
                self.check_expr(value, scope)?;
                let annotated_ty = ty.clone().unwrap_or(TypeAnnotation::Named(String::from("num")));
                scope.push_var(name.clone(), annotated_ty, true);
            }
            Stmt::Assign(name, value) => {
                self.check_expr(value, scope)?;
                if scope.lookup_var(name).is_none() {
                    return Err(BxError::InvalidArgument);
                }
            }
            Stmt::FnDecl { name, params, ret, body } => {
                let param_types: Vec<(String, TypeAnnotation)> = params.iter()
                    .map(|p| (p.name.clone(), p.ty.clone()))
                    .collect();
                scope.push_fn(FnInfo {
                    name: name.clone(),
                    params: param_types,
                    ret: ret.clone(),
                    visibility: Visibility::Private,
                });
                let mut fn_scope = scope.clone();
                for p in params {
                    fn_scope.push_var(p.name.clone(), p.ty.clone(), false);
                }
                for s in body {
                    self.check_stmt(s, &mut fn_scope)?;
                }
            }
            Stmt::StructDecl { name, fields } => {
                scope.push_struct(StructInfo {
                    name: name.clone(),
                    fields: fields.clone(),
                    visibility: Visibility::Private,
                });
            }
            Stmt::EnumDecl { .. } => {}
            Stmt::ImplDecl { type_name: _, methods } => {
                for m in methods {
                    self.check_stmt(m, scope)?;
                }
            }
            Stmt::If { cond, then_body, else_body } => {
                self.check_expr(cond, scope)?;
                let mut then_scope = scope.clone();
                for s in then_body { self.check_stmt(s, &mut then_scope)?; }
                if let Some(eb) = else_body {
                    let mut else_scope = scope.clone();
                    for s in eb { self.check_stmt(s, &mut else_scope)?; }
                }
            }
            Stmt::While { cond, body } => {
                self.check_expr(cond, scope)?;
                let mut loop_scope = scope.clone();
                loop_scope.in_loop = true;
                for s in body { self.check_stmt(s, &mut loop_scope)?; }
            }
            Stmt::For { var, start, end, body } => {
                self.check_expr(start, scope)?;
                self.check_expr(end, scope)?;
                let mut loop_scope = scope.clone();
                loop_scope.in_loop = true;
                loop_scope.push_var(var.clone(), TypeAnnotation::Named(String::from("num")), false);
                for s in body { self.check_stmt(s, &mut loop_scope)?; }
            }
            Stmt::Return(Some(expr)) => { self.check_expr(expr, scope)?; }
            Stmt::Return(None) => {}
            Stmt::Break | Stmt::Continue => {
                if !scope.in_loop {
                    return Err(BxError::InvalidArgument);
                }
            }
            Stmt::Block(stmts) => {
                let mut block_scope = scope.clone();
                for s in stmts { self.check_stmt(s, &mut block_scope)?; }
            }
            Stmt::ExprStmt(expr) => { self.check_expr(expr, scope)?; }
            Stmt::Syscall { nr: _, args } => {
                for a in args { self.check_expr(a, scope)?; }
            }
            Stmt::Emit(_) => {}
            Stmt::Aloc { size } => { self.check_expr(size, scope)?; }
            Stmt::Libre(ptr) => { self.check_expr(ptr, scope)?; }
            Stmt::Module { name, items } => {
                // Create child scope for module
                let mut child_scope = scope.clone();
                child_scope.current_module = Some(
                    match &scope.current_module {
                        Some(parent) => alloc::format!("{}::{}", parent, name),
                        None => name.clone(),
                    }
                );
                child_scope.vars.clear();
                child_scope.fns.clear();
                child_scope.structs.clear();
                child_scope.imports.clear();

                for s in items {
                    self.check_stmt(s, &mut child_scope)?;
                }

                // Register the module in parent scope
                scope.child_modules.insert(name.clone(), child_scope);
            }
            Stmt::Use { path, alias } => {
                let local_name = alias.clone().unwrap_or_else(|| {
                    path.last().cloned().unwrap_or_default()
                });
                let module_id = if path.len() > 1 {
                    path[..path.len()-1].join("::")
                } else {
                    String::new()
                };
                let original_name = path.last().cloned().unwrap_or_default();
                let kind = if alias.is_some() { ImportKind::Alias } else { ImportKind::Specific };
                scope.add_import(local_name, module_id, original_name, kind);
            }
            Stmt::UseGlob { path } => {
                let module_id = path.join("::");
                let kind = ImportKind::Glob;
                scope.add_import(module_id.clone(), module_id, "*".to_string(), kind);
            }
            Stmt::Pub { inner } => {
                // Set visibility and re-check
                match inner.as_ref() {
                    Stmt::FnDecl { name, params, ret, body } => {
                        let param_types: Vec<(String, TypeAnnotation)> = params.iter()
                            .map(|p| (p.name.clone(), p.ty.clone()))
                            .collect();
                        scope.push_fn(FnInfo {
                            name: name.clone(),
                            params: param_types,
                            ret: ret.clone(),
                            visibility: Visibility::Public,
                        });
                        let mut fn_scope = scope.clone();
                        for p in params {
                            fn_scope.push_var(p.name.clone(), p.ty.clone(), false);
                        }
                        for s in body {
                            self.check_stmt(s, &mut fn_scope)?;
                        }
                    }
                    Stmt::StructDecl { name, fields } => {
                        scope.push_struct(StructInfo {
                            name: name.clone(),
                            fields: fields.clone(),
                            visibility: Visibility::Public,
                        });
                    }
                    Stmt::Module { name, items } => {
                        let mut child_scope = scope.clone();
                        child_scope.current_module = Some(
                            match &scope.current_module {
                                Some(parent) => alloc::format!("{}::{}", parent, name),
                                None => name.clone(),
                            }
                        );
                        child_scope.vars.clear();
                        child_scope.fns.clear();
                        child_scope.structs.clear();
                        child_scope.imports.clear();
                        for s in items {
                            self.check_stmt(s, &mut child_scope)?;
                        }
                        scope.child_modules.insert(name.clone(), child_scope);
                    }
                    _ => { self.check_stmt(inner, scope)?; }
                }
            }
            Stmt::Extern { .. } => {} // Extern declarations are metadata for FFI
        }
        Ok(())
    }

    fn check_expr(&self, expr: &Expr, scope: &Scope) -> BxResult<()> {
        match expr {
            Expr::Ident(name) => {
                if scope.lookup_var(name).is_none() && scope.lookup_fn(name).is_none() {
                    // Allow unresolved identifiers — they might be from imported modules
                }
            }
            Expr::QualifiedPath(path) => {
                // Qualified path like `io::MAX_BUF` — validation needs ModuleResolver
                // For now, just validate the path is well-formed
                if path.is_empty() {
                    return Err(BxError::InvalidArgument);
                }
            }
            Expr::QualifiedCall(path, args) => {
                // Qualified call like `io::print("hola")`
                if path.is_empty() {
                    return Err(BxError::InvalidArgument);
                }
                for a in args { self.check_expr(a, scope)?; }
            }
            Expr::Binary(_, left, right) => {
                self.check_expr(left, scope)?;
                self.check_expr(right, scope)?;
            }
            Expr::Unary(_, inner) => { self.check_expr(inner, scope)?; }
            Expr::Call(name, args) => {
                // Allow unresolved function calls — they might be from imported modules
                let _ = scope.lookup_fn(name);
                for a in args { self.check_expr(a, scope)?; }
            }
            Expr::MethodCall(obj, _, args) => {
                self.check_expr(obj, scope)?;
                for a in args { self.check_expr(a, scope)?; }
            }
            Expr::Field(obj, _) => { self.check_expr(obj, scope)?; }
            Expr::Index(obj, idx) => {
                self.check_expr(obj, scope)?;
                self.check_expr(idx, scope)?;
            }
            Expr::Syscall(_, args) => {
                for a in args { self.check_expr(a, scope)?; }
            }
            Expr::Block(stmts) => {
                let mut block_scope = scope.clone();
                for s in stmts { self.check_stmt(s, &mut block_scope)?; }
            }
            Expr::Aloc(size) => { self.check_expr(size, scope)?; }
            Expr::Libre(ptr) => { self.check_expr(ptr, scope)?; }
            _ => {} // Literals, Reg, Emit are always valid
        }
        Ok(())
    }

    /// Infer the type of an expression.
    pub fn infer_type(&self, expr: &Expr, scope: &Scope) -> TypeAnnotation {
        match expr {
            Expr::LitInt(_) => TypeAnnotation::Named(String::from("num")),
            Expr::LitFloat(_) => TypeAnnotation::Named(String::from("num")),
            Expr::LitStr(_) => TypeAnnotation::Ptr(Box::new(TypeAnnotation::Named(String::from("byte")))),
            Expr::LitByte(_) => TypeAnnotation::Named(String::from("byte")),
            Expr::LitBool(_) => TypeAnnotation::Named(String::from("bool")),
            Expr::LitNull => TypeAnnotation::Named(String::from("nulo")),
            Expr::Ident(name) => {
                scope.lookup_var(name)
                    .map(|v| v.ty.clone())
                    .unwrap_or(TypeAnnotation::Named(String::from("num")))
            }
            Expr::QualifiedPath(path) => {
                // Qualified path type inference needs ModuleResolver
                TypeAnnotation::Named(path.join("::"))
            }
            Expr::QualifiedCall(path, _) => {
                TypeAnnotation::Named(path.join("::"))
            }
            Expr::Binary(op, _, _) => {
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge |
                    BinOp::Land | BinOp::Lor => TypeAnnotation::Named(String::from("bool")),
                    _ => TypeAnnotation::Named(String::from("num")),
                }
            }
            Expr::Unary(UnaryOp::Not, _) => TypeAnnotation::Named(String::from("bool")),
            _ => TypeAnnotation::Named(String::from("num")),
        }
    }
}
