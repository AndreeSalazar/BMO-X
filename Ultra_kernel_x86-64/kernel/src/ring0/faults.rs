//! On-screen CPU fault reporter.
//!
//! The faggin `s1_cpu` stage installs exception handlers that print to COM1
//! serial and halt. On a headless machine (no serial cable) a Ring 3 fault
//! therefore freezes the display with no clue why. This module patches the
//! live IDT (`ctx.idt_ptr`, same table `timer::init` patches for vector 48)
//! so the most common faults paint their vector / error code / faulting RIP
//! / CR2 into the dashboard log before halting — making a CPL3 crash visible.
//!
//! The handlers are terminal: they gather state, draw, and `hlt` forever. No
//! register save/restore is needed because control never returns.

use boot_context::BootContext;
use core::arch::naked_asm;

use crate::ring0::dev::console::serial_write;

const KERNEL_CS: u16 = 0x08;

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
    fn trap_gate(handler: u64, ist: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CS,
            ist,
            attributes: 0x8E, // present, DPL0, interrupt gate
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

// One naked stub per vector.
//
// ISOLATING stubs (#UD/#GP/#PF): a fault from CPL3 kills only that task —
// swapgs to the kernel GS (percpu accessors are gs-relative), hand the fault
// to `fault_dispatch`, and if it returns a context (rax != 0) restore it via
// the shared trap epilogue: the kernel LIVES through the crash. A kernel
// fault (or dispatch returning 0) falls through to the terminal halt.
//
// TERMINAL stub (#DF): a double fault means the machine state is already
// beyond rescue — report and halt, as before.
macro_rules! err_stub_isolating {
    ($name:ident, $vec:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() -> ! {
            naked_asm!(
                // CPL3 fault? Load the kernel GS before any percpu access.
                "cmp qword ptr [rsp + 16], 0x08",
                "je 2f",
                "swapgs",
                "2:",
                "mov rdi, {v}",        // vector
                "mov rsi, [rsp]",      // error code (CPU-pushed)
                "mov rdx, [rsp + 8]",  // faulting RIP
                "mov rcx, cr2",        // fault address
                "mov r8, [rsp + 16]",  // faulting CS (kernel vs user decision)
                "mov r9, [rsp + 32]",  // faulting RSP (err: err,rip,cs,rfl,RSP,ss)
                // The CPU fault frame is fully captured in registers; the
                // dying context is never resumed, so the frame itself is
                // dead weight — realign for the SysV call.
                "and rsp, -16",
                "call {h}",            // rax = next context_rsp, 0 = terminal
                "test rax, rax",
                "jz 3f",
                // Shared trap epilogue (same shape as timer/syscall).
                "mov rsp, rax",
                "cmp qword ptr [rsp + {firma}], {magia}",
                "jne 6f",
                // RFBM = -1: lo que XCR0 tenga habilitado. rax/rdx se
                // recuperan de los pops de abajo.
                "mov eax, -1", "mov edx, -1",
                "xrstor64 [rsp]",
                "mov rsp, [rsp + {area}]",
                "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
                "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
                "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
                "cmp qword ptr [rsp + 8], 0x08",
                "je 4f",
                "cmp qword ptr [rsp + 8], 0x23",
                "jne 7f",
                "swapgs",
                "4: iretq",
                "3: cli",
                "5: hlt",
                "jmp 5b",
                "6: mov rdi, {m_sello}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
                "7: mov rdi, {m_cs}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
                v = const $vec,
                h = sym fault_dispatch,
                podrido = sym contexto_podrido,
                area = const crate::ring0::trap::XSAVE_AREA,
                firma = const crate::ring0::trap::SELLO_FIRMA,
                magia = const crate::ring0::trap::SELLO_MAGIA,
                m_sello = const PODRIDO_SELLO,
                m_cs = const PODRIDO_CS,
            );
        }
    };
}

macro_rules! noerr_stub_isolating {
    ($name:ident, $vec:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() -> ! {
            naked_asm!(
                "cmp qword ptr [rsp + 8], 0x08",
                "je 2f",
                "swapgs",
                "2:",
                "mov rdi, {v}",        // vector
                "xor esi, esi",        // no error code
                "mov rdx, [rsp]",      // faulting RIP
                "mov rcx, cr2",        // fault address
                "mov r8, [rsp + 8]",   // faulting CS
                "mov r9, [rsp + 24]",  // faulting RSP (no-err: rip,cs,rfl,RSP,ss)
                "and rsp, -16",
                "call {h}",
                "test rax, rax",
                "jz 3f",
                "mov rsp, rax",
                "cmp qword ptr [rsp + {firma}], {magia}",
                "jne 6f",
                // RFBM = -1: lo que XCR0 tenga habilitado. rax/rdx se
                // recuperan de los pops de abajo.
                "mov eax, -1", "mov edx, -1",
                "xrstor64 [rsp]",
                "mov rsp, [rsp + {area}]",
                "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
                "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
                "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
                "cmp qword ptr [rsp + 8], 0x08",
                "je 4f",
                "cmp qword ptr [rsp + 8], 0x23",
                "jne 7f",
                "swapgs",
                "4: iretq",
                "3: cli",
                "5: hlt",
                "jmp 5b",
                "6: mov rdi, {m_sello}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
                "7: mov rdi, {m_cs}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
                v = const $vec,
                h = sym fault_dispatch,
                podrido = sym contexto_podrido,
                area = const crate::ring0::trap::XSAVE_AREA,
                firma = const crate::ring0::trap::SELLO_FIRMA,
                magia = const crate::ring0::trap::SELLO_MAGIA,
                m_sello = const PODRIDO_SELLO,
                m_cs = const PODRIDO_CS,
            );
        }
    };
}

macro_rules! err_stub_terminal {
    ($name:ident, $vec:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() -> ! {
            naked_asm!(
                "mov rdi, {v}",        // vector
                "mov rsi, [rsp]",      // error code (CPU-pushed)
                "mov rdx, [rsp + 8]",  // faulting RIP
                "mov rcx, cr2",        // fault address
                "mov r8, [rsp + 32]",  // faulting RSP
                "call {h}",
                "cli",
                "2: hlt",
                "jmp 2b",
                v = const $vec,
                h = sym fault_report,
            );
        }
    };
}

noerr_stub_isolating!(stub_ud, 6); // #UD invalid opcode → kill task if CPL3
err_stub_terminal!(stub_df, 8); //    #DF double fault → always terminal
err_stub_isolating!(stub_gp, 13); //  #GP general protection → kill if CPL3
err_stub_isolating!(stub_pf, 14); //  #PF page fault → kill if CPL3

/// Triage: a CPL3 fault kills ONLY the faulting task — revoke its
/// capabilities, mark it Exited, pick the next runnable context and hand it
/// back to the stub's shared epilogue. BMO keeps running. A Ring 0 fault is
/// a kernel bug: full terminal report, return 0 (stub halts).
extern "C" fn fault_dispatch(
    vector: u64,
    error: u64,
    rip: u64,
    cr2: u64,
    cs: u64,
    fault_rsp: u64,
) -> u64 {
    if cs & 3 == 3 {
        let pid = crate::ring0::scheduler::current_pid();
        let tid = crate::ring0::scheduler::current_tid();
        // Capabilities die with the process (same order as EXIT: revoke
        // completes before the final switch — no lock nesting).
        crate::ring0::cap::revoke_all(pid);
        // One red line in the rolling log, painted under the kernel CR3
        // (the user CR3 may not map the framebuffer identity range).
        let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
        let cur = crate::ring0::mm::vmm::read_cr3();
        if kpml4 != 0 && cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(kpml4);
        }
        let mut l = Line::new();
        l.s("*** ring3 fault vec=0x");
        l.hex(vector, 2);
        l.s(" rip=0x");
        l.hex(rip, 10);
        l.s(" tid=");
        l.hex(tid as u64, 2);
        l.s(" - task killed, BMO alive");
        serial_write("[fault] ");
        serial_write(l.as_str());
        serial_write("\n");
        if crate::info::has_fb() {
            crate::ring0::core::phase::dashboard_log(l.as_str());
        }
        // Queda GRABADO en el anillo de CABINA, no solo pintado: el aislamiento
        // de faults sirve precisamente porque la máquina sigue viva después, y
        // entonces alguien puede leer qué mató a la tarea. `record` es seguro
        // aquí (guard de reentrancia, sin locks que puedan colgarse).
        crate::ring0::cabina::fault("ring3", "fault en CPL3: tarea eliminada, BMO sigue vivo", rip);
        let _ = (error, cr2, fault_rsp);
        // schedule() below loads the NEXT task's CR3 itself.
        return crate::ring0::scheduler::kill_current_and_pick();
    }
    fault_report(vector, error, rip, cr2, fault_rsp)
}

