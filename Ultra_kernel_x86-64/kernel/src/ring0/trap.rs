//! Unified trap frame — the single context representation for every Ring 0
//! entry (SYSCALL, IRQ, first switch into a task).
//!
//! Both trap entries push 15 GPRs in the same order, then hand control to a
//! Rust dispatcher with a pointer to this frame. Below the frame sits the
//! extended-state image (64-byte aligned) and a back-pointer slot:
//!
//! ```text
//! high ─►  [ss][rsp][rflags][cs][rip]   trap tail (5 u64)
//!          [rax]...[r15]                15 GPRs (pushed rax first)
//!          gpr_base = &frame
//!          [gpr_base back-ptr]          at xsave_base + XSAVE_AREA
//! low  ─►  [xsave XSAVE_AREA B, 64-al]  xsave_base = "context rsp"
//! ```
//!
//! A context is fully described by its `xsave_base`: the shared epilogue does
//! `rsp = xsave_base; xrstor; rsp = [rsp+XSAVE_AREA]; pop 15; iretq`.
//! Context switch = choosing the next `xsave_base`.
//!
//! ## Por qué XSAVE y no FXSAVE
//!
//! `FXSAVE` guarda 512 bytes: x87 y SSE. En este Ryzen el firmware ya dejó
//! `CR4.OSXSAVE` puesto y `XCR0 = 0x7`, o sea **AVX habilitado antes de que el
//! kernel arrancara**. Un programa Ring 3 que usara `YMM` perdía la mitad alta
//! de sus registros en el primer cambio de tarea, sin fault y sin aviso.
//!
//! ## La máscara es todo unos, y eso es lo que lo hace portable
//!
//! `XSAVE`/`XRSTOR` operan sobre `RFBM ∩ XCR0`. Con `RFBM = -1` se guarda
//! **exactamente lo que este CPU tenga habilitado**, sea cual sea. No hay que
//! saber qué CPU es ni leer `XCR0` desde el ensamblador: la máscara es una
//! constante correcta en cualquier máquina. Un valor concreto (0x7) habría
//! sido un número de este Ryzen metido en el camino más crítico del kernel.
//!
//! ## El área es fija, y por eso se verifica al arrancar
//!
//! El ensamblador necesita el desplazamiento del back-pointer como constante,
//! así que el área tiene tamaño fijo. Lo que ocupa de verdad lo decide el CPU
//! (CPUID hoja 0xD), así que `cpu_vendor::xsave::init()` lo comprueba **antes
//! de que exista el primer trap** y se planta si no cabe. Reservar de más
//! cuesta unos KiB; quedarse corto desborda la pila de una tarea.

/// Frame passed to Rust dispatchers. Field order matches the push order:
/// `push rax, rcx, rdx, rbx, rbp, rsi, rdi, r8..r15` leaves `r15` lowest.
#[repr(C)]
pub struct TrapFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    /// Valid only when the trap came from Ring 3 (or was fabricated).
    pub rsp: u64,
    pub ss: u64,
}

pub const RFLAGS_IF: u64 = 1 << 9;

pub const KERNEL_CS: u64 = 0x08;
pub const KERNEL_SS: u64 = 0x08;
pub const USER_CS: u64 = 0x23;
pub const USER_SS: u64 = 0x1B;

/// Bytes reservados para la imagen de estado extendido en CADA contexto.
///
/// Es constante porque el ensamblador de los stubs necesita el desplazamiento
/// del back-pointer como inmediato. 3072 cubre de sobra lo que pide cualquier
/// x86-64 actual sin AMX: este Zen 3 necesita 832 con `XCR0 = 0x7`, y un CPU
/// con AVX-512 pide ~2696. `cpu_vendor::xsave::init()` comprueba contra CPUID
/// que de verdad cabe, y lo hace ANTES del primer trap.
///
/// Múltiplo de 64 porque `XSAVE` exige esa alineación.
pub const XSAVE_AREA: usize = 3072;

/// Área + back-pointer + margen para realinear a 64. Es lo que los stubs
/// restan de la pila antes del `and rsp, -64`.
pub const XSAVE_RESERVA: usize = XSAVE_AREA + 8 + 64;

/// GPR block (15*8) + back-pointer slot (8) + alignment slack (8).
const FRAME_BYTES_BELOW_TAIL: usize = 15 * 8 + 8 + 8;
/// Bytes consumed from the stack top: tail (40) + GPRs + back-ptr + xsave
/// + alignment slack.
const CONTEXT_BYTES: usize = 40 + FRAME_BYTES_BELOW_TAIL + XSAVE_AREA + 64;

/// Fabricate the initial context of a task on its own kernel stack.
/// Returns the `fxsave_base` that becomes the task's `context_rsp`.
///
/// `user` selects a Ring 3 frame (iretq switches stack + CPL); kernel tasks
/// get a same-privilege frame whose post-iretq RSP lands 8 mod 16, matching
/// the SysV post-`call` alignment Rust code expects.
pub unsafe fn fabricate(
    stack_top: u64,
    entry: u64,
    arg: u64,
    user: bool,
    user_rsp: u64,
) -> u64 {
    let gpr_base = (stack_top - 168) & !0xF | 0x8; // ≡ 8 (mod 16)
    // 64-alineado: XSAVE lo exige y da #GP si no.
    let xsave_base = (gpr_base - (XSAVE_AREA as u64 + 8)) & !63;

    core::ptr::write_bytes(xsave_base as *mut u8, 0, XSAVE_AREA);
    // Cabecera XSAVE (offset 512): XSTATE_BV = 0 significa "todos los
    // componentes en estado INICIAL", que es exactamente lo que quiere una
    // tarea nueva — XRSTOR los pone a sus valores por defecto sin leer nada
    // más. XCOMP_BV = 0 declara formato estándar, que es el que usan los
    // stubs (`xsave64`, no `xsavec64`).
    //
    // MXCSR sí hay que ponerlo aunque XSTATE_BV sea 0: XRSTOR lo carga del
    // área igualmente y da #GP si tiene bits reservados. Un área a ceros
    // dejaría MXCSR = 0, o sea TODAS las excepciones de coma flotante
    // desenmascaradas — la tarea moriría en la primera división inexacta.
    (xsave_base as *mut u16).write_volatile(0x037F); // FCW
    ((xsave_base + 24) as *mut u32).write_volatile(0x1F80); // MXCSR
    ((xsave_base + XSAVE_AREA as u64) as *mut u64).write_volatile(gpr_base); // back-ptr

    let frame = &mut *(gpr_base as *mut TrapFrame);
    core::ptr::write_bytes(frame as *mut TrapFrame as *mut u8, 0, core::mem::size_of::<TrapFrame>());
    frame.rdi = arg;
    frame.rip = entry;
    frame.rflags = RFLAGS_IF | 0x2;
    if user {
        frame.cs = USER_CS;
        frame.ss = USER_SS;
        frame.rsp = user_rsp;
    } else {
        frame.cs = KERNEL_CS;
        frame.ss = KERNEL_SS;
        frame.rsp = gpr_base + 144; // informational (same-CPL iretq ignores it)
    }
    xsave_base
}

/// Minimum kernel stack size that fits one fabricated context plus working
/// room for the task's own frames.
pub const MIN_TASK_STACK: usize = CONTEXT_BYTES + 4096;
