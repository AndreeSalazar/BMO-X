//! **LOS ICONOS DEL SISTEMA**: los de las cosas que no traen el suyo.
//!
//! === Por que hacen falta, teniendo iconos ya ===
//!
//! El escritorio sabe pintar iconos desde hace dias, pero los saca **de dentro
//! del `.bex`** (`scene/launcher.rs`): la app trae su propia cara como un
//! recurso de su paquete, y por eso no hay `.lnk` que se despegue.
//!
//! * **Una carpeta no tiene `.bex`.** Ni un `.txt`, ni un nodo que no se pudo
//! leer. No hay de donde sacarles la cara, asi que o la trae el sistema o la
//! rejilla se queda con un cuadrito de color -- que dice la clase pero no se
//! reconoce de un vistazo, que es justo lo unico que un icono tiene que hacer.
//!
//! === Escritos como DIBUJO, y eso no es capricho ===
//!
//! Es la misma decision que ya esta escrita en `build.ps1` para el formato
//! `BICO`, y en `ring0/core/gato.rs`:
//!
//! > una rejilla de dieciseis lineas se lee, se corrige y **se ve mal cuando
//! > esta mal**; un array de 256 enteros no.
//!
//! === Un dibujo por FORMA, un color por CLASE ===
//!
//! Los simbolos no son colores: son PAPELES. El cuerpo, el brillo, la sombra y
//! el contorno se resuelven contra el color de la clase en el momento de
//! pintar, asi que hay **un** dibujo de carpeta y no uno por color.
//!
//! Y el color es el mismo que su caja en el grafo de al lado, a proposito:
//! mirar el mismo nodo en los dos paneles no puede darle dos colores. La forma
//! dice QUE ES y el color dice lo mismo -- se refuerzan en vez de competir.
//!
//! === Y por que esto NO es un `.maqueta` ===
//!
//! Porque MAQUETA maqueta: reparte cajas, no dibuja mapas de bits. Su ley L7
//! --la que le prohibe ser un navegador-- es justo lo que la deja terminable, y
//! meterle pixeles seria la primera grieta.
//!
//! Lo que SI le toca a MAQUETA de todo esto es la PALETA, y ya esta dicho en
//! `tema/tema.maqueta`: *"mientras no exista el emisor, este fichero es la
//! FUENTE y las constantes de Rust son la copia"*. Esa deuda sigue abierta y
//! los colores de aqui salen de `class_color`, que es donde ya vivian.

use bmo_userland as bmo;

/// Lado del dibujo. El mismo 16 que `BICO`, y por la misma razon: se guarda
/// pequeno y se agranda si hace falta.
pub(crate) const LADO: u32 = 16;

/// Los papeles, no los colores:
///
/// ```text
///   .  transparente -- se ve lo que haya debajo
///   o  contorno     -- oscuro fijo, para que la silueta se recorte
///   +  brillo       -- el color de la clase, aclarado
///   #  cuerpo       -- el color de la clase
///   -  sombra       -- el color de la clase, oscurecido
/// ```
///
/// El contorno es FIJO y no sale de la clase: un contorno tenido del mismo
/// color que el cuerpo deja de recortar la silueta y el icono se convierte en
/// una mancha.
const CONTORNO: u32 = 0x0010_141B;

/// **La carpeta.** La pestana arriba a la izquierda, que es lo que la hace
/// reconocible en cualquier sistema desde hace cuarenta anos.
pub(crate) const CARPETA: [&str; LADO as usize] = [
    "................",
    "................",
    "..oooo..........",
    ".o++++o.........",
    ".o++++oooooooo..",
    ".o############o.",
    ".o############o.",
    ".o############o.",
    ".o############o.",
    ".o############o.",
    ".o############o.",
    ".o------------o.",
    "..oooooooooooo..",
    "................",
    "................",
    "................",
];

