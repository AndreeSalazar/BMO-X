//! BSP local-APIC periodic tick on the unified trap frame.
//!
//! `s2_mem` calibrates and starts LAPIC vector 48. Ring 0 replaces that
//! vector's boot-time halt stub with this handler, which shares the exact
//! frame layout and epilogue with the SYSCALL entry: contexts captured here
//! and contexts captured there are interchangeable to the scheduler.

use boot_context::BootContext;
use core::arch::{asm, naked_asm};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::ring0::task::percpu;
use crate::ring0::plat::trap::TrapFrame;

const TIMER_VECTOR: usize = 48;
const SPURIOUS_VECTOR: usize = 0xFF;
const KERNEL_CS: u16 = 0x08;
const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
const MSR_APIC_BASE: u32 = 0x1B;
const APIC_BASE_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const APIC_ENABLED: u64 = 1 << 11;
const APIC_X2_MODE: u64 = 1 << 10;
const LAPIC_EOI_OFFSET: u64 = 0xB0;
const LAPIC_LVT_TIMER_OFFSET: u64 = 0x320;
const LAPIC_LVT_MASKED: u32 = 1 << 16;

static TICKS: AtomicU64 = AtomicU64::new(0);
static mut LAPIC_EOI: *mut u32 = core::ptr::null_mut();

#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    fn interrupt_gate(handler: u64) -> Self {
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CS,
            ist: 0,
            attributes: 0x8E,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

