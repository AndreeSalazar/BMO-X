//! **El contrato que cruza de C a Rust, comprobado.**
//!
//! `toolchain/forge/sem-asm/tables/bmo/bmo.h` es la cara en C de esta misma
//! superficie: lo que un programa escrito en BMO C usa para hablar con el
//! kernel. Los numeros son **los mismos**; los nombres, no.
//!
//! ```text
//!    BMO_OP_CONSOLA_ESCRIBIR  0x06   <-  el .h, que lee el compilador de C
//!    TASK_OP_CONSOLE_WRITE    0x06   <-  el ABI, que lee el kernel
//! ```
//!
//! # Por que hacia falta, y por que nadie lo vigilaba
//!
//! Entre estos dos ficheros **no hay compilador**. Rust no ve el `.h` y el
//! preprocesador de C no ve el ABI, asi que cambiar un numero en un lado deja
//! dos verdades para la misma operacion **sin que nada falle al compilar**. El
//! sintoma llegaria en metal, como un programa de C que pide una cosa y recibe
//! otra.
//!
//! Es el mismo agujero que el 2026-08-12 dejo cuatro `TASK_OP_*` fuera del ABI
//! durante semanas -- pero peor, porque alli al menos los dos lados eran Rust.
//!
//! # ** LA REGLA QUE HACE QUE ESTA LISTA NO SE CONGELE
//!
//! El diccionario de abajo es a mano, y **tiene que serlo**: `BMO_OP_CEDER` y
//! `TASK_OP_YIELD` son el mismo numero con dos nombres, y eso no se deduce.
//!
//! Lo que NO es a mano es la cobertura: **la prueba lee el `.h`, y falla si
//! encuentra un `#define BMO_*` que el diccionario no menciona.** O sea que
//! anadir una constante al `.h` sin declarar su pareja rompe el banco.
//!
//! Esa inversion es la leccion del guardian de operaciones, que llevaba una
//! lista de 34 nombres y se le habian escapado cuatro:
//!
//! > Un guardian con lista tiene el mismo fallo que vigila -- **salvo que la
//! > fuente de la verdad sea el otro fichero y la lista solo sirva para
//! > traducir**.
//!
//! # Por que es una prueba y no un paso de `build.ps1`
//!
//! Porque asi corre en los dos sitios: en `cargo test` de quien acaba de clonar
//! y en el banco del build, que ya ejecuta este crate. Un guardian que solo vive
//! en el guion de despliegue no protege a quien todavia no despliega.

use std::path::Path;

