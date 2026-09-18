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

// ===================================================================
//  N2: la lamina DE FICHERO (2026-09-18)
// ===================================================================

/// La lamina de example.com, tal cual la sirve la antena.
fn lamina_ejemplo() -> Vec<u8> {
    std::fs::read(PathBuf::from("../../../../toolchain/tools/antena/ejemplo.lamina"))
        .expect("no encuentro `toolchain/tools/antena/ejemplo.lamina`")
}

/// Corre NAVEGAR con un DIRECTOR de mentira, la lamina dada en
/// `datos/ejemplo.lam` (8.3: el FAT32 de BMO-X no lee nombres largos), y los
/// eventos que se le entreguen antes de la `q`.
fn con_lamina(lamina: &[u8], eventos: &[u64]) -> Machine {
    use bmo_abi::syscalls::surface::SUP_EV_CARACTER;
    let e = emitido(&fuente());
    let mut m = maquina(&e);
    m.padre = 7;
    m.poner_archivo("datos/ejemplo.lam", lamina);
    for &ev in eventos {
        m.buzon_pendiente.push_back(ev);
    }
    m.buzon_pendiente.push_back(SUP_EV_CARACTER | 0x100 | b'q' as u64);
    run(m, 400_000_000)
}

/// Los pixeles de la primera superficie ofrecida: `(ancho, alto, pixel(x, y))`.
fn pantalla(m: &Machine) -> (u64, u64, impl Fn(u64, u64) -> u32 + '_) {
    use bmo_abi::syscalls::surface::SUP_CABECERA;
    let (base, desde, _, _) = m.ofertas[0];
    let s = base + desde;
    let campo = |i: u64| -> u32 {
        (0..4).fold(0u32, |v, k| v | (m.read_u8_pub(s + i * 4 + k) as u32) << (k * 8))
    };
    let (w, h) = (campo(1) as u64, campo(2) as u64);
    let pixel = move |x: u64, y: u64| -> u32 {
        let i = s + SUP_CABECERA + (y * w + x) * 4;
        (0..4).fold(0u32, |v, k| v | (m.read_u8_pub(i + k) as u32) << (k * 8))
    };
    (w, h, pixel)
}

