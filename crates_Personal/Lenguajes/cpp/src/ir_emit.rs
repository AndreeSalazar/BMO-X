//! C++ IR emitter: converts the C++ AST into a bmo_abi::ir::IrModule.
//!
//! v1.9: Real expression emission using local accumulator pattern.

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
    /// Local expression accumulator for binary ops.
    locals: Vec<IrExpr>,
    class_types: Vec<(String, u16, u32)>,
}

impl Emitter {
    fn new() -> Self {
        Self { module: IrModule::new(0), locals: Vec::new(), class_types: Vec::new() }
    }

    fn push_expr(&mut self, e: IrExpr) -> u16 {
        let idx = self.locals.len() as u16;
        self.locals.push(e);
        idx
    }

    fn ir_type(&mut self, ts: &TypeSpec) -> u16 {
        let ir = match ts {
            TypeSpec::Void => IrType::Void,
            TypeSpec::Char | TypeSpec::UnsignedChar | TypeSpec::Bool => IrType::I8,
            TypeSpec::Short | TypeSpec::UnsignedShort => IrType::I16,
            TypeSpec::Int | TypeSpec::UnsignedInt => IrType::I32,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong
            | TypeSpec::UnsignedLongLong => IrType::I64,
            TypeSpec::Float => IrType::F32,
            TypeSpec::Double => IrType::F64,
            TypeSpec::Ptr(_) | TypeSpec::Ref(_) | TypeSpec::Auto
            | TypeSpec::ClassRef(_) | TypeSpec::Template(_, _) => IrType::Pointer,
        };
        self.module.add_type(ir).unwrap_or(0)
    }

    fn emit_program(&mut self, p: &Program) {
        self.module.name = self.module.add_string("main").unwrap_or(0);
        for cls in &p.classes { self.emit_class_decl(cls); }
        for g in &p.globals {
            if let GlobalDecl::Var(ts, name, init) = g {
                let name_idx = self.module.add_string(name).unwrap_or(0);
                let ty_idx = self.ir_type(ts);
                let init_expr = init.as_ref().map(|e| self.emit_expr(e));
                self.module.add_global(IrGlobal { name: name_idx, ty: ty_idx, init: init_expr, read_only: false });
            }
        }
        for f in &p.functions { self.emit_function(f, None); }
        for cls in &p.classes {
            for m in &cls.methods { self.emit_method(m, &cls.name); }
            if let Some(ctor) = &cls.constructor { self.emit_method(ctor, &cls.name); }
        }
    }

    fn emit_class_decl(&mut self, cls: &Class) {
        let name_idx = self.module.add_string(&cls.name).unwrap_or(0);
        let total_size: u32 = if cls.vtable { 8 } else { 0 }
            + cls.members.iter().map(|m| m.typ.size()).sum::<u32>();
        let type_id = self.module.add_type(IrType::Pointer).unwrap_or(0);
        self.class_types.push((cls.name.clone(), type_id, cls.methods.iter().filter(|m| m.is_virtual).count() as u32));
    }

    fn emit_method(&mut self, m: &Method, class_name: &str) {
        let full_name = format!("{}::{}", class_name, m.name);
        let name_idx = self.module.add_string(&full_name).unwrap_or(0);
        let mut func = IrFunction::new(name_idx);
        func.convention = CallingConvention::BmoX86_64;
        func.return_type = self.ir_type(&m.ret_type);
        func.public = true;
        func.add_arg(self.ir_type(&TypeSpec::ClassRef(class_name.to_string())));
        func.add_local(0, IrType::Pointer);
        for (i, p) in m.params.iter().enumerate() {
            func.add_arg(self.ir_type(&p.typ));
            func.add_local((i + 1) as u16, IrType::I64);
        }
        self.locals.clear();
        let mut block = IrBlock::new(0);
        self.emit_stmts(&m.body, &mut block);
        block.push(IrStmt::Return(None));
        func.block_count = 1;
        func.blocks[0] = block;
        self.module.add_function(func);
    }

    fn emit_function(&mut self, f: &Function, _ns: Option<&str>) {
        let name_idx = self.module.add_string(&f.name).unwrap_or(0);
        let mut func = IrFunction::new(name_idx);
        func.convention = CallingConvention::BmoX86_64;
        func.return_type = self.ir_type(&f.ret_type);
        func.public = true;
        for (i, p) in f.params.iter().enumerate() {
            func.add_arg(self.ir_type(&p.typ));
            func.add_local(i as u16, IrType::I64);
        }
        self.locals.clear();
        let mut block = IrBlock::new(0);
        self.emit_stmts(&f.body, &mut block);
        block.push(IrStmt::Return(None));
        func.block_count = 1;
        func.blocks[0] = block;
        self.module.add_function(func);
    }

    fn emit_stmts(&mut self, stmts: &[Stmt], block: &mut IrBlock) {
        for s in stmts { self.emit_stmt(s, block); }
    }

