//! `lexico` -- de bytes a piezas.
//!
//! ## Que hace, y sobre todo que NO hace
//!
//! Convierte texto en una lista de [`Pieza`]. **No conoce la gramatica**: no
//! sabe que `si` lleva un bloque detras ni que `funcion` empieza una
//! declaracion. Solo sabe reconocer palabras, numeros, textos, signos y el
//! margen.
//!
//! Ese limite es el corte del modulo, y se nota en una cosa concreta: **aqui no
//! se puede escribir un error de gramatica**. Un `si` suelto sin condicion no
//! es asunto de este fichero; un `"` sin cerrar si.
//!
//! ## Las tres piezas invisibles
//!
//! ```text
//!    Sangra / Desangra   el margen, que en INTI es estructura
//!    FinLinea            el final de una sentencia
//! ```
//!
//! `FinLinea` **no sale** dentro de una pareja abierta (`(`, `[`, `{`), que es
//! la unica continuacion de linea que existe. Sin eso, una lista larga habria
//! que escribirla en un renglon.
//!
//! ## Y por que el vocabulario entra por argumento
//!
//! Porque este modulo no elige el idioma: lo recibe. Es lo que hace que el
//! ingles sea una columna de `palabras.toml` y no un fork -- ver el modulo
//! `palabras`.

pub mod pieza;
pub mod sangria;

pub use pieza::{Base, Clase, Numero, Pieza, Signo};

use crate::aviso::{codigos, Aviso, Cosecha, Sitio};
use crate::palabras::Vocabulario;
use sangria::{medir_margen, Sangrador};

/// Barre un fuente entero.
///
/// Devuelve las piezas **y** todo lo que hay que decir, en vez de parar en el
/// primer fallo: arreglar un fichero adivinando cuantos errores quedan es la
/// experiencia que este lenguaje intenta no dar.
pub fn barrer(fuente: &str, vocab: &Vocabulario) -> Cosecha<Vec<Pieza>> {
    let mut piezas: Vec<Pieza> = Vec::new();
    let mut avisos: Vec<Aviso> = Vec::new();
    let mut sangrador = Sangrador::nuevo();
    // Parejas abiertas, con el sitio de la que abrio: sin el sitio, el aviso de
    // "esto no se cierra" no puede decir donde empezo, que es el unico dato que
    // hace falta para arreglarlo.
    let mut parejas: Vec<(Signo, Sitio)> = Vec::new();

    for (i, linea) in fuente.lines().enumerate() {
        let n = i + 1;

        let (cuerpo, columna_inicial) = if parejas.is_empty() {
            let (ancho, resto, av) = medir_margen(linea, n);
            avisos.extend(av);

            // Una linea vacia o de solo comentario no toca el margen. Si lo
            // tocara, un comentario pegado al borde izquierdo cerraria el
            // bloque en el que esta escrito.
            let limpio = resto.trim_end();
            if limpio.is_empty() || limpio.starts_with('#') {
                continue;
            }

            let sitio = Sitio::nuevo(n, ancho + 1);
            let (clases, av) = sangrador.medir(ancho, sitio, linea);
            avisos.extend(av);
            for c in clases {
                piezas.push(Pieza::nueva(c, sitio));
            }
            (resto, ancho)
        } else {
            // Continuacion: dentro de una pareja el margen no significa nada.
            let sin_margen = linea.trim_start();
            let comidos = linea.chars().count() - sin_margen.chars().count();
            (sin_margen, comidos)
        };

        barrer_linea(
            cuerpo,
            n,
            columna_inicial,
            vocab,
            &mut piezas,
            &mut avisos,
            &mut parejas,
        );

        if parejas.is_empty() {
            let col = columna_inicial + cuerpo.chars().count() + 1;
            piezas.push(Pieza::nueva(Clase::FinLinea, Sitio::nuevo(n, col)));
        }
    }

    // Lo que se abrio y no se cerro.
    for (signo, sitio) in &parejas {
        avisos.push(
            Aviso::nuevo(
                codigos::PAREJA_ROTA,
                format!("Este `{}` no se cierra en ningun sitio.", signo.texto()),
                *sitio,
            )
            .con_habia(format!(
                "Se abrio en la linea {} y el fichero se acaba sin el `{}`.",
                sitio.linea,
                signo.pareja().map(|p| p.texto()).unwrap_or("?")
            ))
            .con_hacer(format!(
                "anade `{}`",
                signo.pareja().map(|p| p.texto()).unwrap_or("?")
            )),
        );
    }

    let ultima = piezas.last().map(|p| p.sitio).unwrap_or_default();
    for c in sangrador.cerrar() {
        piezas.push(Pieza::nueva(c, ultima));
    }
    piezas.push(Pieza::nueva(Clase::Fin, ultima));

    Cosecha::con(piezas, avisos)
}