/// La pareja: `(nombre en el .h, nombre en el ABI, valor)`.
///
/// El valor se escribe aqui **a proposito**, aunque parezca redundante: si
/// alguien cambia los dos ficheros a la vez y se equivoca en los dos igual, la
/// unica forma de cazarlo es que haya un tercer sitio que diga cual era. Es el
/// mismo motivo por el que el perfil del CPU declara lo que espera del silicio.
const PAREJAS: &[(&str, &str, u64)] = &[
    // Las tres puertas.
    ("BMO_INVOKE", "NR_INVOKE", 0x00),
    ("BMO_CHANNEL_KICK", "NR_CHANNEL_KICK", 0x01),
    ("BMO_WAIT", "NR_WAIT", 0x02),
    ("BMO_TAREA_ACTUAL", "CURRENT_TASK", 0xFFFF_FFFF_FFFF_FFFE),
    // Quien soy y que hago.
    ("BMO_OP_PID", "TASK_OP_GET_PID", 0x01),
    ("BMO_OP_TID", "TASK_OP_GET_TID", 0x02),
    ("BMO_OP_CEDER", "TASK_OP_YIELD", 0x03),
    ("BMO_OP_SALIR", "TASK_OP_EXIT", 0x04),
    // La consola, en los dos sentidos.
    ("BMO_OP_CONSOLA_ESCRIBIR", "TASK_OP_CONSOLE_WRITE", 0x06),
    ("BMO_OP_CONSOLA_LEER", "TASK_OP_CONSOLE_READ", 0x0F),
    // Lo que se reclama en exclusiva.
    ("BMO_OP_PANTALLA_RECLAMAR", "TASK_OP_FRAMEBUFFER_CLAIM", 0x09),
    ("BMO_OP_ENTRADA_RECLAMAR", "TASK_OP_INPUT_CLAIM", 0x0A),
    ("BMO_OP_SONIDO_RECLAMAR", "TASK_OP_AUDIO_CLAIM", 0x21),
    ("BMO_OP_SONIDO_SOLTAR", "TASK_OP_AUDIO_RELEASE", 0x22),
    // Ficheros y lanzamiento.
    ("BMO_OP_RUTA", "TASK_OP_RUTA", 0x0B),
    ("BMO_OP_EJECUTAR", "TASK_OP_EJECUTAR", 0x0C),
    ("BMO_OP_ARCHIVO_ABRIR", "TASK_OP_ARCHIVO_ABRIR", 0x10),
    ("BMO_OP_ARCHIVO_CREAR", "TASK_OP_ARCHIVO_CREAR", 0x11),
    // Preguntar por el sistema.
    ("BMO_OP_INFO", "TASK_OP_INFO", 0x13),
    ("BMO_INFO_RAM_TOTAL", "INFO_RAM_TOTAL", 0x01),
    ("BMO_INFO_RAM_LIBRE", "INFO_RAM_LIBRE", 0x02),
    ("BMO_INFO_TSC_HZ", "INFO_TSC_HZ", 0x05),
    ("BMO_INFO_NET_PRESENTE", "INFO_NET_PRESENTE", 0x27),
    ("BMO_INFO_NET_VENDOR_DEVICE", "INFO_NET_VENDOR_DEVICE", 0x28),
    ("BMO_INFO_NET_MAC", "INFO_NET_MAC", 0x29),
    ("BMO_INFO_NET_PHY_CRUDO", "INFO_NET_PHY_CRUDO", 0x2A),
    ("BMO_INFO_NET_MEGABITS", "INFO_NET_MEGABITS", 0x2B),
    ("BMO_INFO_NET_RX_ARMADO", "INFO_NET_RX_ARMADO", 0x2C),
    ("BMO_INFO_NET_RX_TRAMAS", "INFO_NET_RX_TRAMAS", 0x2D),
    ("BMO_INFO_NET_PCI", "INFO_NET_PCI", 0x2E),
    // El metro de la puerta. Cruza porque quien lo lee es un programa de C
    // (`c/coste.bex`) y el que lo sirve es el kernel: dos lados, un numero.
    ("BMO_INFO_SYSCALL_CUENTA", "INFO_SYSCALL_CUENTA", 0x2F),
    ("BMO_INFO_SYSCALL_CICLOS", "INFO_SYSCALL_CICLOS", 0x30),
    // Y el reparto dentro del stub, por el mismo motivo y con mas razon: estas
    // dos las escribe el ENSAMBLADOR de `entry.rs` y las lee un `.bex` de C.
    ("BMO_INFO_SYSCALL_CICLOS_GUARDA", "INFO_SYSCALL_CICLOS_GUARDA", 0x35),
    ("BMO_INFO_SYSCALL_CICLOS_RESTAURA", "INFO_SYSCALL_CICLOS_RESTAURA", 0x36),
    // El presupuesto. Cruza porque lo DECLARA el kernel y lo JUZGA un `.bex`
    // de C: si los dos lados no leen el mismo campo, el veredicto es de otra
    // fila y nadie se entera.
    ("BMO_INFO_PRESUPUESTO_PUERTA", "INFO_PRESUPUESTO_PUERTA", 0x37),
    ("BMO_INFO_PRESUPUESTO_DISPATCH", "INFO_PRESUPUESTO_DISPATCH", 0x38),
    ("BMO_INFO_PRESUPUESTO_HANDLE", "INFO_PRESUPUESTO_HANDLE", 0x39),
    // Un presupuesto tiene DUENO: la maquina en que se midio. Si el silicio no
    // cuadra, las tres filas de arriba contestan cero y el juez se calla. Y el
    // suelo del cruce es lo que permite separar el merito de BMO del coste del
    // CPU. Ver R-CPU8 y R-CPU10.
    ("BMO_INFO_PRESUPUESTO_MAQUINA", "INFO_PRESUPUESTO_MAQUINA", 0x3D),
    ("BMO_INFO_SUELO_CRUCE", "INFO_SUELO_CRUCE", 0x3E),
    // El censo de extensiones. Cruza por el mismo motivo que el metro: lo
    // sirve el kernel y lo leen dos lados distintos -- el escritorio en Rust y
    // cualquier `.bex` de C que quiera saber si tiene RDRAND antes de usarlo.
    ("BMO_INFO_CPU_EXT_N", "INFO_CPU_EXT_N", 0x31),
    ("BMO_INFO_CPU_EXT_HAY", "INFO_CPU_EXT_HAY", 0x32),
    ("BMO_INFO_CPU_EXT_USA", "INFO_CPU_EXT_USA", 0x33),
    ("BMO_INFO_CPU_EXT_AVERIAS", "INFO_CPU_EXT_AVERIAS", 0x34),
    ("BMO_INFO_TXT_EXT_NOMBRE", "INFO_TXT_EXT_NOMBRE", 0x05),
    ("BMO_INFO_TXT_EXT_NOTA", "INFO_TXT_EXT_NOTA", 0x06),
    ("BMO_INFO_CPU_HILOS", "INFO_CPU_HILOS", 0x06),
    ("BMO_INFO_CPU_NUCLEOS", "INFO_CPU_NUCLEOS", 0x07),
    ("BMO_INFO_TICKS", "INFO_TICKS", 0x0B),
    // A que velocidad va el nucleo DE VERDAD, y lo que gasta. El kernel sabia
    // medirlo y ningun programa de C podia preguntarlo: las filas existian en
    // Rust y no cruzaban al `.h`.
    ("BMO_INFO_CPU_HZ_REAL", "INFO_CPU_HZ_REAL", 0x20),
    ("BMO_INFO_CPU_MW_PAQUETE", "INFO_CPU_MW_PAQUETE", 0x21),
    ("BMO_INFO_CPU_MW_NUCLEO_ACTUAL", "INFO_CPU_MW_NUCLEO_ACTUAL", 0x22),
    ("BMO_INFO_CPU_SENSORES", "INFO_CPU_SENSORES", 0x23),
    ("BMO_INFO_SMP_VIVOS", "INFO_SMP_VIVOS", 0x1B),
];

