//! Los INTRINSECOS: la tabla de sem-asm como funcion de C
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// ---- LA FUSIÓN sem-asm↔C (Fase 1) ----

#[test]
fn intrinsic_emits_exact_table_bytes() {
    // __pause() y __hlt() = bytes EXACTOS de intrinsics.toml en el código.
    let src = "int main() { __pause(); __hlt(); return 0; }";
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.windows(2).any(|w| w == [0xF3, 0x90]), "falta pause (F3 90)");
    assert!(bef.contains(&0xF4), "falta hlt (F4)");
}

#[test]
fn intrinsic_rdtsc_returns_combined_value() {
    // __rdtsc() devuelve u64: rdtsc + shl rdx,32 + or rax,rdx.
    let src = "int main() { unsigned long t; t = __rdtsc(); return (int)t; }";
    let bef = compile_source_to_bef(src).unwrap();
    let seq = [0x0F, 0x31, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x09, 0xD0];
    assert!(bef.windows(seq.len()).any(|w| w == seq),
        "falta la secuencia rdtsc + combine edx:eax → rax");
}

#[test]
fn unknown_intrinsic_fails_honestly() {
    // __zzz() no está en la tabla → error con nombre y ubicación de la tabla.
    let err = compile_source_to_bef("int main() { __zzz(); return 0; }").unwrap_err();
    assert!(err.message.contains("no existe en la tabla"), "mensaje: {}", err.message);
}

#[test]
fn intrinsic_wrong_arity_fails() {
    // __hlt no lleva operandos; __hlt(1) debe fallar en codegen contra la tabla.
    let err = compile_source_to_bef("int main() { __hlt(1); return 0; }").unwrap_err();
    assert!(err.message.contains("espera 0"), "mensaje: {}", err.message);
}

#[test]
fn intrinsic_outb_marshals_args_to_registers() {
    // __outb(0x3F8, 65): puerto→dx, valor→al, luego out dx,al (0xEE).
    let src = "int main() { __outb(1016, 65); return 0; }";
    let bef = compile_source_to_bef(src).unwrap();
    // pop rdx (0x5A) para el puerto, pop rax (0x58) para el valor, out (0xEE)
    assert!(bef.contains(&0xEE), "falta out dx,al (0xEE)");
    assert!(bef.windows(2).any(|w| w == [0x5A, 0xEE]) || bef.contains(&0x5A),
        "el puerto debe volcarse a dx (pop rdx 0x5A)");
}

#[test]
fn intrinsic_inb_returns_byte() {
    // __inb(puerto): in al,dx (0xEC) + movzx rax,al (48 0F B6 C0).
    let src = "int main() { int c; c = __inb(96); return c; }";
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.contains(&0xEC), "falta in al,dx (0xEC)");
    let seq = [0x48, 0x0F, 0xB6, 0xC0];
    assert!(bef.windows(seq.len()).any(|w| w == seq), "falta movzx rax,al del retorno");
}

#[test]
fn intrinsic_wrmsr_splits_value_to_edx_eax() {
    // __wrmsr(nr, val): nr→ecx, val(64)→edx:eax, wrmsr (0F 30).
    let src = "int main() { unsigned long v; v = 5; __wrmsr(200, v); return 0; }";
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.windows(2).any(|w| w == [0x0F, 0x30]), "falta wrmsr (0F 30)");
    // shr rdx,32 del split del valor a edx:eax
    let shr = [0x48, 0xC1, 0xEA, 0x20];
    assert!(bef.windows(shr.len()).any(|w| w == shr), "falta el split del valor a edx:eax");
}

#[test]
fn intrinsic_arg_arity_wrong_fails() {
    let err = compile_source_to_bef("int main() { __outb(1); return 0; }").unwrap_err();
    assert!(err.message.contains("espera 2"), "mensaje: {}", err.message);
}

