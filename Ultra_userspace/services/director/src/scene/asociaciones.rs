//! **CON QUE SE ABRE CADA COSA** -- la tabla de tipos (2026-09-13).
//!
//! [consumo] NADA      una tabla y una busqueda: no corre por su cuenta (L6h)
//!
//! Eddi: *"que una app lo encuentre TODO al leer: imagenes, audio, otros ... y
//! si es JPG o PNG que entren en la app de BMO-X"*.
//!
//! ** Es una TABLA y no un `match` repartido por el escritorio. El explorador y
//! la biblioteca preguntan aqui, y manana lo hara el escritorio: anadir un tipo
//! es una fila, y no hay dos sitios que puedan decir dos cosas distintas de un
//! `.mus`.
//!
//! ** Y dice la verdad sobre lo que HOY no se abre. Una imagen no tiene visor
//! todavia: la fila lo dice con su motivo (`Abre::Falta`) en vez de abrir el
//! fichero como texto y ensenar bytes crudos, que se leeria como "el formato
//! esta roto" cuando lo que falta es la app.
//!
//! [!] Ninguna extension pasa de TRES letras: el FAT32 del kernel es 8.3, y un
//! tipo de cuatro letras no llegaria nunca aqui con su nombre (ver `.ibx`).

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Clase {
    App,
    Imagen,
    Audio,
    Texto,
    Otro,
}

impl Clase {
    /// Las que se ensenan en la biblioteca, en su orden. `Otro` no esta a
    /// proposito: una biblioteca que lista lo que no sabe abrir es un `ls`.
    pub(crate) const VISIBLES: [Clase; 4] = [Clase::App, Clase::Imagen, Clase::Audio, Clase::Texto];

    pub(crate) fn nombre(self) -> &'static str {
        match self {
            Clase::App => "apps",
            Clase::Imagen => "imagenes",
            Clase::Audio => "audio",
            Clase::Texto => "texto",
            Clase::Otro => "otro",
        }
    }

    /// La tecla que la filtra. La inicial, que es la que la mano busca.
    pub(crate) fn tecla(self) -> u8 {
        match self {
            Clase::App => b'A',
            Clase::Imagen => b'I',
            Clase::Audio => b'M',
            Clase::Texto => b'T',
            Clase::Otro => b'O',
        }
    }

    /// Un color por clase, el mismo en toda la biblioteca.
    pub(crate) fn color(self) -> u32 {
        match self {
            Clase::App => 0x0060_A5FA,
            Clase::Imagen => 0x00F0_A860,
            Clase::Audio => 0x00C0_84FC,
            Clase::Texto => 0x007E_E787,
            Clase::Otro => 0x0080_8890,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Abre {
    /// Es un programa: se lanza tal cual.
    Programa,
    /// Se lanza ESTA app con la ruta del fichero como argumento.
    Con(&'static [u8]),
    /// Texto: el visor de la ventana de datos.
    Visor,
    /// Hoy no hay con que, y este es el motivo.
    Falta(&'static str),
}

struct Tipo {
    ext: &'static [u8],
    clase: Clase,
    abre: Abre,
}

const TIPOS: &[Tipo] = &[
    Tipo { ext: b"bex", clase: Clase::App, abre: Abre::Programa },
    Tipo { ext: b"ibx", clase: Clase::App, abre: Abre::Programa },
    // ** El reproductor recibe la ruta por los ARGUMENTOS (`task/argumentos.rs`).
    Tipo { ext: b"mus", clase: Clase::Audio, abre: Abre::Con(b"inti/musica.ibx") },
    Tipo { ext: b"wav", clase: Clase::Audio, abre: Abre::Falta("WAV: el tubo USB esta, falta leer el formato") },
    // ** Las tres que `bmo-imagen` sabe descifrar: al visor, que pinta la imagen
    // y no sus bytes (2026-09-13).
    Tipo { ext: b"bic", clase: Clase::Imagen, abre: Abre::Visor },
    Tipo { ext: b"bmp", clase: Clase::Imagen, abre: Abre::Visor },
    Tipo { ext: b"qoi", clase: Clase::Imagen, abre: Abre::Visor },
    Tipo { ext: b"png", clase: Clase::Imagen, abre: Abre::Falta("PNG: falta el descompresor (inflate)") },
    Tipo { ext: b"jpg", clase: Clase::Imagen, abre: Abre::Falta("JPG: falta el decodificador (Huffman + IDCT)") },
    Tipo { ext: b"txt", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"log", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"md", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"csv", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"ini", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"cfg", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"c", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"h", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"cob", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"cbl", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"adb", clase: Clase::Texto, abre: Abre::Visor },
    Tipo { ext: b"rs", clase: Clase::Texto, abre: Abre::Visor },
];

/// **Que es este fichero y con que se abre**, por su nombre.
pub(crate) fn de(nombre: &[u8]) -> (Clase, Abre) {
    let Some(punto) = nombre.iter().rposition(|&c| c == b'.') else {
        return (Clase::Otro, Abre::Falta("sin extension: no se con que abrirlo"));
    };
    let ext = &nombre[punto + 1..];
    for t in TIPOS {
        if ext.eq_ignore_ascii_case(t.ext) {
            return (t.clase, t.abre);
        }
    }
    (Clase::Otro, Abre::Falta("no hay app para este tipo todavia"))
}