/// `#define`s del `.h` que NO son parte del contrato y por eso no tienen pareja.
///
/// Uno solo, y es el centinela de doble inclusion. Se declara aqui en vez de
/// filtrarse con una heuristica --*"los que acaban en `_H`"*-- porque una
/// heuristica es una regla que nadie escribio: el dia que alguien anada
/// `BMO_ALGO_H` que SI sea del contrato, una heuristica lo perdonaria y esta
/// lista lo caza.
const NO_SON_CONTRATO: &[&str] = &["BMO_BMO_H"];

fn ruta_del_h() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../toolchain/forge/sem-asm/tables/bmo/bmo.h")
}

/// Todos los `#define BMO_x valor` del fichero, con su valor ya en numero.
fn defines_del_h(texto: &str) -> Vec<(String, Option<u64>)> {
    let mut v = Vec::new();
    for linea in texto.lines() {
        let l = linea.trim();
        let Some(resto) = l.strip_prefix("#define ") else { continue };
        let mut trozos = resto.split_whitespace();
        let Some(nombre) = trozos.next() else { continue };
        if !nombre.starts_with("BMO_") {
            continue;
        }
        let valor = trozos.next().and_then(|s| {
            let s = s.trim();
            if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                u64::from_str_radix(hex, 16).ok()
            } else {
                s.parse::<u64>().ok()
            }
        });
        v.push((nombre.to_string(), valor));
    }
    v
}