/// Small fixed-capacity line builder (no alloc, exception-context safe).
#[derive(Clone, Copy)]
struct Line {
    b: [u8; 80],
    n: usize,
}

impl Line {
    fn new() -> Self {
        Self { b: [0; 80], n: 0 }
    }
    fn s(&mut self, s: &str) {
        for &c in s.as_bytes() {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    fn hex(&mut self, mut v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        let mut tmp = [0u8; 16];
        for i in 0..digits {
            tmp[digits - 1 - i] = H[(v & 0xF) as usize];
            v >>= 4;
        }
        for i in 0..digits {
            if self.n < self.b.len() {
                self.b[self.n] = tmp[i];
                self.n += 1;
            }
        }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.b[..self.n]).unwrap_or("")
    }
}


/// Terminal fault reporter. Draws to the top of the dashboard log (rows that
/// stay visible) so a Ring 3 crash is unmistakable instead of a silent hang.
/// Informe terminal de un fallo de Ring 0: pantalla completa, y reinicia.
///
/// Antes pintaba quince renglones apretados en las filas del panel y se
/// quedaba en `hlt` para siempre. Dos problemas: con la pantalla cedida a
/// Ring 3 el informe quedaba flotando sobre el escritorio de otro, y una
/// maquina congelada obliga a alguien a levantarse a pulsar el boton — o se
/// queda muerta hasta que alguien la encuentre.
extern "C" fn fault_report(vector: u64, error: u64, rip: u64, cr2: u64, fault_rsp: u64) -> ! {
    // Antes de pintar, CR3 del kernel: un fallo tomado bajo un CR3 de usuario
    // no mapea el framebuffer y el primer pixel daria #PF dentro de este mismo
    // manejador — recursion infinita y pantalla congelada en vez de informe.
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if kpml4 != 0 {
        crate::ring0::mm::vmm::switch_to(kpml4);
    }
    let name = match vector {
        6 => "#UD instruccion invalida",
        8 => "#DF doble fallo",
        13 => "#GP violacion de proteccion",
        14 => "#PF fallo de pagina",
        _ => "excepcion desconocida",
    };
    // Ultima entrada de la bitacora antes de detener la maquina.
    crate::ring0::cabina::panic_ev("ring0", name, rip);

    let mut inf = Informe::nuevo();

    let mut l = Line::new();
    l.s("vec=0x"); l.hex(vector, 2);
    l.s("  err=0x"); l.hex(error, 8);
    inf.push(l);

    let mut l = Line::new();
    l.s("rip=0x"); l.hex(rip, 16);
    inf.push(l);

    let mut l = Line::new();
    l.s("cr2=0x"); l.hex(cr2, 16);
    inf.push(l);

    // El RSP de la instruccion que fallo. Para el iretq que entra en CPL3
    // deberia ser la pila alta de la tarea; si es otra cosa, este numero dice
    // donde estaba el CPU de verdad.
    let mut l = Line::new();
    l.s("rsp=0x"); l.hex(fault_rsp, 16);
    inf.push(l);

    // Ultimo cambio a tarea de usuario: el contexto que entrego el
    // planificador, su back-pointer EN ESE INSTANTE, y la misma ranura releida
    // AHORA. b valido + n cero => lo pisaron entre el cambio y el epilogo.
    let snap = crate::ring0::scheduler::switch_snap();
    let live = if snap[0] != 0 {
        unsafe { ((snap[0] + crate::ring0::trap::XSAVE_AREA as u64) as *const u64).read_volatile() }
    } else {
        0
    };
    let mut l = Line::new();
    l.s("sw"); l.hex(snap[3], 2);
    l.s(" c="); l.hex(snap[0], 12);
    l.s(" b="); l.hex(snap[1], 12);
    l.s(" n="); l.hex(live, 12);
    inf.push(l);

    // La ultima escritura del RPC en un frame ajeno. Si el contexto que
    // revento es ese, la ruta culpable es esa; si no, queda descartada.
    let ue = crate::ring0::endpoint::ultima_escritura();
    let mut l = Line::new();
    l.s("rpc t="); l.hex(ue[0], 2);
    l.s(" ctx="); l.hex(ue[1], 12);
    l.s(" gpr="); l.hex(ue[2], 12);
    inf.push(l);

    // GS partido en dos: los MSR contra la direccion del PerCpu que deberian
    // tener. Si difieren, algun camino movio GS despues de init_bsp.
    let (gsb, kgs, pcaddr) = crate::ring0::percpu::gs_diag();
    let mut l = Line::new();
    l.s("gs b="); l.hex(gsb, 12);
    l.s(" k="); l.hex(kgs, 12);
    l.s(" pc="); l.hex(pcaddr, 12);
    inf.push(l);

    let mut l = Line::new();
    l.s("ticks="); l.hex(crate::ring0::timer::ticks(), 8);
    inf.push(l);

    // Si ese RSP cae en un rango plausible, los 5 operandos del iretq que el
    // CPU intento cargar. Basura aqui => el planificador entrego un contexto
    // podrido; coherentes => el problema es el destino.
    let mapped = fault_rsp >= 0xFFFF_8000_0000_0000
        || (fault_rsp >= 0x1000 && fault_rsp < 0x1_0000_0000);
    if mapped {
        let p = fault_rsp as *const u64;
        let (irip, ics, irfl, irsp, iss) = unsafe {
            (
                p.read_volatile(),
                p.add(1).read_volatile(),
                p.add(2).read_volatile(),
                p.add(3).read_volatile(),
                p.add(4).read_volatile(),
            )
        };
        let mut l = Line::new();
        l.s("iq rip="); l.hex(irip, 12);
        l.s(" cs="); l.hex(ics, 4);
        l.s(" ss="); l.hex(iss, 4);
        inf.push(l);
        let mut l = Line::new();
        l.s("iq rsp="); l.hex(irsp, 12);
        l.s(" rfl="); l.hex(irfl, 6);
        inf.push(l);
    }

    pantalla_de_fallo(name, &inf)
}

// ── La pantalla de fallo ────────────────────────────────────────────────

/// Azul de BMO. No es el de Microsoft ni pretende serlo: una pantalla de
/// pánico es una pieza de diseño estándar de cualquier sistema operativo, y
/// ésta lleva la cara de éste. Lo que sí se le copia al mundo entero es la
/// idea buena — **azul, letra grande, y los números que hacen falta**.
const FALLO_FONDO: u32 = 0x0011_3A6E;
const FALLO_TITULO: u32 = 0x00FF_FFFF;
const FALLO_TEXTO: u32 = 0x00C8_DCF0;
const FALLO_DATO: u32 = 0x00FF_D2_5A;
const FALLO_BARRA: u32 = 0x004C_9BE8;

/// Segundos que el informe se queda en pantalla antes de reiniciar.
///
/// Bastante para leerlo y, sobre todo, para **fotografiarlo**: aquí la foto es
/// el depurador. Poco para no dejar la máquina muerta si esto pasa mientras
/// nadie mira.
const FALLO_SEGUNDOS: u64 = 20;

/// Filas del informe, en el orden en que se pintan. `faults.rs` las llena.
struct Informe {
    lineas: [Line; 12],
    n: usize,
}

impl Informe {
    fn nuevo() -> Self {
        Self { lineas: [Line::new(); 12], n: 0 }
    }
    fn push(&mut self, l: Line) {
        if self.n < self.lineas.len() {
            self.lineas[self.n] = l;
            self.n += 1;
        }
        // Todo lo que se pinta va TAMBIÉN por serie, que es lo único que
        // sobrevive a un reinicio automático.
        serial_write("[fault] ");
        serial_write(l.as_str());
        serial_write("\n");
    }
}

/// Pinta el informe a pantalla completa, cuenta atrás, y reinicia.
///
/// ★ Usa `hay_fb_crudo`, no `has_fb`: si un proceso Ring 3 tenía cedida la
/// pantalla, un fallo de kernel **se la quita**. La máquina se está muriendo y
/// esto es lo único que va a quedar.
///
/// ★ Y reinicia en vez de quedarse en `hlt` para siempre. Un kernel congelado
/// obliga a alguien a levantarse y pulsar el botón; peor aún, si pasa mientras
/// nadie mira, la máquina se queda muerta hasta que alguien la encuentre.
fn pantalla_de_fallo(titulo: &str, informe: &Informe) -> ! {
    use crate::ring0::core::splash as sp;

    if !crate::info::hay_fb_crudo() {
        // Sin pantalla no hay nada que pintar, pero el reinicio sigue siendo
        // mejor que el congelado.
        crate::ring0::reinicio::ahora();
    }

    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    sp::fallo_fondo(FALLO_FONDO);

    let x = (w / 12).max(48);
    let mut y = (h / 6).max(60);

    sp::fallo_texto_grande(x, y, "BMO-X se ha detenido", FALLO_TITULO, 2);
    y += sp::ALTO_LINEA * 3;

    sp::fallo_texto(
        x,
        y,
        "Un fallo en Ring 0 no se puede aislar: el kernel es el suelo",
        FALLO_TEXTO,
    );
    y += sp::ALTO_LINEA;
    sp::fallo_texto(x, y, "de todo lo demas. Esto es lo que se sabe:", FALLO_TEXTO);
    y += sp::ALTO_LINEA * 2;

    sp::fallo_texto(x, y, titulo, FALLO_TITULO);
    y += sp::ALTO_LINEA * 2;

    for i in 0..informe.n {
        sp::fallo_texto(x, y, informe.lineas[i].as_str(), FALLO_DATO);
        y += sp::ALTO_LINEA;
    }

    // ── Cuenta atrás ──
    let barra_y = h - h / 8;
    let barra_w = w - x * 2;
    let alto = 10u32;
    sp::fallo_texto(
        x,
        barra_y - sp::ALTO_LINEA - 8,
        "Reiniciando. Si quieres la foto, esta es tu ventana.",
        FALLO_TEXTO,
    );

    let hz = crate::ring0::scheduler::tsc_freq();
    if hz == 0 {
        // Sin TSC calibrado no hay cuenta atrás honesta. Se pinta la barra
        // llena y se reinicia: mentir con una barra que no mide nada sería
        // peor que no tenerla.
        sp::fallo_rect(x, barra_y, barra_w, alto, FALLO_BARRA);
        for _ in 0..80_000_000u64 {
            core::hint::spin_loop();
        }
        crate::ring0::reinicio::ahora();
    }

    let inicio = crate::ring0::scheduler::rdtsc();
    let total = hz * FALLO_SEGUNDOS;
    loop {
        let pasado = crate::ring0::scheduler::rdtsc().wrapping_sub(inicio);
        if pasado >= total {
            break;
        }
        // La barra MENGUA: se ve cuánto queda, no cuánto ha pasado.
        let restante = ((total - pasado) as u128 * barra_w as u128 / total as u128) as u32;
        sp::fallo_rect(x, barra_y, barra_w, alto, FALLO_FONDO);
        sp::fallo_rect(x, barra_y, restante, alto, FALLO_BARRA);
    }
    crate::ring0::reinicio::ahora();
}

/// Motivos con los que un epílogo se niega a restaurar un contexto.
pub const PODRIDO_SELLO: u64 = 1;
pub const PODRIDO_CS: u64 = 2;

/// El epílogo de trap se planta ANTES de restaurar un contexto que no cuadra.
///
/// ## Por qué existe
///
/// Un `iretq` con `cs=0` da `#GP(0)` y el reporte que sale de ahí describe el
/// sitio donde el CPU se enteró, no el sitio donde se rompió: `rip` apunta al
/// propio `iretq`, y el contexto culpable ya no se puede nombrar. Eso es lo que
/// costó una foto y una tarde. Dos comparaciones lo convierten en un informe
/// que dice QUÉ contexto y DE QUIÉN era:
///
/// - `PODRIDO_SELLO`: la firma del área no está. Alguien escribió POR DEBAJO
///   del frame, sobre el área de estado extendido.
/// - `PODRIDO_CS`: el `cs` guardado no es ni 0x08 ni 0x23. Alguien escribió
///   ENCIMA, sobre la cola de cinco palabras que consume el `iretq`.
///
/// `rsp` es la dirección exacta que el epílogo iba a usar: la base del área
/// para el sello, el frame para el `cs`. Es terminal a propósito — restaurar
/// un contexto podrido es exactamente lo que no queremos que pase.
pub extern "C" fn contexto_podrido(motivo: u64, rsp: u64) -> ! {
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if kpml4 != 0 {
        crate::ring0::mm::vmm::switch_to(kpml4);
    }
    let nombre = if motivo == PODRIDO_SELLO {
        "CONTEXTO PODRIDO: el sello no esta"
    } else {
        "CONTEXTO PODRIDO: cs imposible"
    };
    crate::ring0::cabina::panic_ev("ring0", nombre, rsp);

    let mut inf = Informe::nuevo();

    let mut l = Line::new();
    l.s("rsp=0x"); l.hex(rsp, 12);
    l.s("  motivo="); l.hex(motivo, 2);
    inf.push(l);

    // Con el sello, `rsp` YA es la base del area. Con el `cs` hay que llegar a
    // ella: `rsp` es la cola del frame (gpr_base+120), y el back-pointer —el
    // unico puntero que ata frame y area— vive entre 8 y 71 bytes por debajo
    // del bloque de GPR. Se busca ahi el qword que apunta a `gpr_base`.
    let base = if motivo == PODRIDO_SELLO {
        rsp
    } else {
        let gpr_base = rsp.wrapping_sub(120);
        let mut hallada = 0u64;
        let mut off = 8u64;
        while off <= 72 {
            let slot = gpr_base.wrapping_sub(off);
            if unsafe { (slot as *const u64).read_volatile() } == gpr_base {
                hallada = slot.wrapping_sub(crate::ring0::trap::XSAVE_AREA as u64);
                break;
            }
            off += 8;
        }
        hallada
    };
    let (firma, dueno) = crate::ring0::trap::leer_sello(base);
    let mut l = Line::new();
    l.s("sello=0x"); l.hex(firma, 8);
    l.s("  dueno=tid "); l.hex(dueno, 4);
    l.s("  area="); l.hex(base, 12);
    inf.push(l);

    // Las cinco palabras que el iretq iba a consumir. Si aqui sale 0x37F y
    // 0x1F80, lo que hay encima del contexto es la imagen XSAVE de OTRO:
    // alguien trapeo sobre esta pila.
    let p = rsp as *const u64;
    let (w0, w1, w2, w3, w4) = unsafe {
        (
            p.read_volatile(),
            p.add(1).read_volatile(),
            p.add(2).read_volatile(),
            p.add(3).read_volatile(),
            p.add(4).read_volatile(),
        )
    };
    let mut l = Line::new();
    l.s("w0="); l.hex(w0, 12);
    l.s(" w1="); l.hex(w1, 4);
    l.s(" w2="); l.hex(w2, 6);
    inf.push(l);
    let mut l = Line::new();
    l.s("w3="); l.hex(w3, 12);
    l.s(" w4="); l.hex(w4, 4);
    inf.push(l);

    let snap = crate::ring0::scheduler::switch_snap();
    let mut l = Line::new();
    l.s("sw"); l.hex(snap[3], 2);
    l.s(" c="); l.hex(snap[0], 12);
    l.s(" b="); l.hex(snap[1], 12);
    inf.push(l);

    let ue = crate::ring0::endpoint::ultima_escritura();
    let mut l = Line::new();
    l.s("rpc t="); l.hex(ue[0], 2);
    l.s(" ctx="); l.hex(ue[1], 12);
    l.s(" gpr="); l.hex(ue[2], 12);
    inf.push(l);

    pantalla_de_fallo(nombre, &inf)
}

/// Patch the live IDT so #UD/#DF/#GP/#PF report on screen. Uses IST1 (set up
/// by the faggin TSS) so a fault on a bad stack still lands somewhere sane.
pub fn init(ctx: &BootContext) {
    if ctx.idt_ptr == 0 {
        return;
    }
    let idt = ctx.idt_ptr as *mut IdtEntry;
    unsafe {
        idt.add(6)
            .write_volatile(IdtEntry::trap_gate(stub_ud as *const () as u64, 1));
        idt.add(8)
            .write_volatile(IdtEntry::trap_gate(stub_df as *const () as u64, 1));
        idt.add(13)
            .write_volatile(IdtEntry::trap_gate(stub_gp as *const () as u64, 1));
        idt.add(14)
            .write_volatile(IdtEntry::trap_gate(stub_pf as *const () as u64, 1));
    }
    serial_write("[fault] on-screen exception reporter armed (#UD/#DF/#GP/#PF)\n");
}
