//! **NAVEGAR v1, EJECUTADO: una ventana con el mensaje, o el mensaje por
//! consola si nadie compone. Y no finge.**
//!
//! `Ultra_userspace/apps/navegar/navegar.inti` es la cara de la LAMINA en
//! BMO-X (`docs/plan/PLAN_NAVEGAR.md`). Esta prueba lo corre en el emulador
//! por los DOS caminos que la app tiene:
//!
//!   - sin padre (lanzada desde el shell de Ring 0): el mensaje por consola,
//!     con las tres palabras que el plan prohibe tapar, y codigo 1.
//!   - con un DIRECTOR de mentira (`padre = 7`): OFRECE una superficie de
//!     480x112 con buzon, pinta el mensaje dentro, y se cierra cuando por el
//!     buzon llega una `q` -- que es lo que el DIRECTOR real dejaria ahi.
//!
//! ** Lo que no prueba: que el DIRECTOR de verdad la componga. Eso es el
//! Ryzen (N1 del plan).

use std::path::PathBuf;

use bmo_lower::emu::{run, Machine};

fn fuente() -> String {
    std::fs::read_to_string(PathBuf::from("../../../../Ultra_userspace/apps/navegar/navegar.inti"))
        .expect("no encuentro `Ultra_userspace/apps/navegar/navegar.inti`")
}

fn emitido(texto: &str) -> bmo_inti_x86_64::Emitido {
    let arbol = bmo_inti_front::armar(texto);
    assert!(!arbol.hay_errores(), "el programa no se lee: {}", arbol.pintar("navegar.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let nec = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec).valor;
    bmo_inti_x86_64::emitir(&ir)
}

/// La maquina con el codigo y la fuente (tabla congelada) en su sitio.
fn maquina(e: &bmo_inti_x86_64::Emitido) -> Machine {
    let mut m = Machine::new(e.codigo.clone());
    let (rodata, donde) = bmo_inti_x86_64::rodata_de(e);
    if !rodata.is_empty() {
        let base = m.load_data(&rodata);
        for (off, i) in &e.reubicaciones {
            if let Some(d) = donde.get(*i as usize) {
                m.code[*off..*off + 8].copy_from_slice(&(base + d).to_le_bytes());
            }
        }
    }
    m
}

/// Lo que escribio por consola, en palabras de ocho bytes cortadas en el cero.
fn dice(m: &Machine) -> String {
    let mut s = String::new();
    for c in m.syscalls.iter().filter(|c| c.operation == 0x06 && c.capability == 0xFFFF_FFFF_FFFF_FFFE) {
        for b in c.arg0.to_le_bytes() {
            if b == 0 {
                break;
            }
            s.push(b as char);
        }
    }
    s
}

fn salida(m: &Machine) -> Option<u64> {
    m.syscalls.iter().rev().find(|c| c.operation == 0x04 && c.capability == 0xFFFF_FFFF_FFFF_FFFE).map(|c| c.arg0)
}

#[test]
fn sin_nadie_que_componga_dice_que_falta_la_antena_por_consola() {
    let e = emitido(&fuente());
    assert!(e.arranca, "navegar tiene `principal`");
    let m = run(maquina(&e), 20_000_000);
    let d = dice(&m);
    assert!(d.starts_with("NAVEGAR"), "empieza por su nombre: {:?}", d);
    assert!(d.contains("ANTENA"), "nombra a la antena: {:?}", d);
    assert!(d.contains("Ninguna conectada"), "dice que no hay ninguna: {:?}", d);
    assert!(d.contains("no finge que si"), "y que no finge: {:?}", d);
    assert!(d.contains("toolchain/tools/antena"), "y dice donde esta la antena: {:?}", d);
    assert_eq!(d.matches('\n').count(), 4, "cuatro lineas: {:?}", d);
    assert!(m.ofertas.is_empty(), "sin padre no se ofrece ninguna superficie");
    assert_eq!(salida(&m), Some(1), "y se va con codigo 1: no pudo, y lo dice");
}

#[test]
fn con_escritorio_abre_una_ventana_con_el_mensaje_y_esc_la_cierra() {
    use bmo_abi::syscalls::surface::{SUP_CABECERA, SUP_EV_CARACTER, SUP_MAGIC};
    let e = emitido(&fuente());
    let mut m = maquina(&e);
    m.padre = 7;
    // El DIRECTOR de mentira le deja una `q` en el buzon en cuanto duerma.
    m.buzon_pendiente.push_back(SUP_EV_CARACTER | 0x100 | b'q' as u64);
    let m = run(m, 200_000_000);

    assert_eq!(m.ofertas.len(), 1, "ofrecio UNA superficie: {:?}", m.ofertas);
    let (base, desde, bytes, destino) = m.ofertas[0];
    assert_eq!(destino, 7);
    let s = base + desde;
    let campo = |i: u64| -> u32 {
        let mut v = 0u32;
        for k in 0..4 {
            v |= (m.read_u8_pub(s + i * 4 + k) as u32) << (k * 8);
        }
        v
    };
    assert_eq!(campo(0), SUP_MAGIC as u32, "BSUP");
    assert_eq!((campo(1), campo(2)), (480, 112), "480x112");
    assert_eq!(campo(7), 64, "64 ranuras");
    assert!(campo(5) >= 1, "pinto al menos una vez (la secuencia subio)");
    assert_eq!(bytes, SUP_CABECERA + 480 * 112 * 4 + 16 + 64 * 8, "cabecera + pixeles + buzon");

    // Hay texto: pixeles blancos (el titulo) y naranjas (el aviso) sobre el fondo.
    let pixel = |x: u64, y: u64| -> u32 {
        let i = s + SUP_CABECERA + (y * 480 + x) * 4;
        (0..4).fold(0u32, |v, k| v | (m.read_u8_pub(i + k) as u32) << (k * 8))
    };
    let mut blancos = 0;
    let mut naranjas = 0;
    for y in 0..112 {
        for x in 0..480 {
            match pixel(x, y) {
                0x00FF_FFFF => blancos += 1,
                0x00FF_AA00 => naranjas += 1, // AVISO = 16755200
                _ => {}
            }
        }
    }
    assert!(blancos > 200, "el titulo, en blanco: {} pixeles", blancos);
    assert!(naranjas > 200, "el aviso de la antena, en naranja: {} pixeles", naranjas);
    assert_eq!(pixel(0, 0), 0x0014_1414, "el fondo");

    // Y se cerro por la `q`: nada por consola, codigo 0, y el buzon vacio.
    assert!(m.buzon_pendiente.is_empty(), "el DIRECTOR entrego la tecla");
    assert_eq!(dice(&m), "", "con ventana no escribe por consola");
    assert_eq!(salida(&m), Some(0), "se cerro limpia");
}
