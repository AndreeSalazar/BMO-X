//! **LA CONSOLA DE ESTRATOS**: el terminal del pie de la ventana, `Ctrl+n`.
//!
//! === Por que un terminal PROPIO, habiendo uno ===
//!
//! No es un atajo ni una comodidad. Es un DESAMBIGUADOR, y la frase que lo
//! justifica ya estaba escrita el dia que se guardo el primer fichero:
//!
//! > ya hay un `escribe` y va a la FAT32. Dos ordenes con el mismo verbo
//! > escribiendo en volumenes distintos es como se guarda algo donde no se
//! > queria -- y aqui uno de los dos lo lee Windows y el otro no.
//!
//! El terminal de abajo del escritorio habla con FAT32. **Este habla con
//! ESTRATOS y con nada mas**, porque vive dentro de la ventana de ESTRATOS. No
//! hay que acordarse de a que volumen va una orden: va al que estas mirando.
//!
//! Es la misma ley que ya mudo `sellar` del terminal principal a esta ventana
//! --*el verbo vive donde vive el objeto*-- llevada hasta el final.
//!
//! === Por que un terminal Y una ventana con paneles ===
//!
//! Porque hay cosas que un raton no puede decir. Pulsar una carpeta la abre;
//! ningun clic **escribe un nombre**. En cuanto aparezca crear, renombrar o
//! borrar, el nombre hay que teclearlo -- y un cuadro de dialogo por cada verbo
//! es lo que convierte un escritorio en un formulario.
//!
//! El reparto es el que ya usa esta casa: **los paneles para mirar y senalar,
//! el terminal para decir**.
//!
//! === `Ctrl+n`, y por que ese ===
//!
//! Lo pidio el dueno por parecido con el panel de VS Code. Y sale bien por una
//! razon que no es el parecido: la `n` produce el byte `0xF1` en la
//! distribucion espanola (`ring0/dev/keyboard.rs`), `MOD_CTRL` llega a Ring 3 y
//! **no choca con AltGr**, que en espanol es `Ctrl+Alt` -- la trampa que ya
//! costo una sesion entera de teclado.
//!
//! === QUIEN SE QUEDA LAS TECLAS ===
//!
//! * Lo pidio el dueno tal cual: *"al seleccionar el terminal eso es prioridad
//! para que se active el atajo"*. Y hace falta, no es un adorno: las flechas
//! **ya significan algo** en esta ventana --mueven la seleccion del explorador--
//! y en un terminal significan otra cosa.
//!
//! ```text
//!   consola cerrada   las teclas son del EXPLORADOR (flechas, ENTRAR, S, V)
//!   consola abierta   las teclas son de la CONSOLA
//!   ESC               las devuelve al explorador sin cerrarla
//! ```
//!
//! Sin esa regla las dos se pelearian por la misma flecha, y el que pierde ese
//! tipo de pelea siempre es el que mira la pantalla.
//!
//! === Lo que NO hace, dicho aqui ===
//!
//! `nuevo` **solo funciona en la raiz**, porque `crear_fichero` del kernel
//! escribe la entrada en la raiz y punto (`fsys/estratos/escribir.rs`: lee
//! `dir::raiz()` y anade ahi). Estando en `/datos` y creando en `/` la ventana
//! estaria mintiendo sobre su propio contexto -- que es justo lo que esta
//! consola existe para evitar. Asi que se niega y dice por que.

use bmo_userland as bmo;

use super::zonas::Zona;
use super::{INK, INK_BAD, INK_DIM, INK_OK};
use crate::text::decimal;

/// Lo que cabe en una linea de salida.
const COLS: usize = 110;
/// Cuantas lineas de salida se guardan y se ven.
///
/// Seis. No es un terminal de trabajo: es donde se escribe una orden y se lee
/// lo que contesto. Un historial largo aqui se comeria el sitio de los paneles,
/// que es lo que de verdad se esta mirando.
const LINEAS: usize = 6;
/// Lo mas largo que se puede teclear de una vez.
const LINEA_MAX: usize = 96;

/// Alto de la zona, con sus seis lineas y el renglon de escribir.
pub(crate) const ALTO: u32 = LINEAS as u32 * bmo::GLIFO_ALTO + bmo::GLIFO_ALTO + 14;