#[allow(clippy::too_many_arguments)]
fn barrer_linea(
    cuerpo: &str,
    linea: usize,
    columna_inicial: usize,
    vocab: &Vocabulario,
    piezas: &mut Vec<Pieza>,
    avisos: &mut Vec<Aviso>,
    parejas: &mut Vec<(Signo, Sitio)>,
) {
    // En caracteres y no en bytes: una tilde ocupa dos bytes y desplazaria
    // todas las columnas de la linea, y con ellas el dedo de los avisos.
    let cs: Vec<char> = cuerpo.chars().collect();
    let mut i = 0usize;

    // El sitio de la posicion `j` de este cuerpo.
    let sitio_de = |j: usize| Sitio::nuevo(linea, columna_inicial + j + 1);

    while i < cs.len() {
        let c = cs[i];

        if c == ' ' {
            i += 1;
            continue;
        }
        if c == '#' {
            break; // comentario hasta el final
        }

        // -------------------------------------------------------- textos
        if c == '"' {
            let (texto, siguiente, av) = leer_texto(&cs, i, linea, columna_inicial, cuerpo);
            avisos.extend(av);
            if let Some(t) = texto {
                piezas.push(Pieza::nueva(Clase::Texto(t), sitio_de(i)));
            }
            i = siguiente;
            continue;
        }

        // La comilla simple no existe, y decirlo bien vale mas que reconocerla.
        if c == '\'' {
            avisos.push(
                Aviso::nuevo(
                    codigos::COMILLA_SIMPLE,
                    "En INTI los textos van entre comillas dobles.",
                    sitio_de(i),
                )
                .con_linea(cuerpo)
                .con_habia("La comilla simple no significa nada aqui: hay una sola forma de escribir un texto.".to_string())
                .con_hacer("usa \" en los dos extremos"),
            );
            // Se salta hasta la siguiente comilla simple para no llenar la
            // linea de avisos por lo mismo.
            i += 1;
            while i < cs.len() && cs[i] != '\'' {
                i += 1;
            }
            i += 1;
            continue;
        }

        // ------------------------------------------------------- numeros
        if c.is_ascii_digit() {
            let (num, siguiente, av) = leer_numero(&cs, i, linea, columna_inicial, cuerpo);
            avisos.extend(av);
            if let Some(nu) = num {
                piezas.push(Pieza::nueva(Clase::Numero(nu), sitio_de(i)));
            }
            i = siguiente;
            continue;
        }

        // ------------------------------------------- palabras y nombres
        if es_inicio_de_nombre(c) {
            let arranca = i;
            while i < cs.len() && es_cuerpo_de_nombre(cs[i]) {
                i += 1;
            }
            let palabra: String = cs[arranca..i].iter().collect();

            if let Some(s) = vocab.reconocer(&palabra) {
                piezas.push(Pieza::nueva(Clase::Palabra(s), sitio_de(arranca)));
            } else if palabra.chars().next().map(|p| p.is_uppercase()).unwrap_or(false) {
                piezas.push(Pieza::nueva(Clase::Tipo(palabra), sitio_de(arranca)));
            } else {
                if !palabra.is_ascii() {
                    avisos.push(
                        Aviso::nuevo(
                            codigos::NOMBRE_NO_ASCII,
                            "Ese nombre lleva letras de fuera del ASCII.",
                            sitio_de(arranca),
                        )
                        .con_linea(cuerpo)
                        .con_habia(format!(
                            "`{}` vale, y aun asi conviene saberlo: las herramientas del sistema \
                             comprueban que las fuentes sean ASCII.",
                            palabra
                        ))
                        .con_hacer(format!("si te da igual, escribelo `{}`", crate::palabras::sin_tildes(&palabra))),
                    );
                }
                piezas.push(Pieza::nueva(Clase::Nombre(palabra), sitio_de(arranca)));
            }
            continue;
        }

        // --------------------------------------------------------- signos
        let (signo, ancho) = match c {
            '(' => (Some(Signo::ParenAbre), 1),
            ')' => (Some(Signo::ParenCierra), 1),
            '[' => (Some(Signo::CorcheteAbre), 1),
            ']' => (Some(Signo::CorcheteCierra), 1),
            '{' => (Some(Signo::LlaveAbre), 1),
            '}' => (Some(Signo::LlaveCierra), 1),
            ',' => (Some(Signo::Coma), 1),
            ':' => (Some(Signo::DosPuntos), 1),
            '.' => (Some(Signo::Punto), 1),
            '=' => (Some(Signo::Igual), 1),
            '+' => (Some(Signo::Mas), 1),
            '-' => (Some(Signo::Menos), 1),
            '*' => (Some(Signo::Por), 1),
            '/' => (Some(Signo::Barra), 1),
            '<' => {
                if cs.get(i + 1) == Some(&'=') {
                    (Some(Signo::MenorIgual), 2)
                } else {
                    (Some(Signo::Menor), 1)
                }
            }
            '>' => {
                if cs.get(i + 1) == Some(&'=') {
                    (Some(Signo::MayorIgual), 2)
                } else {
                    (Some(Signo::Mayor), 1)
                }
            }
            _ => (None, 1),
        };

        match signo {
            Some(s) => {
                let sitio = sitio_de(i);
                if s.abre() {
                    parejas.push((s, sitio));
                } else if let Some(esperada) = cierra_a(s) {
                    match parejas.pop() {
                        Some((abierta, _)) if abierta == esperada => {}
                        Some((abierta, sitio_abierta)) => {
                            avisos.push(
                                Aviso::nuevo(
                                    codigos::PAREJA_ROTA,
                                    format!(
                                        "Aqui va `{}`, no `{}`.",
                                        abierta.pareja().map(|p| p.texto()).unwrap_or("?"),
                                        s.texto()
                                    ),
                                    sitio,
                                )
                                .con_linea(cuerpo)
                                .con_habia(format!(
                                    "Lo que esta abierto es el `{}` de la linea {}.",
                                    abierta.texto(),
                                    sitio_abierta.linea
                                ))
                                .con_hacer(format!(
                                    "cierra con `{}`",
                                    abierta.pareja().map(|p| p.texto()).unwrap_or("?")
                                )),
                            );
                        }
                        None => {
                            avisos.push(
                                Aviso::nuevo(
                                    codigos::PAREJA_ROTA,
                                    format!("Este `{}` cierra algo que no esta abierto.", s.texto()),
                                    sitio,
                                )
                                .con_linea(cuerpo)
                                .con_hacer("quitalo, o abre la pareja antes"),
                            );
                        }
                    }
                }
                piezas.push(Pieza::nueva(Clase::Signo(s), sitio));
                i += ancho;
            }
            None => {
                avisos.push(
                    Aviso::nuevo(
                        codigos::SIGNO_DESCONOCIDO,
                        format!("El signo `{}` no es de este lenguaje.", c),
                        sitio_de(i),
                    )
                    .con_linea(cuerpo)
                    .con_habia(sugerencia_de_signo(c))
                    .con_hacer(sugerencia_de_arreglo(c)),
                );
                i += 1;
            }
        }
    }
}