/// ** LOS NUMEROS DE LAS DOS CARAS SON EL MISMO.
///
/// Si esto falla, un programa de BMO C esta pidiendo una operacion y el kernel
/// esta ejecutando otra -- y el sintoma llega en metal, no aqui.
#[test]
fn el_h_y_el_abi_dicen_el_mismo_numero() {
    let texto = std::fs::read_to_string(ruta_del_h()).expect("bmo.h tiene que estar donde dice");
    let del_h: std::collections::HashMap<_, _> = defines_del_h(&texto).into_iter().collect();

    let mut malas = Vec::new();
    for (en_c, en_rust, esperado) in PAREJAS {
        match del_h.get(*en_c) {
            None => malas.push(format!("{en_c} ({en_rust}) ya no esta en bmo.h")),
            Some(None) => malas.push(format!("{en_c} no tiene un valor que se pueda leer")),
            Some(Some(v)) if v != esperado => {
                malas.push(format!("{en_c} vale {v:#x} en el .h y el contrato dice {esperado:#x}"))
            }
            Some(Some(_)) => {}
        }
    }
    assert!(malas.is_empty(), "el .h y el ABI se han separado:\n  {}", malas.join("\n  "));
}

/// ** Y LO MISMO CONTRA EL ABI DE VERDAD, no contra el numero escrito arriba.
///
/// Sin esta prueba, la tabla de [`PAREJAS`] seria una tercera verdad que se
/// puede quedar vieja igual que las otras dos. Aqui se compara contra las
/// constantes REALES de este crate.
#[test]
fn las_parejas_dicen_lo_que_el_abi_dice() {
    use bmo_abi::syscalls::surface as s;
    // Se comprueban las que se pueden nombrar como constante; el resto ya queda
    // cubierto por el fichero del ABI, que es de donde salieron.
    let reales: &[(&str, u64)] = &[
        ("BMO_INVOKE", s::NR_INVOKE as u64),
        ("BMO_CHANNEL_KICK", s::NR_CHANNEL_KICK as u64),
        ("BMO_WAIT", s::NR_WAIT as u64),
        ("BMO_TAREA_ACTUAL", s::CURRENT_TASK),
        ("BMO_OP_PID", s::TASK_OP_GET_PID),
        ("BMO_OP_TID", s::TASK_OP_GET_TID),
        ("BMO_OP_CEDER", s::TASK_OP_YIELD),
        ("BMO_OP_SALIR", s::TASK_OP_EXIT),
        ("BMO_OP_CONSOLA_ESCRIBIR", s::TASK_OP_CONSOLE_WRITE),
        ("BMO_OP_CONSOLA_LEER", s::TASK_OP_CONSOLE_READ),
        ("BMO_OP_PANTALLA_RECLAMAR", s::TASK_OP_FRAMEBUFFER_CLAIM),
        ("BMO_OP_ENTRADA_RECLAMAR", s::TASK_OP_INPUT_CLAIM),
        ("BMO_OP_SONIDO_RECLAMAR", s::TASK_OP_AUDIO_CLAIM),
        ("BMO_OP_SONIDO_SOLTAR", s::TASK_OP_AUDIO_RELEASE),
        ("BMO_OP_RUTA", s::TASK_OP_RUTA),
        ("BMO_OP_EJECUTAR", s::TASK_OP_EJECUTAR),
        ("BMO_OP_ARCHIVO_ABRIR", s::TASK_OP_ARCHIVO_ABRIR),
        ("BMO_OP_ARCHIVO_CREAR", s::TASK_OP_ARCHIVO_CREAR),
        ("BMO_OP_INFO", s::TASK_OP_INFO),
        ("BMO_INFO_RAM_TOTAL", s::INFO_RAM_TOTAL),
        ("BMO_INFO_RAM_LIBRE", s::INFO_RAM_LIBRE),
        ("BMO_INFO_TSC_HZ", s::INFO_TSC_HZ),
        ("BMO_INFO_NET_PRESENTE", s::INFO_NET_PRESENTE),
        ("BMO_INFO_NET_VENDOR_DEVICE", s::INFO_NET_VENDOR_DEVICE),
        ("BMO_INFO_NET_MAC", s::INFO_NET_MAC),
        ("BMO_INFO_NET_PHY_CRUDO", s::INFO_NET_PHY_CRUDO),
        ("BMO_INFO_NET_MEGABITS", s::INFO_NET_MEGABITS),
        ("BMO_INFO_NET_RX_ARMADO", s::INFO_NET_RX_ARMADO),
        ("BMO_INFO_NET_RX_TRAMAS", s::INFO_NET_RX_TRAMAS),
        ("BMO_INFO_NET_PCI", s::INFO_NET_PCI),
        ("BMO_INFO_SYSCALL_CUENTA", s::INFO_SYSCALL_CUENTA),
        ("BMO_INFO_SYSCALL_CICLOS", s::INFO_SYSCALL_CICLOS),
        ("BMO_INFO_SYSCALL_CICLOS_GUARDA", s::INFO_SYSCALL_CICLOS_GUARDA),
        ("BMO_INFO_SYSCALL_CICLOS_RESTAURA", s::INFO_SYSCALL_CICLOS_RESTAURA),
        ("BMO_INFO_PRESUPUESTO_PUERTA", s::INFO_PRESUPUESTO_PUERTA),
        ("BMO_INFO_PRESUPUESTO_DISPATCH", s::INFO_PRESUPUESTO_DISPATCH),
        ("BMO_INFO_PRESUPUESTO_HANDLE", s::INFO_PRESUPUESTO_HANDLE),
        ("BMO_INFO_PRESUPUESTO_MAQUINA", s::INFO_PRESUPUESTO_MAQUINA),
        ("BMO_INFO_SUELO_CRUCE", s::INFO_SUELO_CRUCE),
        ("BMO_INFO_CPU_EXT_N", s::INFO_CPU_EXT_N),
        ("BMO_INFO_CPU_EXT_HAY", s::INFO_CPU_EXT_HAY),
        ("BMO_INFO_CPU_EXT_USA", s::INFO_CPU_EXT_USA),
        ("BMO_INFO_CPU_EXT_AVERIAS", s::INFO_CPU_EXT_AVERIAS),
        ("BMO_INFO_TXT_EXT_NOMBRE", s::INFO_TXT_EXT_NOMBRE),
        ("BMO_INFO_TXT_EXT_NOTA", s::INFO_TXT_EXT_NOTA),
        ("BMO_INFO_CPU_HILOS", s::INFO_CPU_HILOS),
        ("BMO_INFO_CPU_NUCLEOS", s::INFO_CPU_NUCLEOS),
        ("BMO_INFO_TICKS", s::INFO_TICKS),
    ];
    for (en_c, valor_real) in reales {
        let (_, _, escrito) = PAREJAS
            .iter()
            .find(|(c, _, _)| c == en_c)
            .unwrap_or_else(|| panic!("{en_c} no esta en PAREJAS"));
        assert_eq!(
            escrito, valor_real,
            "{en_c}: la tabla de parejas dice {escrito:#x} y el ABI dice {valor_real:#x}"
        );
    }
}

