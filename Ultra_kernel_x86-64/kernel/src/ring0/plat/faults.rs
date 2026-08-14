//! On-screen CPU fault reporter.
//!
//! The faggin `s1_cpu` stage installs exception handlers that print to COM1
//! serial and halt. On a headless machine (no serial cable) a Ring 3 fault
//! therefore freezes the display with no clue why. This module patches the
//! live IDT (`ctx.idt_ptr`, same table `timer::init` patches for vector 48)
//! so the most common faults paint their vector / error code / faulting RIP
//! / CR2 into the dashboard log before halting -- making a CPL3 crash visible.
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
// ISOLATING stubs (#UD/#GP/#PF): a fault from CPL3 kills only that task --
// swapgs to the kernel GS (percpu accessors are gs-relative), hand the fault
// to `fault_dispatch`, and if it returns a context (rax != 0) restore it via
// the shared trap epilogue: the kernel LIVES through the crash. A kernel
// fault (or dispatch returning 0) falls through to the terminal halt.
//
// TERMINAL stub (#DF): a double fault means the machine state is already
// beyond rescue -- report and halt, as before.
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
                // dead weight -- realign for the SysV call.
                "and rsp, -16",
                "call {h}",            // rax = next context_rsp, 0 = terminal
                "test rax, rax",
                "jz 3f",
                // Shared trap epilogue (same shape as timer/syscall).
                "mov rsp, rax",
                "cmp qword ptr [rsp + {firma}], {magia}",
                "jne 6f",
                // La cabecera XSAVE: ver el epilogo del timer.
                "mov rdx, qword ptr [rsp + {bv}]",
                "and rdx, qword ptr [rip + {no_xcr0}]",
                "jnz 8f",
                "mov rax, qword ptr [rsp + {cero}]",
                "or rax, qword ptr [rsp + {cero} + 8]",
                "or rax, qword ptr [rsp + {cero} + 16]",
                "or rax, qword ptr [rsp + {cero} + 24]",
                "or rax, qword ptr [rsp + {cero} + 32]",
                "or rax, qword ptr [rsp + {cero} + 40]",
                "or rax, qword ptr [rsp + {cero} + 48]",
                "jnz 8f",
                // Un solo uso: ver el epilogo del timer.
                "mov qword ptr [rsp + {firma}], 0",
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
                "8: mov rdi, {m_cab}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
                v = const $vec,
                h = sym fault_dispatch,
                podrido = sym contexto_podrido,
                no_xcr0 = sym crate::ring0::plat::trap::XSAVE_NO_XCR0,
                area = const crate::ring0::plat::trap::XSAVE_AREA,
                firma = const crate::ring0::plat::trap::SELLO_FIRMA,
                magia = const crate::ring0::plat::trap::SELLO_MAGIA,
                bv = const crate::ring0::plat::trap::XSAVE_BV,
                cero = const crate::ring0::plat::trap::XSAVE_CERO_DESDE,
                m_sello = const PODRIDO_SELLO,
                m_cs = const PODRIDO_CS,
                m_cab = const PODRIDO_CABECERA,
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
                // La cabecera XSAVE: ver el epilogo del timer.
                "mov rdx, qword ptr [rsp + {bv}]",
                "and rdx, qword ptr [rip + {no_xcr0}]",
                "jnz 8f",
                "mov rax, qword ptr [rsp + {cero}]",
                "or rax, qword ptr [rsp + {cero} + 8]",
                "or rax, qword ptr [rsp + {cero} + 16]",
                "or rax, qword ptr [rsp + {cero} + 24]",
                "or rax, qword ptr [rsp + {cero} + 32]",
                "or rax, qword ptr [rsp + {cero} + 40]",
                "or rax, qword ptr [rsp + {cero} + 48]",
                "jnz 8f",
                // Un solo uso: ver el epilogo del timer.
                "mov qword ptr [rsp + {firma}], 0",
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
                "8: mov rdi, {m_cab}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
                v = const $vec,
                h = sym fault_dispatch,
                podrido = sym contexto_podrido,
                no_xcr0 = sym crate::ring0::plat::trap::XSAVE_NO_XCR0,
                area = const crate::ring0::plat::trap::XSAVE_AREA,
                firma = const crate::ring0::plat::trap::SELLO_FIRMA,
                magia = const crate::ring0::plat::trap::SELLO_MAGIA,
                bv = const crate::ring0::plat::trap::XSAVE_BV,
                cero = const crate::ring0::plat::trap::XSAVE_CERO_DESDE,
                m_sello = const PODRIDO_SELLO,
                m_cs = const PODRIDO_CS,
                m_cab = const PODRIDO_CABECERA,
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

