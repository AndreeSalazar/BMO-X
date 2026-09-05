//! **CARRIL ROJO** -- si esto falla no hay pantalla que contarlo.
//!
//! [carril]  ROJO      el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//!
//! [cuesta]  MAQUINA -- aqui viven la IDT, los stubs de excepcion y el
//!           reparto. Un descriptor mal puesto no da un fallo: da un TRIPLE
//!           FAULT, que es la maquina reiniciando sin decir una palabra.
//!
//! [riesgo]  AJENO -- lo que llega aqui lo escribe el PROCESADOR en la pila, y
//!           en el peor caso lo escribio encima un contexto podrido. Por eso
//!           existe `contexto_podrido`.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! ** La diferencia con el carril amarillo de al lado, en una frase: **si esto
//! se equivoca, no hay nadie que lo cuente**. Si se equivoca el informe, hay
//! una pantalla azul que dice algo falso -- caro, pero recuperable.

use boot_context::BootContext;
use core::arch::naked_asm;
use crate::ring0::dev::console::serial_write;
use super::amarilla::{fault_report, pantalla_de_fallo};
use super::verde::{Informe, Line};

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
        // ** La frase se acorto el 2026-08-25 y el motivo es aritmetica, no
        // estilo: con la anterior --45 columnas-- el `rip` no cabia en la fila
        // de 80 del panel y salio `=4001`, o sea cinco digitos de una direccion
        // de diez. El unico dato de esta linea, cortado por la mitad. Ver
        // `cabina/cockpit.rs`, donde el ancho ya se reparte con el numero
        // primero, y `mm/vmm.rs`, que tenia el mismo problema el mismo dia.
        crate::ring0::cabina::fault("ring3", "CPL3: tarea eliminada, BMO sigue vivo", rip);
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
        // == *** Y LOS DOS NUMEROS QUE LO DECIDEN TODO (2026-09-04) =========
        //
        // El veredicto de arriba se calcula CON `cr2` y `error`... y despues los
        // tira: `let _ = (error, cr2, ...)`. O sea que la pantalla daba la
        // conclusion y se quedaba con la prueba.
        //
        // ** Y eso costo un dia entero. El 05-09 el mismo `rip` salio dos veces
        // --`0x004000725C`-- y al desmontar el `.bex` en ese offset habia
        // `0F 9C C0`, que es `setl al`: **una instruccion que no toca memoria y
        // no puede dar un #PF**. Sin `cr2` no habia forma de saber si el `rip`
        // apuntaba a la instruccion que fallo o al sitio al que intento SALTAR.
        //
        // En un fallo de pagina, `cr2` no es un dato mas: **es la respuesta**.
        // Es la direccion que no estaba. Y el bit 4 del codigo de error dice si
        // el CPU la queria para EJECUTAR, que es justo lo que distingue las dos
        // lecturas del mismo `rip`.
        //
        // Los bits van en PALABRAS y no en hexadecimal, por la leccion de la
        // estacion 11: un numero en una foto tomada con una camara no lo
        // descifra nadie a las once de la noche.
        {
            let mut n = Line::new();
            n.s("    cr2=0x");
            n.hex(cr2, 10);
            n.s(" err=0x");
            n.hex(error, 2);
            n.s(" ");
            if error & 0x10 != 0 {
                // *** Si esto sale, el `rip` NO es donde fallo: es adonde iba.
                n.s("FETCH");
            } else if error & 0x02 != 0 {
                n.s("ESCRIB");
            } else {
                n.s("LEE");
            }
            n.s(if error & 0x01 != 0 { " protegida" } else { " NO-presente" });
            if cr2 == rip {
                // Saltar a una direccion que no existe y fallar leyendo la
                // instruccion se ven igual en el `rip`. Esto los separa.
                n.s(" (cr2 == rip)");
            }
            // == *** LA PANTALLA QUE YA NO ES SUYA ==========================
            //
            // El 05-09 esta linea recien puesta contesto a la primera:
            //
            //     cr2=0x00D015C278 err=0x06 ESCRIB NO-presente
            //
            // `FRAMEBUFFER_VA_BASE` es `0xD000_0000`, asi que eso es el
            // framebuffer + 1.426.552 bytes -- **la fila 185 de la pantalla**.
            // DOOM murio a mitad de un volcado, escribiendo donde ya no habia
            // nada, porque el dueno le habia quitado la pantalla con
            // `Ctrl+Alt+Esc`.
            //
            // ** Y eso NO es un fallo de nadie: es el rescate haciendo su
            // trabajo. Quitar la pantalla es DESMAPEARLA, y un programa que
            // sigue pintando se lleva un `#PF`. Muere feo, pero muere -- y
            // desde el 05-09 el DIRECTOR lo iba a cerrar un milisegundo
            // despues de todas formas.
            //
            // Lo que se arregla aqui no es la muerte: es que **el renglon rojo
            // no lo explicaba**. Una direccion en hexadecimal manda a leer el
            // kernel entero; esta frase se lee y se acabo. Es la leccion de la
            // estacion 11, otra vez, en el sitio donde mas asusta.
            let fb0 = crate::ring0::mm::vmm::FRAMEBUFFER_VA_BASE;
            if cr2 >= fb0 && cr2 < fb0 + 64 * 1024 * 1024 {
                let mut f = Line::new();
                f.s("    ESCRIBIA EN LA PANTALLA QUE YA NO ES SUYA");
                let ancho = unsafe { crate::info::FB_WIDTH } as u64;
                if ancho != 0 {
                    f.s(" -- fila ");
                    f.dec((cr2 - fb0) / (ancho * 4));
                }
                serial_write("[fault] ");
                serial_write(f.as_str());
                serial_write("
");
                if crate::info::has_fb() {
                    crate::ring0::core::dashboard::dashboard_log(f.as_str());
                }
            }
            serial_write("[fault] ");
            serial_write(n.as_str());
            serial_write("
");
            if crate::info::has_fb() {
                crate::ring0::core::dashboard::dashboard_log(n.as_str());
            }
        }
        crate::ring0::core::autopsy::registrar(vector, error, rip, cr2, fault_rsp, pid, tid, &cap);
        let _ = fault_rsp;
        // schedule() below loads the NEXT task's CR3 itself.
        return crate::ring0::task::scheduler::kill_current_and_pick();
    }
    fault_report(vector, error, rip, cr2, fault_rsp)
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