/// Los papeles de la tinta. Se guarda el papel y no el color para que la
/// paleta pueda cambiar sin tocar lo que ya se escribio.
const T_DIM: u8 = 0;
const T_INK: u8 = 1;
const T_OK: u8 = 2;
const T_BAD: u8 = 3;

fn color_de(t: u8) -> u32 {
    match t {
        T_INK => INK,
        T_OK => INK_OK,
        T_BAD => INK_BAD,
        _ => INK_DIM,
    }
}

pub(crate) struct Consola {
    /// Se ve? La abre y la cierra `Ctrl+n`.
    pub(crate) abierta: bool,
    /// Tiene las teclas? Ver la cabecera: abrirla se las da, `ESC` las
    /// devuelve **sin cerrarla** -- para poder seguir leyendo lo que contesto
    /// mientras navegas con las flechas.
    pub(crate) activa: bool,
    linea: [u8; LINEA_MAX],
    n: usize,
    salida: [[u8; COLS]; LINEAS],
    tinta: [u8; LINEAS],
    /// Cuantas lineas de salida hay escritas, hasta `LINEAS`.
    usadas: usize,
    /// La ultima orden, para la flecha arriba.
    ///
    /// UNA, no un historial. En un terminal de dos verbos, repetir la anterior
    /// cubre casi todo lo que un historial daria, y guardar ocho lineas de 96
    /// bytes para eso es pagar por lo que no se usa.
    ultima: [u8; LINEA_MAX],
    ultima_n: usize,
}

impl Consola {
    pub(crate) const fn nueva() -> Self {
        Self {
            abierta: false,
            activa: false,
            linea: [0; LINEA_MAX],
            n: 0,
            salida: [[0; COLS]; LINEAS],
            tinta: [T_DIM; LINEAS],
            usadas: 0,
            ultima: [0; LINEA_MAX],
            ultima_n: 0,
        }
    }

    /// Escribe una linea en la salida, desplazando lo viejo hacia arriba.
    fn di(&mut self, texto: &[u8], t: u8) {
        if self.usadas == LINEAS {
            // Lleno: todo sube una y la ultima queda libre. Se copia y no se
            // lleva un indice de anillo porque son seis lineas: el anillo
            // costaria mas de leer que lo que ahorra.
            for i in 1..LINEAS {
                self.salida[i - 1] = self.salida[i];
                self.tinta[i - 1] = self.tinta[i];
            }
            self.usadas = LINEAS - 1;
        }
        let fila = self.usadas;
        self.salida[fila] = [0; COLS];
        let k = texto.len().min(COLS);
        self.salida[fila][..k].copy_from_slice(&texto[..k]);
        self.tinta[fila] = t;
        self.usadas += 1;
    }

    /// Dos trozos en una linea. Ahorra un buffer temporal en cada uso.
    fn di2(&mut self, a: &[u8], b: &[u8], t: u8) {
        let mut buf = [0u8; COLS];
        let ka = a.len().min(COLS);
        buf[..ka].copy_from_slice(&a[..ka]);
        let kb = b.len().min(COLS - ka);
        buf[ka..ka + kb].copy_from_slice(&b[..kb]);
        self.di(&buf[..ka + kb], t);
    }

    /// **Abre la consola y le da las teclas.** Si ya estaba abierta, se las
    /// devuelve -- que es lo que uno espera al volver a pulsar el atajo.
    pub(crate) fn alternar(&mut self) {
        if self.abierta && self.activa {
            self.abierta = false;
            self.activa = false;
        } else {
            if !self.abierta {
                self.abierta = true;
                self.di(b"consola de ESTRATOS. `ayuda` lista lo que hay.", T_DIM);
            }
            self.activa = true;
        }
    }

