//! **El trampolín**: de 16 bits a 64, y dónde aterriza el AP.
//!
//! ═══ ⚠️ El fallo que hundía a la versión vieja ═══
//!
//! Estaba ensamblada como código de **64 bits** (`mov rax, …`, `retfq`) para un
//! núcleo que arranca en **modo real de 16 bits**. Ahí un prefijo REX no existe:
//! `0x48` es `dec ax`. Ejecutaba basura desde la primera instrucción, y por eso
//! da igual cuántas veces se llamara: no podía funcionar.
//!
//! Aquí el bloque lleva `.code16` / `.code32` / `.code64` y **los saltos lejanos
//! se emiten a mano** (`66 EA imm32 imm16`), porque el destino es *"donde va a
//! estar copiado"* —`0x8000`— y no donde el enlazador lo puso.
//!
//! Se comprobó sacando los bytes del ELF ya enlazado:
//! `fa · 31 c0 · 8e d8 · 66 0f 01 16` y **cero bytes `0x48`**.

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

    // lgdt con base de 32 bits: hace falta el prefijo de tamaño de operando.
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

    // PAE, que el modo largo exige.
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax

    // ★ El CR3 DEL KERNEL. No se construye una tabla nueva: la del kernel ya
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

/// Dónde empieza y cuánto mide el trampolín, para copiarlo.
pub fn bytes() -> (*const u8, usize) {
    let ini = core::ptr::addr_of!(smp_tramp_ini) as *const u8;
    let fin = core::ptr::addr_of!(smp_tramp_fin) as *const u8;
    (ini, fin as usize - ini as usize)
}

/// Cuántos APs han contestado.
pub static VIVOS: AtomicU32 = AtomicU32::new(0);
/// Qué APIC IDs contestaron, un bit cada uno.
///
/// El número solo no sirve: *"despertaron 4 de 5"* deja sin decir **cuál falta**,
/// y cuál falta es justo el dato con el que se mira el siguiente.
pub static MASCARA: AtomicU32 = AtomicU32::new(0);

/// El APIC ID **por CPUID**, no por LAPIC.
///
/// A propósito: la MMIO del LAPIC vive en `0xFEE0_0000` y el kernel sólo la
/// alcanza por el physmap. Un AP recién llegado no tiene por qué poder tocarla,
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
/// No toca nada del kernel: ni CABINA, ni el planificador, ni un driver. Sólo
/// dos atómicas. Es el contrato de `docs/SMP_MAESTRO.md` — un obrero que no
/// comparte estado no puede correr una carrera, y por eso esto es seguro con los
/// 209 `static mut` que hay ahí fuera.
#[unsafe(no_mangle)]
pub extern "C" fn smp_ap_entrada() -> ! {
    let id = apic_id();
    MASCARA.fetch_or(1u32 << (id & 31), Ordering::SeqCst);
    VIVOS.fetch_add(1, Ordering::SeqCst);
    loop {
        unsafe { core::arch::asm!("cli; hlt", options(nomem, nostack)) };
    }
}
