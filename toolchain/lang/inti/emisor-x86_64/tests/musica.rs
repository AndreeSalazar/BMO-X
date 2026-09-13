//! **EL REPRODUCTOR DE INTI: ejecutado, por el altavoz y por el tubo.**
//!
//! Peticion de Eddi (2026-09-13): *"Inti vamos a construir TODO app y
//! reproductor de musica"*.
//!
//! ## Lo que se comprueba, y por que asi
//!
//! Las frecuencias y las duraciones que tienen que salir estan escritas a mano,
//! sacadas de `<bmo/musica.h>` -- la tabla que ya usa `c/vivaldi.bex`. Si el
//! reproductor de INTI y la cabecera de C discreparan, sonarian distinto dos
//! programas que tocan la misma partitura.
//!
//! *** Y el tubo se prueba MIRANDO LAS MUESTRAS: los bytes que el programa
//! dejo en el bloque que ofrecio. Un reproductor que ofrece el bloque y anuncia
//! bytes escritos sin escribirlos pasaria cualquier prueba que solo mirara las
//! cuentas -- y en el audifono sonaria a silencio.

use std::path::PathBuf;

use bmo_lower::emu::{run, Machine};

fn fuente() -> String {
    std::fs::read_to_string(PathBuf::from("../ejemplos/musica.inti"))
        .expect("no encuentro `ejemplos/musica.inti`")
}

