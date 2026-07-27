//! x86-64 SYSCALL entry and BMO ABI v2 dispatcher (3 frozen syscalls).
//!
//! The entry builds the unified trap frame (see trap.rs): swapgs, switch to
//! the per-CPU syscall stack, synthesize the 5-word trap tail (user SS/RSP/
//! RFLAGS/CS/RIP from the SYSCALL contract), push 15 GPRs, FXSAVE, then call
//! the Rust dispatcher with the frame pointer.
//!
//! Return is via `iretq`, never `sysretq`: one return path for traps and
//! syscalls, no non-canonical-RCX #GP hazard in Ring 0, and — critically —
//! the dispatcher may answer with a *different* context than the one that
//! entered (YIELD/WAIT/EXIT switch right at the syscall boundary).
//!
//! The surface is frozen at `INVOKE`, `CHANNEL_KICK`, `WAIT`. Everything
//! else is a capability operation resolved through `cap::resolve` — new
//! functionality adds operations and handle kinds, never syscalls.

use core::arch::{asm, naked_asm};

use crate::ring0::cap;
use crate::ring0::channel;
use crate::ring0::endpoint;
use crate::ring0::percpu;
use crate::ring0::scheduler;
use crate::ring0::trap::TrapFrame;

// Minimal no-alloc view of the canonical bmo-abi v2 contract. Keeping
// these values here avoids linking the full alloc-using ABI implementation
// into Ring 0; build.ps1 rejects values that drift from bmo-abi.
const NR_INVOKE: u32 = 0x00;
const NR_CHANNEL_KICK: u32 = 0x01;
const NR_WAIT: u32 = 0x02;
const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
const TASK_OP_GET_PID: u64 = 0x01;
const TASK_OP_GET_TID: u64 = 0x02;
const TASK_OP_YIELD: u64 = 0x03;
const TASK_OP_EXIT: u64 = 0x04;
const TASK_OP_CHANNEL_OPEN: u64 = 0x05;
const TASK_OP_CONSOLE_WRITE: u64 = 0x06;
/// Crea un endpoint atendido por este proceso. arg0 = estuario por el que se
/// le entregaran las llamadas. Devuelve el handle del endpoint.
const TASK_OP_ENDPOINT_CREATE: u64 = 0x07;
/// Pide el derecho a LLAMAR al endpoint `arg0`. Devuelve el handle de cliente.
///
/// Puerta de descubrimiento provisional, con el mismo aviso que lleva
/// `TASK_OP_CONSOLE_WRITE`: hoy cualquier proceso puede pedir cualquier
/// endpoint por su índice, y eso NO es disciplina de capabilities. Existe para
/// arrancar, y muere cuando haya un servicio de nombres que entregue el handle
/// a quien deba tenerlo. Se dice aquí para que nadie lo confunda con el
/// diseño final.
const TASK_OP_ENDPOINT_CONNECT: u64 = 0x08;
/// Reclamar la pantalla. Devuelve un handle `KIND_FRAMEBUFFER` y, con él, el
/// framebuffer mapeado en el espacio del proceso. Ver `ring0/fb.rs`: a partir
/// de aquí el kernel deja de dibujar y el proceso escribe píxeles con `mov`.
const TASK_OP_FRAMEBUFFER_CLAIM: u64 = 0x09;
/// Reclamar el raton. Devuelve un handle `KIND_INPUT`: el kernel lee el HID,
/// Ring 3 decide que hace con las coordenadas. Ver `ring0/input.rs`.
const TASK_OP_INPUT_CLAIM: u64 = 0x0A;
const CHANNEL_OP_GET_SEQ: u64 = 0x01;
const CHANNEL_OP_GET_INDEX: u64 = 0x02;
const ERROR_INVALID_ARGUMENT: u32 = 7;
const ERROR_UNSUPPORTED: u32 = 10;

#[repr(C)]
struct BmoStatus {
    code: u32,
    flags: u32,
    value: u64,
}

impl BmoStatus {
    const fn ok_value(value: u64) -> Self { Self { code: 0, flags: 0, value } }
    const fn err(code: u32) -> Self { Self { code, flags: 0, value: 0 } }
    const fn err_with_flags(code: u32, flags: u32) -> Self { Self { code, flags, value: 0 } }
}

const _: () = assert!(core::mem::size_of::<BmoStatus>() == 16);