/// **El fichero.** Una hoja con renglones. Sin la esquina doblada: a 16 pixeles
/// el doblez son tres pixeles que se leen como suciedad, no como un doblez.
pub(crate) const FICHERO: [&str; LADO as usize] = [
    "................",
    "...ooooooooooo..",
    "...o+++++++++o..",
    "...o+++++++++o..",
    "...o#########o..",
    "...o##-----##o..",
    "...o#########o..",
    "...o##-----##o..",
    "...o#########o..",
    "...o##-----##o..",
    "...o#########o..",
    "...o##---####o..",
    "...o#########o..",
    "...ooooooooooo..",
    "................",
    "................",
];

/// El dibujo que le toca a una clase de nodo.
///
/// `None` es **lo que no se pudo leer**, y no lleva dibujo a proposito: no se
/// sabe que es, asi que dibujarle una hoja o una carpeta seria contestar por el
/// disco. Lo pinta [`pintar`] como una caja con interrogacion.
pub(crate) fn para(kind: u64) -> Option<&'static [&'static str; LADO as usize]> {
    match kind {
        bmo::estratos::DIRECTORIO => Some(&CARPETA),
        bmo::estratos::ARCHIVO => Some(&FICHERO),
        _ => None,
    }
}

/// **Pinta el icono de `kind` en `(x, y)`, del color de su clase.**
///
/// `escala` multiplica el lado: a 1 son 16 pixeles, que es lo que cabe en una
/// fila de la rejilla; a 2 son 32, que es lo que usa el lanzador.
pub(crate) fn pintar(p: &bmo::Pantalla, x: u32, y: u32, kind: u64, color: u32, escala: u32) {
    let Some(arte) = para(kind) else {
        // Lo ilegible: una caja del color de su clase con una interrogacion.
        // Se reutiliza el glifo de la fuente en vez de dibujar un signo a mano
        // -- un `?` de 16 pixeles dibujado a base de `o` sale sucio, y ademas
        // seria un signo mas que mantener.
        let lado = LADO * escala;
        p.rect(x, y, lado, lado, color);
        p.glifo_escala(
            x + lado / 2 - 4 * escala,
            y + lado / 2 - 8 * escala / 2,
            b'?',
            CONTORNO,
            escala,
        );
        return;
    };
    let claro = aclarar(color);
    let oscuro = oscurecer(color);
    for (fy, fila) in arte.iter().enumerate() {
        for (fx, ch) in fila.bytes().enumerate() {
            let c = match ch {
                // El transparente se SALTA, no se pinta de negro. Sin esto, un
                // icono redondo se pinta dentro de su cuadro y la lista se
                // llena de sellos -- es la misma nota que ya tiene el lanzador.
                b'.' => continue,
                b'o' => CONTORNO,
                b'+' => claro,
                b'-' => oscuro,
                _ => color,
            };
            p.rect(
                x + fx as u32 * escala,
                y + fy as u32 * escala,
                escala,
                escala,
                c,
            );
        }
    }
}

fn aclarar(c: u32) -> u32 {
    let r = (((c >> 16) & 0xFF) + 0x28).min(0xFF);
    let g = (((c >> 8) & 0xFF) + 0x28).min(0xFF);
    let b = ((c & 0xFF) + 0x28).min(0xFF);
    (r << 16) | (g << 8) | b
}

fn oscurecer(c: u32) -> u32 {
    let r = ((c >> 16) & 0xFF) * 6 / 10;
    let g = ((c >> 8) & 0xFF) * 6 / 10;
    let b = (c & 0xFF) * 6 / 10;
    (r << 16) | (g << 8) | b
}

/// [!] Los dibujos miden lo que dicen que miden, y esto lo comprueba EL
/// COMPILADOR.
///
/// Una fila de quince caracteres no da un error: da un icono con una columna
/// menos que se nota mirandolo de cerca y nadie mira de cerca. `build.ps1` hace
/// esta misma comprobacion para los `BICO` **en tiempo de construccion**; aqui
/// sale mas barata todavia, porque no llega ni a compilar.
const _: () = {
    let mut i = 0;
    while i < LADO as usize {
        assert!(CARPETA[i].len() == LADO as usize, "una fila de la carpeta no mide 16");
        assert!(FICHERO[i].len() == LADO as usize, "una fila del fichero no mide 16");
        i += 1;
    }
};
