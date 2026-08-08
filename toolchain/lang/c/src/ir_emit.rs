//! IR emitter: converts the C AST (ast.rs) into a bmo_abi::ir::IrModule.
//!
//! This is the bridge between the language-specific AST and the unified IR.
//! Once emitted as IrModule, any backend (x86-64, ARM64, RISC-V) can consume it.

use bmo_abi::ir::*;
use bmo_abi::types::convention::CallingConvention;
use crate::ast::*;

pub fn compile_to_ir(program: &Program) -> IrModule {
    let mut e = Emitter::new();
    e.emit_program(program);
    e.module
}

struct Emitter {
    module: IrModule,
}

impl Emitter {
    fn new() -> Self {
        Self { module: IrModule::new(0) }
    }

    // -- Type conversion -------------------------------------------

    fn ir_type(&mut self, ts: &TypeSpec) -> u16 {
        let ir = match ts {
            TypeSpec::Void => IrType::Void,
            TypeSpec::Char | TypeSpec::UnsignedChar => IrType::I8,
            TypeSpec::Short | TypeSpec::UnsignedShort => IrType::I16,
            TypeSpec::Int | TypeSpec::UnsignedInt => IrType::I32,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong
            | TypeSpec::UnsignedLongLong => IrType::I64,
            TypeSpec::Float => IrType::F32,
            TypeSpec::Double => IrType::F64,
            TypeSpec::Ptr(_) => IrType::Pointer,
            TypeSpec::Array(_, _) => IrType::Pointer, // decae a puntero en IR
            TypeSpec::StructRef(_) | TypeSpec::UnionRef(_) => IrType::Pointer,
        };
        self.module.add_type(ir).unwrap_or(0)
    }

    // -- Program -> Module ------------------------------------------

    fn emit_program(&mut self, p: &Program) {
        self.module.name = self.module.add_string("main").unwrap_or(0);

        for g in &p.globals {
            self.emit_global(g);
        }
        for f in &p.functions {
            self.emit_function(f);
        }
    }

    fn emit_global(&mut self, g: &GlobalDecl) {
        match g {
            GlobalDecl::Var(ts, name, init) => {
                let name_idx = self.module.add_string(name).unwrap_or(0);
                let ty_idx = self.ir_type(ts);
                let init_expr = init.as_ref().map(|e| self.emit_expr(e, &mut vec![]));
                self.module.add_global(IrGlobal {
                    name: name_idx,
                    ty: ty_idx,
                    init: init_expr,
                    read_only: false,
                });
            }
            // * `int t[4] = {1,2,3,4}` NO SE PUEDE REPRESENTAR AQUI, y se dice.
            //
            // `IrGlobal::init` es **un** `Option<IrExpr>`, y una lista de
            // inicializacion son N escrituras con su offset. No es que falte
            // escribir la conversion: es que el tipo no da para expresarla.
            //
            // Se registra el global --existe, tiene tipo y tamano-- **sin** su
            // inicializador, y esto NO es un cero silencioso disfrazado: este
            // modulo esta fuera del camino de compilacion. `compile_source_to_bef`
            // va `parse` -> `codegen` directo a bytes; el unico llamante de
            // `compile_to_ir` es su propia funcion publica en `lib.rs`, que nadie
            // usa. Si algun dia se cablea, **esto es lo primero que hay que
            // arreglar**, y hace falta un `IrGlobal` con bytes iniciales en vez
            // de una expresion.
            GlobalDecl::VarLista(ts, name, _escrituras) => {
                let name_idx = self.module.add_string(name).unwrap_or(0);
                let ty_idx = self.ir_type(ts);
                self.module.add_global(IrGlobal {
                    name: name_idx,
                    ty: ty_idx,
                    init: None,
                    read_only: false,
                });
            }
            GlobalDecl::Struct(_, _) | GlobalDecl::Union(_, _) => {
                // Struct/union declarations are type metadata, not globals.
                // They're consumed by the parser during codegen.
            }
        }
    }

