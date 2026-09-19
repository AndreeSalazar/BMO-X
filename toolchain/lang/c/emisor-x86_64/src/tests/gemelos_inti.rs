//! **LOS GEMELOS: la superficie en INTI dice los mismos bytes que la de C.**
//!
//! N0c de `docs/plan/PLAN_NAVEGAR.md` (2026-09-16). C e INTI no se enlazan
//! --convenciones de llamada distintas, sin IR comun-- y COOPERAN por
//! contrato: los numeros de la cabecera BSUP y del buzon viven una vez en
//! `bmo_abi::syscalls::surface::superficie`, y cada lenguaje lleva su copia con
//! juez. Esta prueba es la otra mitad de esa cooperacion: **C es el ORACULO**.
//! La superficie de C ya corre en el Ryzen (DOOM, el bloc de notas, el cubo);
//! la de INTI (`tables/lang/inti/runtime/superficie/`) acaba de escribirse.
//! Si discrepan, gana C hasta que el metal diga otra cosa.
//!
//! ## Lo que se compara
//!
//! El MISMO dibujo, escrito dos veces --una en C con `<bmo/superficie.h>` y
//! `<bmo/fuente.h>`, otra en INTI con `usa superficie`--, corrido en el mismo
//! emulador con el mismo DIRECTOR de mentira (`padre = 7`), y el bloque que
//! cada uno OFRECIO comparado byte a byte: los 32 de la cabecera, los pixeles
//! y el buzon. No se compara "lo que dice la consola": se compara lo que el
//! DIRECTOR iba a componer.
//!
//! ## Lo que NO prueba
//!
//! Que el DIRECTOR de verdad lo componga. Eso es el Ryzen (N1 del plan), y
//! aqui solo se puede demostrar que INTI escribe en el bloque exactamente lo
//! que C escribe -- que es lo que el DIRECTOR ya sabe leer.

use super::*;

/// 64x32, ocho ranuras, fondo gris, un rectangulo azul y "Hola" en blanco.
/// Los numeros son los mismos en los dos programas, escritos dos veces a
/// proposito: si uno cambia y el otro no, la prueba lo dice.
const ANCHO: u64 = 64;
const ALTO: u64 = 32;
const RANURAS: u64 = 8;

const EN_C: &str = "\
#define BMO_MONTON_BYTES (256 * 1024)
#include <stdlib.h>
#include <bmo/bmo.h>
#include <bmo/superficie.h>
#include <bmo/fuente.h>