#[unsafe(naked)]
unsafe extern "C" fn timer_entry() -> ! {
    naked_asm!(
        // The CPU pushed rip/cs/rflags (plus rsp/ss from Ring 3). swapgs only
        // if we interrupted userland; Ring 0 already runs on its PerCpu GS.
        "cmp qword ptr [rsp+8], 0x08",
        "je 1f",
        "swapgs",
        // 15 GPRs, same push order as the SYSCALL entry.
        "1: push rax", "push rcx", "push rdx", "push rbx", "push rbp",
        "push rsi", "push rdi", "push r8", "push r9", "push r10",
        "push r11", "push r12", "push r13", "push r14", "push r15",
        "mov rbp, rsp",
        "sub rsp, {reserva}",
        "and rsp, -64",                // XSAVE exige 64 bytes de alineacion
        // La CABECERA a cero ANTES del xsave64. No es precaucion: `XSAVE` NO
        // escribe los 48 bytes reservados de la cabecera (528..575) -- escribe
        // XSTATE_BV, pone XCOMP_BV a cero, y el resto lo deja COMO ESTABA. Y
        // `XRSTOR` da #GP(0) si no son todos cero.
        //
        // El area se talla en la pila con un `sub`+`and`, o sea sobre lo que
        // hubiera ahi. Sin esto, la basura que trajera la pila en esos 48 bytes
        // sobrevivia al guardado y reventaba en el `xrstor64` del epilogo --
        // intermitente, y peor en la pila de arranque, donde lo que hay debajo
        // cambia. `trap::fabricate` ya lo hacia bien (pone a cero los 1024); los
        // stubs no. Esta era la asimetria.
        //
        // * Y EL XSTATE_BV TAMBIEN, que es la mitad que faltaba.
        //
        // `XSAVE` no escribe XSTATE_BV entero. Lo que hace es:
        //
        //     XSTATE_BV <- (XSTATE_BV_viejo AND NOT RFBM) OR (XINUSE AND RFBM)
        //
        // y `RFBM = EDX:EAX AND XCR0 = 0x7`. O sea que **conserva todos los
        // bits fuera de XCR0** del valor que hubiera antes. Si el area se talla
        // sobre basura, esa basura sobrevive al guardado en los bits altos y
        // `XRSTOR` da #GP(0) por encender componentes que este CPU no tiene.
        //
        // La firma que lo delato: los dos volcados dieron 0x5F0FCB y 0x37B, y
        // los dos son "el valor viejo con los tres bits bajos puestos a 3" --
        // 3 es justo XINUSE&7 (x87 y SSE en uso, AVX en estado inicial).
        "mov qword ptr [rsp+{bv}], 0",
        // Se ponen a cero LAS MISMAS siete palabras que el epilogo comprueba.
        "mov qword ptr [rsp+{cero}], 0",
        "mov qword ptr [rsp+{cero}+8], 0",
        "mov qword ptr [rsp+{cero}+16], 0",
        "mov qword ptr [rsp+{cero}+24], 0",
        "mov qword ptr [rsp+{cero}+32], 0",
        "mov qword ptr [rsp+{cero}+40], 0",
        "mov qword ptr [rsp+{cero}+48], 0",
        "mov [rsp+{area}], rbp",
        "mov qword ptr [rsp+{firma}], {magia}",
        // RFBM = -1: lo que XCR0 tenga habilitado. rax/rdx ya estan salvados.
        "mov eax, -1", "mov edx, -1",
        "xsave64 [rsp]",
        // La cabecera Y la base, leidas AQUI mismo, sin nadie en medio. Ver
        // `trap::tras_xsave`: es lo que dice si el xsave64 escribio la cabecera
        // donde creemos, o si hay dos direcciones en juego.
        "mov rax, qword ptr [rsp+{bv}]",
        "mov qword ptr [rip+{bv_x}], rax",
        "mov qword ptr [rip+{base_x}], rsp",
        "mov gs:[0x10], rsp",
        "cld",
        "mov rdi, rbp",
        "call {dispatch}",
        // Shared trap epilogue: rax = xsave-base of the context to run.
        "mov rsp, rax",
        "cmp qword ptr [rsp+{firma}], {magia}",
        "jne 3f",
        // La CABECERA, antes de borrar el sello: asi el informe la lee intacta
        // y puede decir de quien era el contexto. rax/rdx se pueden pisar aqui
        // -- los recuperan los pops de abajo.
        "mov rdx, qword ptr [rsp+{bv}]",
        "and rdx, qword ptr [rip+{no_xcr0}]",
        "jnz 8f",
        "mov rax, qword ptr [rsp+{cero}]",
        "or rax, qword ptr [rsp+{cero}+8]",
        "or rax, qword ptr [rsp+{cero}+16]",
        "or rax, qword ptr [rsp+{cero}+24]",
        "or rax, qword ptr [rsp+{cero}+32]",
        "or rax, qword ptr [rsp+{cero}+40]",
        "or rax, qword ptr [rsp+{cero}+48]",
        "jnz 8f",
        // UN SOLO USO: al restaurarlo se borra el sello. Un contexto que ya
        // se consumio no puede volver a pasar por bueno -- si alguien lo
        // intenta, se planta con nombre en vez de reventar en el xrstor.
        "mov qword ptr [rsp+{firma}], 0",
        "mov eax, -1", "mov edx, -1",
        "xrstor64 [rsp]",
        "mov rsp, [rsp+{area}]",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
        "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
        "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
        "cmp qword ptr [rsp+8], 0x08",
        "je 2f",
        "cmp qword ptr [rsp+8], 0x23",
        "jne 4f",
        "swapgs",
        "2: iretq",
        // Los dos "no restaures esto": sello roto (rsp = base del area) y cs
        // imposible (rsp = cola del frame). No vuelven.
        "3: mov rdi, {m_sello}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "4: mov rdi, {m_cs}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "8: mov rdi, {m_cab}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        dispatch = sym timer_dispatch,
        podrido = sym crate::ring0::plat::faults::contexto_podrido,
        no_xcr0 = sym crate::ring0::plat::trap::XSAVE_NO_XCR0,
        bv_x = sym crate::ring0::plat::trap::BV_TRAS_XSAVE,
        base_x = sym crate::ring0::plat::trap::BASE_TRAS_XSAVE,
        area = const crate::ring0::plat::trap::XSAVE_AREA,
        firma = const crate::ring0::plat::trap::SELLO_FIRMA,
        magia = const crate::ring0::plat::trap::SELLO_MAGIA,
        bv = const crate::ring0::plat::trap::XSAVE_BV,
        cero = const crate::ring0::plat::trap::XSAVE_CERO_DESDE,
        m_sello = const crate::ring0::plat::faults::PODRIDO_SELLO,
        m_cs = const crate::ring0::plat::faults::PODRIDO_CS,
        m_cab = const crate::ring0::plat::faults::PODRIDO_CABECERA,
        reserva = const crate::ring0::plat::trap::XSAVE_RESERVA,
    );
}

#[unsafe(naked)]
unsafe extern "C" fn spurious_entry() -> ! {
    // A LAPIC spurious interrupt has no in-service bit and needs no EOI.
    naked_asm!("iretq");
}