const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;
const RFLAGS_TF: u64 = 1 << 8;
const RFLAGS_IF: u64 = 1 << 9;
const RFLAGS_DF: u64 = 1 << 10;
const RFLAGS_NT: u64 = 1 << 14;
const RFLAGS_AC: u64 = 1 << 18;
const KERNEL_CS: u64 = 0x08;
// Legacy STAR layout; kept armed although the exit path is iretq-only.
const SYSRET_SELECTOR_BASE: u64 = 0x10;

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() -> ! {
    naked_asm!(
        "swapgs",
        "mov gs:[0x08], rsp",          // stash user RSP
        "mov rsp, gs:[0x00]",          // per-CPU syscall stack
        // Trap tail: ss, rsp, rflags, cs, rip (SYSCALL contract values).
        "push 0x1B",                   // user SS
        "push qword ptr gs:[0x08]",    // user RSP
        "push r11",                    // user RFLAGS
        "push 0x23",                   // user CS
        "push rcx",                    // user RIP
        // 15 GPRs (push order; pops restore the reverse).
        "push rax", "push rcx", "push rdx", "push rbx", "push rbp",
        "push rsi", "push rdi", "push r8", "push r9", "push r10",
        "push r11", "push r12", "push r13", "push r14", "push r15",
        "mov rbp, rsp",
        "sub rsp, {reserva}",
        "and rsp, -64",                // XSAVE exige 64 bytes de alineacion
        "mov [rsp+{area}], rbp",       // back-pointer to the GPR block
        "mov qword ptr [rsp+{firma}], {magia}", // sello del contexto
        // RFBM = -1: guarda lo que XCR0 tenga habilitado, sea lo que sea.
        // rax y rdx ya estan salvados en el bloque de GPR de arriba.
        "mov eax, -1", "mov edx, -1",
        "xsave64 [rsp]",
        "mov gs:[0x10], rsp",          // publish this context
        "cld",
        "mov rdi, rbp",
        "call {dispatch}",
        // Shared trap epilogue: rax = xsave-base of the context to run.
        "mov rsp, rax",
        "cmp qword ptr [rsp+{firma}], {magia}",
        "jne 3f",
        "mov eax, -1", "mov edx, -1",
        "xrstor64 [rsp]",
        "mov rsp, [rsp+{area}]",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
        "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
        "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
        "cmp qword ptr [rsp+8], 0x08", // returning to kernel CS?
        "je 1f",
        "cmp qword ptr [rsp+8], 0x23", // ...o a usuario. Cualquier otra cosa
        "jne 4f",                      // no es un contexto, es basura.
        "swapgs",
        "1: iretq",
        "3: mov rdi, {m_sello}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "4: mov rdi, {m_cs}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        dispatch = sym dispatch,
        podrido = sym crate::ring0::faults::contexto_podrido,
        area = const crate::ring0::trap::XSAVE_AREA,
        firma = const crate::ring0::trap::SELLO_FIRMA,
        magia = const crate::ring0::trap::SELLO_MAGIA,
        m_sello = const crate::ring0::faults::PODRIDO_SELLO,
        m_cs = const crate::ring0::faults::PODRIDO_CS,
        reserva = const crate::ring0::trap::XSAVE_RESERVA,
    );
}

unsafe fn wrmsr(msr: u32, value: u64) {
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") value as u32,
        in("edx") (value >> 32) as u64,
        options(nostack),
    );
}

pub fn init() {
    let star = (SYSRET_SELECTOR_BASE << 48) | (KERNEL_CS << 32);
    unsafe {
        wrmsr(MSR_STAR, star);
        wrmsr(MSR_LSTAR, syscall_entry as *const () as u64);
        // Do not let hostile user flags trigger #DB/#AC before the entry stub
        // has switched away from the user stack. Interrupts stay masked for
        // the whole dispatch; the iretq restores the user IF.
        wrmsr(
            MSR_SFMASK,
            RFLAGS_TF | RFLAGS_IF | RFLAGS_DF | RFLAGS_NT | RFLAGS_AC,
        );
    }
}

#[inline]
fn unsupported() -> BmoStatus {
    BmoStatus::err(ERROR_UNSUPPORTED)
}

#[inline]
fn cap_err(err: (u32, u32)) -> BmoStatus {
    BmoStatus::err_with_flags(err.0, err.1)
}

