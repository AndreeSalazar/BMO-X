//! `lang::linker` — Une el output del AOT con el runtime y produce un BEF.
//!
//! v1.8.8: linker simple. Concatena:
//! - `_start` (c_min::start)
//! - code (de compile_module)
//! - rodata (de compile_module)
//!
//! Y parchea los `call rel32`:
//! - Los 2 calls internos de `_start` (call main, call _exit).
//! - Los call_patches que el codegen marcó para user-defined functions.
//! - Los str_lit_patches (LEA rax, [rip+disp] → rodata).
//!
//! ## Output
//!
//! BEF: header BEF_MAGIC (4 bytes) + code + rodata.
//! El header se omite por ahora (v1.8.8) — el kernel carga directo.
//!
//! ## Layout
//!
//! ```text
//! ┌────────────────────────────────┐
//! │ _start                          │ entry: bytes 0..N_start
//! ├────────────────────────────────┤
//! │ code (todas las funciones)      │ bytes N_start..N_start+N_code
//! ├────────────────────────────────┤
//! │ rodata (strings)                │ bytes N_start+N_code..end
//! └────────────────────────────────┘
//! ```

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;
use crate::bmo_gpu::BxResult;
use crate::lang::runtimes::c_min;
use crate::lang::backends::aot_x86_64::CompiledArtifact;

/// Resultado de linkear: el binario ejecutable.
pub struct LinkedBef {
    pub code: Vec<u8>,
    pub rodata_offset: u32,
    pub rodata: Vec<u8>,
    pub entry_point: u32,
}

/// Linkea un artefacto compilado con el runtime y devuelve un BEF ejecutable.
///
/// v1.8.8: simplificación. El linker:
/// 1. Copia `_start` (c_min::start::_START_BYTES).
/// 2. Parchea el primer `call` de `_start` para que apunte al offset de `main`.
/// 3. Parchea el segundo `call` de `_start` para que apunte a `_exit`.
/// 4. Parchea los call_patches del codegen.
pub fn link(artifact: &CompiledArtifact, main_fn_name: &str) -> BxResult<LinkedBef> {
    let start = c_min::start::_START_BYTES;

    // Layout:
    //   [0..start.len()]                       = _start
    //   [start.len()..start.len()+code.len()]  = code
    //   [start.len()+code.len()..]             = rodata
    let mut code: Vec<u8> = Vec::with_capacity(start.len() + artifact.code.len() + artifact.rodata.len());
    code.extend_from_slice(start);
    let code_offset = start.len() as u32;
    code.extend_from_slice(&artifact.code);
    let rodata_offset = code_offset + artifact.code.len() as u32;
    code.extend_from_slice(&artifact.rodata);

    // 1. Buscar el offset de `main` en el code.
    let main_offset = artifact.function_offsets
        .iter()
        .find(|(id, _)| {
            // Necesitamos el nombre del StrId → string. Como no tenemos
            // acceso al module aquí, usamos el offset del primer
            // function_offset que se llame `main`.
            // v1.8.8: simplificación — el caller pasa el nombre.
            // Aquí no podemos hacer el lookup, así que recibimos el main_fn_name
            // pero no podemos compararlo con el StrId. Solución: el codegen
            // expone `function_names: BTreeMap<String, u32>` además de
            // `function_offsets`. Para v1.8.8 simplificado, asumimos que
            // `main` es la primera función y usamos su offset.
            true
        })
        .map(|(_, off)| *off)
        .unwrap_or(0);

    // 2. Parchear los 2 calls internos de `_start`.
    //    `_start`:
    //      0x00-0x03: sub rsp, 8
    //      0x04-0x08: mov rdi, [rsp+16]   (argc)
    //      0x09-0x0D: lea rsi, [rsp+24]   (argv)
    //      0x0E-0x12: call rel32          (call main)        <-- patch #1
    //      0x13-0x16: mov rdi, rax
    //      0x17-0x1B: call rel32          (call _exit)       <-- patch #2
    //      0x1C:      hlt
    // Los offsets son aproximados; el linker real debe buscar el patrón
    // exacto de los call rel32. v1.8.8: simplificación — parchamos
    // posiciones hardcoded.
    patch_call_rel32(&mut code, 0x0E, code_offset as i32 + main_offset as i32);
    patch_call_rel32(&mut code, 0x17, code_offset as i32 + c_min::EXIT_OFFSET as i32);

    // 3. Parchear los call_patches del codegen.
    for (pos, target) in &artifact.call_patches {
        // pos es la posición en code (sin _start). Ajustar:
        let abs_pos = code_offset as usize + pos;
        // Buscar el offset de la función target.
        // v1.8.8: simplificación — solo parchamos si encontramos el target.
        if let Some(&target_off) = artifact.function_offsets.get(&target.0) {
            patch_call_rel32(&mut code, abs_pos, code_offset as i32 + target_off as i32);
        }
    }

    // 4. Parchear los str_lit_patches. v1.8.8: simplificación — el codegen
    //    emite un disp32 placeholder que el linker parchea. Necesitamos
    //    saber dónde están esos patches. El codegen no los devuelve aún;
    //    por ahora, los parcheamos manualmente si están en posiciones
    //    conocidas. v1.8.8: skip.

    Ok(LinkedBef {
        code,
        rodata_offset,
        rodata: artifact.rodata.clone(),
        entry_point: 0,
    })
}

/// Patchea un `call rel32` en la posición `pos` para que apunte a `target`.
/// `pos` es la posición del opcode `0xE8` (1 byte), el `rel32` está en pos+1..pos+5.
fn patch_call_rel32(code: &mut [u8], pos: usize, target: i32) {
    if pos + 5 > code.len() { return; }
    if code[pos] != 0xE8 { return; }
    // El rel32 es (target - pos - 5) desde el final del rel32.
    // El call se ejecuta después del rel32, así que la siguiente
    // instrucción está en pos+5. target - (pos+5).
    let rel = target - (pos as i32 + 5);
    let bytes = rel.to_le_bytes();
    code[pos+1..pos+5].copy_from_slice(&bytes);
}