int main(void) {
    BMO_SUPERFICIE *s;
    unsigned int *px;
    int x;
    int y;
    s = bmo_superficie_crear_con_buzon(64, 32, 8);
    if (s == 0) {
        return 1;
    }
    px = bmo_superficie_pixeles(s);
    y = 0;
    while (y < 32) {
        x = 0;
        while (x < 64) {
            px[y * 64 + x] = 0x00202020;
            x = x + 1;
        }
        y = y + 1;
    }
    y = 20;
    while (y < 30) {
        x = 40;
        while (x < 60) {
            px[y * 64 + x] = 0x000000FF;
            x = x + 1;
        }
        y = y + 1;
    }
    bmo_texto(px, 64, 64, 32, 8, 4, \"Hola\", 0x00FFFFFF);
    bmo_superficie_lista(s);
    return 0;
}
";

const EN_INTI: &str = "\
perfil llano
usa bmo
usa superficie

funcion principal devuelve entero32
    cambiante s es natural64 = superficie_crear_con_buzon(64, 32, 8)
    si s = 0
        devuelve 1
    limpia(s, 2105376)
    rectangulo(s, 40, 20, 20, 10, 255)
    texto_en(s, 8, 4, ocho_bytes(\"Hola\"), 16777215)
    pinta(s)
    devuelve 0
";

/// El bloque ofrecido, tal y como lo veria el DIRECTOR: `(cabecera y todo lo
/// demas)` desde la base de la oferta, `bytes` bytes.
fn ofrecido(m: &bmo_lower::emu::Machine) -> Vec<u8> {
    assert_eq!(m.ofertas.len(), 1, "se ofrece UNA superficie: {:?}", m.ofertas);
    let (base, desde, bytes, destino) = m.ofertas[0];
    assert_eq!(destino, 7, "se ofrece a quien nos lanzo");
    (0..bytes).map(|i| m.read_u8_pub(base + desde + i)).collect()
}

fn maquina_inti(fuente: &str) -> bmo_lower::emu::Machine {
    let arbol = bmo_inti_front::armar(fuente);
    assert!(!arbol.hay_errores(), "el gemelo de INTI no se lee: {}", arbol.pintar("gemelo.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let nec = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec).valor;
    let e = bmo_inti_x86_64::emitir(&ir);
    assert!(e.arranca);
    let mut m = bmo_lower::emu::Machine::new(e.codigo.clone());
    // Las tablas congeladas (la fuente) en su sitio: ver `tests/fuente.rs` de INTI.
    let (rodata, donde) = bmo_inti_x86_64::rodata_de(&e);
    if !rodata.is_empty() {
        let base = m.load_data(&rodata);
        for (off, i) in &e.reubicaciones {
            if let Some(d) = donde.get(*i as usize) {
                m.code[*off..*off + 8].copy_from_slice(&(base + d).to_le_bytes());
            }
        }
    }
    m.padre = 7;
    bmo_lower::emu::run(m, 50_000_000)
}

fn cabecera(bloque: &[u8]) -> [u32; 8] {
    let mut c = [0u32; 8];
    for (i, campo) in c.iter_mut().enumerate() {
        *campo = u32::from_le_bytes(bloque[i * 4..i * 4 + 4].try_into().unwrap());
    }
    c
}

#[test]
fn la_superficie_de_inti_y_la_de_c_son_la_misma() {
    let bef = compile_with_preprocessor(EN_C, std::path::Path::new("gemelo.c"), CStandard::C11)
        .expect("el gemelo de C debe compilar");
    let c = maquina_de_bef_con(&bef, |m| m.padre = 7);
    let inti = maquina_inti(EN_INTI);

    let de_c = ofrecido(&c);
    let de_inti = ofrecido(&inti);

    // 1. La cabecera, campo a campo, con el contrato del ABI por delante.
    use bmo_abi::syscalls::surface::{SUP_BGRA32, SUP_BUZON_CABECERA, SUP_BUZON_RANURA, SUP_CABECERA, SUP_MAGIC};
    let cab_c = cabecera(&de_c);
    let cab_i = cabecera(&de_inti);
    let pixeles = SUP_CABECERA + ANCHO * ALTO * 4;
    let esperada = [
        SUP_MAGIC as u32,
        ANCHO as u32,
        ALTO as u32,
        ANCHO as u32,
        SUP_BGRA32 as u32,
        1, // la secuencia: UN dibujo entero
        pixeles as u32,
        RANURAS as u32,
    ];
    assert_eq!(cab_c, esperada, "la cabecera de C es la del contrato");
    assert_eq!(cab_i, esperada, "y la de INTI tambien");

    // 2. El bloque entero: cabecera + pixeles + buzon, byte a byte.
    let total = pixeles + SUP_BUZON_CABECERA + RANURAS * SUP_BUZON_RANURA;
    assert_eq!(de_c.len() as u64, total, "C ofrecio cabecera + pixeles + buzon");
    assert_eq!(de_inti.len() as u64, total, "INTI ofrecio lo mismo");
    let mut distintos = Vec::new();
    for i in 0..total as usize {
        if de_c[i] != de_inti[i] {
            distintos.push(i);
        }
    }
    assert!(
        distintos.is_empty(),
        "{} byte(s) distintos entre C e INTI; el primero en {} (pixel ({}, {})): C {:#04x}, INTI {:#04x}",
        distintos.len(),
        distintos[0],
        ((distintos[0] as u64).saturating_sub(SUP_CABECERA) / 4) % ANCHO,
        ((distintos[0] as u64).saturating_sub(SUP_CABECERA) / 4) / ANCHO,
        de_c[distintos[0]],
        de_inti[distintos[0]]
    );

    // 3. Y que el dibujo es el que se pidio, no dos bloques igual de vacios.
    let pixel = |b: &[u8], x: u64, y: u64| -> u32 {
        let i = (SUP_CABECERA + (y * ANCHO + x) * 4) as usize;
        u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
    };
    assert_eq!(pixel(&de_inti, 0, 0), 0x0020_2020, "el fondo");
    assert_eq!(pixel(&de_inti, 45, 25), 0x0000_00FF, "el rectangulo");
    assert_eq!(pixel(&de_inti, 39, 25), 0x0020_2020, "y su borde izquierdo");
    let blancos = (0..ALTO).flat_map(|y| (0..ANCHO).map(move |x| (x, y))).filter(|&(x, y)| pixel(&de_inti, x, y) == 0x00FF_FFFF).count();
    assert!(blancos > 40, "\"Hola\" tiene pixeles blancos: {}", blancos);
}

/// Sin nadie que componga (lanzado desde el shell), `crear` devuelve 0 en los
/// dos lenguajes y no se ofrece nada. Es el camino que `roja.h` conto el
/// 12-09: una superficie que nadie va a componer no vuelve viva.
#[test]
fn sin_padre_no_hay_superficie_en_ninguno_de_los_dos() {
    let bef = compile_with_preprocessor(EN_C, std::path::Path::new("gemelo.c"), CStandard::C11).unwrap();
    let c = maquina_de_bef_con(&bef, |m| m.padre = 0);
    assert!(c.ofertas.is_empty(), "C no ofrecio nada");

    let arbol = bmo_inti_front::armar(EN_INTI);
    assert!(!arbol.hay_errores());
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(&arbol.valor, bmo_inti_front::disposicion::Medidas::cargar(&raices));
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let nec = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec).valor;
    let e = bmo_inti_x86_64::emitir(&ir);
    let m = bmo_lower::emu::run(bmo_lower::emu::Machine::new(e.codigo), 50_000_000);
    assert!(m.ofertas.is_empty(), "INTI tampoco");
    // Y se fue por el `devuelve 1`: la salida lleva el 1.
    let salida = m.syscalls.iter().rev().find(|c| c.operation == 0x04).map(|c| c.arg0);
    assert_eq!(salida, Some(1), "el programa dijo que no pudo, con codigo 1");
}