#[unsafe(no_mangle)]
extern "C" fn timer_dispatch(_frame: &mut TrapFrame) -> u64 {
    // Donde tallo su area este trap, y para quien. Ver `trap::publicaciones`:
    // es lo que permite ver DOS areas solapadas en la misma pila cuando un
    // contexto aparece pisado con el sello intacto.
    crate::ring0::plat::trap::registrar_publicacion(
        crate::ring0::task::percpu::trap_rsp(),
        crate::ring0::task::scheduler::current_tid(),
    );
    let n = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
    // Budgeted estuary service before the scheduler decision: pending
    // submissions become completions and their WAITers wake this tick.
    // Must run before on_timer so no scheduler lock is held here.
    crate::ring0::obj::channel::service_all();
    crate::ring0::task::scheduler::on_timer();
    // Heartbeat from IRQ context every 64 ticks: paints even when no task
    // ever reaches the shell poll loop. If this row stops updating, ticks
    // stopped -- the hang is running with IF masked (a syscall dispatch or a
    // cli section); if it updates, the counters say where execution lives.
    // NOTA: CABINA se pinta desde el loop del shell (bajo CR3 del kernel,
    // seguro). Pintar desde AQUI (contexto IRQ: switch de CR3 + 4 filas de
    // framebuffer) era pesado y causaba cuelgue->reset en el arranque. El
    // shell ya la mantiene always-on, asi que este llamado sobra.
    let _ = n;

    // SAFETY: initialized before vector 48 is installed and only used by the
    // BSP while SMP is disabled.
    unsafe { LAPIC_EOI.write_volatile(0) };
    percpu::trap_rsp()
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nostack));
    ((high as u64) << 32) | low as u64
}

pub fn init(ctx: &BootContext) -> bool {
    if ctx.idt_ptr == 0 {
        return false;
    }

    let apic = unsafe { rdmsr(MSR_APIC_BASE) };
    if apic & APIC_ENABLED == 0 || apic & APIC_X2_MODE != 0 {
        return false;
    }

    let lapic_base = HIGH_MEM_BASE + (apic & APIC_BASE_MASK);
    let lvt_timer = unsafe {
        ((lapic_base + LAPIC_LVT_TIMER_OFFSET) as *const u32).read_volatile()
    };
    if lvt_timer & LAPIC_LVT_MASKED != 0 || lvt_timer as u8 as usize != TIMER_VECTOR {
        return false;
    }

    let flags: u64;
    unsafe {
        asm!("pushfq", "pop {}", "cli", out(reg) flags);
        LAPIC_EOI = (lapic_base + LAPIC_EOI_OFFSET) as *mut u32;
        let idt = ctx.idt_ptr as *mut IdtEntry;
        idt.add(TIMER_VECTOR).write_volatile(IdtEntry::interrupt_gate(timer_entry as *const () as u64));
        idt.add(SPURIOUS_VECTOR).write_volatile(IdtEntry::interrupt_gate(spurious_entry as *const () as u64));
        if flags & (1 << 9) != 0 {
            asm!("sti", options(nomem, nostack));
        }
    }
    true
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// **Fin de interrupcion.** Lo comparten todos los vectores que lleguen por el
/// LAPIC, y por eso vive aqui: el puntero se resuelve UNA vez al instalar el
/// temporizador, leyendo `IA32_APIC_BASE`. Que cada manejador se lo buscara por
/// su cuenta serian dos caminos que tienen que dar la misma direccion.
///
/// Sin esto, el LAPIC deja el bit en servicio puesto y **no vuelve a entregar
/// ninguna interrupcion de prioridad igual o menor**: la maquina no se cuelga,
/// se queda sorda -- que es peor, porque parece que funciona.
pub fn eoi() {
    unsafe {
        if !LAPIC_EOI.is_null() {
            LAPIC_EOI.write_volatile(0);
        }
    }
}

/// La tabla de interrupciones viva, para que otro vector pueda instalarse.
pub fn instalar_vector(idt_ptr: u64, vector: usize, handler: u64) -> bool {
    if idt_ptr == 0 || vector > 255 {
        return false;
    }
    let flags: u64;
    unsafe {
        asm!("pushfq", "pop {}", "cli", out(reg) flags);
        let idt = idt_ptr as *mut IdtEntry;
        idt.add(vector).write_volatile(IdtEntry::interrupt_gate(handler));
        if flags & (1 << 9) != 0 {
            asm!("sti", options(nomem, nostack));
        }
    }
    true
}

/// Enable maskable interrupts only after every Ring 0 subsystem is ready.
pub fn enable() {
    unsafe { asm!("sti", options(nomem, nostack)); }
}