fn emitido(texto: &str) -> bmo_inti_x86_64::Emitido {
    let arbol = bmo_inti_front::armar(texto);
    assert!(!arbol.hay_errores(), "el programa no se lee: {}", arbol.pintar("musica.inti"));
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

/// Prepara la maquina: la melodia en `datos/tema.mus` si se da, y lo demas lo
/// siembra quien llama.
fn maquina(tema: Option<&[u8]>) -> Machine {
    let e = emitido(&fuente());
    assert!(e.arranca, "el reproductor tiene `principal`");
    let mut m = Machine::new(e.codigo);
    if let Some(t) = tema {
        m.poner_archivo("datos/tema.mus", t);
    }
    m
}

fn toca(tema: &[u8]) -> Machine {
    run(maquina(Some(tema)), 50_000_000)
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

/// *** EL COMPILADOR DE VERDAD LO ACEPTA, no solo la tuberia de esta prueba.
///
/// Las demas pruebas bajan el arbol a mano y se saltan las comprobaciones que
/// `inti` hace antes de escribir el `.ibx`. El 2026-09-13 las nueve pasaban y el
/// build decia `no compilo musica.inti`: una funcion se llamaba `numero`, que
/// en el catalogo es una operacion que PUEDE FALLAR (E0060). Sin esta prueba el
/// banco estaba verde con un programa que no llegaba al disco.
#[test]
fn el_compilador_de_verdad_lo_acepta() {
    let salida = std::env::temp_dir().join("bmo_prueba_musica.ibx");
    let s = std::process::Command::new(env!("CARGO_BIN_EXE_inti"))
        .args(["../ejemplos/musica.inti", "-o"])
        .arg(&salida)
        .output()
        .expect("no puedo ejecutar el compilador");
    assert!(
        s.status.success(),
        "inti rechazo musica.inti:\n{}",
        String::from_utf8_lossy(&s.stderr)
    );
    assert!(salida.exists(), "dijo que si y no escribio el .ibx");
}

/// ** EL ALTAVOZ, pitido a pitido: la nota suena el 85% de su figura y calla el
/// resto, y lo de mas de 250 ms va a trozos. El `(0, 0)` del final es el
/// `callar` de antes de soltar el aparato.
#[test]
fn el_altavoz_toca_la_partitura_con_su_hueco() {
    let m = toca(b"T120 LA4 -:2 DO5:2\n");
    assert_eq!(
        m.partitura(),
        &[(440, 250), (440, 175), (0, 75), (0, 250), (523, 212), (0, 38), (0, 0)][..],
        "LA4 negra a 120 = 500 ms (425 + 75); silencio de corchea; DO5 corchea"
    );
    let d = dice(&m);
    assert!(d.contains("altavoz "), "sin audifono dice que va por el altavoz: {:?}", d);
    assert!(d.contains("fin     0"), "{:?}", d);
}

/// Sostenido, bemol y el salto de octava: `SI#4` es `DO5`, y `DOB5` es `SI4`.
#[test]
fn los_accidentes_cruzan_la_octava() {
    let m = toca(b"SI#4:1 DOB5:1 SOL#5:1 SIB3:1");
    let notas: Vec<u64> = m.partitura().iter().filter(|p| p.0 != 0).map(|p| p.0).collect();
    assert_eq!(notas, vec![523, 494, 831, 233]);
}

/// ** LOS ARGUMENTOS eligen la melodia: la ruta la trae el kernel, no el programa.
#[test]
fn los_argumentos_eligen_la_melodia() {
    let mut m = maquina(None);
    m.poner_argumentos("datos/otra.mus");
    m.poner_archivo("datos/otra.mus", b"SOL4:1");
    let m = run(m, 50_000_000);
    assert_eq!(m.partitura(), &[(392, 106), (0, 19), (0, 0)][..]);
}

#[test]
fn sin_fichero_dice_1_y_no_toca_nada() {
    let m = run(maquina(None), 50_000_000);
    assert!(dice(&m).contains("fin     1"), "{:?}", dice(&m));
    assert!(m.partitura().is_empty(), "sin melodia ni siquiera reclama el sonido");
}

/// *** UNA PARTITURA ROTA SE PARA, y dice EN QUE BYTE.
///
/// Lo que ya sono, sono; lo de detras no se adivina. `T120 LA4 ` son 9 bytes.
#[test]
fn una_partitura_rota_se_para_y_dice_donde() {
    let m = toca(b"T120 LA4 XX4 DO5");
    let d = dice(&m);
    assert!(d.contains("mal en  9"), "{:?}", d);
    assert!(d.contains("fin     5"), "{:?}", d);
    assert_eq!(m.partitura(), &[(440, 250), (440, 175), (0, 75), (0, 0)][..]);
}

/// Cada forma de escribirlo mal, y ninguna suena.
#[test]
fn las_formas_de_escribirlo_mal() {
    for mal in ["LA9", "LA4:0", "LA4:65", "LA4X", "T", "DOB0", "SI#8", "LA", "LA4:"] {
        let m = toca(mal.as_bytes());
        assert!(dice(&m).contains("fin     5"), "`{}` tenia que estar mal: {:?}", mal, dice(&m));
        assert_eq!(m.partitura(), &[(0, 0)][..], "`{}` no puede sonar", mal);
    }
}

/// Los comentarios y los blancos no son notas.
#[test]
fn los_comentarios_no_suenan() {
    let m = toca(b"; LA4 no\n\n  T120 ; ni esto\nLA4:1\t\r\n");
    assert_eq!(m.partitura(), &[(440, 106), (0, 19), (0, 0)][..]);
}

/// El tema que va al disco se toca entero y sin un error.
#[test]
fn el_tema_del_disco_se_toca_entero() {
    let tema = std::fs::read("../ejemplos/tema.mus").expect("no encuentro `ejemplos/tema.mus`");
    let m = toca(&tema);
    let d = dice(&m);
    assert!(d.contains("fin     0"), "{:?}", d);
    let notas = m.partitura().iter().filter(|p| p.0 != 0).count();
    assert!(notas >= 48, "el ritornello dos veces: {} pitidos", notas);
    // MI5 negra a 132: 454 ms, suenan 385.
    assert_eq!(&m.partitura()[..3], &[(659, 250), (659, 135), (0, 69)][..]);
}

/// *** CON AUDIFONO: muestras de verdad en el bloque ofrecido.
///
/// `T150 LA4:1` son 100 ms: 4.800 muestras de dos canales de 16 bits, 19.200
/// bytes. Suena el 85% (4.080 muestras) a +-3000, y el resto es cero.
#[test]
fn con_tubo_suenan_muestras_de_verdad() {
    let mut m = maquina(Some(b"T150 LA4:1"));
    m.poner_tubo(48_000, 192);
    let m = run(m, 50_000_000);
    let d = dice(&m);
    assert!(d.contains("tubo USB"), "{:?}", d);
    assert!(d.contains("fin     0"), "{:?}", d);
    let (ofrecido, escrito, armado) = m.tubo_estado();
    assert_eq!(escrito, 19_200, "4.800 muestras x 4 bytes");
    assert!(!armado, "lo armo el, y lo desarma al acabar");
    assert_eq!(m.partitura(), &[(0, 0)][..], "ni un pitido: todo va por el tubo");

    let base = ofrecido.expect("nunca ofrecio el bloque");
    let byte = |i: u64| m.read_u8_pub(base + i);
    // Muestra 0: primera media onda, +3000 = 0x0BB8, en los dos canales.
    assert_eq!([byte(0), byte(1), byte(2), byte(3)], [0xB8, 0x0B, 0xB8, 0x0B]);
    // Muestra 55: 55 x 880 / 48000 = 1 -> segunda media onda, -3000 = 0xF448.
    assert_eq!([byte(220), byte(221)], [0x48, 0xF4]);
    // Muestra 4.080: ya en el hueco de articulacion.
    assert_eq!([byte(16_320), byte(16_321)], [0, 0]);
}
