//! **La sonda de la VENTANA, calibrada en el emulador antes del metal.**
//!
//! `toolchain/lang/inti/sondas/ventana.inti` pinta una superficie y escribe
//! por consola lo que LEE de vuelta. Aqui se comprueba que, con el DIRECTOR
//! de mentira, lee lo que pinto: si en el Ryzen leyera otra cosa, el que
//! miente es el metal y no el instrumento.

use std::path::PathBuf;

use bmo_lower::emu::{run, Machine};

fn fuente() -> String {
    std::fs::read_to_string(PathBuf::from("../sondas/ventana.inti"))
        .expect("no encuentro `sondas/ventana.inti`")
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
fn la_sonda_lee_de_vuelta_lo_que_pinto() {
    let e = emitido(&fuente());
    let mut m = maquina(&e);
    m.padre = 7;
    let m = run(m, 200_000_000);
    let d = dice(&m);
    assert!(d.contains("pix 0,0 0000000000141414"), "el fondo se lee de vuelta: {:?}", d);
    assert!(d.contains("pix 8,8 0000000000ffaa00"), "la caja naranja se lee de vuelta: {:?}", d);
    assert!(d.contains("pix 3,220000000000ffffff"), "una columna de la H se lee de vuelta: {:?}", d);
    assert!(d.contains("secuenc 0000000000000001"), "pinto una vez: {:?}", d);
    assert!(!m.ofertas.is_empty(), "ofrecio la superficie");
    assert_eq!(salida(&m), Some(0), "se va sola con 0: {:?}", d);
}

#[test]
fn sin_padre_lo_dice_y_sale_con_1() {
    let e = emitido(&fuente());
    let m = run(maquina(&e), 20_000_000);
    assert!(dice(&m).contains("sin padre"), "{:?}", dice(&m));
    assert_eq!(salida(&m), Some(1));
}