    fn emit_function(&mut self, f: &Function) {
        let name_idx = self.module.add_string(&f.name).unwrap_or(0);
        let mut irf = IrFunction::new(name_idx);
        irf.convention = CallingConvention::BmoX86_64;
        irf.return_type = self.ir_type(&f.ret_type);
        irf.public = true;

        // Register parameters as locals
        for (i, p) in f.params.iter().enumerate() {
            let type_id = self.ir_type(&p.typ);
            irf.add_arg(type_id);
            irf.add_local(i as u16, self.ty_to_ir(&p.typ));
        }

        // Register local variables
        let mut local_map: Vec<(String, u16)> = Vec::new();
        let param_count = f.params.len() as u16;
        for (i, name) in f.var_names.iter().enumerate() {
            let idx = param_count + i as u16;
            irf.add_local(idx, IrType::I64); // Default to I64 for simplicity
            local_map.push((name.clone(), idx));
        }

        // Emit body as a single basic block
        let mut block = IrBlock::new(0);
        self.emit_stmts(&f.body, &mut block, &mut irf, &local_map, &f.params);
        irf.block_count = 1;
        irf.blocks[0] = block;

        self.module.add_function(irf);
    }

    fn ty_to_ir(&self, ts: &TypeSpec) -> IrType {
        match ts {
            TypeSpec::Void => IrType::Void,
            TypeSpec::Char | TypeSpec::UnsignedChar => IrType::I8,
            TypeSpec::Short | TypeSpec::UnsignedShort => IrType::I16,
            TypeSpec::Int | TypeSpec::UnsignedInt => IrType::I32,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong
            | TypeSpec::UnsignedLongLong => IrType::I64,
            TypeSpec::Float => IrType::F32,
            TypeSpec::Double => IrType::F64,
            TypeSpec::Ptr(_) => IrType::Pointer,
            TypeSpec::Array(_, _) => IrType::Pointer, // decae a puntero en IR
            TypeSpec::StructRef(_) | TypeSpec::UnionRef(_) => IrType::Pointer,
        }
    }

    // -- Statement emission ----------------------------------------

    fn emit_stmts(
        &mut self,
        stmts: &[Stmt],
        block: &mut IrBlock,
        irf: &mut IrFunction,
        local_map: &[(String, u16)],
        params: &[Param],
    ) {
        let mut locals = vec![];
        for s in stmts {
            self.emit_stmt(s, block, irf, local_map, params, &mut locals);
        }
    }

    fn emit_stmt(
        &mut self,
        s: &Stmt,
        block: &mut IrBlock,
        irf: &mut IrFunction,
        local_map: &[(String, u16)],
        params: &[Param],
        locals: &mut Vec<IrExpr>,
    ) {
        match s {
            Stmt::Return(opt) => {
                let val = opt.as_ref().map(|e| self.emit_expr(e, locals));
                block.push(IrStmt::Return(val.map(|_| 0u16)));
            }
            Stmt::Expr(e) => {
                self.emit_expr(e, locals);
            }
            Stmt::DeclAssign(ts, name, init) => {
                let idx = (irf.local_count) as u16;
                irf.add_local(idx, self.ty_to_ir(ts));
                if let Some(init_expr) = init {
                    let val = self.emit_expr(init_expr, locals);
                    block.push(IrStmt::Assign(idx, val));
                }
            }
            Stmt::If(cond, then, else_opt) => {
                let cond_val = self.emit_expr(cond, locals);
                // Create then/else blocks
                let then_label = irf.block_count;
                let else_label = then_label + 1;
                let end_label = else_label + 1;

                let mut then_block = IrBlock::new(then_label);
                self.emit_stmts(&match &**then { Stmt::Block(b) => b.clone(), s => vec![s.clone()] }, &mut then_block, irf, local_map, params);
                then_block.push(IrStmt::Jump(end_label));
                irf.blocks[then_label as usize] = then_block;

                let mut else_block = IrBlock::new(else_label);
                if let Some(else_stmts) = else_opt {
                    self.emit_stmts(&match &**else_stmts { Stmt::Block(b) => b.clone(), s => vec![s.clone()] }, &mut else_block, irf, local_map, params);
                }
                else_block.push(IrStmt::Jump(end_label));
                irf.blocks[else_label as usize] = else_block;

                irf.block_count = end_label + 1;
                block.push(IrStmt::Branch { cond: 0, then_block: then_label, else_block: else_label });
            }
            _ => {
                // Stub for other statement types
                self.emit_stub_stmt(s, block, irf, local_map, locals);
            }
        }
    }

