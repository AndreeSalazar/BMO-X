//! **El trampolin**: de 16 bits a 64, y donde aterriza el AP.
//!
//! === [!] El fallo que hundia a la version vieja ===
//!
//! Estaba ensamblada como codigo de **64 bits** (`mov rax, ...`, `retfq`) para un
//! nucleo que arranca en **modo real de 16 bits**. Ahi un prefijo REX no existe:
//! `0x48` es `dec ax`. Ejecutaba basura desde la primera instruccion, y por eso
//! da igual cuantas veces se llamara: no podia funcionar.
//!
//! Aqui el bloque lleva `.code16` / `.code32` / `.code64` y **los saltos lejanos
//! se emiten a mano** (`66 EA imm32 imm16`), porque el destino es *"donde va a
//! estar copiado"* --`0x8000`-- y no donde el enlazador lo puso.
//!
//! Se comprobo sacando los bytes del ELF ya enlazado:
//! `fa - 31 c0 - 8e d8 - 66 0f 01 16` y **cero bytes `0x48`**.
//!
//! === [!] DOS INVARIANTES DE FUERA DE LOS QUE ESTO DEPENDE ===
//!
//! Ninguno se ve desde este fichero, y romper cualquiera de los dos mata al AP
//! sin dejar un mensaje. Quedan escritos aqui porque es donde se muere:
//!
//! 1. **El identity map NO puede tener NX.** Este codigo enciende la paginacion
//!    con el `CR3` del kernel **mientras se ejecuta en `0x8000`**: la instruccion
//!    siguiente al `mov cr0, eax` ya se busca a traves de las tablas. Hoy
//!    funciona porque `s2_mem` mapea `0..4 GiB` con `PTE_PRESENT | PTE_WRITABLE`
//!    y **sin** el bit NX. El dia que alguien endurezca ese mapa poniendo NX en
//!    lo que no es kernel, esto se convierte en un `#PF` en la primera
//!    instruccion tras activar paginacion -- y sin nadie que lo cuente, porque el
//!    AP todavia no ha llegado a ningun sitio.
//!
//! 2. **Un AP que toma una excepcion esta muerto de una forma fea.** Carga la
//!    IDT del kernel, pero **no tiene GS por-CPU** y **no tiene `CR4.OSXSAVE`**;
//!    los stubs de trampa hacen `swapgs` y `xsave64`. O sea: cualquier excepcion
//!    aqui es `#UD` dentro del manejador -> doble fallo -> triple fallo. Es
//!    aceptable *solo* porque este AP no hace nada que pueda fallar --dos
//!    atomicas y `hlt`--, y **deja de serlo el dia que se le de trabajo de
//!    verdad**. Entonces hace falta GS por-CPU antes de soltarlo.

use core::sync::atomic::{AtomicU32, Ordering};

core::arch::global_asm!(
    r#"
.section .text.smp_tramp,"ax"
.globl smp_tramp_ini
.globl smp_tramp_fin

// El AP entra AQUI en modo real de 16 bits, con CS:IP = 0x0800:0x0000.
// Todo lo que toca son direcciones absolutas: este codigo se ejecuta copiado en
// 0x8000, no donde se ensamblo.
.code16
smp_tramp_ini:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax

    // lgdt con base de 32 bits: hace falta el prefijo de tamano de operando.
    .byte 0x66
    lgdt [0x9000]

    // Modo protegido.
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    // Salto lejano a 32 bits: 0x66 0xEA imm32 imm16.
    .byte 0x66, 0xEA
    .long 0x8000 + (smp_pm32 - smp_tramp_ini)
    .word 0x10

.code32
smp_pm32:
    mov ax, 0x18
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov esp, 0x9FF0

    // PAE (0x20), que el modo largo exige, y SSE en la misma escritura:
    // OSFXSR (bit 9) + OSXMMEXCPT (bit 10) = 0x600.
    //
    // * SIN ESTO HAY UN #UD ESPERANDO. Un AP sale del reset con CR4 = 0, o sea
    // con SSE apagado -- y el destino de este salto es codigo RUST compilado para
    // x86-64, cuya linea base INCLUYE SSE2: en cuanto el compilador emita un
    // `movaps` para mover 16 bytes o poner a cero un hueco, el nucleo se muere
    // con una excepcion que no dice nada. Hoy `smp_ap_entrada` es tan pequena
    // que probablemente no emita ninguna; el dia que ese obrero haga trabajo de
    // verdad, seguro. Cuesta un OR y quita una clase entera de fallo futuro.
    mov eax, cr4
    or eax, 0x620
    mov cr4, eax

    // Y CR0 en condiciones para SSE: MP (bit 1) puesto, EM (bit 2) quitado.
    // EM=1 significa "emula la FPU", y con eso SSE tambien es #UD.
    mov eax, cr0
    and eax, 0xFFFFFFFB
    or eax, 0x2
    mov cr0, eax

    // * El CR3 DEL KERNEL. No se construye una tabla nueva: la del kernel ya
    // identity-mapea 0..32 MiB, que es todo lo que hay que ver desde aqui.
    mov eax, [0x9020]
    mov cr3, eax

    // EFER: LME + NXE. NXE va ANTES de encender la paginacion porque las tablas
    // del kernel usan el bit NX, y sin NXE ese bit esta "reservado": el primer
    // acceso seria un #PF que nadie sabria explicar.
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x900
    wrmsr

    // Paginacion: aqui se activa el modo largo de verdad.
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    // Salto lejano a 64 bits: 0xEA imm32 imm16.
    .byte 0xEA
    .long 0x8000 + (smp_lm64 - smp_tramp_ini)
    .word 0x20

.code64
smp_lm64:
    mov rax, 0x9010
    lidt [rax]
    mov rsp, [0x9030]
    mov rax, [0x9028]
    jmp rax

smp_tramp_fin:
.code64
"#
);

