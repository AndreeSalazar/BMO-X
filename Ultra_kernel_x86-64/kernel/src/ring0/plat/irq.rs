//! **La interrupcion del DISCO.** Que el aparato avise en vez de que se le
//! pregunte.
//!
//! ## Que cambia, dicho en una frase
//!
//! Hasta ahora el kernel le daba una orden al disco y **se quedaba mirando el
//! registro** hasta que cambiara: millones de lecturas de MMIO --que no pasan
//! por cache, cada una un viaje al chipset-- para averiguar algo que el aparato
//! sabia desde el primer microsegundo y no tenia como decir.
//!
//! Con MSI lo dice: cuando termina, **escribe el numero de vector en el LAPIC**
//! y el CPU entra aqui. Sin IOAPIC, sin tablas de routing del firmware, sin
//! preguntarle a nadie por que patilla entra este aparato -- el disco y el CPU
//! ya se conocen, y lo que faltaba era la frase, no el intermediario.
//!
//! ## Por que el stub es el MISMO que el del temporizador
//!
//! Porque un manejador de interrupcion **es una frontera de trap**, y en este
//! kernel las fronteras de trap son intercambiables: el contexto que captura
//! esta entrada y el que captura un SYSCALL tienen la misma forma, asi que el
//! planificador puede cambiar de tarea desde cualquiera de las dos.
//!
//! Hoy este manejador no cambia de tarea --solo atiende al disco y vuelve-- pero
//! el dia que una tarea duerma esperando al disco, **el sitio donde se la
//! despierta es este**, y no hara falta tocar el stub.
//!
//! ## Y si la interrupcion NO llega
//!
//! Se sigue trabajando igual. `bmo-ahci` mira el contador que este manejador
//! sube y, si no se mueve, vuelve a leer el registro por MMIO como toda la vida.
//! Un camino nuevo que solo funciona cuando el hardware colabora **no puede ser
//! el unico camino**: la placa que no enrute MSI se quedaria sin disco, o sea
//! sin arrancar, y el sintoma no se pareceria en nada a la causa.

use core::arch::naked_asm;

use crate::ring0::plat::trap::TrapFrame;
use crate::ring0::task::percpu;

/// El vector del disco. El 48 es el temporizador; este va justo detras.
pub const VECTOR_DISCO: usize = 49;

#[unsafe(naked)]
unsafe extern "C" fn disco_entry() -> ! {
    naked_asm!(
        // Copia exacta del prologo del temporizador. Ver `plat/timer.rs` para el
        // porque de cada linea -- sobre todo el de poner a cero la cabecera del
        // area de XSAVE, que costo tres dias y tres fotos.
        "cmp qword ptr [rsp+8], 0x08",
        "je 1f",
        "swapgs",
        "1: push rax", "push rcx", "push rdx", "push rbx", "push rbp",
        "push rsi", "push rdi", "push r8", "push r9", "push r10",
        "push r11", "push r12", "push r13", "push r14", "push r15",
        "mov rbp, rsp",
        "sub rsp, {reserva}",
        "and rsp, -64",
        "mov qword ptr [rsp+{bv}], 0",
        "mov qword ptr [rsp+{cero}], 0",
        "mov qword ptr [rsp+{cero}+8], 0",
        "mov qword ptr [rsp+{cero}+16], 0",
        "mov qword ptr [rsp+{cero}+24], 0",
        "mov qword ptr [rsp+{cero}+32], 0",
        "mov qword ptr [rsp+{cero}+40], 0",
        "mov qword ptr [rsp+{cero}+48], 0",
        "mov [rsp+{area}], rbp",
        "mov qword ptr [rsp+{firma}], {magia}",
        "mov eax, -1", "mov edx, -1",
        "xsave64 [rsp]",
        "mov gs:[0x10], rsp",
        "cld",
        "mov rdi, rbp",
        "call {dispatch}",
        // Epilogo compartido: rax = base del area del contexto a ejecutar.
        "mov rsp, rax",
        "cmp qword ptr [rsp+{firma}], {magia}",
        "jne 3f",
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
        "3: mov rdi, {m_sello}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "4: mov rdi, {m_cs}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        "8: mov rdi, {m_cab}", "mov rsi, rsp", "and rsp, -16", "call {podrido}",
        dispatch = sym disco_dispatch,
        podrido = sym crate::ring0::plat::faults::contexto_podrido,
        no_xcr0 = sym crate::ring0::plat::trap::XSAVE_NO_XCR0,
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

#[unsafe(no_mangle)]
extern "C" fn disco_dispatch(_frame: &mut TrapFrame) -> u64 {
    // ** LO MINIMO Y NADA MAS. Un manejador de interrupcion corre con la tarea
    // que estuviera dentro parada y --si es larga-- puede llegar la siguiente
    // encima. Aqui se limpia el aviso del aparato y se apunta que llego; QUIEN
    // esperaba ese dato lo recoge en su propio turno.
    //
    // En concreto NO se lee `PRDBC` aqui: eso es contestarle a quien pregunte, y
    // preguntar es cosa del que pidio.
    crate::ring0::dev::disk::atender_irq();
    crate::ring0::plat::timer::eoi();
    // Se devuelve el MISMO contexto: este manejador todavia no cambia de tarea.
    // El dia que alguien duerma esperando al disco, aqui se llamara al
    // planificador y esta linea pasara a devolver el que el elija -- el stub ya
    // esta preparado para eso, que es la mitad del trabajo.
    percpu::trap_rsp()
}

/// Instala el vector del disco en la IDT viva. `false` si no hay IDT.
pub fn instalar(idt_ptr: u64) -> bool {
    crate::ring0::plat::timer::instalar_vector(
        idt_ptr,
        VECTOR_DISCO,
        disco_entry as *const () as u64,
    )
}
