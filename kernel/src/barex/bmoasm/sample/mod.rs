//! Muestras de programas BMO Simple. Sirven de documentación viva y
//! de input para tests del lexer/parser/emitter.

/// `exit_zero.bmo` — exit syscall, lo más mínimo posible.
pub const EXIT_ZERO: &str = r#"
def principal() {
    reg rax = 60       // exit syscall #
    reg rdi = 0        // exit code
    syscall
}
"#;

/// `spin_lock.bmo` — demostración de `atomico` + `pausa` + `cuando zf`.
pub const SPIN_LOCK: &str = r#"
def adquirir(candado: ptr) {
    bucle {
        atomico {
            // bts qword [candado], 0   — set & test bit 0
            emit 0xF0 0x48 0x0F 0xBA 0x2F 0x00
        }
        cuando zf {           // si bit ya estaba 0 (libre), salir
            retorna
        }
        pausa                  // hint al CPU: spin loop
    }
}
"#;

/// `medir_ciclos.bmo` — usa rdtsc para medir un bloque.
pub const MEDIR_CICLOS: &str = r#"
def medir() -> num {
    lfence                    // serializa antes
    rdtsc                     // resultado en RDX:RAX
    let inicio: num = reg rax
    // ... bloque a medir ...
    nop
    nop
    nop
    lfence
    rdtsc
    let fin: num = reg rax
    retorna fin resta inicio
}
"#;

/// `tabla_salto.bmo` — switch/jump table sin macros.
pub const TABLA_SALTO: &str = r#"
def manejar(op: num) {
    match op {
        caso 0 { retorna }
        caso 1 { nop }
        caso 2 { syscall }
        defecto { int3 }     // breakpoint si op desconocido
    }
}
"#;

/// `align_funcion.bmo` — alinea la función a 64 B (cache line Zen 3).
pub const ALIGN_FUNCION: &str = r#"
align 64
def critical_path() {
    paralelo {
        para i desde 0 hasta 16 paso 1 {
            // hint al emisor: usa AVX2/AVX-512 si está disponible
            cerca ptr 0x1000   // prefetch L1
        }
    }
}
"#;

/// Devuelve todas las muestras como slice (útil para tests de iteración).
pub const ALL_SAMPLES: &[&str] = &[
    EXIT_ZERO, SPIN_LOCK, MEDIR_CICLOS, TABLA_SALTO, ALIGN_FUNCION,
];
