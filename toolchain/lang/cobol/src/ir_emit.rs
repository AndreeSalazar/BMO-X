//! IR emitter: converts the COBOL AST into a bmo_abi::ir::IrModule.

use bmo_abi::ir::*;
use bmo_abi::types::convention::CallingConvention;
use crate::ast::{CobolProgram, CobolStatement, DisplayArg};

pub fn compile_to_ir(program: &CobolProgram) -> IrModule {
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

    fn emit_program(&mut self, p: &CobolProgram) {
        self.module.name = self.module.add_string(&p.program_id).unwrap_or(0);

        // Emit implicit _start function wrapping the COBOL statements
        let mut func = IrFunction::new(0);
        func.convention = CallingConvention::BmoX86_64;
        func.return_type = self.module.add_type(IrType::Void).unwrap_or(0);
        func.public = true;

        // Allocate one local for the implicit WORKING-STORAGE data area
        func.add_local(0, IrType::I64);
        for (i, _item) in p.data_items.iter().enumerate() {
            func.add_local((i + 1) as u16, IrType::I64);
        }

        let mut block = IrBlock::new(0);
        self.emit_stmts(&p.statements, &mut block, &mut func);
        block.push(IrStmt::Return(None));
        func.block_count = 1;
        func.blocks[0] = block;

        self.module.add_function(func);
    }

    fn emit_stmts(&mut self, stmts: &[CobolStatement], block: &mut IrBlock, func: &mut IrFunction) {
        for s in stmts {
            self.emit_stmt(s, block, func);
        }
    }

    fn emit_stmt(&mut self, s: &CobolStatement, block: &mut IrBlock, _func: &mut IrFunction) {
        match s {
            // El camino de IR solo sabe de literales todavia. Una variable
            // necesita formateo en ejecucion, que es codigo y no una cadena en
            // una tabla — se emite por `codegen.rs`, no por aqui.
            CobolStatement::Display(DisplayArg::Literal(msg)) => {
                let nr = 0x1F0u32; // NR_DIAG_PRINT
                let str_idx = self.module.add_string(msg).unwrap_or(0);
                block.push(IrStmt::Expr(IrExpr::Syscall {
                    nr,
                    args: 0,
                    arg_count: 1,
                }));
            }
            CobolStatement::StopRun => {
                // Process termination is emitted by the BEF entry wrapper as
                // INVOKE(CURRENT_TASK, EXIT); the language IR only returns.
                block.push(IrStmt::Return(None));
            }
            CobolStatement::Syscall(def, _args) => {
                block.push(IrStmt::Expr(IrExpr::Syscall {
                    nr: def.nr,
                    args: 0,
                    arg_count: def.arg_count as u16,
                }));
            }
            CobolStatement::Move(src, dst) => {
                // MOVE src TO dst → assign
                let _d = self.module.add_string(dst);
                let _s = self.module.add_string(src);
                block.push(IrStmt::Assign(0, IrExpr::ConstI64(0)));
            }
            CobolStatement::Add(src, dst, _) => {
                let _s = self.module.add_string(src);
                let _d = self.module.add_string(dst);
                block.push(IrStmt::Assign(0, IrExpr::ConstI64(0)));
            }
            CobolStatement::Compute(target, expr, _) => {
                let _t = self.module.add_string(target);
                let _e = self.module.add_string(expr);
                block.push(IrStmt::Assign(0, IrExpr::ConstI64(0)));
            }
            // Stubs for remaining COBOL statements
            _ => {
                block.push(IrStmt::Expr(IrExpr::ConstI64(0)));
            }
        }
    }
}