fn invoke_current_task(operation: u64, arg0: u64) -> BmoStatus {
    match operation {
        TASK_OP_GET_PID => BmoStatus::ok_value(scheduler::current_pid() as u64),
        TASK_OP_GET_TID => BmoStatus::ok_value(scheduler::current_tid() as u64),
        // These switch at the syscall boundary; when (if) this context runs
        // again it resumes here and reports success.
        TASK_OP_YIELD => {
            scheduler::yield_current();
            BmoStatus::ok_value(0)
        }
        TASK_OP_EXIT => {
            let _ = arg0;
            // Capabilities die with the process; every outstanding handle
            // becomes invalid before the final switch (no SCHED/CAP lock
            // nesting: revoke completes first).
            cap::revoke_all(scheduler::current_pid());
            scheduler::exit_current();
            BmoStatus::ok_value(0)
        }
        // Bootstrap console: render up to 8 packed bytes (LE, NUL-stop) to
        // the kernel's on-screen log + serial. This is how the first Ring 3
        // program draws — the whole point of the CPL3→CPL0 demo. It writes
        // nothing but text and cannot escalate; the caller only ever paints
        // into the kernel-owned console surface.
        TASK_OP_CONSOLE_WRITE => {
            crate::ring0::uconsole::write_packed(arg0);
            BmoStatus::ok_value(0)
        }
        // Discover the caller's seeded estuary capability for index arg0.
        // The handle is the process's own; nothing new is granted here.
        TASK_OP_CHANNEL_OPEN => {
            if arg0 >= boot_context::MAX_CHANNEL_PAGES as u64 {
                return BmoStatus::err(ERROR_INVALID_ARGUMENT);
            }
            match cap::find(scheduler::current_pid(), cap::KIND_CHANNEL, arg0) {
                Some(handle) => BmoStatus::ok_value(handle),
                None => cap_err((cap::ERROR_PERMISSION_DENIED, cap::FLAG_NEEDS_CAP)),
            }
        }
        TASK_OP_ENDPOINT_CREATE => {
            match endpoint::crear(scheduler::current_pid(), arg0 as usize) {
                Some(handle) => BmoStatus::ok_value(handle),
                None => BmoStatus::err(endpoint::ERROR_BUSY),
            }
        }
        TASK_OP_ENDPOINT_CONNECT => {
            match endpoint::conceder_cliente(arg0 as usize, scheduler::current_pid()) {
                Some(handle) => BmoStatus::ok_value(handle),
                None => BmoStatus::err(endpoint::ERROR_ENDPOINT_DEAD),
            }
        }
        // La pantalla. El espacio de direcciones en el que se mapea es el que
        // está cargado AHORA: durante un SYSCALL desde Ring 3, CR3 sigue
        // siendo el del llamante — el cambio de CR3 sólo ocurre en un cambio
        // de contexto, y aquí todavía no ha habido ninguno.
        TASK_OP_INPUT_CLAIM => {
            let _ = arg0;
            match crate::ring0::input::reclamar(scheduler::current_pid()) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        TASK_OP_FRAMEBUFFER_CLAIM => {
            let _ = arg0;
            match crate::ring0::fb::reclamar(
                scheduler::current_pid(),
                crate::ring0::mm::vmm::read_cr3(),
            ) {
                Ok(handle) => BmoStatus::ok_value(handle),
                Err(code) => BmoStatus::err(code),
            }
        }
        _ => unsupported(),
    }
}

/// Synchronous control operations on a resolved capability.
fn invoke_channel(resolved: cap::Resolved, operation: u64) -> BmoStatus {
    let index = resolved.object as usize;
    match operation {
        CHANNEL_OP_GET_SEQ => BmoStatus::ok_value(channel::complete_seq(index)),
        CHANNEL_OP_GET_INDEX => BmoStatus::ok_value(resolved.object),
        _ => unsupported(),
    }
}

/// `INVOKE(capability, operation, a0..a3)` — the single synchronous door.
fn invoke(frame: &TrapFrame) -> BmoStatus {
    if frame.rdi == CURRENT_TASK {
        return invoke_current_task(frame.rsi, frame.rdx);
    }
    let pid = scheduler::current_pid();
    // Un endpoint se resuelve con WRITE (llamar), no con READ: son derechos
    // distintos y el cliente solo tiene el de llamar.
    if let Ok(r) = cap::resolve(pid, frame.rdi, cap::RIGHT_WRITE) {
        match r.kind {
            cap::KIND_ENDPOINT => {
                // Argumentos: rdi(cap), rsi(op), rdx, r10, r8.
                //
                // ★ NO rcx. En `SYSCALL` el CPU mete ahí el RIP de retorno —
                // por eso el prólogo hace `push rcx` como RIP de usuario— y
                // r11 se lleva RFLAGS. Un argumento en rcx no es el dato del
                // cliente: es la dirección a la que va a volver. Por eso la
                // convención salta a r10, igual que en Linux.
                let res = endpoint::llamar(
                    r.object as usize,
                    frame.rsi,
                    [frame.rdx, frame.r10, frame.r8],
                );
                return BmoStatus { code: res.code, flags: 0, value: res.value };
            }
            cap::KIND_REPLY => {
                let res = endpoint::responder(
                    pid, frame.rdi, r.object, frame.rsi as u32, frame.rdx,
                );
                return BmoStatus { code: res.code, flags: 0, value: res.value };
            }
            _ => {}
        }
    }
    match cap::resolve(pid, frame.rdi, cap::RIGHT_READ) {
        Ok(resolved) => match resolved.kind {
            cap::KIND_CHANNEL => invoke_channel(resolved, frame.rsi),
            // La pantalla sólo contesta preguntas: dónde está y qué forma
            // tiene. Los píxeles no pasan por aquí — para eso está mapeada.
            // El raton solo contesta donde esta y que botones tiene. Dibujar
            // el cursor es una decision de aspecto, y eso no es del kernel.
            cap::KIND_INPUT => match crate::ring0::input::operacion(frame.rsi) {
                Some(v) => BmoStatus::ok_value(v),
                None => unsupported(),
            },
            cap::KIND_FRAMEBUFFER => {
                match crate::ring0::fb::operacion(resolved.object, frame.rsi) {
                    Some(v) => BmoStatus::ok_value(v),
                    None => unsupported(),
                }
            }
            _ => unsupported(),
        },
        Err(err) => cap_err(err),
    }
}

/// `CHANNEL_KICK(capability, published_sequence)` — notify the consumer.
/// Services the estuary with the per-kick budget and wakes its waiters.
fn channel_kick(frame: &TrapFrame) -> BmoStatus {
    let pid = scheduler::current_pid();
    match cap::resolve(pid, frame.rdi, cap::RIGHT_WRITE) {
        Ok(resolved) if resolved.kind == cap::KIND_CHANNEL => {
            let processed = channel::service(resolved.object as usize);
            BmoStatus::ok_value(processed as u64)
        }
        Ok(_) => unsupported(),
        Err(err) => cap_err(err),
    }
}

/// `WAIT(waitable, observed_sequence, timeout_ns)` — block until the
/// waitable's sequence moves past `observed_sequence` or the timeout
/// expires (0 = no timeout). `waitable = 0` is a pure timed sleep.
///
/// The observed-sequence compare happens under the scheduler lock, so a
/// kick between the caller's read and this syscall can never be lost.
fn wait(frame: &TrapFrame) -> BmoStatus {
    let timeout_ns = frame.rdx;
    let deadline = if timeout_ns == 0 {
        0
    } else {
        scheduler::rdtsc() + scheduler::ns_to_tsc(timeout_ns)
    };
    if frame.rdi == 0 {
        scheduler::wait_current(0, deadline);
        return BmoStatus::ok_value(0);
    }
    let pid = scheduler::current_pid();
    // El servidor esperando llamadas en su endpoint.
    if let Ok(r) = cap::resolve(pid, frame.rdi, cap::RIGHT_WAIT) {
        if r.kind == cap::KIND_ENDPOINT {
            let res = endpoint::esperar(r.object as usize, pid, deadline);
            return BmoStatus { code: res.code, flags: 0, value: res.value };
        }
    }
    match cap::resolve(pid, frame.rdi, cap::RIGHT_WAIT) {
        Ok(resolved) if resolved.kind == cap::KIND_CHANNEL => {
            let index = resolved.object as usize;
            let seq = scheduler::wait_current_checked(
                channel::wait_key(index),
                deadline,
                frame.rsi,
                || channel::complete_seq(index),
            );
            // Advisory: userland re-reads the shared sequence on resume.
            BmoStatus::ok_value(seq)
        }
        Ok(_) => unsupported(),
        Err(err) => cap_err(err),
    }
}

#[unsafe(no_mangle)]
extern "C" fn dispatch(frame: &mut TrapFrame) -> u64 {
    let status = match frame.rax as u32 {
        NR_INVOKE => invoke(frame),
        NR_CHANNEL_KICK => channel_kick(frame),
        NR_WAIT => wait(frame),
        _ => unsupported(),
    };
    frame.rax = (status.code as u64) | ((status.flags as u64) << 32);
    frame.rdx = status.value;
    percpu::trap_rsp()
}