/// ** LA PRUEBA QUE IMPIDE QUE ESTA LISTA SE CONGELE.
///
/// Si aparece un `#define BMO_*` en el `.h` que nadie ha emparejado, esto falla.
/// O sea que **no se puede anadir una constante al lado de C sin declarar cual
/// es su pareja en Rust**, que es exactamente lo que llevaba semanas pasando con
/// las cuatro operaciones que el ABI no tenia.
///
/// Es la inversion que hace util a un guardian con lista: la fuente de la verdad
/// es el OTRO fichero, y la lista solo sirve para traducir.
#[test]
fn ningun_define_del_h_se_queda_sin_pareja() {
    let texto = std::fs::read_to_string(ruta_del_h()).expect("bmo.h tiene que estar donde dice");
    let huerfanos: Vec<String> = defines_del_h(&texto)
        .into_iter()
        .map(|(n, _)| n)
        .filter(|n| !NO_SON_CONTRATO.contains(&n.as_str()))
        .filter(|n| !PAREJAS.iter().any(|(c, _, _)| c == n))
        .collect();
    assert!(
        huerfanos.is_empty(),
        "bmo.h tiene constantes sin pareja en el ABI. Anade su fila a PAREJAS \
         --o a NO_SON_CONTRATO si de verdad no cruzan-- porque entre el .h y \
         Rust no hay compilador que las cace:\n  {}",
        huerfanos.join("\n  ")
    );
}

/// El `.h` tiene que existir donde se dice. Si alguien lo mueve, esta prueba lo
/// dice con su ruta en vez de fallar con un error de fichero a medio camino.
#[test]
fn el_h_esta_donde_este_contrato_cree() {
    let r = ruta_del_h();
    assert!(r.exists(), "no esta bmo.h en {}", r.display());
}