    /// Una tecla, cuando la consola tiene las teclas.
    ///
    /// Devuelve `true` si algo cambio y hay que repintar.
    pub(crate) fn tecla(&mut self, c: u8) -> bool {
        match c {
            // ESC devuelve las teclas al explorador SIN cerrar: lo que contesto
            // la orden sigue en pantalla mientras navegas.
            0x1B => {
                self.activa = false;
                true
            }
            b'\r' | b'\n' => {
                self.ejecutar();
                true
            }
            0x08 => {
                if self.n > 0 {
                    self.n -= 1;
                }
                true
            }
            // Flecha arriba: la ultima orden.
            0x80 => {
                self.linea = self.ultima;
                self.n = self.ultima_n;
                true
            }
            // Flecha abajo: limpia el renglon.
            0x81 => {
                self.n = 0;
                true
            }
            // Los imprimibles, y solo esos. Un byte de control metido en la
            // linea se veria como un glifo raro y se pasaria al parser.
            0x20..=0xFE => {
                if self.n < LINEA_MAX {
                    self.linea[self.n] = c;
                    self.n += 1;
                }
                true
            }
            _ => false,
        }
    }

    // == LAS ORDENES ========================================================
    //
    // Todas actuan sobre ESTRATOS y sobre el nodo donde esta el cursor. No hay
    // ni una que toque FAT32, y esa es la unica razon por la que esta consola
    // existe aparte de la de abajo del escritorio.

    fn ejecutar(&mut self) {
        let n = self.n;
        let mut linea = [0u8; LINEA_MAX];
        linea[..n].copy_from_slice(&self.linea[..n]);
        self.n = 0;
        let orden = recortar(&linea[..n]);
        if orden.is_empty() {
            return;
        }
        self.ultima = linea;
        self.ultima_n = n;
        // El eco, para que la salida diga a que orden contesta.
        self.di2(b"> ", orden, T_INK);

        let (verbo, resto) = partir(orden);
        match verbo {
            b"ayuda" | b"?" => self.ayuda(),
            b"ls" | b"dir" => self.ls(),
            b"donde" | b"pwd" => self.donde(),
            b"cd" => self.cd(resto),
            b"nuevo" => self.nuevo(resto),
            b"sella" | b"sellar" => self.sella(),
            // Un verbo que no existe se dice, y se dice DONDE mirar. Un "no
            // reconocido" a secas deja al que lo escribio sin siguiente paso.
            _ => {
                self.di2(verbo, b": no la conozco. `ayuda` lista lo que hay.", T_BAD);
            }
        }
    }

    fn ayuda(&mut self) {
        self.di(b"ls            lo que hay en este nodo", T_DIM);
        self.di(b"cd NOMBRE     baja. `cd ..` sube, `cd /` a la raiz", T_DIM);
        self.di(b"donde         la ruta donde estas", T_DIM);
        self.di(b"nuevo N TEXTO crea un fichero. ESCRIBE EN EL DISCO", T_DIM);
        self.di(b"sella         commitea. ESCRIBE EN EL DISCO", T_DIM);
        self.di(b"ESC devuelve las teclas al explorador. Ctrl+n cierra.", T_DIM);
    }

    fn donde(&mut self) {
        let mut buf = [0u8; COLS];
        let mut k = 1;
        buf[0] = b'/';
        let hondo = bmo::estratos::hondo();
        let mut nivel = 1u64;
        while nivel <= hondo {
            let mut nom = [0u8; 40];
            let m = bmo::estratos::nombre_nivel(nivel, &mut nom);
            if k + m + 1 >= COLS {
                break;
            }
            buf[k..k + m].copy_from_slice(&nom[..m]);
            k += m;
            buf[k] = b'/';
            k += 1;
            nivel += 1;
        }
        self.di(&buf[..k], T_INK);
    }