    fn emit_stub_stmt(&mut self, _s: &Stmt, _block: &mut IrBlock, _irf: &mut IrFunction, _local_map: &[(String, u16)], _locals: &mut Vec<IrExpr>) {
        // Placeholder for While, For, Switch, Goto, Label, Printf, etc.
    }

    // -- Expression emission ---------------------------------------

    fn emit_expr(&mut self, e: &Expr, locals: &mut Vec<IrExpr>) -> IrExpr {
        match e {
            Expr::Int(v) => IrExpr::ConstI64(*v),
            Expr::StringLit(s) => {
                let idx = self.module.add_string(s).unwrap_or(0);
                IrExpr::ConstStr(idx)
            }
            Expr::CharLit(c) => IrExpr::ConstI64(*c as i64),
            Expr::Var(name) => IrExpr::Local(0), // Simplified -- need local_map
            Expr::Call(name, args) => {
                let name_idx = self.module.add_string(name).unwrap_or(0);
                IrExpr::Call { func: name_idx, args: 0, arg_count: args.len() as u16 }
            }
            Expr::Add(l, r) => {
                let lhs = self.emit_expr(l, locals);
                let rhs = self.emit_expr(r, locals);
                let li = locals.len() as u16; locals.push(lhs);
                let ri = locals.len() as u16; locals.push(rhs);
                IrExpr::Binary { op: IrBinOp::Add, lhs: li, rhs: ri }
            }
            Expr::Sub(l, r) => {
                let lhs = self.emit_expr(l, locals);
                let rhs = self.emit_expr(r, locals);
                let li = locals.len() as u16; locals.push(lhs);
                let ri = locals.len() as u16; locals.push(rhs);
                IrExpr::Binary { op: IrBinOp::Sub, lhs: li, rhs: ri }
            }
            Expr::Mul(l, r) => {
                let lhs = self.emit_expr(l, locals);
                let rhs = self.emit_expr(r, locals);
                let li = locals.len() as u16; locals.push(lhs);
                let ri = locals.len() as u16; locals.push(rhs);
                IrExpr::Binary { op: IrBinOp::Mul, lhs: li, rhs: ri }
            }
            Expr::Div(l, r) => {
                let lhs = self.emit_expr(l, locals);
                let rhs = self.emit_expr(r, locals);
                let li = locals.len() as u16; locals.push(lhs);
                let ri = locals.len() as u16; locals.push(rhs);
                IrExpr::Binary { op: IrBinOp::Div, lhs: li, rhs: ri }
            }
            Expr::Neg(inner) => {
                let expr = self.emit_expr(inner, locals);
                let ei = locals.len() as u16; locals.push(expr);
                IrExpr::Unary { op: IrUnOp::Neg, expr: ei }
            }
            Expr::Not(inner) => {
                let expr = self.emit_expr(inner, locals);
                let ei = locals.len() as u16; locals.push(expr);
                IrExpr::Unary { op: IrUnOp::Not, expr: ei }
            }
            Expr::Syscall(def, args) => {
                IrExpr::Syscall { nr: def.nr, args: 0, arg_count: args.len() as u16 }
            }
            _ => IrExpr::ConstI64(0), // Stub for other expressions
        }
    }
}
