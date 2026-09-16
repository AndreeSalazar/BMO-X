//! **NAVEGAR v0, EJECUTADO: dice que hace falta una antena, y no finge.**
//!
//! `Ultra_userspace/apps/navegar/navegar.inti` es la cara de la LAMINA en
//! BMO-X (`docs/plan/PLAN_NAVEGAR.md`). Su version 0 solo tiene un mensaje,
//! y esta prueba lo corre en el emulador para que el mensaje sea lo que el
//! fuente dice y no lo que alguien recuerda que decia.
//!
//! ** Lo que se exige no es la frase entera: son las TRES palabras que la
//! regla del plan prohibe tapar -- que falta una ANTENA, que no hay NINGUNA,
//! y que no FINGE. Cambiar el texto esta permitido; quitar una de esas no.

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

#[test]
fn navegar_dice_que_falta_la_antena_y_no_finge() {
    let e = emitido(&fuente());
    assert!(e.arranca, "navegar tiene `principal`");
    let m = run(Machine::new(e.codigo), 5_000_000);
    let d = dice(&m);
    assert!(d.starts_with("NAVEGAR"), "empieza por su nombre: {:?}", d);
    assert!(d.contains("ANTENA"), "nombra a la antena: {:?}", d);
    assert!(d.contains("Ninguna conectada"), "dice que no hay ninguna: {:?}", d);
    assert!(d.contains("no finge que si"), "y que no finge: {:?}", d);
    assert!(d.contains("toolchain/tools/antena"), "y dice donde esta la antena: {:?}", d);
    assert_eq!(d.matches('\n').count(), 4, "cuatro lineas, ni una en blanco: {:?}", d);
    // Solo habla por la consola: ni un fichero, ni memoria, ni una puerta
    // que no sea la de escribir -- y la ultima es la salida de `principal`.
    // Es una version 0 y lo demuestra.
    let (salida, resto) = m.syscalls.split_last().expect("al menos la salida");
    assert!(
        resto.iter().all(|c| c.operation == 0x06),
        "v0 solo escribe por consola; hizo otra cosa: {:?}",
        resto.iter().map(|c| c.operation).collect::<Vec<_>>()
    );
    assert_ne!(salida.operation, 0x06, "la ultima puerta es salir, no escribir");
}
