//! Operaciones sobre la propia tarea -- L1.
//!
//! `CURRENT_TASK` es un pseudo-handle process-local: no otorga autoridad
//! sobre nadie mas y nunca debe transferirse. Por eso estas operaciones no
//! necesitan una capability previa y sirven de bootstrap para cualquier
//! programa recien nacido.

use bmo_abi::syscalls::surface::{CURRENT_TASK, NR_INVOKE, TASK_OP_EXIT, TASK_OP_YIELD};

use crate::x86::{self, RAX, RDI, RSI};

fn invoke_current_task(code: &mut Vec<u8>, operation: u64) {
    x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
    x86::mov_r32_imm32(code, RSI, operation as u32);
    if NR_INVOKE == 0 {
        x86::zero_r32(code, RAX);
    } else {
        x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    }
    x86::syscall(code);
}

/// Termina el proceso. No retorna: el scheduler lo cosecha.
///
/// Emite ademas una red de seguridad (`pause`/`jmp -4`) por si `EXIT` alguna
/// vez retornara -- sin ella el CPU se saldria del final del codigo y
/// ejecutaria lo que hubiera despues. Es el mismo cierre que usa
/// `tools/hello-bex`.
pub fn exit(code: &mut Vec<u8>) {
    invoke_current_task(code, TASK_OP_EXIT);
    code.extend_from_slice(&[0xF3, 0x90]); // pause
    code.extend_from_slice(&[0xEB, 0xFC]); // jmp -4
}

/// Cede el CPU voluntariamente.
pub fn yield_now(code: &mut Vec<u8>) {
    invoke_current_task(code, TASK_OP_YIELD);
}

/// `INVOKE(CURRENT_TASK, <op>)` para una operacion sin argumentos que el
/// frontend elija -- el resultado queda en `rdx`, el estado en `rax`.
///
/// Existe para que una L2 pueda usar `GET_PID`/`GET_TID` sin que L1 tenga
/// que crecer una funcion por operacion.
pub fn invoke_no_args(code: &mut Vec<u8>, operation: u64) {
    invoke_current_task(code, operation);
}

/// Las operaciones que hoy acepta `CURRENT_TASK`. Se listan aqui --en vez de
/// reexportar el modulo del ABI, que es privado-- para que una L2 las nombre
/// sin declarar `bmo-abi` como dependencia propia.
pub mod ops {
    use bmo_abi::syscalls::surface;

    pub const GET_PID: u64 = surface::TASK_OP_GET_PID;
    pub const GET_TID: u64 = surface::TASK_OP_GET_TID;
    pub const YIELD: u64 = surface::TASK_OP_YIELD;
    pub const EXIT: u64 = surface::TASK_OP_EXIT;
    pub const CHANNEL_OPEN: u64 = surface::TASK_OP_CHANNEL_OPEN;
    pub const CONSOLE_WRITE: u64 = surface::TASK_OP_CONSOLE_WRITE;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bmo_abi::syscalls::surface::CURRENT_TASK as CT;

    #[test]
    fn exit_emits_invoke_then_a_spin_net() {
        let mut code = Vec::new();
        exit(&mut code);

        let mut expected = Vec::new();
        expected.extend_from_slice(&[0x48, 0xBF]);
        expected.extend_from_slice(&CT.to_le_bytes());
        expected.push(0xBE);
        expected.extend_from_slice(&(TASK_OP_EXIT as u32).to_le_bytes());
        expected.extend_from_slice(&[0x31, 0xC0]);
        expected.extend_from_slice(&[0x0F, 0x05]);
        expected.extend_from_slice(&[0xF3, 0x90]);
        expected.extend_from_slice(&[0xEB, 0xFC]);

        assert_eq!(code, expected);
    }
}
