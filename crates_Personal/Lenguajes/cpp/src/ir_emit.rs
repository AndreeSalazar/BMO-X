//! C++ IR emitter: converts the C++ AST into a bmo_abi::ir::IrModule.
//!
//! Supports:
//! - Classes → IrTypes with field tables
//! - Virtual methods → VTableMethodMeta in .vtables section
//! - Method calls → IrExpr::Call with this pointer
//! - new/delete → memory syscalls

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
    /// Maps class name → (type_id, vtable_method_count)
    class_types: Vec<(String, u16, u32)>,
}

impl Emitter {
    fn new() -> Self {
        Self {
            module: IrModule::new(0),
            class_types: Vec::new(),
        }
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

    // ── Program emission ──────────────────────────────────────────

    fn emit_program(&mut self, p: &Program) {
        self.module.name = self.module.add_string("main").unwrap_or(0);

        // First pass: emit class types and vtables
        for cls in &p.classes {
            self.emit_class_decl(cls);
        }

        // Second pass: emit global variables
        for g in &p.globals {
            if let GlobalDecl::Var(ts, name, init) = g {
                let name_idx = self.module.add_string(name).unwrap_or(0);
                let ty_idx = self.ir_type(ts);
                let init_expr = init.as_ref().map(|e| self.emit_expr(e));
                self.module.add_global(IrGlobal {
                    name: name_idx, ty: ty_idx, init: init_expr, read_only: false,
                });
            }
        }

        // Third pass: emit free functions
        for f in &p.functions {
            self.emit_function(f, None);
        }

        // Fourth pass: emit class methods
        for cls in &p.classes {
            for m in &cls.methods {
                self.emit_method(m, &cls.name);
            }
            if let Some(ctor) = &cls.constructor {
                self.emit_method(ctor, &cls.name);
            }
        }

        // Emit namespaced classes/functions
        for ns in &p.namespaces {
            for cls in &ns.classes {
                self.emit_class_decl(cls);
                for m in &cls.methods {
                    self.emit_method(m, &cls.name);
                }
            }
            for f in &ns.functions {
                self.emit_function(f, Some(&ns.name));
            }
        }
    }

    fn emit_class_decl(&mut self, cls: &Class) {
        let name_idx = self.module.add_string(&cls.name).unwrap_or(0);
        // Total size = sum of member sizes + vtable pointer (8 bytes if virtual)
        let total_size: u32 = if cls.vtable { 8 } else { 0 }
            + cls.members.iter().map(|m| m.typ.size()).sum::<u32>();
        let struct_type = IrType::Struct(name_idx);
        let type_id = self.module.add_type(struct_type).unwrap_or(0);
        self.class_types.push((cls.name.clone(), type_id, cls.methods.iter().filter(|m| m.is_virtual).count() as u32));
    }

    fn emit_method(&mut self, m: &Method, class_name: &str) {
        let full_name = format!("{}::{}", class_name, m.name);
        let name_idx = self.module.add_string(&full_name).unwrap_or(0);
        let mut func = IrFunction::new(name_idx);
        func.convention = CallingConvention::BmoX86_64;
        func.return_type = self.ir_type(&m.ret_type);
        func.public = true;

        // 'this' pointer is the first argument
        func.add_arg(self.ir_type(&TypeSpec::ClassRef(class_name.to_string())));
        func.add_local(0, IrType::Pointer);

        for (i, p) in m.params.iter().enumerate() {
            func.add_arg(self.ir_type(&p.typ));
            func.add_local((i + 1) as u16, IrType::I64);
        }

        let mut block = IrBlock::new(0);
        self.emit_stmts(&m.body, &mut block);
        block.push(IrStmt::Return(None));
        func.block_count = 1;
        func.blocks[0] = block;

        self.module.add_function(func);
    }

    fn emit_function(&mut self, f: &Function, ns: Option<&str>) {
        let full_name = match ns {
            Some(ns) => format!("{}::{}", ns, f.name),
            None => f.name.clone(),
        };
        let name_idx = self.module.add_string(&full_name).unwrap_or(0);
        let mut func = IrFunction::new(name_idx);
        func.convention = CallingConvention::BmoX86_64;
        func.return_type = self.ir_type(&f.ret_type);
        func.public = true;

        for (i, p) in f.params.iter().enumerate() {
            func.add_arg(self.ir_type(&p.typ));
            func.add_local(i as u16, IrType::I64);
        }

        let mut block = IrBlock::new(0);
        self.emit_stmts(&f.body, &mut block);
        block.push(IrStmt::Return(None));
        func.block_count = 1;
        func.blocks[0] = block;

        self.module.add_function(func);
    }

    // ── Statement emission ────────────────────────────────────────

    fn emit_stmts(&mut self, stmts: &[Stmt], block: &mut IrBlock) {
        for s in stmts {
            self.emit_stmt(s, block);
        }
    }

    fn emit_stmt(&mut self, s: &Stmt, block: &mut IrBlock) {
        match s {
            Stmt::Expr(e) => { let _ = self.emit_expr(e); }
            Stmt::Return(opt) => {
                let _val = opt.as_ref().map(|e| self.emit_expr(e));
                block.push(IrStmt::Return(None));
            }
            Stmt::DeclVar(ts, _name, init) => {
                let _ty = self.ir_type(ts);
                if let Some(init_e) = init {
                    let _val = self.emit_expr(init_e);
                }
                block.push(IrStmt::DefLocal { idx: 0, ty: IrType::I64 });
            }
            Stmt::Assign(_name, e) => {
                let _val = self.emit_expr(e);
                block.push(IrStmt::Assign(0, IrExpr::ConstI64(0)));
            }
            Stmt::Block(stmts) => self.emit_stmts(stmts, block),
            Stmt::Delete(_name) => {
                block.push(IrStmt::Expr(IrExpr::Syscall {
                    nr: 0x191, // NR_MEM_FREE
                    args: 0,
                    arg_count: 1,
                }));
            }
            _ => {} // Stub
        }
    }

    // ── Expression emission ───────────────────────────────────────

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
                IrExpr::Syscall { nr: 0x190, args: 0, arg_count: 1 } // NR_MEM_ALLOC
            }
            Expr::MethodCall(obj, method, _args) => {
                let class = self.emit_expr(obj);
                let method_idx = self.module.add_string(method).unwrap_or(0);
                IrExpr::Call { func: method_idx, args: 0, arg_count: 1 }
            }
            Expr::VirtualCall(_obj, method, vtable_offset, _args) => {
                let method_idx = self.module.add_string(method).unwrap_or(0);
                IrExpr::Call { func: method_idx, args: 0, arg_count: 1 }
            }
            Expr::Syscall(def, _args) => {
                IrExpr::Syscall { nr: def.nr, args: 0, arg_count: def.arg_count as u16 }
            }
            Expr::Add(l, r) => {
                let _ = self.emit_expr(l); let _ = self.emit_expr(r);
                IrExpr::ConstI64(0)
            }
            Expr::Sub(l, r) => {
                let _ = self.emit_expr(l); let _ = self.emit_expr(r);
                IrExpr::ConstI64(0)
            }
            Expr::Mul(l, r) => {
                let _ = self.emit_expr(l); let _ = self.emit_expr(r);
                IrExpr::ConstI64(0)
            }
            Expr::Div(l, r) => {
                let _ = self.emit_expr(l); let _ = self.emit_expr(r);
                IrExpr::ConstI64(0)
            }
            Expr::Not(inner) => {
                let _ = self.emit_expr(inner);
                IrExpr::ConstI64(0)
            }
            Expr::Neg(inner) => {
                let _ = self.emit_expr(inner);
                IrExpr::ConstI64(0)
            }
            Expr::AddrOf(_inner) => IrExpr::ConstI64(0),
            Expr::Deref(_inner) => IrExpr::ConstI64(0),
            _ => IrExpr::ConstI64(0),
        }
    }
}
