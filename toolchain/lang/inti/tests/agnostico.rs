//! INTI es AGNOSTICO, y aqui se comprueba en vez de prometerse.
//!
//! ## Por que este fichero existe
//!
//! Regla de Eddi, 2026-08-19: *"INTI es agnostico, entonces mis lenguajes son
//! agnosticos, lo mismo con BMO ABI; no va a representar x86-64 exclusivo, si
//! no seria atado"*.
//!
//! El problema de una regla asi es que **se cumple sola el primer dia y se
//! rompe el tercero**, cuando alguien necesita saber el tamano de un puntero
//! para calcular algo y escribe un `8` en el sitio equivocado. No se rompe por
//! descuido: se rompe porque es *lo mas facil* en ese momento, igual que meter
//! el syscall en el compilador.
//!
//! Asi que la regla se vigila con un test, como la codificacion se vigila con
//! `ascii-sweep`.
//!
//! ## La ley
//!
//! > **El frontend de INTI no puede nombrar una maquina.** Ni registros, ni
//! > opcodes, ni anchos de palabra, ni convenciones de llamada.
//!
//! Lo que SI puede saber de la maquina vive en `tables/arch/<arquitectura>/`,
//! que es una carpeta de **datos**. Cambiar de arquitectura es anadir una
//! carpeta, no tocar el compilador -- que es exactamente lo que ya prometio
//! `intrinsics.toml` con *"anadir una instruccion = 1 entrada TOML, CERO
//! Rust"*.
//!
//! ## Lo que este test NO puede probar
//!
//! Que INTI corra en ARM. Eso solo lo prueba un ARM. Lo que prueba es que
//! **nadie ha escrito todavia la linea que lo impediria**, y eso se puede saber
//! hoy y sin hardware.

use std::path::{Path, PathBuf};

/// Palabras que solo significan algo dentro de una maquina concreta.
///
/// La lista es corta a proposito: cada entrada tiene que ser una palabra que
/// **no puede aparecer por casualidad**. `mov` esta fuera porque aparece en
/// "movimiento".
///
/// Y se buscan como PALABRA ENTERA, no como trozo. El primer intento buscaba
/// trozos y acuso a `conversion` de nombrar `rsi`: un test que da falsos
/// positivos se desactiva en una semana, y entonces ya no vigila nada.
const DE_UNA_MAQUINA: &[&str] = &[
    "x86", "amd64", "i386", "aarch64", "riscv", "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rsp",
    "rbp", "xmm", "sysv", "modrm", "opcode", "sse2", "avx",
];

/// `linea` contiene `palabra` como palabra entera.
fn contiene_palabra(linea: &str, palabra: &str) -> bool {
    let bytes = linea.as_bytes();
    let mut desde = 0usize;
    while let Some(pos) = linea[desde..].find(palabra) {
        let i = desde + pos;
        let j = i + palabra.len();
        let antes_pega = i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
        let despues_pega =
            j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_');
        if !antes_pega && !despues_pega {
            return true;
        }
        desde = j;
    }
    false
}

fn fuentes() -> Vec<PathBuf> {
    fn anda(dir: &Path, v: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).expect("no puedo leer src/") {
            let p = e.expect("entrada rara").path();
            if p.is_dir() {
                anda(&p, v);
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                v.push(p);
            }
        }
    }
    let mut v = Vec::new();
    anda(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut v);
    v.sort();
    v
}

/// El frontend no nombra una maquina. Ni una vez, ni en un comentario.
///
/// Los comentarios cuentan **a proposito**: un comentario que explica algo en
/// terminos de `rax` es un comentario que habra que reescribir el dia del
/// puerto, y ademas es la senal de que alguien estaba pensando en x86 mientras
/// escribia una parte que no deberia saber de eso.
#[test]
fn el_frontend_no_nombra_ninguna_maquina() {
    let mut culpables = Vec::new();

    for f in fuentes() {
        let texto = std::fs::read_to_string(&f).expect("no puedo leer");
        let bajo = texto.to_lowercase();
        for (n, linea) in bajo.lines().enumerate() {
            for palabra in DE_UNA_MAQUINA {
                if contiene_palabra(linea, palabra) {
                    culpables.push(format!(
                        "{}:{} nombra `{}`",
                        f.file_name().unwrap().to_string_lossy(),
                        n + 1,
                        palabra
                    ));
                }
            }
        }
    }

    assert!(
        culpables.is_empty(),
        "el frontend de INTI tiene que ser agnostico y aqui hay maquina:\n  {}",
        culpables.join("\n  ")
    );
}

/// Y tampoco supone un tamano de puntero.
///
/// Se busca la forma en que ese supuesto se cuela de verdad: un `8` escrito a
/// mano al lado de la palabra "puntero", o un `usize` usado como si fuera el
/// ancho de la maquina de destino.
///
/// (`usize` sale por todas partes como indice de un `Vec`, que es del
/// compilador y no del programa compilado. Por eso se busca la pareja
/// `puntero`+numero y no `usize` a secas: un test que salta por todo no lo lee
/// nadie.)
#[test]
fn el_frontend_no_supone_un_tamano_de_puntero() {
    let mut culpables = Vec::new();

    for f in fuentes() {
        let texto = std::fs::read_to_string(&f).expect("no puedo leer");
        for (n, linea) in texto.to_lowercase().lines().enumerate() {
            let habla_de_punteros = linea.contains("puntero") || linea.contains("direccion");
            let pone_un_ancho = linea.contains("8 bytes")
                || linea.contains("64 bits")
                || linea.contains("32 bits");
            if habla_de_punteros && pone_un_ancho {
                culpables.push(format!(
                    "{}:{}: {}",
                    f.file_name().unwrap().to_string_lossy(),
                    n + 1,
                    linea.trim()
                ));
            }
        }
    }

    assert!(
        culpables.is_empty(),
        "el ancho de un puntero lo dice la arquitectura, no el frontend:\n  {}",
        culpables.join("\n  ")
    );
}

/// La cara amable de la misma regla: **el arbol sirve para las dos maquinas**.
///
/// Se lee un fuente y se comprueba que en el arbol no aparece nada que solo
/// tenga sentido en un procesador. Es la version positiva, y sirve de ejemplo
/// de lo que se espera de las fases que vengan.
#[test]
fn el_mismo_arbol_vale_para_cualquier_maquina() {
    let fuente = "\
perfil llano
usa metal

funcion lee_tecla devuelve natural8
    crudo
        repite mientras (entrada_puerto(0x64) bits_y 1) = 0
            espera()

        devuelve entrada_puerto(0x60)
";
    let c = bmo_inti_front::leer(fuente);
    assert!(!c.hay_errores(), "{}", c.pintar("tecla.inti"));

    // `0x64` y `entrada_puerto` estan en el programa del USUARIO, dentro de un
    // `crudo` o de una llamada a la biblioteca. El compilador los trata como un
    // numero y un nombre cualesquiera: no sabe que son un puerto de E/S, y por
    // eso el mismo arbol se puede emitir para otra maquina --que dira que ese
    // nombre no existe alli, y tendra razon.
    let texto = format!("{:?}", c.valor);
    let bajo = texto.to_lowercase();
    for palabra in DE_UNA_MAQUINA {
        assert!(
            !contiene_palabra(&bajo, palabra),
            "el arbol nombra `{}`",
            palabra
        );
    }
}