    fn ls(&mut self) {
        let cuantos = bmo::estratos::hijos();
        if cuantos == 0 {
            self.di(b"  (vacio)", T_DIM);
            return;
        }
        let mut i = 0u64;
        // El tope son las lineas que caben: escupir cuarenta entradas en una
        // salida de seis dejaria ver solo las ultimas, que es la mitad inutil.
        // Se ensena lo que cabe y se DICE cuantas quedan.
        let caben = (LINEAS - 1) as u64;
        while i < cuantos && i < caben {
            let mut nom = [0u8; 64];
            let m = bmo::estratos::hijo_nombre(i, &mut nom);
            let mut buf = [0u8; COLS];
            buf[0] = b' ';
            buf[1] = b' ';
            let k = m.min(COLS - 24);
            buf[2..2 + k].copy_from_slice(&nom[..k]);
            let mut j = 2 + k;
            // [!] Aqui decia `j < 22 && j < COLS`, y la segunda mitad **no se
            // evaluaba nunca**: `COLS` es 110. La intencion --no salirse del
            // buffer si algun dia la fila se estrecha-- era buena; escrita asi
            // era una guarda que no guarda, y clippy la rechaza por su nombre.
            // Decidida al COMPILAR, dice lo mismo y ademas es verdad.
            const COL_TIPO: usize = if 22 < COLS { 22 } else { COLS };
            while j < COL_TIPO {
                buf[j] = b' ';
                j += 1;
            }
            let etiqueta: &[u8] = match bmo::estratos::hijo_tipo(i) {
                bmo::estratos::DIRECTORIO => b"<DIR>  ",
                bmo::estratos::ARCHIVO => b"       ",
                _ => b"<ROTO> ",
            };
            let ke = etiqueta.len().min(COLS - j);
            buf[j..j + ke].copy_from_slice(&etiqueta[..ke]);
            j += ke;
            let mut d = [0u8; 10];
            let kd = decimal(bmo::estratos::hijo_bytes(i), &mut d);
            let kd = kd.min(COLS - j);
            buf[j..j + kd].copy_from_slice(&d[..kd]);
            j += kd;
            self.di(&buf[..j], T_INK);
            i += 1;
        }
        if cuantos > caben {
            let mut d = [0u8; 10];
            let kd = decimal(cuantos - caben, &mut d);
            let mut buf = [0u8; 24];
            buf[..2].copy_from_slice(b"y ");
            buf[2..2 + kd].copy_from_slice(&d[..kd]);
            let cola = b" mas: mirala en la rejilla";
            self.di2(&buf[..2 + kd], cola, T_DIM);
        }
    }

    fn cd(&mut self, a: &[u8]) {
        if a.is_empty() {
            self.di(b"cd QUE. `cd ..` sube, `cd /` va a la raiz.", T_BAD);
            return;
        }
        if a == b".." {
            if bmo::estratos::subir() {
                self.donde();
            } else {
                self.di(b"ya estas en la raiz.", T_DIM);
            }
            return;
        }
        if a == b"/" {
            while bmo::estratos::subir() {}
            self.donde();
            return;
        }
        let cuantos = bmo::estratos::hijos();
        let mut i = 0u64;
        while i < cuantos {
            let mut nom = [0u8; 64];
            let m = bmo::estratos::hijo_nombre(i, &mut nom);
            if igual_sin_caja(&nom[..m], a) {
                // Se dice POR QUE no se entra, y son dos motivos distintos: un
                // archivo no tiene dentro, y un nodo roto no se sabe.
                match bmo::estratos::hijo_tipo(i) {
                    bmo::estratos::DIRECTORIO => {
                        if bmo::estratos::entrar(i) {
                            self.donde();
                        } else {
                            self.di(b"no se pudo entrar. el motivo esta en F11.", T_BAD);
                        }
                    }
                    bmo::estratos::ARCHIVO => {
                        self.di2(a, b" es un archivo: no tiene dentro.", T_DIM);
                    }
                    _ => self.di2(a, b" no se pudo leer.", T_BAD),
                }
                return;
            }
            i += 1;
        }
        self.di2(a, b": aqui no hay nada con ese nombre.", T_BAD);
    }

