//! Adapter: BMO AST → common IR.
//!
//! Por ahora el BMO AST **es** el common IR (compatible 1:1 con
//! `common::ast`). En el futuro, este adapter puede agregar conversiones
//! específicas (e.g. renombrar variantes, agregar info de source map).

#![allow(dead_code)]

use crate::lang::common::ast::Module;
use crate::lang::bmo::parser::ast::Ast;

/// Convierte un BMO AST al Module canónico del common IR.
///
/// v1.8.8: copia directa. La estructura de `Ast` ya coincide.
pub fn lower_to_ir(ast: &Ast, name: &str) -> Module {
    let mut module = Module::new(name);

    // Re-intern todos los strings del BMO AST
    for item in &ast.items {
        if let Some(stmt) = item_to_stmt(item, &mut module) {
            module.add_item(stmt);
        }
    }
    module
}

use crate::lang::bmo::parser::ast::{Ast as _Ast, Stmt, Param, TypeAnnotation, Expr, BinOp as BmoBinOp, UnaryOp as BmoUnaryOp, ExternItem};
use crate::lang::common::ast as ir;
use crate::lang::common::types::IrTypeId;

fn item_to_stmt(item: &Stmt, _module: &mut Module) -> Option<ir::Item> {
    match item {
        Stmt::Extern { items } => {
            // Tomar el primer item del extern.
            items.first().and_then(|ext| match ext {
                ExternItem::Fn { name, params, ret } => {
                    // Crear un function declaration con body vacío
                    // (no podemos crear un Module-level function con body vacío
                    // en common IR, pero el Extern es lo correcto).
                    let s = _module.intern(name);
                    Some(ir::Item::Extern {
                        name: s,
                        kind: ir::ExternKind::Function {
                            params: params.iter().map(|p| {
                                // Convertir el tipo de BMO a IrTypeId
                                // Por ahora un placeholder: el backend usará
                                // el layout SysV AMD64 por defecto.
                                let _ = &p.ty;
                                IrTypeId::default()
                            }).collect(),
                            ret: ret.as_ref().map(|_| IrTypeId::default()).unwrap_or_default(),
                        },
                        span: crate::lang::common::source::Span::ZERO,
                    })
                }
            })
        }
        _ => {
            // Otros items (Function, Let, etc.) se manejan en el parser
            // cuando se mueve a common IR.
            None
        }
    }
}

// Por ahora, los tipos BMO se mapean a IrTypeId::default() (placeholder).
// En la fase de reescritura completa del parser a common IR, esto se
// resuelve con una tabla de tipos real.
