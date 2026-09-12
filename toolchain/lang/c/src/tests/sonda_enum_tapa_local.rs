//! **UNA CONSTANTE DE `enum` TAPABA A UNA VARIABLE LOCAL.** 2026-09-12.
//!
//! # El fallo del fondo de DOOM, por fin con nombre
//!
//! Siete instrumentos en el Ryzen para llegar aqui. El ultimo (C7) dijo que lo
//! que entra en `R_RenderSegLoop` esta bien (`yl 50 yh 129 fc 168`) y que el
//! plano se queda con `top 0 bottom 2`. Y `llvm-objdump` sobre los bytes de
//! verdad de esa funcion en `doom.bex` lo enseno sin discusion:
//!
//! ```text
//!    6d023  movsxd rax, eax         ; top = ceilingclip[rw_x]+1 ... y no se guarda
//!    6d03c  mov    rax, 0x2         ; if (bottom >= floorclip[rw_x]) -- "bottom" = 2
//!    6d0a6  mov    rax, 0x0         ; if (top <= bottom)            -- "top"    = 0
//!    6d0c9  mov    rax, 0x0         ; ceilingplane->top[rw_x] = top
//!    6d10d  mov    rax, 0x2         ; ceilingplane->bottom[rw_x] = bottom
//! ```
//!
//! `top` y `bottom` no se leian: se emitian como CONSTANTES. Porque `p_spec.h`
//! declara `typedef enum { top, middle, bottom } bwhere_e;` --top 0, bottom 2--
//! y el unity build lo ve antes que `r_segs.c`. Y BMO C resolvia el nombre
//! como constante de enum ANTES que como local. La asignacion calculaba el
//! valor y no lo guardaba en ningun sitio.
//!
//! En C la local declarada dentro de la funcion TAPA a la constante del
//! fichero: es el mismo espacio de nombres ordinario y el ambito de dentro gana.
//!
//! [!] Y NO era el metal. Lo sospeche dos veces --registros r12..r15, la regla
//! AH/CH/DH/BH del emulador-- y las dos sobraban: el banco no lo veia porque
//! ninguna sonda tenia ese `enum` delante. El bloque recortado y la funcion
//! entera salieron verdes por la misma razon.

use super::*;

fn cuadra(nombre: &str, espera: &str, fuente: &str) {
    let bef = compile_source_to_bef(fuente).expect("tiene que compilar");
    assert_eq!(ejecutar_bef(&bef).trim_end(), espera, "{nombre}");
}

/// *** La forma de DOOM: enum global, locales con el mismo nombre.
#[test]
fn la_local_tapa_a_la_constante_del_enum() {
    cuadra("local tapa enum", "130,167,1",
        "typedef enum { top, middle, bottom } bwhere_e; \
         int main(){ int top; int bottom; top = 129 + 1; bottom = 168 - 1; \
           printf(\"%d,%d,%d\", top, bottom, top <= bottom); return 0; }");
}

/// Un parametro tambien la tapa.
#[test]
fn el_parametro_tapa_a_la_constante_del_enum() {
    cuadra("parametro tapa enum", "41",
        "enum { middle = 7 }; int f(int middle){ return middle + 1; } \
         int main(){ printf(\"%d\", f(40)); return 0; }");
}

/// Y fuera del ambito de la local, la constante sigue siendo la constante.
#[test]
fn fuera_de_la_funcion_la_constante_sigue_valiendo() {
    cuadra("la constante fuera", "2,99",
        "typedef enum { top, middle, bottom } bwhere_e; \
         int f(void){ int bottom; bottom = 99; return bottom; } \
         int main(){ printf(\"%d,%d\", bottom, f()); return 0; }");
}