unsafe extern "C" {
    static smp_tramp_ini: u8;
    static smp_tramp_fin: u8;
}

/// Donde empieza y cuanto mide el trampolin, para copiarlo.
pub fn bytes() -> (*const u8, usize) {
    let ini = core::ptr::addr_of!(smp_tramp_ini) as *const u8;
    let fin = core::ptr::addr_of!(smp_tramp_fin) as *const u8;
    (ini, fin as usize - ini as usize)
}

/// Cuantos APs han contestado.
pub static VIVOS: AtomicU32 = AtomicU32::new(0);
/// Que APIC IDs contestaron, un bit cada uno.
///
/// El numero solo no sirve: *"despertaron 4 de 5"* deja sin decir **cual falta**,
/// y cual falta es justo el dato con el que se mira el siguiente.
///
/// [!] Son 32 bits y el bit se elige con `id & 31`. En esta maquina los APIC IDs
/// van de 0 a 11 y la mascara es exacta. **En un x2APIC con IDs grandes y
/// dispersos, dos nucleos distintos pueden caer en el mismo bit** -- la cuenta
/// (`VIVOS`) seguiria siendo correcta, la mascara no. Se deja asi a proposito
/// mientras el censo quepa: ampliarla exige decidir que hacer con IDs de 32 bits
/// en un panel de una linea, y ese problema todavia no existe.
pub static MASCARA: AtomicU32 = AtomicU32::new(0);

/// El APIC ID **por CPUID**, no por LAPIC.
///
/// A proposito: la MMIO del LAPIC vive en `0xFEE0_0000` y el kernel solo la
/// alcanza por el physmap. Un AP recien llegado no tiene por que poder tocarla,
/// y `CPUID.1:EBX[31:24]` da el mismo dato **sin un solo acceso a memoria**.
pub fn apic_id() -> u32 {
    let ebx: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {0:e}, ebx",
            "pop rbx",
            out(reg) ebx,
            inout("eax") 1u32 => _,
            out("ecx") _,
            out("edx") _,
            options(nostack),
        );
    }
    ebx >> 24
}

/// Donde aterriza el AP, ya en 64 bits. **Se apunta y se para.**
///
/// No toca nada del kernel: ni CABINA, ni el planificador, ni un driver. Solo
/// dos atomicas. Es el contrato de `docs/SMP_MAESTRO.md` -- un obrero que no
/// comparte estado no puede correr una carrera, y por eso esto es seguro con los
/// 209 `static mut` que hay ahi fuera.
#[unsafe(no_mangle)]
pub extern "C" fn smp_ap_entrada() -> ! {
    let id = apic_id();
    MASCARA.fetch_or(1u32 << (id & 31), Ordering::SeqCst);
    // * `fetch_add` devuelve el valor ANTERIOR, y eso es justo un indice unico
    // 0..n-1 sin repartirlo desde fuera ni pasarlo por la pagina compartida.
    // El contador y el reparto de indices salen de la misma operacion atomica.
    let indice = VIVOS.fetch_add(1, Ordering::SeqCst);
    // Y de aqui al bucle de trabajo. Antes esto era `cli; hlt` para siempre: un
    // nucleo despierto al que no se le puede dar una tarea sirve exactamente lo
    // mismo que uno dormido.
    super::crew::obrero(indice)
}