/// Lo que se puede sugerir cuando aparece un signo de otro lenguaje. Aqui es
/// donde se paga la deuda de venir de C o de Python: la persona escribio lo que
/// sabia, y el mensaje tiene que reconocerlo en vez de decir "caracter no
/// valido".
fn sugerencia_de_signo(c: char) -> String {
    match c {
        ';' => "En INTI una linea acaba donde acaba la linea.".to_string(),
        '&' | '|' => "Los operadores de logica son palabras: `y`, `o`, `no`.".to_string(),
        '!' => "La negacion se escribe `no`.".to_string(),
        '%' => "El resto de una division se escribe `resto`.".to_string(),
        '?' => "No hay operador ternario: se escribe con `si`.".to_string(),
        '\\' => "La barra invertida solo vive dentro de un texto.".to_string(),
        '@' => "No hay decoradores.".to_string(),
        _ => String::new(),
    }
}

fn sugerencia_de_arreglo(c: char) -> String {
    match c {
        ';' => "borra el `;`".to_string(),
        '&' => "escribe `y`".to_string(),
        '|' => "escribe `o`".to_string(),
        '!' => "escribe `no`".to_string(),
        '%' => "escribe `resto`".to_string(),
        _ => "quitalo".to_string(),
    }
}

fn cierra_a(s: Signo) -> Option<Signo> {
    match s {
        Signo::ParenCierra => Some(Signo::ParenAbre),
        Signo::CorcheteCierra => Some(Signo::CorcheteAbre),
        Signo::LlaveCierra => Some(Signo::LlaveAbre),
        _ => None,
    }
}

