//! Generador del font Ring 0 de BMO-X -- estetica terminal 80s/cyberpunk.
//!
//! Cada glifo se define como arte ASCII auditable de 13 filas x 8 columnas
//! ('#' = pixel), que mapean a las filas 2..14 de la celda 8x16 (filas 0-1 y
//! 15 quedan como interlineado). Trazos de 2 px NATIVOS: el renderer ya no
//! necesita engordar nada -- dibuja exacto, con esquinas nitidas.
//!
//! Salidas, y las TRES salen del MISMO arte (2026-09-11):
//!
//!   `font16_data.rs`   una expresion `[[u8; 16]; N]` que el kernel embebe
//!                      con `include!`
//!   `font16_extra.rs`  el byte Latin-1 de cada glifo extra, en su orden
//!   `fuente/datos.h`   **la misma tabla en C, para REX**: sin ella una app
//!                      dentro de su ventana no sabe escribir una letra --
//!                      el DIRECTOR pinta con la fuente del kernel y una app
//!                      solo tiene pixeles. Se genera AQUI y no se copia,
//!                      porque dos tablas de glifos a mano son dos fuentes
//!                      que se separan el dia que alguien corrija una letra
//!
//! Regenerar: `cargo run -p bmo-fontgen`.

const ROWS: usize = 13;