noerr_stub_isolating!(stub_ud, 6); // #UD invalid opcode -> kill task if CPL3
err_stub_terminal!(stub_df, 8); //    #DF double fault -> always terminal
err_stub_isolating!(stub_gp, 13); //  #GP general protection -> kill if CPL3
err_stub_isolating!(stub_pf, 14); //  #PF page fault -> kill if CPL3

/// Triage: a CPL3 fault kills ONLY the faulting task -- revoke its
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
        let pid = crate::ring0::task::scheduler::current_pid();
        let tid = crate::ring0::task::scheduler::current_tid();
        // ** LO PRIMERO DE TODO: sacarle los datos al proceso MIENTRAS SU
        // ESPACIO DE DIRECCIONES SIGUE PUESTO.
        //
        // Debajo se cambia a la tabla del kernel para poder pintar, y hace
        // falta. Pero la imagen del proceso (1 GiB) y su pila (2 GiB) viven en
        // el PDPT[1], que `new_address_space` reserva POR PROCESO: de las
        // tablas solo se comparte el PDPT[0]. Bajo el CR3 del kernel esas
        // direcciones son de otro o de nadie.
        //
        // Hasta hoy la autopsia leia la pila DESPUES del cambio, asi que sus
        // cuatro palabras no se sabe de donde salieron -- y se razono sobre
        // ellas. Un dato de origen desconocido es peor que un hueco.
        //
        // Aqui se leen los bytes crudos y ya esta: el formateo, el veredicto y
        // todo lo demas pasan luego y no vuelven a tocar memoria de nadie.
        let cap = crate::ring0::core::autopsy::Captura::tomar(rip, fault_rsp);
        // Capabilities die with the process (same order as EXIT: revoke
        // completes before the final switch -- no lock nesting).
        crate::ring0::obj::cap::revoke_all(pid);
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
            crate::ring0::core::dashboard::dashboard_log(l.as_str());
        }
        // Queda GRABADO en el anillo de CABINA, no solo pintado: el aislamiento
        // de faults sirve precisamente porque la maquina sigue viva despues, y
        // entonces alguien puede leer que mato a la tarea. `record` es seguro
        // aqui (guard de reentrancia, sin locks que puedan colgarse).
        crate::ring0::cabina::fault("ring3", "fault en CPL3: tarea eliminada, BMO sigue vivo", rip);
        // ** Y EL VEREDICTO EN LA PANTALLA, no solo dentro de la autopsia.
        //
        // La linea de arriba es la que se ve sin pedir nada, y hasta hoy era un
        // vector y un `rip`. El 2026-08-14 eso costo una tarde: el compositor
        // murio con `#PF` en `rip=0x4000001B` --que es la sonda de pila-- y la
        // pantalla no dijo que se hubiera desbordado la pila, aunque el kernel
        // tenia `cr2` y sabia donde acaba la pila.
        //
        // Va en su PROPIA linea y no pegada a la anterior: `Line` son 80 bytes
        // y la primera ya gasta 73. Anadirlo al final la habria cortado justo
        // por donde esta lo nuevo, que es la peor forma de no decir nada.
        {
            let mut v = Line::new();
            v.s("    ");
            v.s(crate::ring0::core::autopsy::veredicto_corto(vector, error, cr2, &cap));
            serial_write("[fault] ");
            serial_write(v.as_str());
            serial_write("\n");
            if crate::info::has_fb() {
                crate::ring0::core::dashboard::dashboard_log(v.as_str());
            }
        }
        // ** Y LA AUTOPSIA ENTERA, no una linea.
        //
        // La linea de arriba lleva el `rip` y nada mas: sirve para saber QUE
        // paso y no para saber DONDE. El informe completo --vector, codigo de
        // error, la direccion que se toco, la pila, QUE PROGRAMA era y lo
        // ultimo que llego a escribir-- se guarda aqui, en RAM, para que Ring 3
        // lo lea cuando quiera y lo escriba a un fichero.
        //
        // Se hace DESPUES de CABINA a proposito: si esto se colgara --no puede,
        // pero el orden es una decision-- la linea roja ya estaria puesta.
        crate::ring0::core::autopsy::registrar(vector, error, rip, cr2, fault_rsp, pid, tid, &cap);
        let _ = (error, cr2, fault_rsp);
        // schedule() below loads the NEXT task's CR3 itself.
        return crate::ring0::task::scheduler::kill_current_and_pick();
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
/// maquina congelada obliga a alguien a levantarse a pulsar el boton -- o se
/// queda muerta hasta que alguien la encuentre.
extern "C" fn fault_report(vector: u64, error: u64, rip: u64, cr2: u64, fault_rsp: u64) -> ! {
    // Antes de pintar, CR3 del kernel: un fallo tomado bajo un CR3 de usuario
    // no mapea el framebuffer y el primer pixel daria #PF dentro de este mismo
    // manejador -- recursion infinita y pantalla congelada en vez de informe.
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if kpml4 != 0 {
        crate::ring0::mm::vmm::switch_to(kpml4);
    }
    // -- En INGLES, y solo ASCII --
    //
    // No es una preferencia de estilo. Esta pantalla se lee EN UNA FOTO, y su
    // trabajo entero es que un caracter no se confunda con otro. El espanol
    // mete tildes y enye, y esos glifos viven en la tabla de extras Latin-1 del
    // font: son los que peor se distinguen a 8 px y los primeros que se rompen
    // si algo va mal con la fuente. El ingles cabe en ASCII puro.
    //
    // Ademas los campos que van debajo (vec, err, rip, cr2, rsp) ya son ingles
    // por ser los nombres del hardware, asi que la pantalla deja de estar a
    // medias en dos idiomas.
    //
    // El resto del sistema sigue en espanol: comentarios, CABINA, el shell. Lo
    // que cambia es SOLO lo que se fotografia.
    let name = match vector {
        6 => "#UD invalid opcode",
        8 => "#DF double fault",
        13 => "#GP general protection",
        14 => "#PF page fault",
        _ => "unknown exception",
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
    let snap = crate::ring0::task::scheduler::switch_snap();
    let live = if snap[0] != 0 {
        unsafe { ((snap[0] + crate::ring0::plat::trap::XSAVE_AREA as u64) as *const u64).read_volatile() }
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
    let ue = crate::ring0::obj::endpoint::last_write();
    let mut l = Line::new();
    l.s("rpc t="); l.hex(ue[0], 2);
    l.s(" ctx="); l.hex(ue[1], 12);
    l.s(" gpr="); l.hex(ue[2], 12);
    inf.push(l);

    // GS partido en dos: los MSR contra la direccion del PerCpu que deberian
    // tener. Si difieren, algun camino movio GS despues de init_bsp.
    let (gsb, kgs, pcaddr) = crate::ring0::task::percpu::gs_diag();
    let mut l = Line::new();
    l.s("gs b="); l.hex(gsb, 12);
    l.s(" k="); l.hex(kgs, 12);
    l.s(" pc="); l.hex(pcaddr, 12);
    inf.push(l);

    let mut l = Line::new();
    l.s("ticks="); l.hex(crate::ring0::plat::timer::ticks(), 8);
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

// -- La pantalla de fallo ------------------------------------------------

/// Azul de BMO. No es el de Microsoft ni pretende serlo: una pantalla de
/// panico es una pieza de diseno estandar de cualquier sistema operativo, y
/// esta lleva la cara de este. Lo que si se le copia al mundo entero es la
/// idea buena -- **azul, letra grande, y los numeros que hacen falta**.
const FALLO_FONDO: u32 = 0x0011_3A6E;
const FALLO_TITULO: u32 = 0x00FF_FFFF;
const FALLO_TEXTO: u32 = 0x00C8_DCF0;
const FALLO_DATO: u32 = 0x00FF_D2_5A;
const FALLO_BARRA: u32 = 0x004C_9BE8;

/// Segundos que el informe se queda en pantalla antes de reiniciar.
///
/// Bastante para leerlo y, sobre todo, para **fotografiarlo**: aqui la foto es
/// el depurador. Poco para no dejar la maquina muerta si esto pasa mientras
/// nadie mira.
const FALLO_SEGUNDOS: u64 = 20;

/// Filas del informe, en el orden en que se pintan. `faults.rs` las llena.
struct Informe {
    /// * 16 y no 12. Los dos informes llegaron a llenar las doce EXACTAS, y
    /// `push` descarta en silencio a partir del tope: la siguiente fila que
    /// alguien anadiera se perderia sin un solo aviso, justo en la herramienta
    /// que usamos para depurar cuando no hay otra. Un margen de cuatro cuesta
    /// 352 bytes de una pila que ya no va a servir para nada mas.
    lineas: [Line; 16],
    n: usize,
}

impl Informe {
    fn nuevo() -> Self {
        Self { lineas: [Line::new(); 16], n: 0 }
    }
    fn push(&mut self, l: Line) {
        if self.n < self.lineas.len() {
            self.lineas[self.n] = l;
            self.n += 1;
        }
        // Todo lo que se pinta va TAMBIEN por serie, que es lo unico que
        // sobrevive a un reinicio automatico.
        serial_write("[fault] ");
        serial_write(l.as_str());
        serial_write("\n");
    }
}

/// Pinta el informe a pantalla completa, cuenta atras, y reinicia.
///
/// * Usa `hay_fb_crudo`, no `has_fb`: si un proceso Ring 3 tenia cedida la
/// pantalla, un fallo de kernel **se la quita**. La maquina se esta muriendo y
/// esto es lo unico que va a quedar.
///
/// * Y reinicia en vez de quedarse en `hlt` para siempre. Un kernel congelado
/// obliga a alguien a levantarse y pulsar el boton; peor aun, si pasa mientras
/// nadie mira, la maquina se queda muerta hasta que alguien la encuentre.
fn pantalla_de_fallo(titulo: &str, informe: &Informe) -> ! {
    use crate::ring0::core::splash as sp;

    if !crate::info::hay_fb_crudo() {
        // Sin pantalla no hay nada que pintar, pero el reinicio sigue siendo
        // mejor que el congelado.
        crate::ring0::plat::reinicio::ahora();
    }

    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    sp::fallo_fondo(FALLO_FONDO);

    let x = (w / 12).max(48);
    let mut y = (h / 6).max(60);

    sp::fallo_texto_grande(x, y, "BMO-X has stopped", FALLO_TITULO, 2);
    y += sp::ALTO_LINEA * 3;

    sp::fallo_texto(
        x,
        y,
        "A Ring 0 fault cannot be isolated: the kernel is the floor",
        FALLO_TEXTO,
    );
    y += sp::ALTO_LINEA;
    sp::fallo_texto(x, y, "everything else stands on. This is what is known:", FALLO_TEXTO);
    y += sp::ALTO_LINEA * 2;

    sp::fallo_texto(x, y, titulo, FALLO_TITULO);
    y += sp::ALTO_LINEA * 2;

    for i in 0..informe.n {
        sp::fallo_texto(x, y, informe.lineas[i].as_str(), FALLO_DATO);
        y += sp::ALTO_LINEA;
    }

    // -- Cuenta atras --
    let barra_y = h - h / 8;
    let barra_w = w - x * 2;
    let alto = 10u32;
    sp::fallo_texto(
        x,
        barra_y - sp::ALTO_LINEA - 8,
        "Rebooting. If you want the photo, this is your window.",
        FALLO_TEXTO,
    );

    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        // Sin TSC calibrado no hay cuenta atras honesta. Se pinta la barra
        // llena y se reinicia: mentir con una barra que no mide nada seria
        // peor que no tenerla.
        sp::fallo_rect(x, barra_y, barra_w, alto, FALLO_BARRA);
        for _ in 0..80_000_000u64 {
            core::hint::spin_loop();
        }
        crate::ring0::plat::reinicio::ahora();
    }

    let inicio = crate::ring0::task::scheduler::rdtsc();
    let total = hz * FALLO_SEGUNDOS;
    // La barra llena, UNA vez. A partir de aqui solo se borra lo que mengua.
    sp::fallo_rect(x, barra_y, barra_w, alto, FALLO_BARRA);
    let mut anterior = barra_w;
    loop {
        let pasado = crate::ring0::task::scheduler::rdtsc().wrapping_sub(inicio);
        if pasado >= total {
            break;
        }
        // La barra MENGUA: se ve cuanto queda, no cuanto ha pasado.
        let restante = ((total - pasado) as u128 * barra_w as u128 / total as u128) as u32;
        // * Repintar por DANO, no la barra entera.
        //
        // Antes este bucle borraba y redibujaba los ~1200 px de la barra en
        // CADA vuelta, tan rapido como el CPU pudiera: decenas de miles de
        // pasadas por segundo sobre memoria de video sin cache y sin ninguna
        // sincronizacion con el refresco del panel. Lo que se ve entonces no es
        // que el framebuffer sea debil: es que el panel captura la barra a
        // medio reescribir, y muestra una banda de la pasada anterior mezclada
        // con la nueva. Un LCD refresca 60 veces por segundo; escribirle 40.000
        // no lo hace ir mas rapido, lo hace ensenar basura.
        //
        // Ahora solo se borra la tira que acaba de desaparecer, y solo cuando
        // el ancho cambia de pixel entero. Es el mismo principio que el cursor
        // del compositor --repintar el dano, no la escena-- y aqui se nota mas
        // porque no hay nada mas en pantalla que lo disimule.
        if restante < anterior {
            sp::fallo_rect(x + restante, barra_y, anterior - restante, alto, FALLO_FONDO);
            anterior = restante;
        }
    }
    crate::ring0::plat::reinicio::ahora();
}

/// Motivos con los que un epilogo se niega a restaurar un contexto.
pub const PODRIDO_SELLO: u64 = 1;
pub const PODRIDO_CS: u64 = 2;
pub const PODRIDO_CABECERA: u64 = 3;

/// El epilogo de trap se planta ANTES de restaurar un contexto que no cuadra.
///
/// ## Por que existe
///
/// Un `iretq` con `cs=0` da `#GP(0)` y el reporte que sale de ahi describe el
/// sitio donde el CPU se entero, no el sitio donde se rompio: `rip` apunta al
/// propio `iretq`, y el contexto culpable ya no se puede nombrar. Eso es lo que
/// costo una foto y una tarde. Dos comparaciones lo convierten en un informe
/// que dice QUE contexto y DE QUIEN era:
///
/// - `PODRIDO_SELLO`: la firma del area no esta. Alguien escribio POR DEBAJO
///   del frame, sobre el area de estado extendido.
/// - `PODRIDO_CS`: el `cs` guardado no es ni 0x08 ni 0x23. Alguien escribio
///   ENCIMA, sobre la cola de cinco palabras que consume el `iretq`.
/// - `PODRIDO_CABECERA`: la cabecera XSAVE (`+512`) no es una cabecera. Alguien
///   escribio EN MEDIO -- entre el area de registros y el sello.
///
/// El tercero cerro el hueco que dejaban los otros dos. El sello vigila el
/// final del area y el back-pointer el borde de arriba: los dos EXTREMOS. Un
/// `#GP(0)` en el propio `xrstor64`, con el sello intacto, describia el sitio
/// donde el CPU se entero y no el sitio donde se rompio -- exactamente el mismo
/// problema que el `iretq` con `cs=0` antes de que existiera esta funcion.
///
/// `rsp` es la direccion exacta que el epilogo iba a usar: la base del area
/// para el sello y para la cabecera, el frame para el `cs`. Es terminal a
/// proposito -- restaurar un contexto podrido es exactamente lo que no queremos
/// que pase.
pub extern "C" fn contexto_podrido(motivo: u64, rsp: u64) -> ! {
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if kpml4 != 0 {
        crate::ring0::mm::vmm::switch_to(kpml4);
    }
    let name = match motivo {
        PODRIDO_SELLO => "ROTTEN CONTEXT: the seal is gone",
        PODRIDO_CABECERA => "ROTTEN CONTEXT: XSAVE header",
        _ => "ROTTEN CONTEXT: impossible cs",
    };
    crate::ring0::cabina::panic_ev("ring0", name, rsp);

    let mut inf = Informe::nuevo();

    let mut l = Line::new();
    l.s("rsp=0x"); l.hex(rsp, 12);
    l.s("  motivo="); l.hex(motivo, 2);
    inf.push(l);

    // Con el sello, `rsp` YA es la base del area. Con el `cs` hay que llegar a
    // ella: `rsp` es la cola del frame (gpr_base+120), y el back-pointer --el
    // unico puntero que ata frame y area-- vive entre 8 y 71 bytes por debajo
    // del bloque de GPR. Se busca ahi el qword que apunta a `gpr_base`.
    // Con el sello y con la cabecera, `rsp` YA es la base del area -- las dos
    // guardias se disparan antes de que el epilogo salte al frame. Solo el `cs`
    // se comprueba despues, y por eso solo ese caso tiene que buscarla.
    let base = if motivo != PODRIDO_CS {
        rsp
    } else {
        let gpr_base = rsp.wrapping_sub(120);
        let mut hallada = 0u64;
        let mut off = 8u64;
        while off <= 72 {
            let slot = gpr_base.wrapping_sub(off);
            if unsafe { (slot as *const u64).read_volatile() } == gpr_base {
                hallada = slot.wrapping_sub(crate::ring0::plat::trap::XSAVE_AREA as u64);
                break;
            }
            off += 8;
        }
        hallada
    };
    let (firma, owner) = crate::ring0::plat::trap::leer_sello(base);
    let mut l = Line::new();
    l.s("sello=0x"); l.hex(firma, 8);
    l.s("  dueno=tid "); l.hex(owner, 4);
    l.s("  area="); l.hex(base, 12);
    inf.push(l);

    // La cabecera XSAVE, la parte del area que nadie miraba. Se pinta en los
    // tres motivos: cuando la guardia de cabecera es la que salta dice QUE
    // campo esta mal, y en los otros dos dice si ademas estaba tocada.
    //
    // `bv` contra `noxcr0`: si su AND no es cero, la imagen dice traer un
    // componente que este CPU no tiene habilitado. `cmp` y `rsv` valen cero
    // siempre --los escribe `xsave64` en cada guardado-- asi que cualquier otra
    // cosa ahi es corrupcion, sin interpretacion posible.
    if base != 0 {
        use crate::ring0::plat::trap as t;
        let (bv, cmpbv) = unsafe {
            (
                ((base + t::XSAVE_BV as u64) as *const u64).read_volatile(),
                ((base + t::XSAVE_CERO_DESDE as u64) as *const u64).read_volatile(),
            )
        };
        let mut rsv = 0u64;
        for i in 1..t::XSAVE_CERO_PALABRAS as u64 {
            rsv |= unsafe {
                ((base + t::XSAVE_CERO_DESDE as u64 + i * 8) as *const u64).read_volatile()
            };
        }
        let noxcr0 = unsafe { core::ptr::addr_of!(t::XSAVE_NO_XCR0).read_volatile() };
        let mut l = Line::new();
        l.s("cab bv="); l.hex(bv, 12);
        l.s(" cmp="); l.hex(cmpbv, 8);
        inf.push(l);
        let mut l = Line::new();
        l.s("rsv="); l.hex(rsv, 12);
        l.s("  noxcr0="); l.hex(noxcr0, 12);
        inf.push(l);
        // `bv0` es el MISMO campo leido al entrar en el despachador. Si bv0 ya
        // esta podrido, el culpable esta entre el xsave64 y el call; si bv0
        // esta sano, esta dentro del despachador. Una linea que parte el codigo
        // sospechoso por la mitad.
        let mut l = Line::new();
        l.s("bv0="); l.hex(t::cabecera_al_entrar(), 12);
        l.s("  (dispatch)");
        inf.push(l);
        // Y la version SIN indirecciones: leida por el stub justo despues del
        // xsave64, del rsp que la propia instruccion uso.
        let (bvx, basex) = t::tras_xsave();
        let mut l = Line::new();
        l.s("bvX="); l.hex(bvx, 12);
        l.s(" baseX="); l.hex(basex, 12);
        inf.push(l);
    }

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

    let snap = crate::ring0::task::scheduler::switch_snap();
    let mut l = Line::new();
    l.s("sw"); l.hex(snap[3], 2);
    l.s(" c="); l.hex(snap[0], 12);
    l.s(" b="); l.hex(snap[1], 12);
    inf.push(l);

    // Las ultimas areas talladas, de la mas reciente hacia atras, con su tarea.
    //
    // Aqui esta la respuesta a "quien escribio encima". Si dos de estas bases
    // distan menos de XSAVE_AREA y estan en la misma pila, se solapan -- y el
    // tid de cada una dice de quien es cada trozo. Con el sello intacto, como
    // en la foto del 27, el vandalo tiene que estar en esta lista.
    let pubs = crate::ring0::plat::trap::publicaciones();
    let mut l = Line::new();
    l.s("pub0="); l.hex(pubs[0].0, 12);
    l.s(" t"); l.hex(pubs[0].1 as u64, 2);
    l.s("  pub1="); l.hex(pubs[1].0, 12);
    l.s(" t"); l.hex(pubs[1].1 as u64, 2);
    inf.push(l);
    let mut l = Line::new();
    l.s("pub2="); l.hex(pubs[2].0, 12);
    l.s(" t"); l.hex(pubs[2].1 as u64, 2);
    l.s("  pub3="); l.hex(pubs[3].0, 12);
    l.s(" t"); l.hex(pubs[3].1 as u64, 2);
    inf.push(l);

    let ue = crate::ring0::obj::endpoint::last_write();
    let mut l = Line::new();
    l.s("rpc t="); l.hex(ue[0], 2);
    l.s(" ctx="); l.hex(ue[1], 12);
    l.s(" gpr="); l.hex(ue[2], 12);
    inf.push(l);

    pantalla_de_fallo(name, &inf)
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