    fn emit_stmt(&mut self, s: &Stmt, block: &mut IrBlock) {
        match s {
            Stmt::Expr(e) => { let _ = self.emit_expr(e); }
            Stmt::Return(opt) => {
                if let Some(e) = opt {
                    let val = self.emit_expr(e);
                    let idx = self.push_expr(val);
                    block.push(IrStmt::Return(Some(idx)));
                } else {
                    block.push(IrStmt::Return(None));
                }
            }
            Stmt::DeclVar(_ts, _name, init) => {
                if let Some(init_e) = init {
                    let val = self.emit_expr(init_e);
                    block.push(IrStmt::DefLocal { idx: 0, ty: IrType::I64 });
                    block.push(IrStmt::Assign(0, val));
                }
            }
            Stmt::Assign(name, e) => {
                let val = self.emit_expr(e);
                block.push(IrStmt::Assign(0, val));
            }
            Stmt::Block(stmts) => self.emit_stmts(stmts, block),
            Stmt::Delete(_name) => {
                block.push(IrStmt::Expr(IrExpr::Syscall { nr: 0x191, args: 0, arg_count: 1 }));
            }
            Stmt::If(cond, then_st, else_opt) => {
                let c = self.emit_expr(cond);
                let ci = self.push_expr(c);
                let then_lbl = self.module.function_count as u16; // stub
                let else_lbl = then_lbl + 1;
                block.push(IrStmt::Branch { cond: ci, then_block: then_lbl, else_block: else_lbl });
            }
            _ => {}
        }
    }

    // ── Real expression emission ──────────────────────────────────

    fn emit_expr(&mut self, e: &Expr) -> IrExpr {
        match e {
            Expr::Int(v) => IrExpr::ConstI64(*v),
            Expr::FloatLit(v) => IrExpr::ConstF64(*v),
            Expr::BoolLit(b) => IrExpr::ConstI64(if *b { 1 } else { 0 }),
            Expr::StringLit(s) => {
                let idx = self.module.add_string(s).unwrap_or(0);
                IrExpr::ConstStr(idx)
            }
            Expr::CharLit(c) => IrExpr::ConstI64(*c as i64),
            Expr::This => IrExpr::Arg(0),
            Expr::NullPtr => IrExpr::ConstI64(0),
            Expr::Var(_name) => IrExpr::Local(0),
            Expr::Call(name, args) => {
                let name_idx = self.module.add_string(name).unwrap_or(0);
                IrExpr::Call { func: name_idx, args: 0, arg_count: args.len() as u16 }
            }
            Expr::New(class_name, _args) => {
                let _ = self.module.add_string(class_name);
                IrExpr::Syscall { nr: 0x190, args: 0, arg_count: 1 }
            }
            Expr::MethodCall(obj, method, _args) => {
                let _obj_val = self.emit_expr(obj);
                let method_idx = self.module.add_string(method).unwrap_or(0);
                IrExpr::Call { func: method_idx, args: 0, arg_count: 1 }
            }
            Expr::VirtualCall(obj, method, vtable_offset, _args) => {
                let obj_val = self.emit_expr(obj);
                let this_idx = self.push_expr(obj_val);
                let method_idx = self.module.add_string(method).unwrap_or(0);
                IrExpr::VCall { this: this_idx, vtable_offset: *vtable_offset, sig_type_id: 0, args: method_idx, arg_count: 1 }
            }
            Expr::Syscall(def, _args) => {
                IrExpr::Syscall { nr: def.nr, args: 0, arg_count: def.arg_count as u16 }
            }
            Expr::Add(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Add, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Sub(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Sub, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Mul(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Mul, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Div(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Div, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Eq(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Eq, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Neq(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Ne, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Lt(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Lt, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Le(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Le, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Gt(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Gt, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Ge(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Ge, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Or(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::Or, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::And(l, r) => { let lv = self.emit_expr(l); let rv = self.emit_expr(r); IrExpr::Binary { op: IrBinOp::And, lhs: self.push_expr(lv), rhs: self.push_expr(rv) } },
            Expr::Not(inner) => { let e = self.emit_expr(inner); IrExpr::Unary { op: IrUnOp::Not, expr: self.push_expr(e) } },
            Expr::Neg(inner) => { let e = self.emit_expr(inner); IrExpr::Unary { op: IrUnOp::Neg, expr: self.push_expr(e) } },
            Expr::BitNot(inner) => { let e = self.emit_expr(inner); IrExpr::Unary { op: IrUnOp::BitNot, expr: self.push_expr(e) } },
            Expr::Deref(inner) => { let e = self.emit_expr(inner); IrExpr::Load { base: self.push_expr(e), offset: 0, ty: IrType::I64 } },
            Expr::AddrOf(_inner) => IrExpr::ConstI64(0),
            Expr::Assign(name, val) => {
                let val_expr = self.emit_expr(val);
                let vi = self.push_expr(val_expr);
                IrExpr::Binary { op: IrBinOp::Add, lhs: vi, rhs: vi }
            }
            _ => IrExpr::ConstI64(0),
        }
    }
}