/// ASCII 32..=126, en orden. Filas faltantes = vacias.
const ART: [[&str; ROWS]; 95] = [
    // ' '
    ["", "", "", "", "", "", "", "", "", "", "", "", ""],
    // '!'
    ["..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "", "..##..", "..##..", "", ""],
    // '"'
    [".#..#.", ".#..#.", ".#..#.", "", "", "", "", "", "", "", "", "", ""],
    // '#'
    ["", ".#..#.", ".#..#.", "######", ".#..#.", ".#..#.", "######", ".#..#.", ".#..#.", "", "", "", ""],
    // '$'
    ["..##..", ".#####", "##....", "##....", ".####.", "....##", "....##", "#####.", "..##..", "", "", "", ""],
    // '%'
    ["##...#", "##..##", "...##.", "..##..", ".##...", "##..##", "#...##", "", "", "", "", "", ""],
    // '&'
    [".###..", "##.##.", "##.##.", ".###..", "####..", "##.###", "##.##.", "##.##.", ".##.##", "", "", "", ""],
    // '\''
    ["..##..", "..##..", "", "", "", "", "", "", "", "", "", "", ""],
    // '('
    ["...##.", "..##..", ".##...", ".##...", ".##...", ".##...", ".##...", ".##...", ".##...", "..##..", "...##.", "", ""],
    // ')'
    [".##...", "..##..", "...##.", "...##.", "...##.", "...##.", "...##.", "...##.", "...##.", "..##..", ".##...", "", ""],
    // '*'
    ["", ".#..#.", "..##..", "######", "..##..", ".#..#.", "", "", "", "", "", "", ""],
    // '+'
    ["", "", "..##..", "..##..", "######", "..##..", "..##..", "", "", "", "", "", ""],
    // ','
    ["", "", "", "", "", "", "", "", "", "..##..", "..##..", ".##...", ""],
    // '-'
    ["", "", "", "", "", "######", "", "", "", "", "", "", ""],
    // '.'
    ["", "", "", "", "", "", "", "", "", "..##..", "..##..", "", ""],
    // '/'
    ["....##", "....##", "...##.", "...##.", "..##..", "..##..", ".##...", ".##...", "##....", "##....", "##....", "", ""],
    // '0'
    [".####.", "##..##", "##..##", "##.###", "###.##", "##..##", "##..##", "##..##", "##..##", "##..##", ".####.", "", ""],
    // '1'
    ["..##..", ".###..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "######", "", ""],
    // '2'
    [".####.", "##..##", "....##", "....##", "...##.", "..##..", ".##...", "##....", "##....", "##....", "######", "", ""],
    // '3'
    [".####.", "##..##", "....##", "....##", ".###..", "....##", "....##", "....##", "....##", "##..##", ".####.", "", ""],
    // '4'
    ["##..##", "##..##", "##..##", "##..##", "######", "....##", "....##", "....##", "....##", "....##", "....##", "", ""],
    // '5'
    ["######", "##....", "##....", "##....", "#####.", "....##", "....##", "....##", "....##", "##..##", ".####.", "", ""],
    // '6'
    [".####.", "##..##", "##....", "##....", "#####.", "##..##", "##..##", "##..##", "##..##", "##..##", ".####.", "", ""],
    // '7'
    ["######", "....##", "....##", "...##.", "...##.", "..##..", "..##..", ".##...", ".##...", ".##...", ".##...", "", ""],
    // '8'
    [".####.", "##..##", "##..##", "##..##", ".####.", "##..##", "##..##", "##..##", "##..##", "##..##", ".####.", "", ""],
    // '9'
    [".####.", "##..##", "##..##", "##..##", "##..##", ".#####", "....##", "....##", "....##", "##..##", ".####.", "", ""],
    // ':'
    ["", "", "", "..##..", "..##..", "", "", "..##..", "..##..", "", "", "", ""],
    // ';'
    ["", "", "", "..##..", "..##..", "", "", "..##..", "..##..", ".##...", "", "", ""],
    // '<'
    ["", "...###", "..##..", ".##...", "##....", ".##...", "..##..", "...###", "", "", "", "", ""],
    // '='
    ["", "", "", "######", "", "", "######", "", "", "", "", "", ""],
    // '>'
    ["", "###...", "..##..", "...##.", "....##", "...##.", "..##..", "###...", "", "", "", "", ""],
    // '?'
    [".####.", "##..##", "....##", "...##.", "..##..", "..##..", "", "..##..", "..##..", "", "", "", ""],
    // '@'
    [".####.", "##..##", "##.###", "##.###", "##.###", "##.###", "##....", "##..##", ".####.", "", "", "", ""],
    // 'A'
    [".####.", "##..##", "##..##", "##..##", "######", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "", ""],
    // 'B'
    ["#####.", "##..##", "##..##", "##..##", "#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "#####.", "", ""],
    // 'C'
    [".####.", "##..##", "##....", "##....", "##....", "##....", "##....", "##....", "##....", "##..##", ".####.", "", ""],
    // 'D'
    ["#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "#####.", "", ""],
    // 'E'
    ["######", "##....", "##....", "##....", "#####.", "##....", "##....", "##....", "##....", "##....", "######", "", ""],
    // 'F'
    ["######", "##....", "##....", "##....", "#####.", "##....", "##....", "##....", "##....", "##....", "##....", "", ""],
    // 'G'
    [".####.", "##..##", "##....", "##....", "##....", "##.###", "##..##", "##..##", "##..##", "##..##", ".####.", "", ""],
    // 'H'
    ["##..##", "##..##", "##..##", "##..##", "######", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "", ""],
    // 'I'
    ["######", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "######", "", ""],
    // 'J'
    ["....##", "....##", "....##", "....##", "....##", "....##", "....##", "....##", "....##", "##..##", ".####.", "", ""],
    // 'K'
    ["##..##", "##..##", "##.##.", "####..", "###...", "####..", "##.##.", "##..##", "##..##", "##..##", "##..##", "", ""],
    // 'L'
    ["##....", "##....", "##....", "##....", "##....", "##....", "##....", "##....", "##....", "##....", "######", "", ""],
    // 'M'
    ["##..##", "######", "######", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "", ""],
    // 'N'
    ["##..##", "###.##", "###.##", "######", "##.###", "##.###", "##..##", "##..##", "##..##", "##..##", "##..##", "", ""],
    // 'O'
    [".####.", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", ".####.", "", ""],
    // 'P'
    ["#####.", "##..##", "##..##", "##..##", "#####.", "##....", "##....", "##....", "##....", "##....", "##....", "", ""],
    // 'Q'
    [".####.", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", ".####.", "...###", "", ""],
    // 'R'
    ["#####.", "##..##", "##..##", "##..##", "#####.", "##.##.", "##..##", "##..##", "##..##", "##..##", "##..##", "", ""],
    // 'S'
    [".####.", "##..##", "##....", "##....", ".####.", "....##", "....##", "....##", "....##", "##..##", ".####.", "", ""],
    // 'T'
    ["######", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "", ""],
    // 'U'
    ["##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", ".####.", "", ""],
    // 'V'
    ["##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", ".####.", "..##..", "..##..", "", ""],
    // 'W'
    ["##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "######", "######", "##..##", "", ""],
    // 'X'
    ["##..##", "##..##", "##..##", ".####.", "..##..", "..##..", "..##..", ".####.", "##..##", "##..##", "##..##", "", ""],
    // 'Y'
    ["##..##", "##..##", "##..##", "##..##", ".####.", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "", ""],
    // 'Z'
    ["######", "....##", "....##", "...###", "..###.", ".###..", "###...", "##....", "##....", "##....", "######", "", ""],
    // '['
    [".####.", ".##...", ".##...", ".##...", ".##...", ".##...", ".##...", ".##...", ".##...", ".##...", ".####.", "", ""],
    // '\\'
    ["##....", "##....", ".##...", ".##...", "..##..", "..##..", "...##.", "...##.", "....##", "....##", "....##", "", ""],
    // ']'
    [".####.", "...##.", "...##.", "...##.", "...##.", "...##.", "...##.", "...##.", "...##.", "...##.", ".####.", "", ""],
    // '^'
    ["..##..", ".####.", "##..##", "", "", "", "", "", "", "", "", "", ""],
    // '_'
    ["", "", "", "", "", "", "", "", "", "", "", "", "######"],
    // '`'
    [".##...", "..##..", "", "", "", "", "", "", "", "", "", "", ""],
    // 'a'
    ["", "", "", "", ".####.", "....##", ".#####", "##..##", "##..##", "##..##", ".#####", "", ""],
    // 'b'
    ["##....", "##....", "##....", "##....", "#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "#####.", "", ""],
    // 'c'
    ["", "", "", "", ".####.", "##..##", "##....", "##....", "##....", "##..##", ".####.", "", ""],
    // 'd'
    ["....##", "....##", "....##", "....##", ".#####", "##..##", "##..##", "##..##", "##..##", "##..##", ".#####", "", ""],
    // 'e'
    ["", "", "", "", ".####.", "##..##", "######", "##....", "##....", "##..##", ".####.", "", ""],
    // 'f'
    ["..###.", ".##...", ".##...", ".##...", "#####.", ".##...", ".##...", ".##...", ".##...", ".##...", ".##...", "", ""],
    // 'g'
    ["", "", "", "", ".#####", "##..##", "##..##", "##..##", "##..##", "##..##", ".#####", "....##", ".####."],
    // 'h'
    ["##....", "##....", "##....", "##....", "#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "", ""],
    // 'i'
    ["..##..", "..##..", "", "", ".###..", "..##..", "..##..", "..##..", "..##..", "..##..", "######", "", ""],
    // 'j'
    ["....##", "....##", "", "", "....##", "....##", "....##", "....##", "....##", "....##", "....##", "##..##", ".####."],
    // 'k'
    ["##....", "##....", "##....", "##....", "##..##", "##.##.", "####..", "####..", "##.##.", "##..##", "##..##", "", ""],
    // 'l'
    [".###..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..###.", "", ""],
    // 'm'
    ["", "", "", "", "######", "#.##.#", "#.##.#", "#.##.#", "#.##.#", "#.##.#", "#.##.#", "", ""],
    // 'n'
    ["", "", "", "", "#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "", ""],
    // 'o'
    ["", "", "", "", ".####.", "##..##", "##..##", "##..##", "##..##", "##..##", ".####.", "", ""],
    // 'p'
    ["", "", "", "", "#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "#####.", "##....", "##...."],
    // 'q'
    ["", "", "", "", ".#####", "##..##", "##..##", "##..##", "##..##", "##..##", ".#####", "....##", "....##"],
    // 'r'
    ["", "", "", "", "##.###", "###...", "##....", "##....", "##....", "##....", "##....", "", ""],
    // 's'
    ["", "", "", "", ".#####", "##....", "##....", ".####.", "....##", "....##", "#####.", "", ""],
    // 't'
    [".##...", ".##...", ".##...", ".##...", "#####.", ".##...", ".##...", ".##...", ".##...", ".##...", "..###.", "", ""],
    // 'u'
    ["", "", "", "", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", ".#####", "", ""],
    // 'v'
    ["", "", "", "", "##..##", "##..##", "##..##", "##..##", ".####.", "..##..", "..##..", "", ""],
    // 'w'
    ["", "", "", "", "#.##.#", "#.##.#", "#.##.#", "#.##.#", "#.##.#", "######", ".#..#.", "", ""],
    // 'x'
    ["", "", "", "", "##..##", ".####.", "..##..", "..##..", ".####.", "##..##", "##..##", "", ""],
    // 'y'
    ["", "", "", "", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", ".#####", "....##", ".####."],
    // 'z'
    ["", "", "", "", "######", "....##", "...##.", "..##..", ".##...", "##....", "######", "", ""],
    // '{'
    ["..###.", ".##...", ".##...", ".##...", "##....", ".##...", ".##...", ".##...", ".##...", ".##...", "..###.", "", ""],
    // '|'
    ["..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "", ""],
    // '}'
    [".###..", "...##.", "...##.", "...##.", "....##", "...##.", "...##.", "...##.", "...##.", "...##.", ".###..", "", ""],
    // '~'
    ["", "", "", "", ".##..#", "#.##.#", "#..##.", "", "", "", "", "", ""],
];


// ===========================================================================
//  Glifos ESPANOLES (Latin-1) -- COMPUESTOS, no dibujados a mano
// ===========================================================================
//
// La n es la a con una tilde encima; la a es la a con un acento. Dibujar 23
// mapas de bits a mano seria copiar 23 veces la misma letra con un adorno
// distinto -- y cada copia envejece por su cuenta. El generador los COMPONE:
// toma el glifo ASCII base y le superpone el signo. Cambiar la 'a' arregla
// la 'a' sola.
//
// Las minusculas ocupan las filas 4..10 del arte, asi que el signo entra
// arriba sin tocarlas. Las mayusculas ocupan 0..10: se bajan 2 filas (queda
// sitio de sobra, las filas 11-12 solo las usan los descendentes p/q/g/y).

const ACUTE:  [&str; 2] = ["...##.", "..##.."]; // '  sube hacia la derecha
const DIAER:  [&str; 2] = [".##.##", ".##.##"]; // "
const TILDE:  [&str; 2] = [".##..#", "#..##."]; // ~
const CEDIL:  [&str; 2] = ["..##..", ".###.."]; // , (debajo)

enum Extra {
    /// Arte literal: signos que no derivan de ninguna letra.
    Art([&'static str; ROWS]),
    /// Letra base + signo ENCIMA, bajando la base `shift` filas.
    Above(char, [&'static str; 2], usize),
    /// Igual, pero borrando la cabeza de la base: la 'i' pierde su punto
    /// antes de recibir el acento (i, no i con dos cosas encima).
    AboveDotless(char, [&'static str; 2], usize),
    /// Letra base + signo DEBAJO (cedilla).
    Below(char, [&'static str; 2]),
}

/// `(byte Latin-1, receta)`. El byte es la codificacion en la que el teclado
/// entrega el caracter y en la que la consola lo guarda: un byte por letra,
/// sin UTF-8 en Ring 0.
const EXTRA: [(u8, Extra); 25] = [
    (0xA1, Extra::Art(["..##..", "..##..", "", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "", ""])), // 
    (0xBF, Extra::Art(["..##..", "..##..", "", "..##..", "..##..", ".##...", "##....", "##....", "##..##", "##..##", ".####.", "", ""])), // 
    (0xAA, Extra::Art([".####.", "....##", ".#####", "##..##", ".#####", "", "######", "", "", "", "", "", ""])), // a
    (0xBA, Extra::Art([".####.", "##..##", "##..##", "##..##", ".####.", "", "######", "", "", "", "", "", ""])), // o
    (0xB0, Extra::Art([".####.", "##..##", "##..##", ".####.", "", "", "", "", "", "", "", "", ""])),             //  deg
    (0xB7, Extra::Art(["", "", "", "", "", "..##..", "..##..", "", "", "", "", "", ""])),                          // -
    (0xAC, Extra::Art(["", "", "", "", "######", "....##", "....##", "", "", "", "", "", ""])),                    // !
    (0xF1, Extra::Above('n', TILDE, 0)),        // n
    (0xD1, Extra::Above('N', TILDE, 2)),        // N
    (0xE1, Extra::Above('a', ACUTE, 0)),        // a
    (0xE9, Extra::Above('e', ACUTE, 0)),        // e
    (0xED, Extra::AboveDotless('i', ACUTE, 0)), // i
    (0xF3, Extra::Above('o', ACUTE, 0)),        // o
    (0xFA, Extra::Above('u', ACUTE, 0)),        // u
    (0xFC, Extra::Above('u', DIAER, 0)),        // u
    (0xC1, Extra::Above('A', ACUTE, 2)),        // A
    (0xC9, Extra::Above('E', ACUTE, 2)),        // E
    (0xCD, Extra::Above('I', ACUTE, 2)),        // I
    (0xD3, Extra::Above('O', ACUTE, 2)),        // O
    (0xDA, Extra::Above('U', ACUTE, 2)),        // U
    (0xDC, Extra::Above('U', DIAER, 2)),        // U
    (0xE7, Extra::Below('c', CEDIL)),           // c
    (0xC7, Extra::Below('C', CEDIL)),           // C
    // Los dos signos muertos, sueltos: se imprimen cuando el usuario
    // pulsa el acento y despues algo que no combina (o un espacio).
    (0xB4, Extra::Art(["...##.", "..##..", "", "", "", "", "", "", "", "", "", "", ""])), // acento agudo
    (0xA8, Extra::Art([".##.##", ".##.##", "", "", "", "", "", "", "", "", "", "", ""])), // dieresis
];

fn base_art(ch: char) -> [&'static str; ROWS] {
    ART[(ch as usize) - 32]
}

/// Aplica la receta y devuelve el arte final del glifo.
fn build_extra(e: &Extra) -> [String; ROWS] {
    let mut out: [String; ROWS] = Default::default();
    match e {
        Extra::Art(a) => {
            for r in 0..ROWS { out[r] = a[r].to_string(); }
        }
        Extra::Above(ch, mark, shift) | Extra::AboveDotless(ch, mark, shift) => {
            let dotless = matches!(e, Extra::AboveDotless(..));
            let b = base_art(*ch);
            for r in 0..ROWS {
                // La base baja `shift` filas; si `dotless`, se descarta todo lo
                // que la letra tuviera por encima de su cuerpo.
                if r >= *shift {
                    let src = r - *shift;
                    let keep = !dotless || src >= 4;
                    if keep { out[r] = b[src].to_string(); }
                }
            }
            // El signo va justo ENCIMA del cuerpo, sin pisarlo: se busca la
            // primera fila con tinta y se colocan las dos filas del signo
            // arriba. (Escribirlo a mano costo una N decapitada: el signo
            // sobreescribia las dos primeras filas de la N.)
            let top = (0..ROWS).find(|r| out[*r].contains('#')).unwrap_or(2);
            if top >= 2 {
                out[top - 2] = mark[0].to_string();
                out[top - 1] = mark[1].to_string();
            }
        }
        Extra::Below(ch, mark) => {
            let b = base_art(*ch);
            for r in 0..ROWS { out[r] = b[r].to_string(); }
            out[ROWS - 2] = mark[0].to_string();
            out[ROWS - 1] = mark[1].to_string();
        }
    }
    out
}

fn pack(lines: &[String]) -> [u8; 16] {
    let mut rows = [0u8; 16];
    for (r, line) in lines.iter().enumerate().take(ROWS) {
        let mut byte = 0u8;
        for (c, px) in line.chars().enumerate().take(8) {
            if px == '#' {
                byte |= 0x80 >> c; // bit 7 = columna izquierda (contrato del renderer)
            }
        }
        rows[r + 2] = byte;
    }
    rows
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        "Ultra_kernel_x86-64/kernel/src/ring0/core/font16_data.rs".to_string()
    });
    // El segundo archivo (mismo directorio) lleva los bytes Latin-1 de los
    // glifos extra, EN EL MISMO ORDEN: el kernel busca ahi para traducir un
    // byte >= 0xA0 a su indice de glifo.
    let out_extra = out.replace("font16_data.rs", "font16_extra.rs");
    // ** Y EL TERCERO ES LA MISMA TABLA EN C, PARA REX (2026-09-11).
    //
    // Una app dibuja en SU memoria, asi que escribir una letra es cosa suya: la
    // fuente del kernel no le sirve de nada. Se emite PLANA --un solo indice--
    // porque BMO C digiere mejor un array de una dimension que ciento veinte
    // llaves anidadas, y `glifo * 16 + fila` es la misma cuenta que ya hace el
    // renderer del kernel.
    let out_c = std::env::args().nth(2).unwrap_or_else(|| {
        "toolchain/forge/sem-asm/tables/bmo/fuente/datos.h".to_string()
    });

    // ** LOS LETREROS SE GENERAN, NO SE PONEN A MANO (2026-09-11).
    //
    // `[carril]` (L6g) y `[consumo]` (L6h) se los exige `contrato.py` a TODO
    // fichero de Ring 0, y estos dos lo son aunque los escriba una herramienta.
    // Estaban anadidos a mano: regenerar la fuente los borraba, y el build caia
    // por una regla que nadie habia incumplido. Lo que se genera, se genera
    // entero -- letreros incluidos.
    let letreros = |que: &str| {
        format!(
            "// [carril]  VERDE     {que}\n\
             // [consumo] NADA      no corre: es una tabla\n\
             // [!] `//` y no `//!` a proposito: esto NO es un modulo. `texto.rs` lo\n\
             // mete con `include!` DENTRO de un `static`, o sea que su contenido es una\n\
             // EXPRESION, y una expresion no admite documentacion de modulo.\n\n"
        )
    };

    let glifos = 95 + EXTRA.len();

    // -- 1. la tabla del kernel ------------------------------------------
    let mut s = letreros("una fuente de 8x16, en una tabla");
    s += &format!(
        "// AUTO-GENERADO por toolchain/tools/fontgen -- NO editar a mano.\n\
         // Regenerar: cargo run -p bmo-fontgen\n\
         // Glifos 8x16 (arte en filas 2..14), trazos 2px nativos.\n\
         // Indices 0..94 = ASCII 32..=126; 95..{} = extras Latin-1 (ver font16_extra.rs).\n[\n",
        glifos - 1
    );
    for (i, art) in ART.iter().enumerate() {
        let ch = (32 + i as u8) as char;
        let lines: Vec<String> = art.iter().map(|l| l.to_string()).collect();
        s += &format!("    // {:?}\n    [", ch);
        for b in pack(&lines) { s += &format!("0x{:02X},", b); }
        s += "],\n";
    }
    for (code, recipe) in EXTRA.iter() {
        let lines = build_extra(recipe);
        s += &format!("    // Latin-1 0x{:02X}\n    [", code);
        for b in pack(&lines) { s += &format!("0x{:02X},", b); }
        s += "],\n";
    }
    s += "]\n";
    std::fs::write(&out, &s).expect("escribir font16_data.rs");

    // -- 2. los bytes Latin-1 --------------------------------------------
    let mut e = letreros("los glifos que le faltaban a la tabla de al lado");
    e += "// AUTO-GENERADO por toolchain/tools/fontgen -- NO editar a mano.\n\
          // Byte Latin-1 de cada glifo extra, en el orden en que estan en\n\
          // font16_data.rs a partir del indice 95.\n[\n    ";
    for (code, _) in EXTRA.iter() { e += &format!("0x{:02X},", code); }
    e += "\n]\n";
    std::fs::write(&out_extra, &e).expect("escribir font16_extra.rs");

    // -- 3. la misma tabla, en C, para REX -------------------------------
    //
    // *** EL " * " DE CADA LINEA LO PONE ESTE CODIGO, NO LA SANGRIA DEL LITERAL.
    //
    // La continuacion `\` de Rust se come el salto Y el espacio del principio de
    // la linea siguiente, asi que un literal bonito y alineado emitia
    // `* [carril]` pegado al margen. Y `RE_SELLO_H` de `contrato_rex.py` exige
    // `" * [carril]"` **con su espacio delante**: R11 rechazo este fichero por
    // tres etiquetas que SI estaban escritas.
    //
    // [!] Y lo caro fue donde se vio: `build.ps1 -BuildOnly` pasa en verde --su
    // paso de contrato es mas corto-- y el que lo caza es el DESPLIEGUE, o sea
    // el ultimo sitio antes del disco. Por eso el prefijo deja de ser cosa del
    // que escribe el literal.
    let cabecera = [
        "fuente/datos.h -- los glifos de BMO-X, en C. AUTO-GENERADO.",
        "",
        "NO editar a mano. Regenerar: `cargo run -p bmo-fontgen`, que emite",
        "este fichero Y la tabla del kernel DEL MISMO ARTE: dos tablas de",
        "glifos mantenidas a mano son dos fuentes que se separan el dia que",
        "alguien corrija una letra en una de las dos.",
        "",
        "[carril]  VERDE        una tabla. Aqui no corre nada",
        "[cuesta]  NADA         un glifo mal sale feo, y se ve",
        "[riesgo]  ESPEJO       el kernel tiene la MISMA tabla en",
        "                       `font16_data.rs`, y las dos salen de",
        "                       `toolchain/tools/fontgen`",
    ];
    let mut c = String::new();
    for (i, linea) in cabecera.iter().enumerate() {
        if i == 0 {
            c += &format!("/* {linea}\n");
        } else if linea.is_empty() {
            c += " *\n";
        } else {
            c += &format!(" * {linea}\n");
        }
    }
    c += " */\n#ifndef BMO_FUENTE_DATOS_H\n#define BMO_FUENTE_DATOS_H\n\n";
    c += &format!("#define BMO_FUENTE_GLIFOS {}\n", glifos);
    c += "#define BMO_FUENTE_ASCII 95\n";
    c += &format!("#define BMO_FUENTE_EXTRAS {}\n\n", EXTRA.len());
    c += "/* Glifo `g`, fila `f`: `bmo_fuente_glifos[g * 16 + f]`. El bit 7 es la\n\
          * columna izquierda, el mismo contrato que el renderer del kernel. */\n";
    c += &format!("static unsigned char bmo_fuente_glifos[{} * 16] = {{\n", glifos);
    for (i, art) in ART.iter().enumerate() {
        let lines: Vec<String> = art.iter().map(|l| l.to_string()).collect();
        c += "    ";
        for b in pack(&lines) { c += &format!("0x{:02X},", b); }
        c += &format!("   /* ASCII {} */\n", 32 + i);
    }
    for (code, recipe) in EXTRA.iter() {
        let lines = build_extra(recipe);
        c += "    ";
        for b in pack(&lines) { c += &format!("0x{:02X},", b); }
        c += &format!("   /* Latin-1 0x{:02X} */\n", code);
    }
    c += "};\n\n";
    c += "/* El byte Latin-1 de cada glifo extra, en el mismo orden en que estan\n\
          * arriba a partir de BMO_FUENTE_ASCII. */\n";
    c += "static unsigned char bmo_fuente_latin1[BMO_FUENTE_EXTRAS] = {\n    ";
    for (code, _) in EXTRA.iter() { c += &format!("0x{:02X},", code); }
    c += "\n};\n\n#endif /* BMO_FUENTE_DATOS_H */\n";
    if let Some(dir) = std::path::Path::new(&out_c).parent() {
        std::fs::create_dir_all(dir).expect("crear la carpeta de fuente/");
    }
    std::fs::write(&out_c, &c).expect("escribir fuente/datos.h");

    println!("generado {out} ({glifos} glifos: 95 ASCII + {} Latin-1)", EXTRA.len());
    println!("generado {out_extra}");
    println!("generado {out_c} (la misma tabla, en C, para REX)");
}