fn cuenta(pixel: &dyn Fn(u64, u64) -> u32, w: u64, y0: u64, y1: u64, color: u32) -> usize {
    let mut n = 0;
    for y in y0..y1 {
        for x in 0..w {
            if pixel(x, y) & 0x00FF_FFFF == color {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn con_la_lamina_de_ejemplo_pinta_la_pagina_en_640x400() {
    let m = con_lamina(&lamina_ejemplo(), &[]);
    assert_eq!(m.ofertas.len(), 1, "ofrecio UNA superficie");
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (640, 400), "la ventana de la lamina, no la del mensaje");
    // La CAJA de fondo de example.com, y no el fondo oscuro del mensaje.
    assert_eq!(pixel(0, 0) & 0x00FF_FFFF, 0x00EE_EEEE, "el fondo de la lamina");
    assert_eq!(pixel(639, 399) & 0x00FF_FFFF, 0x00EE_EEEE, "hasta la ultima esquina");
    // El titulo "Example Domain" a escala 2 empieza en (128, 121): 16 filas
    // de glifo x 2 = 32 filas con tinta negra.
    let titulo = cuenta(&pixel, w, 121, 153, 0x0000_0000);
    assert!(titulo > 300, "el titulo a escala 2 tiene tinta: {} pixeles", titulo);
    // El parrafo a escala 1 en 169..217.
    let parrafo = cuenta(&pixel, w, 169, 217, 0x0000_0000);
    assert!(parrafo > 300, "el parrafo tiene tinta: {} pixeles", parrafo);
    // "Learn more" en azul (334488) en la fila 231.
    let enlace = cuenta(&pixel, w, 231, 247, 0x0033_4488);
    assert!(enlace > 50, "el enlace, en su color: {} pixeles", enlace);
    // Y por encima del titulo no hay tinta: la lamina no se desplazo sola.
    assert_eq!(cuenta(&pixel, w, 0, 121, 0x0000_0000), 0, "nada negro antes del titulo");
    assert_eq!(dice(&m), "", "con ventana no escribe por consola");
    assert_eq!(salida(&m), Some(0), "se cerro limpia con la q");
}

/// N3: la lamina llega PRESTADA por el ANTENISTA (`op_tomar`), no del disco.
/// En el disco se deja otra distinta para saber cual pinto.
fn con_prestamo(prestada: &[u8], disco: &[u8]) -> Machine {
    use bmo_abi::syscalls::surface::SUP_EV_CARACTER;
    let e = emitido(&fuente());
    let mut m = maquina(&e);
    m.padre = 7;
    m.prestamo_pendiente = Some(prestada.to_vec());
    m.poner_archivo("datos/ejemplo.lam", disco);
    m.buzon_pendiente.push_back(SUP_EV_CARACTER | 0x100 | b'q' as u64);
    run(m, 400_000_000)
}

#[test]
fn la_lamina_ofrecida_por_el_antenista_se_pinta_sin_tocar_el_disco() {
    // En el disco, una lamina ROJA; prestada, la de example.com.
    let roja = b"LAMINA 640 400 1\nCAJA 0 0 640 400 ff0000\n";
    let m = con_prestamo(&lamina_ejemplo(), roja);
    assert_eq!(m.ofertas.len(), 1, "ofrecio UNA superficie");
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (640, 400));
    assert_eq!(pixel(0, 0) & 0x00FF_FFFF, 0x00EE_EEEE, "el fondo de la PRESTADA, no la roja del disco");
    let titulo = cuenta(&pixel, w, 121, 153, 0x0000_0000);
    assert!(titulo > 300, "el titulo de example.com, desde la memoria prestada: {}", titulo);
    assert_eq!(cuenta(&pixel, w, 0, 400, 0x00FF_0000), 0, "ni un pixel rojo: el disco no se leyo");
    assert_eq!(salida(&m), Some(0));
}

#[test]
fn sin_oferta_va_al_disco_como_la_version_2() {
    // Sin prestamo pendiente: `op_tomar` contesta 0 ocho veces y se lee el disco.
    let roja = b"LAMINA 640 400 1\nCAJA 0 0 640 400 ff0000\n";
    let m = con_lamina(roja, &[]);
    let (w, _, pixel) = pantalla(&m);
    assert_eq!(pixel(0, 0) & 0x00FF_FFFF, 0x00FF_0000, "la del disco");
    assert_eq!(cuenta(&pixel, w, 0, 400, 0x00FF_0000), 640 * 400, "entera");
    assert_eq!(salida(&m), Some(0));
}

#[test]
fn una_lamina_prestada_y_mal_hecha_se_niega_igual() {
    let mala = b"LAMINA 640 800 2\nCAJA 0 0 640 800 eeeeee\nCAJA 600 0 100 10 ff0000\n";
    let m = con_prestamo(mala, &lamina_ejemplo());
    let (w, _, pixel) = pantalla(&m);
    // El rechazo va encima, en naranja (AVISO), como con la del disco.
    let naranja = cuenta(&pixel, w, 8, 24, 0x00FF_AA00);
    assert!(naranja > 100, "el aviso en naranja: {} pixeles", naranja);
    assert_eq!(cuenta(&pixel, w, 0, 400, 0x00FF_0000), 0, "la caja de fuera no se pinta");
    assert_eq!(salida(&m), Some(0));
}

#[test]
fn la_flecha_abajo_desplaza_la_lamina_32_pixeles() {
    // Una tecla cruda pulsada: sin bit alto, bit 8 (hay), bit 9 (pulsada),
    // scancode Set 1 de la flecha abajo en el byte bajo.
    let abajo = 0x100 | 0x200 | 0x50;
    let m = con_lamina(&lamina_ejemplo(), &[abajo]);
    let (w, _, pixel) = pantalla(&m);
    // El titulo estaba en 121..153; ahora en 89..121, y en 121..153 queda
    // solo lo que baja detras (el hueco entre titulo y parrafo, sin tinta
    // hasta 169-32 = 137).
    assert!(cuenta(&pixel, w, 89, 121, 0x0000_0000) > 300, "el titulo subio 32 pixeles");
    assert_eq!(cuenta(&pixel, w, 121, 137, 0x0000_0000), 0, "y debajo del titulo ya no hay titulo");
    assert_eq!(salida(&m), Some(0));
}

#[test]
fn una_lamina_con_una_caja_fuera_se_niega_con_su_nombre() {
    // La caja de la linea 3 se sale por la derecha: 600 + 100 > 640.
    let mala = b"LAMINA 640 800 2\nCAJA 0 0 640 800 eeeeee\nCAJA 600 0 100 10 ff0000\n";
    let m = con_lamina(mala, &[]);
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (640, 400));
    // El aviso va ENCIMA: las 40 primeras filas son el fondo oscuro con el
    // texto en naranja (AVISO) y gris (TEXTO); debajo queda lo que llego a
    // pintarse (la caja de fondo).
    assert_eq!(pixel(0, 0) & 0x00FF_FFFF, 0x0014_1414, "el fondo del aviso");
    let naranja = cuenta(&pixel, w, 8, 24, 0x00FF_AA00);
    assert!(naranja > 100, "el aviso en naranja: {} pixeles", naranja);
    assert_eq!(pixel(0, 60) & 0x00FF_FFFF, 0x00EE_EEEE, "debajo, lo que se pinto antes del rechazo");
    // Y nada rojo: la caja que se salia no se pinto.
    assert_eq!(cuenta(&pixel, w, 0, 400, 0x00FF_0000), 0, "la caja de fuera no se pinta");
    assert_eq!(salida(&m), Some(0));
}

#[test]
fn una_lamina_con_un_color_malo_se_niega_en_su_linea() {
    let mala = b"LAMINA 640 800 2\nCAJA 0 0 640 800 eeeeee\nTEXTO 10 10 1 zz0000 hola\n";
    let m = con_lamina(mala, &[]);
    let (w, _, pixel) = pantalla(&m);
    let naranja = cuenta(&pixel, w, 8, 24, 0x00FF_AA00);
    assert!(naranja > 100, "el aviso en naranja: {} pixeles", naranja);
    // "hola" no se pinto: nada negro en su fila.
    assert_eq!(cuenta(&pixel, w, 40, 60, 0x0000_0000), 0);
}

