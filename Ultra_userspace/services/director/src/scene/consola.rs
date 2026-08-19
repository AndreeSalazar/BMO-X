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
//! === ** TODO ACTUA DONDE ESTAS, y eso costo bajar hasta el disco ===
//!
//! Hasta el 19-08 `nuevo` **se negaba fuera de la raiz**: el kernel anadia la
//! entrada en `dir::raiz()` y punto, asi que crear en `/` mientras la ventana
//! ensena `/datos` habria sido la ventana mintiendo sobre su propio contexto --
//! justo lo que esta consola existe para evitar. Se nego, y dijo por que.
//!
//! Ya no hace falta. La maquina de abajo republica la rama entera hasta la raiz
//! (`escribir::aplicar`), asi que los cuatro verbos mandan **la ruta completa
//! del destino** y actuan donde esta el cursor.
//!
//! Esa es la frase entera de esta consola: **el contexto es el cursor**, que es
//! lo unico que un terminal dentro de una ventana tiene y uno de fuera no.
//!
//! [!] Y despues de cada gesto se manda `recargar`. Sin eso el cursor seguiria
//! ensenando el estrato de antes: borrarias un fichero y ahi seguiria.

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
    /// Ya se saludo alguna vez?
    ///
    /// [!] Sin esto, `alternar` soltaba la linea de bienvenida en CADA
    /// apertura: abrir y cerrar tres veces dejaba tres saludos iguales
    /// apilados, comiendose la mitad de las seis lineas de salida. Visto en el
    /// Ryzen el 2026-08-18 -- y el sintoma no era "un mensaje de mas", era que
    /// la respuesta a la orden que acababas de escribir ya no cabia.
    estrenada: bool,
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
            estrenada: false,
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
                // El saludo es una PRESENTACION, y a uno se le presenta una
                // vez. Repetirlo en cada apertura no informa de nada nuevo y
                // empuja hacia arriba lo unico que importa, que es lo que
                // contesto la ultima orden.
                if !self.estrenada {
                    self.estrenada = true;
                    self.di(b"consola de ESTRATOS. `ayuda` lista lo que hay.", T_DIM);
                }
            }
            self.activa = true;
        }
    }

    /// **Le pone una orden ya escrita.** La usa el menu del clic derecho.
    ///
    /// `ejecutar` la lanza; si no, deja el cursor puesto al final para que
    /// termines de teclear -- que es lo que pasa con `renombra`, donde falta un
    /// nombre que solo sabes tu.
    ///
    /// ** Y abre la consola si estaba cerrada. Una orden que se ejecuta sin que
    /// se vea donde es exactamente lo que este terminal existe para evitar: aqui
    /// se escribe lo que se hace, y se ve escrito.
    pub(crate) fn poner_orden(&mut self, verbo: &str, arg: &[u8], ejecutar: bool) {
        if !self.abierta {
            self.alternar();
        }
        self.activa = true;
        self.n = 0;
        for b in verbo.as_bytes() {
            if self.n < LINEA_MAX {
                self.linea[self.n] = *b;
                self.n += 1;
            }
        }
        if !arg.is_empty() && self.n < LINEA_MAX {
            self.linea[self.n] = b' ';
            self.n += 1;
            for b in arg {
                if self.n < LINEA_MAX {
                    self.linea[self.n] = *b;
                    self.n += 1;
                }
            }
        }
        // El espacio del final cuando falta un argumento: se ve que la orden no
        // esta terminada, sin tener que leerla entera.
        if !ejecutar && self.n < LINEA_MAX {
            self.linea[self.n] = b' ';
            self.n += 1;
        }
        if ejecutar {
            self.ejecutar();
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
            // ** LA BUSCO EL DUENO EL PRIMER DIA Y NO ESTABA.
            //
            // Con seis lineas de salida, limpiar no es una comodidad: es la
            // unica forma de que la respuesta siguiente se lea entera. Se
            // aceptan las dos palabras --la inglesa porque es la que sale de
            // los dedos, la castellana porque es el idioma de la casa-- y no
            // es una concesion: un terminal que rechaza la palabra que todo el
            // mundo teclea gasta una linea de error en decirlo.
            b"clear" | b"limpia" | b"cls" => {
                self.usadas = 0;
            }
            b"ls" | b"dir" => self.ls(),
            b"donde" | b"pwd" => self.donde(),
            b"cd" => self.cd(resto),
            // ** `new` es el nombre y `nuevo` sigue valiendo.
            //
            // Lo pidio el dueno: *"da flojera poner nuevo, ta raro decir
            // nuevo"*. Y no rompe nada de lo ya tecleado -- la casa ya acepta
            // las dos formas en `clear`/`limpia` y en `mkdir`/`carpeta`. Lo que
            // cambia es cual se ANUNCIA en la ayuda, que es lo unico que
            // ensena que palabra usar.
            b"new" | b"nuevo" => self.nuevo(resto),
            b"carpeta" | b"mkdir" => self.carpeta(resto),
            b"copia" | b"copy" | b"cp" => self.copia(resto),
            b"marca" | b"mark" | b"tag" => self.marca(resto),
            b"vuelve" | b"revert" => self.vuelve(resto),
            b"borra" | b"quita" => self.borra(resto),
            b"renombra" | b"mv" => self.renombra(resto),
            b"sella" | b"sellar" => self.sella(),
            // Un verbo que no existe se dice, y se dice DONDE mirar. Un "no
            // reconocido" a secas deja al que lo escribio sin siguiente paso.
            _ => {
                self.di2(verbo, b": no la conozco. `ayuda` lista lo que hay.", T_BAD);
            }
        }
    }

    /// La ayuda LIMPIA antes de escribir, y cabe justa en las seis lineas.
    ///
    /// ** No es un detalle de aspecto. La salida son seis lineas; la ayuda
    /// ocupaba ocho contando el eco, asi que las dos primeras --entre ellas
    /// `ls`, que es la orden mas basica que hay-- se salian por arriba antes de
    /// que nadie pudiera leerlas. Una ayuda que se corta sola es peor que no
    /// tenerla: dice que existe algo y no dice que.
    fn ayuda(&mut self) {
        self.usadas = 0;
        self.di(b"ls  cd NOMBRE  cd ..  cd /  donde   moverse y mirar", T_DIM);
        self.di(b"new N TEXTO     crea un fichero      ESCRIBEN EN EL DISCO", T_DIM);
        self.di(b"carpeta N       crea una carpeta     y todos actuan DONDE", T_DIM);
        self.di(b"copia ORG DST   trae de FAT32        ESTAS, no en la raiz", T_DIM);
        self.di(b"borra N         deja de nombrarla", T_DIM);
        self.di(b"marca NOMBRE    fija esta version    vuelve N: atras", T_DIM);
        self.di(b"ESC suelta las teclas.  Ctrl+n cierra.  arriba: la anterior.", T_DIM);
    }

    /// **La ruta del sitio donde esta el cursor, con `nombre` al final.**
    ///
    /// ** Esto es lo que convierte a esta consola en lo que decia ser. Antes
    /// mandaba solo el nombre, el kernel lo anadia a la raiz, y por eso `nuevo`
    /// se NEGABA fuera de la raiz: crear en `/` mientras la ventana ensena
    /// `/datos` habria sido la ventana mintiendo sobre su propio contexto.
    ///
    /// Ahora se manda el destino entero. **El contexto es el cursor**, que es
    /// lo unico que un terminal dentro de una ventana tiene y uno de fuera no.
    fn ruta_de(&self, nombre: &[u8], dst: &mut [u8; COLS]) -> usize {
        let mut k = 0usize;
        let hondo = bmo::estratos::hondo();
        let mut nivel = 1u64;
        while nivel <= hondo {
            let mut nom = [0u8; 40];
            let m = bmo::estratos::nombre_nivel(nivel, &mut nom);
            if k + m + 1 >= COLS {
                return 0;
            }
            dst[k..k + m].copy_from_slice(&nom[..m]);
            k += m;
            dst[k] = b'/';
            k += 1;
            nivel += 1;
        }
        if k + nombre.len() > COLS {
            return 0;
        }
        dst[k..k + nombre.len()].copy_from_slice(nombre);
        k + nombre.len()
    }

    /// Lo que se hace despues de CUALQUIER gesto que salga bien.
    ///
    /// [!] `recargar` no es un adorno: el cursor guarda el listado de cada nivel
    /// desde que se paso por el, asi que sin esto la ventana seguiria ensenando
    /// el estrato de antes -- borrarias un fichero y ahi seguiria.
    fn hecho(&mut self, g: u64, que: &[u8]) {
        if g == 0 {
            self.di2(que, b": NO se hizo. el volumen sigue igual.", T_BAD);
            self.di(b"el motivo esta en F11.", T_DIM);
            return;
        }
        bmo::estratos::recargar();
        // ** Y la HISTORIA tambien. Tenia el mismo cabo suelto que el cursor:
        // cada gesto publica una version nueva, asi que la lista del historial
        // se quedaba con la cadena de antes -- marcabas algo y no aparecia.
        bmo::estratos::hist_releer();
        let mut d = [0u8; 10];
        let kd = decimal(g, &mut d);
        // ** UNA linea, no dos. Estaba partido --`... generacion` y el numero
        // en la de abajo-- y en pantalla se leia como una frase cortada. Visto
        // en el Ryzen el 2026-08-19, en la foto del primer fichero guardado.
        let mut buf = [0u8; COLS];
        let mut k = que.len().min(COLS);
        buf[..k].copy_from_slice(&que[..k]);
        for b in b" HECHO. generacion " {
            if k < COLS { buf[k] = *b; k += 1; }
        }
        for b in &d[..kd] {
            if k < COLS { buf[k] = *b; k += 1; }
        }
        self.di(&buf[..k], T_OK);
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

    /// **`nuevo NOMBRE TEXTO`** -- crea un fichero DONDE ESTAS.
    fn nuevo(&mut self, a: &[u8]) {
        let (nombre, texto) = partir(a);
        if nombre.is_empty() {
            self.di(b"new NOMBRE TEXTO", T_BAD);
            return;
        }
        if texto.len() as u64 > bmo::estratos::ES_GESTO_MAX {
            let mut d = [0u8; 10];
            let kd = decimal(bmo::estratos::ES_GESTO_MAX, &mut d);
            self.di2(b"no cabe. el tope de hoy son ", &d[..kd], T_BAD);
            self.di(b"bytes: es lo que entra DENTRO del nodo, sin gastar", T_DIM);
            self.di(b"un bloque de datos. mas grande pide un arbol.", T_DIM);
            return;
        }
        let mut ruta = [0u8; COLS];
        let n = self.ruta_de(nombre, &mut ruta);
        if n == 0 {
            self.di(b"esa ruta no cabe.", T_BAD);
            return;
        }
        let g = bmo::estratos::crear_fichero(&ruta[..n], texto);
        self.hecho(g, b"fichero");
    }

    /// **`vuelve N`** -- el arbol de hace N versiones, sin copiar nada.
    ///
    /// ** No pierde lo de en medio: el estrato nuevo tiene por padre la punta
    /// de ahora, asi que la cadena entera sigue ahi y esta vuelta se puede
    /// deshacer igual. Es un *revert*, no un *reset*.
    fn vuelve(&mut self, a: &[u8]) {
        let n = numero(a);
        if n == 0 {
            self.di(b"vuelve N   (N versiones hacia atras, 1 o mas)", T_BAD);
            self.di(b"mira la pestana historial para ver cuales hay.", T_DIM);
            return;
        }
        let g = bmo::estratos::volver(n);
        if g != 0 {
            self.di(b"el arbol es el de entonces. la historia sigue entera:", T_DIM);
            self.di(b"esta vuelta tambien es una version, y se puede deshacer.", T_DIM);
        }
        self.hecho(g, b"vuelta");
    }

    /// **`marca NOMBRE`** -- esta version no se suelta jamas.
    ///
    /// ** La unica orden que no toca ni un fichero y aun asi escribe en el
    /// disco. Lo que publica es un estrato que apunta al MISMO arbol con un
    /// nombre puesto -- un bloque, y a partir de ahi el recolector no puede
    /// llevarse esa version ni lo que cuelga de ella.
    ///
    /// Los gestos normales van sin nombre a proposito: son automaticos y el
    /// volumen tiene que poder adelgazar. Esto es el acto de una persona
    /// diciendo *"a esta quiero poder volver siempre"*.
    fn marca(&mut self, a: &[u8]) {
        if a.is_empty() {
            self.di(b"marca NOMBRE", T_BAD);
            self.di(b"la version de ahora queda fija: nadie la suelta.", T_DIM);
            return;
        }
        let g = bmo::estratos::marcar(a);
        if g != 0 {
            self.di2(b"marcada: ", a, T_DIM);
            self.di(b"esta version ya no se suelta. el resto si adelgaza.", T_DIM);
        }
        self.hecho(g, b"marca");
    }

    /// **`copia ORIGEN DESTINO`** -- trae un fichero de FAT32.
    ///
    /// ** La unica orden de esta consola que menciona el OTRO volumen, y lo dice
    /// en su respuesta. El resto actua solo sobre ESTRATOS; esta cruza, y una
    /// orden que cruza sin decirlo es como se guarda algo donde no se queria.
    ///
    /// El origen es una ruta de FAT32 tal cual --`c/holac.bex`--, y el destino
    /// es un nombre que cae DONDE ESTAS, como los demas verbos.
    fn copia(&mut self, a: &[u8]) {
        let (origen, destino) = partir(a);
        if origen.is_empty() || destino.is_empty() {
            self.di(b"copia ORIGEN DESTINO", T_BAD);
            self.di(b"el origen es de FAT32; el destino cae donde estas.", T_DIM);
            return;
        }
        let mut ruta = [0u8; COLS];
        let n = self.ruta_de(destino, &mut ruta);
        if n == 0 {
            self.di(b"esa ruta no cabe.", T_BAD);
            return;
        }
        let g = bmo::estratos::copiar(&ruta[..n], origen);
        if g != 0 {
            self.di2(b"traido de FAT32: ", origen, T_DIM);
        }
        self.hecho(g, b"copia");
    }

    /// **`carpeta NOMBRE`** -- una carpeta vacia donde estas.
    fn carpeta(&mut self, a: &[u8]) {
        if a.is_empty() {
            self.di(b"carpeta NOMBRE", T_BAD);
            return;
        }
        let mut ruta = [0u8; COLS];
        let n = self.ruta_de(a, &mut ruta);
        if n == 0 {
            self.di(b"esa ruta no cabe.", T_BAD);
            return;
        }
        let g = bmo::estratos::crear_carpeta(&ruta[..n]);
        self.hecho(g, b"carpeta");
    }

    /// **`borra NOMBRE`** -- quita una entrada de donde estas.
    ///
    /// ** No pide confirmacion, y no es un descuido: **en ESTRATOS borrar no
    /// destruye**. Se publica un arbol sin esa entrada; el nodo, su contenido y
    /// el estrato de ayer siguen enteros. Pedir un "seguro?" para algo que no
    /// pierde nada ensena a contestar que si sin leer -- y entonces el dia que
    /// se pregunte de verdad, tampoco se leera.
    fn borra(&mut self, a: &[u8]) {
        if a.is_empty() {
            self.di(b"borra NOMBRE", T_BAD);
            return;
        }
        let mut ruta = [0u8; COLS];
        let n = self.ruta_de(a, &mut ruta);
        if n == 0 {
            self.di(b"esa ruta no cabe.", T_BAD);
            return;
        }
        let g = bmo::estratos::quitar(&ruta[..n]);
        if g != 0 {
            self.di(b"deja de nombrarse. el estrato de ayer lo sigue teniendo.", T_DIM);
        }
        self.hecho(g, b"quitado");
    }

    /// **`renombra VIEJO NUEVO`** -- sin tocar el nodo.
    fn renombra(&mut self, a: &[u8]) {
        let (viejo, nuevo) = partir(a);
        if viejo.is_empty() || nuevo.is_empty() {
            self.di(b"renombra VIEJO NUEVO", T_BAD);
            return;
        }
        let mut ruta = [0u8; COLS];
        let n = self.ruta_de(viejo, &mut ruta);
        if n == 0 {
            self.di(b"esa ruta no cabe.", T_BAD);
            return;
        }
        let g = bmo::estratos::renombrar(&ruta[..n], nuevo);
        if g != 0 {
            self.di(b"el nodo no se ha tocado: su firma sigue valiendo.", T_DIM);
        }
        self.hecho(g, b"renombrado");
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

/// Un entero decimal, o `0` si no lo es.
///
/// El cero vale por las dos cosas --"no es un numero" y "cero"-- y aqui da
/// igual: `vuelve 0` es volver a donde ya estas, que tampoco se hace.
fn numero(s: &[u8]) -> u64 {
    let mut v = 0u64;
    let mut hay = false;
    for b in s {
        if !b.is_ascii_digit() {
            return 0;
        }
        v = v.saturating_mul(10).saturating_add((b - b'0') as u64);
        hay = true;
    }
    if hay { v } else { 0 }
}

/// Compara sin distinguir mayusculas, que es como compara ESTRATOS los nombres.
fn igual_sin_caja(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}