fn es_inicio_de_nombre(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn es_cuerpo_de_nombre(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Lee un texto entre comillas dobles. Los escapes son cinco y se acaba la
/// lista (`GRAMATICA.md` sec. 3).
fn leer_texto(
    cs: &[char],
    inicio: usize,
    linea: usize,
    columna_inicial: usize,
    cuerpo: &str,
) -> (Option<String>, usize, Vec<Aviso>) {
    let mut avisos = Vec::new();
    let mut salida = String::new();
    let mut i = inicio + 1;

    while i < cs.len() {
        let c = cs[i];
        if c == '"' {
            return (Some(salida), i + 1, avisos);
        }
        if c == '\\' {
            let siguiente = cs.get(i + 1).copied();
            match siguiente {
                Some('n') => salida.push('\n'),
                Some('t') => salida.push('\t'),
                Some('"') => salida.push('"'),
                Some('\\') => salida.push('\\'),
                Some('{') => salida.push('{'),
                otro => {
                    avisos.push(
                        Aviso::nuevo(
                            codigos::ESCAPE_RARO,
                            "Esa barra invertida no abre ningun escape.",
                            Sitio::nuevo(linea, columna_inicial + i + 1),
                        )
                        .con_linea(cuerpo)
                        .con_habia(format!(
                            "Detras de la barra hay `{}`, y los escapes son cinco: \\n \\t \\\" \\\\ \\{{",
                            otro.map(|c| c.to_string()).unwrap_or_else(|| "el final de la linea".to_string())
                        ))
                        .con_hacer("si querias una barra de verdad, escribe \\\\"),
                    );
                    if let Some(o) = otro {
                        salida.push(o);
                    }
                }
            }
            i += 2;
            continue;
        }
        salida.push(c);
        i += 1;
    }

    avisos.push(
        Aviso::nuevo(
            codigos::TEXTO_SIN_CERRAR,
            "Este texto empieza y no se cierra.",
            Sitio::nuevo(linea, columna_inicial + inicio + 1),
        )
        .con_linea(cuerpo)
        .con_habia("Un texto no puede seguir en la linea de abajo.".to_string())
        .con_hacer("cierra con \" antes de acabar la linea"),
    );
    (None, cs.len(), avisos)
}

/// Lee un numero. Decimal o hexadecimal, sin separadores.
fn leer_numero(
    cs: &[char],
    inicio: usize,
    linea: usize,
    columna_inicial: usize,
    cuerpo: &str,
) -> (Option<Numero>, usize, Vec<Aviso>) {
    let mut avisos = Vec::new();
    let mut i = inicio;

    // Hexadecimal.
    if cs[i] == '0' && matches!(cs.get(i + 1), Some('x') | Some('X')) {
        let arranca = i;
        i += 2;
        let primero = i;
        while i < cs.len() && cs[i].is_ascii_hexdigit() {
            i += 1;
        }
        if i == primero {
            avisos.push(
                Aviso::nuevo(
                    codigos::NUMERO_RARO,
                    "Este `0x` no lleva ningun digito detras.",
                    Sitio::nuevo(linea, columna_inicial + arranca + 1),
                )
                .con_linea(cuerpo)
                .con_hacer("escribe los digitos, por ejemplo 0x60"),
            );
            return (None, i, avisos);
        }
        let texto: String = cs[arranca..i].iter().collect();
        return (
            Some(Numero {
                texto,
                base: Base::Dieciseis,
                con_punto: false,
            }),
            i,
            avisos,
        );
    }

    let arranca = i;
    while i < cs.len() && cs[i].is_ascii_digit() {
        i += 1;
    }

    // El punto solo es decimal si detras hay un digito: `p.x` no es un numero.
    let mut con_punto = false;
    if cs.get(i) == Some(&'.') && cs.get(i + 1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
        con_punto = true;
        i += 1;
        while i < cs.len() && cs[i].is_ascii_digit() {
            i += 1;
        }
        // Un segundo punto decimal.
        if cs.get(i) == Some(&'.') && cs.get(i + 1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
            avisos.push(
                Aviso::nuevo(
                    codigos::NUMERO_RARO,
                    "Este numero lleva dos puntos decimales.",
                    Sitio::nuevo(linea, columna_inicial + i + 1),
                )
                .con_linea(cuerpo)
                .con_hacer("deja un solo punto"),
            );
            i += 1;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
        }
    }

    // Notacion cientifica: `1e30`. Se acepta y se marca como decimal.
    if matches!(cs.get(i), Some('e') | Some('E'))
        && cs
            .get(i + 1)
            .map(|c| c.is_ascii_digit() || *c == '-' || *c == '+')
            .unwrap_or(false)
    {
        con_punto = true;
        i += 2;
        while i < cs.len() && cs[i].is_ascii_digit() {
            i += 1;
        }
    }

    let texto: String = cs[arranca..i].iter().collect();
    (
        Some(Numero {
            texto,
            base: Base::Diez,
            con_punto,
        }),
        i,
        avisos,
    )
}

#[cfg(test)]
mod pruebas;