    /// **`nuevo NOMBRE TEXTO`** -- lo unico de esta consola que crea algo.
    fn nuevo(&mut self, a: &[u8]) {
        let (nombre, texto) = partir(a);
        if nombre.is_empty() {
            self.di(b"nuevo NOMBRE TEXTO", T_BAD);
            return;
        }
        // ** SOLO EN LA RAIZ, Y SE NIEGA EN VEZ DE MENTIR.
        //
        // `crear_fichero` del kernel anade la entrada en la RAIZ: no mira donde
        // esta el cursor. Dejarlo pasar desde `/datos` crearia el fichero en
        // `/` mientras la ventana ensena `/datos` -- o sea, la ventana mintiendo
        // sobre su propio contexto, que es exactamente lo que esta consola
        // existe para que no pase.
        if bmo::estratos::hondo() != 0 {
            self.di(b"hoy solo se crea en la RAIZ: el kernel anade la", T_BAD);
            self.di(b"entrada ahi. sube con `cd /` y vuelve a pedirlo.", T_DIM);
            return;
        }
        if texto.len() as u64 > bmo::estratos::ES_CREAR_MAX {
            let mut d = [0u8; 10];
            let kd = decimal(bmo::estratos::ES_CREAR_MAX, &mut d);
            self.di2(b"no cabe. el tope de hoy son ", &d[..kd], T_BAD);
            self.di(b"bytes: es lo que entra DENTRO del nodo, sin gastar", T_DIM);
            self.di(b"un bloque de datos. mas grande pide un arbol.", T_DIM);
            return;
        }
        match bmo::estratos::crear_fichero(nombre, texto) {
            0 => {
                self.di(b"NO se creo. el volumen sigue igual.", T_BAD);
                self.di(b"el motivo esta en F11 (nombre repetido, sin sitio,", T_DIM);
                self.di(b"o la escritura cerrada).", T_DIM);
            }
            g => {
                let mut d = [0u8; 10];
                let kd = decimal(g, &mut d);
                self.di2(b"CREADO. generacion ", &d[..kd], T_OK);
                self.di(b"reinicia y mira si sigue: eso es lo que lo prueba.", T_DIM);
            }
        }
    }

    fn sella(&mut self) {
        match bmo::estratos_sellar() {
            0 => self.di(b"NO se sello. el motivo esta en F11.", T_BAD),
            g => {
                let mut d = [0u8; 10];
                let kd = decimal(g, &mut d);
                self.di2(b"SELLADO. generacion ", &d[..kd], T_OK);
            }
        }
    }
}

/// Pinta la consola en su zona.
pub(crate) fn paint(p: &bmo::Pantalla, z: &Zona, c: &Consola, borde: u32, acento: u32) {
    if !z.hay() || !c.abierta {
        return;
    }
    p.rect(z.x, z.y, z.w, 1, borde);

    let mut y = z.y + 5;
    for i in 0..c.usadas {
        let fila = &c.salida[i];
        let n = fila.iter().position(|&b| b == 0).unwrap_or(COLS);
        p.texto_bytes(z.x + 2, y, &fila[..n], color_de(c.tinta[i]));
        y += bmo::GLIFO_ALTO;
    }

    // El renglon de escribir, abajo del todo de la zona.
    let ry = z.abajo() - bmo::GLIFO_ALTO - 4;
    // ** El indicador dice a QUE volumen le estas hablando, y por eso pone
    // `estratos>` y no `>`. Es un terminal que vive dentro de otra ventana con
    // otro terminal a dos palmos: el que escribe tiene que poder ver cual es
    // sin acordarse.
    //
    // Y se apaga cuando las teclas no son suyas: un cursor encendido en una
    // caja que no recibe lo que tecleas es la peor mentira que puede contar un
    // terminal.
    let ink = if c.activa { acento } else { INK_DIM };
    let x = p.texto(z.x + 2, ry, "estratos>", ink);
    let x = p.texto_bytes(x + bmo::GLIFO_ANCHO, ry, &c.linea[..c.n], INK);
    if c.activa {
        p.rect(x + 1, ry, 2, bmo::GLIFO_ALTO, acento);
    } else {
        p.texto(x + 2 * bmo::GLIFO_ANCHO, ry, "(ESC devolvio las teclas: Ctrl+n las recupera)", INK_DIM);
    }
}

// -- Cortar palabras, que es todo el parser que hace falta ------------------

fn recortar(s: &[u8]) -> &[u8] {
    let mut a = 0;
    while a < s.len() && s[a] == b' ' {
        a += 1;
    }
    let mut b = s.len();
    while b > a && s[b - 1] == b' ' {
        b -= 1;
    }
    &s[a..b]
}

/// Parte en la primera palabra y el resto, ya recortado.
fn partir(s: &[u8]) -> (&[u8], &[u8]) {
    match s.iter().position(|&c| c == b' ') {
        Some(i) => (&s[..i], recortar(&s[i + 1..])),
        None => (s, &s[0..0]),
    }
}

/// Compara sin distinguir mayusculas, que es como compara ESTRATOS los nombres.
fn igual_sin_caja(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}
