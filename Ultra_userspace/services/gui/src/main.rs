//! **El compositor de BMO.** El proceso Ring 3 que es dueño de la pantalla.
//!
//! ## La caja
//!
//! No hay terminal. Había uno planeado —`apps/terminal`, doce líneas de
//! esqueleto— y se ha quitado, porque un terminal de verdad es una pila entera:
//! scrollback, PTY, señales, un intérprete, edición de línea, historial. Nada de
//! eso hace falta para lo único que hoy se quiere hacer desde la pantalla, que
//! es **arrancar un programa**.
//!
//! Así que lo que hay es una caja de una línea, como el `Win+R` de Windows.
//! Escribes una ruta, pulsas Enter, y el `.bex` corre. Es la forma más pequeña
//! de "terminal" que sigue siendo útil, y no arrastra nada de lo otro.
//!
//! ★ Y no es una API prestada de nadie: `Win+R` tampoco lo es allí. Es UI del
//! shell, y por debajo acaba llamando a lo mismo que llamaría cualquiera. Aquí
//! por debajo hay `OP_EJECUTAR` sobre `CURRENT_TASK`, que es una operación más
//! en una tabla — **el ABI de tres syscalls no se toca para esto**.
//!
//! ## Quién manda sobre el teclado
//!
//! Reclamar `KIND_INPUT` ahora cede el teclado además del ratón, y eso tiene
//! consecuencia al otro lado: mientras este proceso viva, el shell de Ring 0 no
//! lee el teclado físico. No es un reparto —los dos drenarían la misma cola y
//! se robarían letras— es una cesión. El cable serie sigue siendo del kernel,
//! que es lo que hace falta cuando esto se rompa.
//!
//! ## La tira de medida sigue
//!
//! Los seis parches de color siguen ahí abajo porque la pregunta que hacen
//! sigue abierta: en la primera foto en hardware la geometría salió exacta pero
//! los colores mucho más claros de lo que dice el código. Hasta que una foto lo
//! zanje, se quedan —
//!
//! - si el parche `0x00FF0000` sale ROJO, el formato es XRGB como creemos;
//! - si sale AZUL, los canales están al revés (BGR) y hay que voltearlos;
//! - si `0x00202020` sale gris medio en vez de casi negro, no es orden de
//!   canales: algo toca la intensidad (el panel, o el propio GOP).

#![no_std]
#![no_main]

use bmo_userland as bmo;

// ── La escena ───────────────────────────────────────────────────────────

const FONDO: u32 = 0x0014_1C2B;
const BARRA: u32 = 0x0028_3448;
const ACENTO: u32 = 0x004C_9BE8;

const BARRA_ALTO: u32 = 44;

/// Los seis parches de medida, con sus valores EXACTOS. No son decorativos:
/// cada uno responde una pregunta distinta sobre el formato.
const MEDIDA: [u32; 6] = [
    0x00FF_0000, // ¿rojo o azul? -> orden de canales
    0x0000_FF00, // verde: el canal de en medio no cambia con el orden
    0x0000_00FF, // el complementario del primero
    0x00FF_FFFF, // blanco: el techo
    0x0080_8080, // gris medio: la mitad
    0x0020_2020, // casi negro: si esto sale claro, no es orden, es intensidad
];
const MEDIDA_LADO: u32 = 56;
const MEDIDA_Y: u32 = BARRA_ALTO + 24;
const MEDIDA_X: u32 = 24;

/// Pulsómetro del ratón: cuántos reportes HID han llegado. Quieto = el ratón no
/// llega; creciendo = late. Ahora que hay fuente podría escribirse el número,
/// pero una barra se lee de un vistazo desde el otro lado del cuarto, que es
/// desde donde se mira una máquina que está arrancando.
const PULSO_X: u32 = 24;
const PULSO_Y: u32 = MEDIDA_Y + MEDIDA_LADO + 32;
const PULSO_ANCHO: u32 = 240;
const PULSO_ALTO: u32 = 14;

// ── La caja ─────────────────────────────────────────────────────────────

const CAJA_ANCHO: u32 = 760;
const CAJA_ALTO: u32 = 428;

/// La rejilla de SALIDA: lo que imprimen los programas que se lanzan desde
/// aquí. Antes no existía y no era un olvido — **no había dónde leerlo**:
/// `OP_CONSOLE_WRITE` iba siempre al panel del kernel, así que un terminal de
/// Ring 3 no podía ver lo que escribía su propio hijo. Con `KIND_CONSOLE` la
/// salida tiene dueño, y el dueño es este proceso.
const SAL_COLS: usize = 88;
const SAL_ROWS: usize = 16;
const SAL_TEXTO: u32 = 0x00C8_D8E8;
const SAL_ECO: u32 = 0x0079_C4F2;
const CAJA_FONDO: u32 = 0x001E_2A40;
const CAJA_BORDE: u32 = 0x004C_9BE8;
const CAMPO_FONDO: u32 = 0x000C_1220;
const TEXTO: u32 = 0x00E6_EDF6;
const TEXTO_TENUE: u32 = 0x008A_9BB4;
const TEXTO_MAL: u32 = 0x00FF_8A7A;
const TEXTO_BIEN: u32 = 0x007E_E787;

/// Cuántos bytes de ruta caben. Es el mismo tope que el renglón del kernel
/// (`RUTA_MAX`), y no por casualidad: escribir más de lo que el otro lado puede
/// aceptar sería dejar que la ruta se corte en silencio a mitad de camino.
const RUTA_MAX: usize = 128;

/// Geometría de la caja, ya resuelta contra el tamaño real del panel.
struct Caja {
    x: u32,
    y: u32,
    campo_x: u32,
    campo_y: u32,
    campo_ancho: u32,
    campo_alto: u32,
    texto_x: u32,
    texto_y: u32,
    estado_y: u32,
    salida_x: u32,
    salida_y: u32,
}

impl Caja {
    fn nueva(ancho: u32, alto: u32) -> Self {
        // Centrada horizontalmente; algo por encima del centro vertical, que es
        // donde el ojo la busca y donde no pisa la tira de medida.
        let x = ancho.saturating_sub(CAJA_ANCHO) / 2;
        let y = alto / 2;
        let campo_x = x + 18;
        let campo_y = y + 54;
        let campo_ancho = CAJA_ANCHO - 36;
        let campo_alto = 28;
        Self {
            x,
            y,
            campo_x,
            campo_y,
            campo_ancho,
            campo_alto,
            texto_x: campo_x + 6,
            texto_y: campo_y + 6,
            // El estado va JUSTO debajo del campo, no al fondo de la caja: el
            // fondo es ahora la salida, y un mensaje de error a veinte líneas
            // de distancia de la línea que lo causó no lo lee nadie.
            estado_y: campo_y + campo_alto + 10,
            salida_x: x + 18,
            salida_y: campo_y + campo_alto + 40,
        }
    }

    /// Alto de la rejilla de salida, en píxeles.
    fn salida_alto(&self) -> u32 {
        SAL_ROWS as u32 * bmo::GLIFO_ALTO
    }

    /// Cuántos caracteres caben en el campo. El resto se recorta al pintar —
    /// nunca al guardar: lo que no se ve sigue estando en la ruta.
    fn visibles(&self) -> usize {
        ((self.campo_ancho - 12) / bmo::GLIFO_ANCHO) as usize
    }

    fn contiene(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + CAJA_ANCHO && y >= self.y && y < self.y + CAJA_ALTO
    }
}

/// Qué color le toca a un píxel según la escena. Es el modelo entero del
/// escritorio, y es lo que permite borrar el cursor sin repintarlo todo: para
/// restaurar una zona basta con volver a preguntar qué había ahí.
///
/// Sabe de rectángulos, no de letras. Por eso `borrar_cursor` avisa cuando ha
/// pasado por encima de la caja: el texto hay que volver a escribirlo.
fn color_escena(c: &Caja, visible: bool, x: u32, y: u32) -> u32 {
    if y < BARRA_ALTO {
        // La marca de referencia dentro de la barra.
        if x >= 16 && x < 32 && y >= 14 && y < 30 {
            return ACENTO;
        }
        return BARRA;
    }
    if visible && c.contiene(x, y) {
        // Borde de 2 px.
        let en_borde = x < c.x + 2
            || x >= c.x + CAJA_ANCHO - 2
            || y < c.y + 2
            || y >= c.y + CAJA_ALTO - 2;
        if en_borde {
            return CAJA_BORDE;
        }
        if x >= c.campo_x
            && x < c.campo_x + c.campo_ancho
            && y >= c.campo_y
            && y < c.campo_y + c.campo_alto
        {
            return CAMPO_FONDO;
        }
        return CAJA_FONDO;
    }
    if y >= MEDIDA_Y && y < MEDIDA_Y + MEDIDA_LADO && x >= MEDIDA_X {
        let i = (x - MEDIDA_X) / MEDIDA_LADO;
        if (i as usize) < MEDIDA.len() {
            return MEDIDA[i as usize];
        }
    }
    FONDO
}

// ── El cursor ───────────────────────────────────────────────────────────

const CUR_ANCHO: usize = 10;
const CUR_ALTO: usize = 16;
/// 0 = transparente, 1 = relleno, 2 = borde.
///
/// Borde oscuro alrededor del relleno claro: es lo que hace que una flecha se
/// vea igual de bien sobre un fondo claro que sobre uno oscuro. No es adorno,
/// es la razón de que todos los cursores del mundo tengan contorno.
const FLECHA: [[u8; CUR_ANCHO]; CUR_ALTO] = [
    [2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 2, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 2, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 1, 2, 0, 0, 0, 0, 0, 0],
    [2, 1, 1, 1, 2, 0, 0, 0, 0, 0],
    [2, 1, 1, 1, 1, 2, 0, 0, 0, 0],
    [2, 1, 1, 1, 1, 1, 2, 0, 0, 0],
    [2, 1, 1, 1, 1, 1, 1, 2, 0, 0],
    [2, 1, 1, 1, 1, 1, 1, 1, 2, 0],
    [2, 1, 1, 1, 1, 1, 2, 2, 2, 2],
    [2, 1, 1, 2, 1, 1, 2, 0, 0, 0],
    [2, 1, 2, 0, 2, 1, 1, 2, 0, 0],
    [2, 2, 0, 0, 2, 1, 1, 2, 0, 0],
    [2, 0, 0, 0, 0, 2, 1, 1, 2, 0],
    [0, 0, 0, 0, 0, 2, 1, 2, 0, 0],
    [0, 0, 0, 0, 0, 0, 2, 2, 0, 0],
];
const CUR_RELLENO: u32 = 0x00FF_FFFF;
const CUR_BORDE: u32 = 0x0000_0000;

fn dibujar_cursor(p: &bmo::Pantalla, x: u32, y: u32) {
    for (fila, linea) in FLECHA.iter().enumerate() {
        for (col, &v) in linea.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let color = if v == 1 { CUR_RELLENO } else { CUR_BORDE };
            p.punto(x + col as u32, y + fila as u32, color);
        }
    }
}

/// Restaura de la escena el rectángulo donde estaba el cursor. Devuelve `true`
/// si ese rectángulo tocaba la caja — y entonces hay letras que reescribir,
/// porque la escena sabe de rectángulos pero no de glifos.
fn borrar_cursor(p: &bmo::Pantalla, c: &Caja, visible: bool, x: u32, y: u32) -> bool {
    let mut toco = false;
    for fila in 0..CUR_ALTO as u32 {
        for col in 0..CUR_ANCHO as u32 {
            let (px, py) = (x + col, y + fila);
            if visible && c.contiene(px, py) {
                toco = true;
            }
            p.punto(px, py, color_escena(c, visible, px, py));
        }
    }
    toco
}

// ── Pintar la caja ──────────────────────────────────────────────────────

/// El marco entero. Se pinta UNA vez; después sólo se repinta el campo.
/// La caja, con algo de forma.
///
/// Era un rectángulo con un borde de 2 px y el título escrito encima del mismo
/// fondo: plano, y con las cuatro esquinas en pico. Cinco cosas lo arreglan sin
/// salir de `rect` y `texto`, que es todo lo que tiene esta pantalla:
///
/// 1. **Sombra** desplazada — un rectángulo oscuro detrás. Es lo que despega la
///    caja del fondo y lo que más se nota en una foto.
/// 2. **Barra de título** con su propio fondo, en vez de texto suelto.
/// 3. **Línea de acento** bajo la barra: separa sin dibujar un borde entero.
/// 4. **Esquinas biseladas** — se repinta el color de fondo en el píxel de cada
///    esquina. Cuatro rectángulos de 1x1 y deja de parecer un cuadro de diálogo
///    de hace treinta años.
/// 5. El campo de entrada con **marco propio** y un `>` de aviso, para que se
///    vea que ahí se escribe.
fn pintar_caja(p: &bmo::Pantalla, c: &Caja) {
    const SOMBRA: u32 = 0x0008_0D16;
    const TITULO_FONDO: u32 = 0x0026_3550;

    // 1. La sombra, primero y desplazada.
    p.rect(c.x + 6, c.y + 6, CAJA_ANCHO, CAJA_ALTO, SOMBRA);

    // El marco y el relleno.
    p.rect(c.x, c.y, CAJA_ANCHO, CAJA_ALTO, CAJA_BORDE);
    p.rect(c.x + 1, c.y + 1, CAJA_ANCHO - 2, CAJA_ALTO - 2, CAJA_FONDO);

    // 4. Biselar: el píxel de cada esquina vuelve al fondo de la pantalla.
    p.rect(c.x, c.y, 1, 1, FONDO);
    p.rect(c.x + CAJA_ANCHO - 1, c.y, 1, 1, FONDO);
    p.rect(c.x, c.y + CAJA_ALTO - 1, 1, 1, FONDO);
    p.rect(c.x + CAJA_ANCHO - 1, c.y + CAJA_ALTO - 1, 1, 1, FONDO);

    // 2 y 3. La barra de título y su acento.
    p.rect(c.x + 1, c.y + 1, CAJA_ANCHO - 2, 26, TITULO_FONDO);
    p.rect(c.x + 1, c.y + 27, CAJA_ANCHO - 2, 1, ACENTO);
    p.texto(c.x + 18, c.y + 6, "BMO-X", ACENTO);
    p.texto(c.x + 82, c.y + 6, "Ejecutar", TEXTO);

    p.texto(
        c.x + 18,
        c.y + 36,
        "ruta de un .bex y Enter.  info / cpu / mem / ls / lee / reboot.",
        TEXTO_TENUE,
    );

    // 5. El campo, con marco y con su aviso.
    p.rect(c.campo_x - 1, c.campo_y - 1, c.campo_ancho + 2, c.campo_alto + 2, ACENTO);
    p.rect(c.campo_x, c.campo_y, c.campo_ancho, c.campo_alto, CAMPO_FONDO);
}

/// El contenido del campo: la ruta y el cursor de escritura.
///
/// Repinta el fondo del campo entero antes de escribir. Es un rectángulo de
/// unos 500x28 px —nada— y evita el clásico de borrar un carácter y que quede
/// medio glifo del anterior porque el nuevo es más estrecho.
fn pintar_campo(p: &bmo::Pantalla, c: &Caja, ruta: &[u8], cur: usize, caret: bool) {
    p.rect(c.campo_x, c.campo_y, c.campo_ancho, c.campo_alto, CAMPO_FONDO);

    // La ventana visible se calcula alrededor del CURSOR, no del final.
    //
    // Antes se enseñaba siempre la cola, que valía mientras sólo se podía
    // escribir al final. Con el cursor moviéndose, eso deja de valer: si te
    // vas al principio de una ruta larga, el cursor se sale por la izquierda y
    // editas a ciegas. La regla es sencilla y es la de cualquier editor —
    // **el cursor SIEMPRE se ve**, y la ventana se desplaza lo mínimo para
    // que así sea.
    let cabe = c.visibles();
    let desde = if ruta.len() <= cabe {
        0
    } else if cur >= cabe {
        // El cursor se salió por la derecha: pegarlo al borde derecho.
        (cur + 1).saturating_sub(cabe).min(ruta.len() - cabe)
    } else {
        0
    };
    let hasta = (desde + cabe).min(ruta.len());
    p.texto_bytes(c.texto_x, c.texto_y, &ruta[desde..hasta], TEXTO);

    if caret {
        let col = cur.saturating_sub(desde) as u32;
        p.rect(
            c.texto_x + col * bmo::GLIFO_ANCHO,
            c.texto_y,
            2,
            bmo::GLIFO_ALTO,
            ACENTO,
        );
    }
}

// ── La salida ───────────────────────────────────────────────────────────

/// Rejilla de caracteres con desplazamiento. Es lo mínimo que se puede llamar
/// terminal: sin colores por celda, sin secuencias de escape, sin scrollback
/// más allá de lo que se ve.
///
/// Deliberado. Un emulador de terminal completo —ANSI, cursor direccionable,
/// regiones— es una pila entera, y hoy lo único que hay al otro lado son
/// programas que escriben líneas. Cuando algo pida más, se añade; adivinarlo
/// ahora sería escribir código que nadie ejercita.
// ── La tinta ────────────────────────────────────────────────────────────
//
// El color va POR LÍNEA y no por celda. Una rejilla de atributos costaría
// 88x16 bytes para decir dieciséis veces lo mismo: este terminal escribe
// líneas enteras —el eco de un comando, un mensaje de error, la salida de un
// programa— y nunca media línea de un color y media de otro.
//
// Antes el único color lo daba `f == s.fila`, o sea "la fila donde está el
// cursor". Eso pinta de otro color la ÚLTIMA línea escrita, que casi nunca es
// la que te interesa: si un programa imprime diez líneas, la marcada es la
// décima y no el comando que lo lanzó.

/// Salida corriente de un programa.
const TINTA_NORMAL: u8 = 0;
/// El comando que escribiste. Es el ancla para leer hacia abajo.
const TINTA_ECO: u8 = 1;
/// Algo salió mal.
const TINTA_MAL: u8 = 2;
/// Algo salió bien y merece verse.
const TINTA_BIEN: u8 = 3;

fn color_tinta(t: u8) -> u32 {
    match t {
        TINTA_ECO => SAL_ECO,
        TINTA_MAL => TEXTO_MAL,
        TINTA_BIEN => TEXTO_BIEN,
        _ => SAL_TEXTO,
    }
}

struct Salida {
    celdas: [[u8; SAL_COLS]; SAL_ROWS],
    /// Con qué color se pinta cada fila.
    tinta: [u8; SAL_ROWS],
    /// Con qué color se escribe a partir de ahora.
    tinta_actual: u8,
    fila: usize,
    col: usize,
    /// Hay algo nuevo que pintar. Repintar la rejilla entera cada fotograma
    /// serían 88x16 glifos por vuelta sobre memoria de vídeo sin caché.
    sucia: bool,
}

impl Salida {
    fn nueva() -> Self {
        Self {
            celdas: [[b' '; SAL_COLS]; SAL_ROWS],
            tinta: [TINTA_NORMAL; SAL_ROWS],
            tinta_actual: TINTA_NORMAL,
            fila: 0,
            col: 0,
            sucia: true,
        }
    }

    /// A partir de aquí se escribe con esta tinta. La fila en curso se marca
    /// ya: quien cambia el color antes de escribir espera que valga para lo
    /// que va a escribir, no para lo siguiente.
    fn con_tinta(&mut self, t: u8) {
        self.tinta_actual = t;
        self.tinta[self.fila] = t;
    }

    /// Sube todo una línea y deja la última en blanco. La tinta viaja con su
    /// línea: si no, al desplazarse el color se quedaría marcando la fila de
    /// otro.
    fn desplazar(&mut self) {
        for f in 1..SAL_ROWS {
            self.celdas[f - 1] = self.celdas[f];
            self.tinta[f - 1] = self.tinta[f];
        }
        self.celdas[SAL_ROWS - 1] = [b' '; SAL_COLS];
        self.tinta[SAL_ROWS - 1] = self.tinta_actual;
    }

    fn salto(&mut self) {
        self.col = 0;
        if self.fila + 1 >= SAL_ROWS {
            self.desplazar();
        } else {
            self.fila += 1;
        }
        self.tinta[self.fila] = self.tinta_actual;
        self.sucia = true;
    }

    fn byte(&mut self, b: u8) {
        match b {
            b'\n' => self.salto(),
            // El retorno de carro solo, sin avance: se ignora. Un programa que
            // escribe "\r\n" no debe producir dos saltos.
            b'\r' => {}
            // Tabulador a la siguiente parada de 8.
            b'\t' => {
                let siguiente = (self.col / 8 + 1) * 8;
                while self.col < siguiente.min(SAL_COLS) {
                    self.celdas[self.fila][self.col] = b' ';
                    self.col += 1;
                }
                if self.col >= SAL_COLS {
                    self.salto();
                }
                self.sucia = true;
            }
            // Los no imprimibles se tiran en vez de dibujarse como basura.
            c if c < 0x20 => {}
            c => {
                if self.col >= SAL_COLS {
                    self.salto();
                }
                self.celdas[self.fila][self.col] = c;
                self.col += 1;
                self.sucia = true;
            }
        }
    }

    fn texto(&mut self, s: &[u8]) {
        for &b in s {
            self.byte(b);
        }
    }

    fn limpiar(&mut self) {
        self.celdas = [[b' '; SAL_COLS]; SAL_ROWS];
        self.tinta = [TINTA_NORMAL; SAL_ROWS];
        self.tinta_actual = TINTA_NORMAL;
        self.fila = 0;
        self.col = 0;
        self.sucia = true;
    }

    // ── Lo que hace falta para PINTAR un informe ────────────────────────
    //
    // Todo esto es de Ring 3 a propósito. El kernel contesta enteros crudos y
    // no sabe nada de KiB, de porcentajes ni de barras: un kernel que decide
    // cómo se ve un número es un kernel con opiniones sobre la interfaz.

    /// Un entero en decimal.
    fn dec(&mut self, mut v: u64) {
        if v == 0 {
            self.byte(b'0');
            return;
        }
        let mut d = [0u8; 20];
        let mut n = 0;
        while v > 0 {
            d[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
        while n > 0 {
            n -= 1;
            self.byte(d[n]);
        }
    }

    /// Un entero alineado a la DERECHA en `ancho` columnas. Es lo que hace que
    /// una columna de números se lea de un vistazo en vez de bailar.
    fn dec_der(&mut self, v: u64, ancho: usize) {
        let mut cifras = 1;
        let mut t = v;
        while t >= 10 {
            t /= 10;
            cifras += 1;
        }
        for _ in cifras..ancho {
            self.byte(b' ');
        }
        self.dec(v);
    }

    /// Bytes en la unidad que toca, con un decimal: `15.9 GiB`.
    ///
    /// Se hace con enteros, dividiendo y sacando el resto — igual que el
    /// decimal de COBOL. Aquí no hay coma flotante y no hace falta.
    fn tamano(&mut self, bytes: u64) {
        const U: [&[u8]; 5] = [b"B", b"KiB", b"MiB", b"GiB", b"TiB"];
        let mut i = 0;
        let mut v = bytes;
        while v >= 1024 && i < 4 {
            v /= 1024;
            i += 1;
        }
        // El primer decimal, sin flotantes: el resto de la última división.
        let frac = if i == 0 {
            0
        } else {
            let divisor = 1u64 << (10 * i);
            ((bytes % divisor) * 10 / divisor) % 10
        };
        self.dec(v);
        if i > 0 {
            self.byte(b'.');
            self.byte(b'0' + frac as u8);
        }
        self.byte(b' ');
        self.texto(U[i]);
    }

    /// Un porcentaje entero, sin dividir por cero.
    fn pct(&mut self, parte: u64, total: u64) {
        if total == 0 {
            self.texto(b"--");
        } else {
            self.dec(parte.saturating_mul(100) / total);
        }
        self.byte(b'%');
    }

    /// Una barra de ocupación de `ancho` columnas: `[####------]`.
    ///
    /// Con caracteres y no con píxeles porque la salida ES una rejilla de
    /// caracteres: una barra dibujada aparte se quedaría quieta cuando el log
    /// desplaza, y una barra que miente de sitio es peor que no tenerla.
    fn barra(&mut self, parte: u64, total: u64, ancho: usize) {
        let llenas = if total == 0 {
            0
        } else {
            (parte.saturating_mul(ancho as u64) / total) as usize
        };
        self.byte(b'[');
        for i in 0..ancho {
            self.byte(if i < llenas { b'#' } else { b'-' });
        }
        self.byte(b']');
    }
}

// ── Los informes del sistema ────────────────────────────────────────────
//
// Se pintan aquí, en Ring 3, con datos que el kernel contesta por `OP_INFO`.
// El kernel da enteros; las unidades, los porcentajes, las barras y el color
// son de este lado.

/// Un rótulo de sección, para que el informe no sea un muro de renglones.
fn seccion(s: &mut Salida, titulo: &[u8]) {
    s.con_tinta(TINTA_ECO);
    s.texto(b"  ");
    s.texto(titulo);
    s.byte(b' ');
    // Una regla hasta el margen: cuesta nada y separa de verdad.
    let usado = 3 + titulo.len();
    for _ in usado..SAL_COLS.saturating_sub(2) {
        s.byte(b'-');
    }
    s.byte(b'\n');
    s.con_tinta(TINTA_NORMAL);
}

/// Un renglón `etiqueta ....... valor`, con la etiqueta a ancho fijo.
fn etiqueta(s: &mut Salida, nombre: &[u8]) {
    s.texto(b"    ");
    s.texto(nombre);
    for _ in nombre.len()..14 {
        s.byte(b' ');
    }
}

fn informe_cpu(s: &mut Salida) {
    let mut buf = [0u8; 64];

    seccion(s, b"procesador");
    let n = bmo::info_texto(bmo::INFO_TXT_CPU_VENDOR, &mut buf);
    etiqueta(s, b"fabricante");
    s.texto(&buf[..n]);
    s.byte(b'\n');

    let n = bmo::info_texto(bmo::INFO_TXT_CPU_NOMBRE, &mut buf);
    etiqueta(s, b"modelo");
    s.texto(&buf[..n]);
    s.byte(b'\n');

    let n = bmo::info_texto(bmo::INFO_TXT_UARCH, &mut buf);
    etiqueta(s, b"uarch");
    s.texto(&buf[..n]);
    let n2 = bmo::info_texto(bmo::INFO_TXT_FAMILIA, &mut buf);
    if n2 > 0 {
        s.texto(b"   familia ");
        s.texto(&buf[..n2]);
    }
    s.byte(b'\n');

    etiqueta(s, b"nucleos");
    s.dec(bmo::info(bmo::INFO_CPU_NUCLEOS));
    s.texto(b" fisicos / ");
    s.dec(bmo::info(bmo::INFO_CPU_HILOS));
    s.texto(b" hilos\n");

    // Hz -> GHz con dos decimales, con enteros. El TSC es la frecuencia MEDIDA
    // en el arranque, no el numero de la etiqueta de la caja.
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    etiqueta(s, b"tsc");
    s.dec(hz / 1_000_000_000);
    s.byte(b'.');
    let frac = (hz % 1_000_000_000) / 10_000_000;
    if frac < 10 {
        s.byte(b'0');
    }
    s.dec(frac);
    s.texto(b" GHz   (medido)\n");
}

fn informe_memoria(s: &mut Salida) {
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    let usada = total.saturating_sub(libre);

    seccion(s, b"memoria");
    etiqueta(s, b"total");
    s.tamano(total);
    s.texto(b"   ");
    s.dec_der(bmo::info(bmo::INFO_RAM_MARCOS), 8);
    s.texto(b" marcos de 4 KiB\n");

    etiqueta(s, b"usada");
    s.tamano(usada);
    s.texto(b"   ");
    s.barra(usada, total, 24);
    s.byte(b' ');
    s.pct(usada, total);
    s.byte(b'\n');

    etiqueta(s, b"libre");
    s.tamano(libre);
    s.texto(b"   ");
    s.dec_der(bmo::info(bmo::INFO_RAM_MARCOS_LIBRES), 8);
    s.texto(b" marcos\n");

    // El tamano REAL del kernel en RAM, medido hasta el final de su .bss.
    etiqueta(s, b"kernel");
    s.tamano(bmo::info(bmo::INFO_KERNEL_BYTES));
    s.texto(b"   en 0x400000\n");
}

fn informe_sistema(s: &mut Salida) {
    s.con_tinta(TINTA_ECO);
    s.texto(b"  BMO-X - informe del sistema\n");
    s.con_tinta(TINTA_NORMAL);

    informe_cpu(s);
    informe_memoria(s);

    seccion(s, b"tareas");
    let total = bmo::info(bmo::INFO_TAREAS_TOTAL);
    let libres = bmo::info(bmo::INFO_TAREAS_LIBRES);
    let ranuras = total + libres;
    etiqueta(s, b"ranuras");
    s.dec(total);
    s.texto(b" en uso de ");
    s.dec(ranuras);
    s.texto(b"   ");
    s.barra(total, ranuras, 24);
    s.byte(b'\n');
    etiqueta(s, b"listas");
    s.dec(bmo::info(bmo::INFO_TAREAS_LISTAS));
    s.texto(b"   ticks ");
    s.dec(bmo::info(bmo::INFO_TICKS));
    s.byte(b'\n');
    etiqueta(s, b"programas");
    let vistos = bmo::info(bmo::INFO_PROGRAMAS);
    let olvidados = bmo::info(bmo::INFO_PROGRAMAS_OLVIDADOS);
    s.dec(vistos + olvidados);
    s.texto(b" lanzados");
    if olvidados > 0 {
        s.texto(b"   (");
        s.dec(olvidados);
        s.texto(b" ya no caben en la bitacora)");
    }
    s.byte(b'\n');

    seccion(s, b"disco");
    etiqueta(s, b"disco");
    if bmo::info(bmo::INFO_DISCO_LISTO) != 0 {
        s.con_tinta(TINTA_BIEN);
        s.texto(b"listo");
    } else {
        s.con_tinta(TINTA_MAL);
        s.texto(b"sin disco");
    }
    s.con_tinta(TINTA_NORMAL);
    s.byte(b'\n');
    etiqueta(s, b"datos");
    if bmo::info(bmo::INFO_DATOS_MONTADO) != 0 {
        s.con_tinta(TINTA_BIEN);
        s.texto(b"montado para escritura");
    } else {
        // La linea que decide si el File I/O de COBOL puede funcionar. Decirlo
        // aqui ahorra buscar el fallo en el programa.
        s.con_tinta(TINTA_MAL);
        s.texto(b"NO montado: sin esto no hay OPEN ni WRITE");
    }
    s.con_tinta(TINTA_NORMAL);
    s.byte(b'\n');
}

// ── La línea de comandos ────────────────────────────────────────────────

/// Qué pidió el usuario. Se separa del bucle porque la decisión "esto es un
/// comando o es una ruta" merece leerse de un vistazo.
enum Orden<'a> {
    Nada,
    Lanzar(&'a [u8]),
    Limpiar,
    Ayuda,
    /// Ensena o esconde la calculadora.
    Calculadora,
    /// `ls [ruta]` — qué hay en el disco. Antes esto no podía existir: no
    /// había capability de directorio, así que había que saberse los nombres
    /// de memoria y teclearlos enteros.
    Listar(&'a [u8]),
    /// `lee <ruta>` — enseña lo que hay DENTRO de un archivo. Es el hermano
    /// de `ls`: aquel dice qué archivos hay, éste los abre.
    Leer(&'a [u8]),
    /// `escribe <ruta> <texto>` — crea un archivo con ese texto.
    ///
    /// Es la primera vez que Ring 3 GUARDA algo. Hasta ahora todo lo que
    /// aparecía en el disco lo había puesto el anfitrión al flashear, o el
    /// kernel con su caja negra; un programa no tenía con qué.
    Escribir(&'a [u8], &'a [u8]),
    /// Parece un archivo, pero no es un `.bex`. No se intenta lanzar: se dice
    /// qué es y con qué se abre.
    NoEsPrograma(&'a [u8]),
    /// `info` — el informe del sistema. `cpu` y `mem` son las dos mitades.
    ///
    /// ★ Esto vivía SOLO en el shell de Ring 0, y no porque hiciera falta el
    /// privilegio: porque los datos estaban a su alcance. Contar RAM no ejerce
    /// ningún poder. Ahora bajan por `OP_INFO` y se pintan aquí, que es donde
    /// está la pantalla.
    Informe,
    Cpu,
    Memoria,
    /// `reboot` — reinicia la máquina y no vuelve.
    ///
    /// Estaba en el shell del kernel desde siempre y aquí contestaba "no lo
    /// conozco", así que la única forma de reiniciar era el botón de la caja.
    /// Reiniciar es tocar puertos de E/S, que Ring 3 no puede hacer: va por
    /// `OP_REINICIAR`, una operación más dentro de `INVOKE`.
    Reiniciar,
    /// Una palabra suelta que no parece una ruta.
    Desconocida,
}

/// Historial de lo escrito. Lo que un terminal sin esto obliga a hacer es
/// reteclear la ruta entera cada vez que te equivocas en una letra — y eso es
/// justo lo que pasaba: seis intentos de `apps/COBOL.bex` escritos a mano.
///
/// Anillo de ocho. No guarda duplicados seguidos: repetir `ls` cinco veces no
/// debe llenar el historial de `ls`.
struct Historial {
    lineas: [[u8; RUTA_MAX]; 8],
    largos: [usize; 8],
    /// Cuantas hay guardadas (tope 8).
    n: usize,
    /// Por donde va el paseo con las flechas. `n` = "estoy escribiendo algo
    /// nuevo", que es distinto de "estoy en la mas reciente".
    cursor: usize,
}

impl Historial {
    fn nuevo() -> Self {
        Self { lineas: [[0u8; RUTA_MAX]; 8], largos: [0; 8], n: 0, cursor: 0 }
    }

    fn empujar(&mut self, linea: &[u8]) {
        if linea.is_empty() {
            return;
        }
        if self.n > 0 && &self.lineas[self.n - 1][..self.largos[self.n - 1]] == linea {
            self.cursor = self.n;
            return;
        }
        if self.n == self.lineas.len() {
            // Lleno: se va la mas vieja.
            for i in 1..self.n {
                self.lineas[i - 1] = self.lineas[i];
                self.largos[i - 1] = self.largos[i];
            }
            self.n -= 1;
        }
        let k = linea.len().min(RUTA_MAX);
        self.lineas[self.n][..k].copy_from_slice(&linea[..k]);
        self.largos[self.n] = k;
        self.n += 1;
        self.cursor = self.n;
    }

    /// Hacia atras. Devuelve el nuevo largo de la linea, o `None` si no hay.
    fn atras(&mut self, dst: &mut [u8; RUTA_MAX]) -> Option<usize> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        let k = self.largos[self.cursor];
        dst[..k].copy_from_slice(&self.lineas[self.cursor][..k]);
        Some(k)
    }

    /// Hacia adelante. Al pasar de la mas reciente se vuelve a linea en
    /// blanco, que es lo que espera cualquiera que haya usado un shell.
    fn adelante(&mut self, dst: &mut [u8; RUTA_MAX]) -> Option<usize> {
        if self.cursor + 1 > self.n {
            return None;
        }
        self.cursor += 1;
        if self.cursor == self.n {
            return Some(0);
        }
        let k = self.largos[self.cursor];
        dst[..k].copy_from_slice(&self.lineas[self.cursor][..k]);
        Some(k)
    }
}

// ── La calculadora ──────────────────────────────────────────────────────
//
// La CARA. El cálculo lo hace `apps/calcgui.bex`, en COBOL, con decimal
// exacto en centavos. Es la separación que Windows no hace —su calculadora
// lleva el motor dentro de la app— y es la que permite cambiar la una sin
// tocar la otra: mañana el motor puede ser Ada y esto no se entera.

const CALC_COLS: usize = 4;
const CALC_ROWS: usize = 5;
const CALC_BOTON: u32 = 72;
const CALC_HUECO: u32 = 6;
const CALC_FONDO: u32 = 0x0018_2434;
const CALC_TECLA: u32 = 0x002B_3B52;
const CALC_TECLA_OP: u32 = 0x003A_5878;
const CALC_TECLA_IGUAL: u32 = 0x004C_9BE8;

/// Las teclas, en el orden en que se dibujan. `\0` = hueco.
const CALC_TECLAS: [[u8; CALC_COLS]; CALC_ROWS] = [
    [b'C', b'/', b'*', b'-'],
    [b'7', b'8', b'9', b'+'],
    [b'4', b'5', b'6', 0],
    [b'1', b'2', b'3', b'='],
    [b'0', b'.', 0, 0],
];

/// Estado de la calculadora. Los operandos se guardan como TEXTO, no como
/// número: quien sabe de números aquí es el COBOL, y convertir dos veces sólo
/// añade sitios donde perder un decimal.
struct Calc {
    visible: bool,
    /// Lo que se está tecleando ahora.
    entrada: [u8; 20],
    n: usize,
    /// El operando de la izquierda, ya cerrado.
    guardado: [u8; 20],
    guardado_n: usize,
    /// 0 = ninguno; 1..4 = + - * /
    op: u8,
    /// Se lanzó el motor y se espera su respuesta.
    esperando: bool,
}

impl Calc {
    fn nueva() -> Self {
        Self {
            visible: false,
            entrada: [0; 20],
            n: 0,
            guardado: [0; 20],
            guardado_n: 0,
            op: 0,
            esperando: false,
        }
    }

    fn meter(&mut self, c: u8) {
        if self.n < self.entrada.len() {
            self.entrada[self.n] = c;
            self.n += 1;
        }
    }

    fn limpiar(&mut self) {
        self.n = 0;
        self.guardado_n = 0;
        self.op = 0;
        self.esperando = false;
    }

    /// Cierra el operando de la izquierda y anota qué operación viene.
    fn operador(&mut self, op: u8) {
        if self.n > 0 {
            self.guardado[..self.n].copy_from_slice(&self.entrada[..self.n]);
            self.guardado_n = self.n;
            self.n = 0;
        }
        self.op = op;
    }

    /// Lo que se enseña en la pantallita: lo que se teclea, o `0` si no hay
    /// nada — una calculadora en blanco confunde.
    fn mostrado(&self) -> &[u8] {
        if self.n == 0 {
            b"0"
        } else {
            &self.entrada[..self.n]
        }
    }
}

/// Geometría del panel, a la derecha de la caja.
struct CalcCaja {
    x: u32,
    y: u32,
    ancho: u32,
    alto: u32,
    pantalla_y: u32,
    rejilla_y: u32,
}

impl CalcCaja {
    fn nueva(c: &Caja) -> Self {
        let ancho = CALC_COLS as u32 * (CALC_BOTON + CALC_HUECO) + CALC_HUECO;
        let alto = CALC_ROWS as u32 * (CALC_BOTON + CALC_HUECO) + CALC_HUECO + 56;
        Self {
            x: c.x + CAJA_ANCHO + 24,
            y: c.y,
            ancho,
            alto,
            pantalla_y: c.y + CALC_HUECO,
            rejilla_y: c.y + CALC_HUECO + 50,
        }
    }

    /// Rectángulo de la tecla `(fila, col)`.
    fn boton(&self, fila: usize, col: usize) -> (u32, u32) {
        (
            self.x + CALC_HUECO + col as u32 * (CALC_BOTON + CALC_HUECO),
            self.rejilla_y + fila as u32 * (CALC_BOTON + CALC_HUECO),
        )
    }

    /// Qué tecla hay bajo `(px, py)`, si hay alguna.
    fn tecla_en(&self, px: u32, py: u32) -> Option<u8> {
        for fila in 0..CALC_ROWS {
            for col in 0..CALC_COLS {
                let t = CALC_TECLAS[fila][col];
                if t == 0 {
                    continue;
                }
                let (bx, by) = self.boton(fila, col);
                if px >= bx && px < bx + CALC_BOTON && py >= by && py < by + CALC_BOTON {
                    return Some(t);
                }
            }
        }
        None
    }

    fn contiene(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.ancho && py >= self.y && py < self.y + self.alto
    }
}

fn pintar_calc(p: &bmo::Pantalla, cc: &CalcCaja, c: &Calc) {
    p.rect(cc.x, cc.y, cc.ancho, cc.alto, CAJA_BORDE);
    p.rect(cc.x + 2, cc.y + 2, cc.ancho - 4, cc.alto - 4, CALC_FONDO);

    // La pantallita, alineada a la DERECHA como cualquier calculadora: los
    // números se comparan por la unidad, no por la primera cifra.
    p.rect(cc.x + CALC_HUECO, cc.pantalla_y, cc.ancho - CALC_HUECO * 2, 40, CAMPO_FONDO);
    let texto = c.mostrado();
    let ancho_texto = texto.len() as u32 * bmo::GLIFO_ANCHO;
    let tx = cc.x + cc.ancho - CALC_HUECO - 8 - ancho_texto;
    p.texto_bytes(tx, cc.pantalla_y + 12, texto, if c.esperando { TEXTO_TENUE } else { TEXTO });

    for fila in 0..CALC_ROWS {
        for col in 0..CALC_COLS {
            let t = CALC_TECLAS[fila][col];
            if t == 0 {
                continue;
            }
            let (bx, by) = cc.boton(fila, col);
            let color = match t {
                b'=' => CALC_TECLA_IGUAL,
                b'+' | b'-' | b'*' | b'/' | b'C' => CALC_TECLA_OP,
                _ => CALC_TECLA,
            };
            p.rect(bx, by, CALC_BOTON, CALC_BOTON, color);
            // La etiqueta, centrada.
            p.glifo(
                bx + CALC_BOTON / 2 - bmo::GLIFO_ANCHO / 2,
                by + CALC_BOTON / 2 - bmo::GLIFO_ALTO / 2,
                t,
                TEXTO,
            );
        }
    }
}

/// Un `u64` a decimal en `dst`. Sin `alloc` no hay `format!`, y un terminal
/// que no sabe escribir un número no sirve para mirar un disco.
fn decimal(mut v: u64, dst: &mut [u8; 10]) -> usize {
    if v == 0 {
        dst[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut n = 0;
    while v > 0 && n < tmp.len() {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    let cuantos = n.min(dst.len());
    for i in 0..cuantos {
        dst[i] = tmp[n - 1 - i];
    }
    cuantos
}

/// **Completar con TAB.** Devuelve el nuevo largo de la linea.
///
/// Mejor que el de Windows a proposito, y la diferencia es una decision, no
/// una casualidad:
///
/// - Windows CICLA: pulsas TAB y te pone un candidato, otra vez y te pone el
///   siguiente. Nunca te ENSENA lo que hay, asi que a ciegas vas probando.
/// - Aqui se completa hasta el PREFIJO COMUN mas largo y, si quedaba mas de
///   un candidato, **se listan todos**. Un TAB te dice cuanto se puede
///   avanzar sin riesgo y que opciones te quedan. Es lo que hace bash, y es
///   lo unico honesto: adivinar por ti cual de cinco querias es mentir.
///
/// Si el unico candidato es una carpeta, se anade la barra — porque lo
/// siguiente que vas a escribir es lo de dentro.
/// ¿Es la entrada `.` o `..`?
///
/// FAT las guarda como entradas de verdad y `entry_at` las devuelve. Aquí no
/// sirven para nada —este terminal no tiene "carpeta actual" a la que volver—
/// y estorban en los dos sitios donde aparecen: envenenan el prefijo común del
/// TAB y ensucian el `ls`.
fn es_punto(nombre: &[u8]) -> bool {
    nombre == b"." || nombre == b".."
}

fn completar(ruta: &mut [u8; RUTA_MAX], n: usize, salida: &mut Salida) -> usize {
    // El ultimo token: lo que hay tras el ultimo espacio. Asi `corre app<TAB>`
    // completa la ruta y no el verbo.
    let inicio = ruta[..n].iter().rposition(|&c| c == b' ').map_or(0, |i| i + 1);
    // ★ La carpeta y el prefijo se COPIAN a locales antes de tocar nada.
    // Tomarlos prestados de `ruta` y luego escribir en `ruta` es exactamente
    // lo que el prestamista de Rust no deja — y hace bien: escribir sobre lo
    // que estas leyendo es como se corrompe un buffer sin enterarse.
    let mut dir = [0u8; RUTA_MAX];
    let mut dir_n = 0usize;
    let mut prefijo = [0u8; 12];
    let mut pref_n = 0usize;
    let pref_ini;
    {
        let token = &ruta[inicio..n];
        let corte = token.iter().rposition(|&c| c == b'/' || c == b'\\');
        let (d0, pi) = match corte {
            Some(i) => (&token[..i], i + 1),
            None => (&token[0..0], 0),
        };
        pref_ini = pi;
        dir_n = d0.len().min(RUTA_MAX);
        dir[..dir_n].copy_from_slice(&d0[..dir_n]);
        let p0 = &token[pref_ini..];
        pref_n = p0.len().min(prefijo.len());
        prefijo[..pref_n].copy_from_slice(&p0[..pref_n]);
    }
    let dir = &dir[..dir_n];
    let prefijo = &prefijo[..pref_n];

    let d = match bmo::Directorio::abrir(dir) {
        Ok(d) => d,
        Err(_) => return n,
    };

    let baja = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
    let mut cuantos = 0usize;
    let mut comun = [0u8; 12];
    let mut comun_n = 0usize;
    let mut unico_es_dir = false;
    // Los candidatos se listan DESPUES, en una segunda pasada: guardarlos
    // todos aqui pediria un vector, y sin `alloc` eso es un array con un tope
    // inventado. Recorrer dos veces cuesta microsegundos y no inventa topes.
    let mut vueltas = 0u32;
    while vueltas < 256 {
        let e = match d.siguiente() { Some(e) => e, None => break };
        vueltas += 1;
        let mut nom = [0u8; 12];
        let largo = e.legible(&mut nom);
        // ★ `.` y `..` FUERA. Eran el motivo de que el TAB no completara
        // NUNCA dentro de una carpeta: entran como candidatos, y el prefijo
        // comun de `.`, `..` y `gui.bex` es la cadena vacia. El TAB listaba
        // todo y no avanzaba ni una letra, que parecia "no busca referencias"
        // cuando lo que hacia era buscarlas y anularse solo.
        if es_punto(&nom[..largo]) { continue; }
        if largo < prefijo.len() { continue; }
        let mut cuadra = true;
        for k in 0..prefijo.len() {
            if baja(nom[k]) != baja(prefijo[k]) { cuadra = false; break; }
        }
        if !cuadra { continue; }
        if cuantos == 0 {
            comun[..largo].copy_from_slice(&nom[..largo]);
            comun_n = largo;
            unico_es_dir = e.es_dir;
        } else {
            // Recortar al prefijo comun con lo que llevabamos.
            let mut k = 0usize;
            while k < comun_n && k < largo && baja(comun[k]) == baja(nom[k]) { k += 1; }
            comun_n = k;
            unico_es_dir = false;
        }
        cuantos += 1;
    }

    if cuantos == 0 {
        return n;
    }

    // Escribir el prefijo comun en el sitio del que habia.
    let mut fin = inicio + pref_ini;
    let mut k = 0usize;
    while k < comun_n && fin < RUTA_MAX {
        ruta[fin] = comun[k];
        fin += 1;
        k += 1;
    }
    if cuantos == 1 && unico_es_dir && fin < RUTA_MAX {
        ruta[fin] = b'/';
        fin += 1;
    }

    // Con mas de uno, ENSENAR lo que hay. Es la diferencia con ciclar.
    if cuantos > 1 {
        let d2 = match bmo::Directorio::abrir(dir) { Ok(d) => d, Err(_) => return fin };
        let mut vueltas = 0u32;
        while vueltas < 256 {
            let e = match d2.siguiente() { Some(e) => e, None => break };
            vueltas += 1;
            let mut nom = [0u8; 12];
            let largo = e.legible(&mut nom);
            if largo < prefijo.len() { continue; }
            let mut cuadra = true;
            for k in 0..prefijo.len() {
                if baja(nom[k]) != baja(prefijo[k]) { cuadra = false; break; }
            }
            if !cuadra { continue; }
            salida.texto(b"  ");
            salida.texto(&nom[..largo]);
            if e.es_dir { salida.byte(b'/'); }
            salida.byte(b'\n');
        }
    }
    fin
}

/// El motivo, en una línea que dice qué hacer.
///
/// Antes los siete casos se aplanaban en "no puedo crear ahi (nombre 8.3?
/// carpeta?)" — un mensaje que le pasa la pregunta al usuario en vez de
/// contestarla. El kernel SÍ sabe cuál de los dos fue.
fn motivo_archivo(e: u32) -> &'static [u8] {
    match e {
        bmo::ERROR_ARCH_CARPETA => b"esa carpeta no existe.",
        bmo::ERROR_ARCH_ES_CARPETA => b"eso es una carpeta, no un archivo: prueba ls.",
        bmo::ERROR_ARCH_NOMBRE => b"el nombre no cabe en 8.3 (8 letras + 3 de extension).",
        bmo::ERROR_ARCH_NO_ESTA => b"ese archivo no esta.",
        bmo::ERROR_ARCH_GRANDE => b"el archivo pasa de 4 KiB: hoy no cabe.",
        bmo::ERROR_ARCH_SOLO_LECTURA => b"el volumen de datos no se puede escribir.",
        bmo::ERROR_ARCH_SIN_HUECO => b"hay demasiados archivos abiertos.",
        _ => b"no se pudo.",
    }
}

fn parece_ruta(t: &[u8]) -> bool {
    t.iter().any(|&c| c == b'/' || c == b'\\' || c == b'.')
}

/// ¿Esto es un PROGRAMA, o sea algo que tenga sentido lanzar?
///
/// ★ Antes bastaba con que llevara un punto o una barra, y por eso escribir
/// `leeme.txt` a pelo intentaba EJECUTARLO. El kernel contestaba "sin firma no
/// hay ejecucion" —que es exactamente lo correcto— y el usuario se quedaba
/// creyendo que el sistema le pedia un permiso especial para LEER un fichero
/// de texto. No se lo pedia: es que nadie le habia dicho que queria leerlo.
///
/// La conclusion de la que hay que huir es "hace falta un modo administrador".
/// Aqui no se afloja ninguna guardia: se deja de adivinar. Solo un `.bex` es
/// un programa; lo demas son datos, y a los datos se los lee.
///
/// `run <ruta>` sigue intentandolo con lo que sea: si alguien lo escribe
/// explicitamente, la respuesta la da el gate y no esta heuristica.
fn parece_programa(t: &[u8]) -> bool {
    let n = t.len();
    if n < 4 {
        return false;
    }
    let cola = &t[n - 4..];
    cola[0] == b'.'
        && (cola[1] | 32) == b'b'
        && (cola[2] | 32) == b'e'
        && (cola[3] | 32) == b'x'
}

/// Parte la línea en verbo y resto.
///
/// ★ Acepta `run <ruta>` ADEMÁS de la ruta pelada, y no por capricho: quien usa
/// esto viene del shell de Ring 0, donde se escribe `run`. Pelearse con la
/// costumbre del usuario es perder — el que se adapta es el programa. Lo que sí
/// se hace es DECIRLO cuando la palabra no es ni comando ni ruta, en vez de
/// contestar "no esta: revisa la ruta" a alguien que escribió `reboot`.
fn interpretar(linea: &[u8]) -> Orden<'_> {
    let linea = {
        let mut i = 0;
        while i < linea.len() && linea[i] == b' ' { i += 1; }
        &linea[i..]
    };
    if linea.is_empty() {
        return Orden::Nada;
    }
    let corte = linea.iter().position(|&c| c == b' ').unwrap_or(linea.len());
    let (verbo, resto) = linea.split_at(corte);
    let resto = {
        let mut i = 0;
        while i < resto.len() && resto[i] == b' ' { i += 1; }
        &resto[i..]
    };
    match verbo {
        // INGLES de primero, y es una decision del dueno: el castellano limita
        // — no hay palabra corta para "flush", los verbos se alargan, y medio
        // mundo del sistema (los campos del hardware, los mensajes de fallo)
        // ya esta en ingles. El castellano entra cuando el sistema este
        // maduro y se pueda hacer entero, no a medias.
        //
        // Los castellanos se quedan como SINONIMOS: no estorban y ya estaban
        // escritos.
        b"run" | b"corre" | b"lanza" => {
            if resto.is_empty() { Orden::Ayuda } else { Orden::Lanzar(resto) }
        }
        b"calc" | b"calculadora" => Orden::Calculadora,
        b"clear" | b"cls" | b"limpia" => Orden::Limpiar,
        b"ls" | b"dir" | b"lista" => Orden::Listar(resto),
        b"cat" | b"lee" => {
            if resto.is_empty() { Orden::Ayuda } else { Orden::Leer(resto) }
        }
        // `escribe <ruta> <texto>`: la ruta es la PRIMERA palabra y el texto
        // es todo lo demas, espacios incluidos. Partir por la ultima palabra
        // obligaria a escribir el texto sin espacios, que no es escribir.
        b"escribe" | b"write" => {
            let k = resto.iter().position(|&c| c == b' ');
            match k {
                Some(k) => {
                    let (ruta, texto) = resto.split_at(k);
                    let mut j = 0;
                    while j < texto.len() && texto[j] == b' ' { j += 1; }
                    Orden::Escribir(ruta, &texto[j..])
                }
                None => Orden::Ayuda,
            }
        }
        b"info" | b"sistema" => Orden::Informe,
        b"cpu" | b"procesador" => Orden::Cpu,
        b"mem" | b"ram" | b"memoria" => Orden::Memoria,
        b"reboot" | b"reinicia" | b"reiniciar" => Orden::Reiniciar,
        b"help" | b"?" | b"ayuda" => Orden::Ayuda,
        _ if parece_programa(linea) => Orden::Lanzar(linea),
        // Parece un archivo pero no es un programa. Antes esto caia en
        // `Lanzar` y el kernel contestaba "sin firma no hay ejecucion" — un
        // mensaje CORRECTO que en este sitio se lee como si el sistema pidiera
        // permisos para abrir un .txt.
        _ if parece_ruta(linea) => Orden::NoEsPrograma(linea),
        _ => Orden::Desconocida,
    }
}

fn pintar_salida(p: &bmo::Pantalla, c: &Caja, s: &Salida) {
    // Fondo entero de la rejilla y encima las filas. Es un rectángulo de
    // 704x256 px: nada comparado con la pantalla, y evita tener que llevar la
    // cuenta de qué celda cambió.
    p.rect(
        c.salida_x,
        c.salida_y,
        SAL_COLS as u32 * bmo::GLIFO_ANCHO,
        c.salida_alto(),
        CAJA_FONDO,
    );
    for (f, fila) in s.celdas.iter().enumerate() {
        let color = color_tinta(s.tinta[f]);
        p.texto_bytes(
            c.salida_x,
            c.salida_y + f as u32 * bmo::GLIFO_ALTO,
            fila,
            color,
        );
    }
}

/// Borra la caja devolviendo cada píxel a lo que la escena dice que hay
/// debajo. Es el precio de que la ventana se pueda invocar y esconder.
///
/// Recorre el rectángulo entero — unos 325k píxeles sobre memoria de vídeo sin
/// caché, que no es gratis. Pero pasa UNA vez por pulsación de atajo, no por
/// fotograma, y la alternativa (guardar lo que había debajo) sería un buffer de
/// 1,3 MB en un proceso con 64 KiB de pila.
fn borrar_caja(p: &bmo::Pantalla, c: &Caja) {
    for fila in 0..CAJA_ALTO {
        for col in 0..CAJA_ANCHO {
            let (x, y) = (c.x + col, c.y + fila);
            p.punto(x, y, color_escena(c, false, x, y));
        }
    }
}

fn pintar_estado(p: &bmo::Pantalla, c: &Caja, msg: &str, color: u32) {
    // Ancho fijo de limpieza: el mensaje anterior puede ser más largo que el
    // nuevo, y media frase vieja detrás de una nueva es peor que ninguna.
    p.rect(c.x + 18, c.estado_y, CAJA_ANCHO - 36, bmo::GLIFO_ALTO, CAJA_FONDO);
    p.texto(c.x + 18, c.estado_y, msg, color);
}

// ── El programa ─────────────────────────────────────────────────────────

/// Cada cuántas vueltas del bucle parpadea el cursor de escritura.
///
/// Se cuenta en fotogramas y no en tiempo porque aquí no hay reloj: los tres
/// syscalls no incluyen "qué hora es". Es un parpadeo que depende de la
/// velocidad de la máquina, y para decir "aquí se escribe" eso basta.
const PARPADEO: u32 = 12_000;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // El aviso va ANTES de reclamar: en cuanto la cesión se consuma, el kernel
    // deja de dibujar y nada de lo que se imprima después llega al panel.
    bmo::consola("reclamo pantalla y entrada\n");

    let Some(p) = bmo::Pantalla::reclamar() else {
        bmo::consola("sin pantalla que reclamar\n");
        bmo::salir()
    };
    // La entrada es opcional a propósito: sin ella hay escritorio, sólo que
    // quieto y mudo. Un compositor que se niega a arrancar porque falta un
    // periférico es un compositor que no arranca el día que el periférico falla.
    let entrada = bmo::Entrada::reclamar();

    // La consola de este terminal. Desde aquí, todo lo que lance escribe en
    // ESTE anillo y no en el panel del kernel — que es lo único que separaba
    // una caja de lanzar de un terminal de verdad.
    let salida_cap = bmo::Consola::crear();

    let caja = Caja::nueva(p.ancho, p.alto);

    // Fondo entero de una pasada, y encima la escena.
    p.limpiar(FONDO);
    p.rect(0, 0, p.ancho, BARRA_ALTO, BARRA);
    p.rect(16, 14, 16, 16, ACENTO);
    let mut i = 0u32;
    while (i as usize) < MEDIDA.len() {
        p.rect(
            MEDIDA_X + i * MEDIDA_LADO,
            MEDIDA_Y,
            MEDIDA_LADO,
            MEDIDA_LADO,
            MEDIDA[i as usize],
        );
        i += 1;
    }

    // Marco del pulsómetro. Si la entrada ni se pudo reclamar, sale en rojo:
    // dos fallos distintos, dos aspectos distintos.
    let marco = if entrada.is_some() { ACENTO } else { 0x00E0_4040 };
    p.rect(PULSO_X - 2, PULSO_Y - 2, PULSO_ANCHO + 4, PULSO_ALTO + 4, marco);
    p.rect(PULSO_X, PULSO_Y, PULSO_ANCHO, PULSO_ALTO, FONDO);

    pintar_caja(&p, &caja);
    let mut ruta = [0u8; RUTA_MAX];
    let mut n = 0usize;
    let mut salida = Salida::nueva();
    let mut historial = Historial::nuevo();
    // Posicion del cursor DENTRO de la linea. Sin esto solo se puede escribir
    // al final y borrar desde el final: equivocarte en la tercera letra de una
    // ruta larga obliga a borrarlo todo hasta ahi.
    let mut cur = 0usize;
    // Portapapeles. Ctrl+C copia la linea entera, Ctrl+V la pega donde este el
    // cursor. Ctrl+ARRIBA / Ctrl+ABAJO hacen lo mismo con las flechas.
    let mut porta = [0u8; RUTA_MAX];
    let mut porta_n = 0usize;
    let mut calc = Calc::nueva();
    let calc_caja = CalcCaja::nueva(&caja);
    // Flanco del botón del ratón: un clic es una BAJADA, no "el botón está
    // pulsado". Sin esto, mantener pulsado teclearía cien veces por segundo.
    let mut boton_antes = false;
    // Mientras el motor no conteste, su salida NO va a la rejilla: es el
    // resultado, no un mensaje. Se acumula aparte.
    let mut resp = [0u8; 24];
    let mut resp_n = 0usize;
    if salida_cap.is_none() {
        salida.texto(b"sin consola: la salida de los programas ira al panel del kernel\n");
    }
    pintar_campo(&p, &caja, &ruta[..n], cur, true);
    pintar_salida(&p, &caja, &salida);
    if entrada.is_some() {
        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
    } else {
        // Decirlo, y decir por qué. Una caja que no responde y no explica nada
        // es peor que no tener caja.
        pintar_estado(&p, &caja, "sin teclado: la entrada no se pudo reclamar", TEXTO_MAL);
    }

    bmo::consola("escritorio pintado\n");

    // ── El bucle de vida ──
    //
    // No termina: si saliera, `revoke_all` devolvería la pantalla y el kernel
    // repintaría su panel encima. Un escritorio es un proceso que VIVE — y de
    // paso esto ejerce el cambio de contexto miles de veces por segundo, que es
    // justo el camino que costó una foto de madrugada.
    let (mut ax, mut ay) = (u32::MAX, u32::MAX);
    let mut pulso_previo = 0u32;
    let mut vueltas = 0u32;
    let mut caret = true;
    // Vueltas desde la última tecla. Se reinicia al escribir para que el
    // cursor esté SIEMPRE encendido mientras se teclea.
    let mut desde_tecla: u32 = 0;
    // ── El atajo: un TOQUE de Ctrl+Alt ──
    //
    // Se dispara al SOLTAR, y sólo si no llegó ningún carácter mientras
    // estaban pulsados. No es una floritura: en la distribución española
    // `Ctrl+Alt` **es** `AltGr` —lo que produce `@`, `#`, `[`, `]`, `\`, `|`
    // y `€`— así que disparar al pulsarlos rompería escribir todos esos
    // caracteres. Con el toque, `Ctrl+Alt` a secas invoca la ventana y
    // `Ctrl+Alt+2` sigue dando `@`.
    let mut combo_antes = false;
    let mut hubo_tecla_en_combo = false;
    let mut visible = true;

    loop {
        vueltas = vueltas.wrapping_add(1);
        let mut repintar_campo = false;

        if let Some(e) = entrada.as_ref() {
            // ── El atajo, ANTES de leer teclas ──
            let m = e.modificadores();
            let ctrl = m & bmo::MOD_CTRL != 0;
            let combo = ctrl && m & bmo::MOD_ALT != 0;
            if combo && !combo_antes {
                hubo_tecla_en_combo = false;
            }
            if !combo && combo_antes && !hubo_tecla_en_combo {
                visible = !visible;
                if visible {
                    pintar_caja(&p, &caja);
                    repintar_campo = true;
                    salida.sucia = true;
                    pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                } else {
                    borrar_caja(&p, &caja);
                }
            }
            combo_antes = combo;

            // ── Teclado ──
            //
            // Se drena hasta vaciar, no una tecla por fotograma: escribiendo
            // rápido llegan varias entre vuelta y vuelta, y quedarse con una
            // sería perder letras de forma que parecería un teclado malo.
            while let Some(c) = e.tecla() {
                // Cualquier tecla durante el combo lo convierte en AltGr y
                // cancela el toque: el usuario estaba escribiendo, no llamando.
                if combo {
                    hubo_tecla_en_combo = true;
                }
                // Con la ventana escondida las teclas no se editan en ningún
                // sitio: se descartan. Volverán cuando se invoque.
                if !visible {
                    continue;
                }
                // Cualquier tecla enciende el cursor y reinicia el parpadeo.
                caret = true;
                desde_tecla = 0;
                repintar_campo = true;
                match c {
                    b'\r' | b'\n' => {
                        // Eco SIEMPRE, también de lo que no se entiende: un
                        // terminal que se traga lo que escribiste deja al
                        // usuario sin saber qué llegó.
                        // El eco lleva un punto medio (0xB7) y no `>`. El `>`
                        // es la marca de Unix y este sistema no es Unix; el
                        // punto medio separa igual de bien y no arrastra la
                        // convencion de otro. Esta en la tabla de extras del
                        // font, asi que se dibuja sin tocar nada mas.
                        // El eco en su tinta y la respuesta en la normal: al
                        // mirar la rejilla, los comandos son las anclas y todo
                        // lo de debajo es lo que contestaron.
                        salida.con_tinta(TINTA_ECO);
                        salida.byte(0xB7);
                        salida.byte(b' ');
                        salida.texto(&ruta[..n]);
                        salida.byte(b'\n');
                        salida.con_tinta(TINTA_NORMAL);

                        // ¿Hay un programa vivo escuchando en esta consola?
                        // Entonces la linea NO es un comando: es SUYA. Es lo
                        // que hace cualquier shell, y sin esto un `ACCEPT` de
                        // COBOL no puede recibir nada nunca — el terminal se
                        // come la respuesta y contesta "no lo conozco".
                        //
                        // La calculadora se excluye a proposito: mientras
                        // espera al motor, ese hijo es SUYO y ya recibio sus
                        // tres lineas. Colar una mas ahi le cambiaria la
                        // cuenta a alguien que no la pidio.
                        let del_hijo = !calc.esperando
                            && salida_cap.as_ref().map(|cc| cc.hay_hijo()).unwrap_or(false);

                        if del_hijo {
                            if let Some(cc) = salida_cap.as_ref() {
                                cc.escribir(&ruta[..n]);
                                // El salto va aparte y SIEMPRE: `read_line`
                                // espera a verlo para dar la linea por
                                // cerrada. Sin el, el programa sigue
                                // esperando algo que ya escribiste.
                                cc.escribir(b"\n");
                            }
                            pintar_estado(&p, &caja, "para el programa", TEXTO_TENUE);
                            n = 0;
                            cur = 0;
                            repintar_campo = true;
                            continue;
                        }

                        // Al historial va lo que es un COMANDO. Un importe
                        // tecleado para un `ACCEPT` es un dato, y mezclarlo
                        // con las rutas ensucia la flecha arriba justo cuando
                        // hace falta repetir el comando de verdad.
                        historial.empujar(&ruta[..n]);
                        match interpretar(&ruta[..n]) {
                            Orden::Nada => {
                                pintar_estado(&p, &caja, "escribe algo", TEXTO_TENUE);
                            }
                            Orden::Listar(ruta_dir) => {
                                match bmo::Directorio::abrir(ruta_dir) {
                                    Ok(d) => {
                                        let mut cuantas = 0u32;
                                        // Tope por si un directorio enorme se
                                        // comiera el fotograma entero.
                                        while cuantas < 256 {
                                            let e = match d.siguiente() {
                                                Some(e) => e,
                                                None => break,
                                            };
                                            let mut nom = [0u8; 12];
                                            let largo = e.legible(&mut nom);
                                            // `.` y `..` no se enseñan: aqui
                                            // no hay carpeta actual a la que
                                            // volver, asi que son ruido.
                                            if es_punto(&nom[..largo]) { continue; }
                                            salida.texto(b"  ");
                                            salida.texto(&nom[..largo]);
                                            // Alinear la columna del tamaño.
                                            let mut k = largo;
                                            while k < 14 { salida.byte(b' '); k += 1; }
                                            if e.es_dir {
                                                salida.texto(b"<DIR>");
                                            } else {
                                                let mut d10 = [0u8; 10];
                                                let n10 = decimal(e.bytes as u64, &mut d10);
                                                salida.texto(&d10[..n10]);
                                            }
                                            salida.byte(b'\n');
                                            cuantas += 1;
                                        }
                                        if cuantas == 0 {
                                            salida.texto(b"  (vacio)
");
                                        }
                                        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                    }
                                    Err(_) => {
                                        salida.texto(b"  no puedo abrir esa carpeta.
");
                                        pintar_estado(&p, &caja, "carpeta no encontrada", TEXTO_MAL);
                                    }
                                }
                                n = 0;
                            }
                            // ── Leer un archivo ──
                            //
                            // El hermano de `ls`: aquel dice QUE hay, este
                            // enseña lo de DENTRO. Es la primera vez que un
                            // programa de Ring 3 abre un archivo del disco.
                            Orden::Leer(ruta_arch) => {
                                match bmo::Archivo::leer_de(ruta_arch) {
                                    Ok(a) => {
                                        let mut trozo = [0u8; 256];
                                        let mut total = 0usize;
                                        // El ultimo byte se guarda segun pasa:
                                        // reconstruirlo al final obligaria a
                                        // saber en que trozo cayo, y el buffer
                                        // ya se ha reutilizado.
                                        let mut ultimo = 0u8;
                                        // De 256 en 256 y con tope: un archivo
                                        // que no sea texto llenaria la rejilla
                                        // de basura y se comeria el fotograma.
                                        loop {
                                            let n = a.leer(&mut trozo);
                                            if n == 0 { break; }
                                            salida.texto(&trozo[..n]);
                                            ultimo = trozo[n - 1];
                                            total += n;
                                            if total >= 2048 {
                                                salida.texto(b"\n  ...(cortado)\n");
                                                ultimo = b'\n';
                                                break;
                                            }
                                        }
                                        if total == 0 {
                                            salida.texto(b"  (vacio)\n");
                                        } else if ultimo != b'\n' {
                                            // Sin esto, el proximo mensaje se
                                            // pega al final del archivo.
                                            salida.byte(b'\n');
                                        }
                                        a.cerrar();
                                        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                    }
                                    Err(e) => {
                                        salida.con_tinta(TINTA_MAL);
                                        salida.texto(b"  ");
                                        salida.texto(motivo_archivo(e));
                                        salida.byte(b'\n');
                                        salida.con_tinta(TINTA_NORMAL);
                                        pintar_estado(&p, &caja, "no se pudo leer", TEXTO_MAL);
                                    }
                                }
                                n = 0;
                            }
                            // ── Escribir un archivo ──
                            //
                            // Lo que NUNCA habia pasado: un programa de Ring 3
                            // dejando algo en el disco. Hasta hoy todo lo que
                            // habia ahi lo puso el anfitrion al flashear o el
                            // kernel con su caja negra.
                            Orden::Escribir(ruta_arch, texto) => {
                                match bmo::Archivo::crear(ruta_arch) {
                                    Ok(a) => {
                                        let puestos = a.escribir(texto);
                                        // El salto final: un archivo de texto
                                        // sin el ultimo salto es el clasico
                                        // que descuadra al siguiente que lo lee.
                                        a.escribir(b"\n");
                                        // ★ Aqui es donde llega al disco. Antes
                                        // de esto no hay nada escrito.
                                        if a.cerrar() {
                                            salida.texto(b"  guardado: ");
                                            let mut d10 = [0u8; 10];
                                            let n10 = decimal(puestos as u64 + 1, &mut d10);
                                            salida.texto(&d10[..n10]);
                                            salida.texto(b" bytes\n");
                                            pintar_estado(&p, &caja, "guardado", TEXTO_BIEN);
                                        } else {
                                            salida.texto(b"  no se guardo nada.\n");
                                            pintar_estado(&p, &caja, "no se pudo guardar", TEXTO_MAL);
                                        }
                                    }
                                    Err(e) => {
                                        salida.con_tinta(TINTA_MAL);
                                        salida.texto(b"  ");
                                        salida.texto(motivo_archivo(e));
                                        salida.byte(b'\n');
                                        salida.con_tinta(TINTA_NORMAL);
                                        pintar_estado(&p, &caja, "no se pudo crear", TEXTO_MAL);
                                    }
                                }
                                n = 0;
                            }
                            Orden::Calculadora => {
                                calc.visible = !calc.visible;
                                if calc.visible {
                                    pintar_calc(&p, &calc_caja, &calc);
                                    salida.texto(b"  calculadora: la cara en Rust, el calculo en COBOL
");
                                } else {
                                    // Devolver esa zona a la escena.
                                    for f in 0..calc_caja.alto {
                                        for co in 0..calc_caja.ancho {
                                            let (px, py) = (calc_caja.x + co, calc_caja.y + f);
                                            p.punto(px, py, color_escena(&caja, visible, px, py));
                                        }
                                    }
                                }
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
                                cur = 0;
                            }
                            Orden::Limpiar => {
                                salida.limpiar();
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Ayuda => {
                                salida.texto(b"  <ruta>       lanza un .bex   (apps/COBOL.bex)\n");
                                salida.texto(b"  run <ruta>   lo mismo, como en el shell de Ring 0\n");
                                salida.texto(b"  cat <ruta>   ensena lo que hay dentro\n");
                                salida.texto(b"  write <ruta> <texto>     lo guarda\n");
                                salida.texto(b"  clear / cls  limpia esta salida\n");
                                salida.texto(b"  TAB          completa   Ctrl+A/E inicio/fin\n");
                                salida.texto(b"  Ctrl+K corta al final    Ctrl+W borra palabra\n");
                                salida.texto(b"  Ctrl+U borra linea       Ctrl+L limpia\n");
                                salida.texto(b"  info         RAM, CPU, tareas y disco\n");
                                salida.texto(b"  cpu / mem    solo esa parte del informe\n");
                                salida.texto(b"  help         esto\n");
                                salida.texto(b"  reboot       reinicia la maquina\n");
                                salida.texto(b"  Ctrl+Alt     esconde o invoca esta ventana\n");
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
                            }
                            // Ni se intenta lanzar. Se dice lo que es y con
                            // que se abre — un mensaje sobre la FIRMA aqui
                            // manda a buscar un permiso que no hace falta.
                            Orden::NoEsPrograma(r) => {
                                salida.con_tinta(TINTA_MAL);
                                salida.texto(b"  eso no es un programa (solo .bex se lanza).\n");
                                salida.texto(b"  para verlo:  cat ");
                                salida.texto(r);
                                salida.byte(b'\n');
                                salida.con_tinta(TINTA_NORMAL);
                                pintar_estado(&p, &caja, "no es un programa: prueba lee", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Informe => {
                                informe_sistema(&mut salida);
                                pintar_estado(&p, &caja, "informe del sistema", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Cpu => {
                                informe_cpu(&mut salida);
                                pintar_estado(&p, &caja, "procesador", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Memoria => {
                                informe_memoria(&mut salida);
                                pintar_estado(&p, &caja, "memoria", TEXTO_TENUE);
                                n = 0;
                            }
                            // Se pinta ANTES de pedirlo: la llamada no vuelve,
                            // asi que un mensaje despues no lo veria nadie. Y
                            // que quede escrito distingue "reinicio pedido" de
                            // "se colgo" en la foto. `Pantalla` escribe directo
                            // al framebuffer, asi que al volver de `texto` ya
                            // esta en el cristal: no hay nada que vaciar.
                            Orden::Reiniciar => {
                                salida.texto(b"  reiniciando...\n");
                                pintar_estado(&p, &caja, "reiniciando", TEXTO_TENUE);
                                bmo::reiniciar();
                            }
                            Orden::Desconocida => {
                                // El mensaje honesto. Antes se contestaba "no
                                // esta: revisa la ruta" a quien escribía
                                // `reboot`, y eso manda a buscar un archivo que
                                // nunca existió en vez de decir la verdad.
                                salida.texto(b"  no es un comando ni una ruta. escribe 'help'.\n");
                                pintar_estado(&p, &caja, "no lo conozco: prueba help", TEXTO_MAL);
                                n = 0;
                            }
                            Orden::Lanzar(objetivo) => {
                                let cap = salida_cap.as_ref().map(|c| c.cap).unwrap_or(0);
                                match bmo::ejecutar_en(objetivo, cap) {
                                    Ok(_) => {
                                        pintar_estado(&p, &caja, "lanzado", TEXTO_BIEN);
                                        // El campo se vacía al lanzar, como el
                                        // Win+R: la caja está para el SIGUIENTE
                                        // programa, no para admirar el anterior.
                                        n = 0;
                                    }
                                    Err(bmo::ERROR_NO_ESTA) => {
                                        pintar_estado(&p, &caja, "no esta: revisa la ruta", TEXTO_MAL)
                                    }
                                    Err(bmo::ERROR_GATE) => pintar_estado(
                                        &p,
                                        &caja,
                                        "rechazado: la firma no cuadra",
                                        TEXTO_MAL,
                                    ),
                                    Err(bmo::ERROR_OCUPADO) => {
                                        pintar_estado(&p, &caja, "no hay hueco ahora mismo", TEXTO_MAL)
                                    }
                                    Err(_) => {
                                        pintar_estado(&p, &caja, "no paso la admision", TEXTO_MAL)
                                    }
                                }
                            }
                        }
                        // El cursor detras de la linea, SIEMPRE. Las ramas que
                        // vacian el campo ponian `n = 0` y dejaban `cur` donde
                        // estaba: la tecla siguiente se escribia en `ruta[cur]`
                        // —fuera de lo que se dibuja— y el campo ensenaba los
                        // bytes VIEJOS del comando anterior. Escribir `2` tras
                        // `run apps/calc.bex` mostraba una `r`. Las ramas de
                        // error conservan la ruta a proposito para poder
                        // corregirla, y ahi `cur` no se mueve: por eso es un
                        // `min` y no un cero.
                        cur = cur.min(n);
                        repintar_campo = true;
                    }
                    // TAB: completar.
                    b'\t' => {
                        let antes = n;
                        n = completar(&mut ruta, n, &mut salida);
                        cur = n;
                        if n == antes {
                            pintar_estado(&p, &caja, "nada que completar", TEXTO_TENUE);
                        }
                        repintar_campo = true;
                    }
                    // Retroceso.
                    0x08 | 0x7F => {
                        if cur > 0 {
                            let mut k = cur;
                            while k < n {
                                ruta[k - 1] = ruta[k];
                                k += 1;
                            }
                            cur -= 1;
                            n -= 1;
                            repintar_campo = true;
                        }
                    }
                    // Escape: borrar la línea entera, igual que en el Win+R.
                    0x1B => {
                        n = 0;
                        cur = 0;
                        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                        repintar_campo = true;
                    }
                    // ── El portapapeles ──
                    //
                    // Ctrl+C copia la línea entera; Ctrl+V la pega donde esté
                    // el cursor. No es un lujo: la mitad de lo que se teclea en
                    // un terminal es una variación de lo anterior, y sin copiar
                    // hay que reescribirlo todo.
                    //
                    // Ctrl+C para copiar y no para interrumpir, que es lo que
                    // significa en Unix. Aquí no hay señales que mandar, y el
                    // dedo que ya sabe Ctrl+C sabe copiar — no interrumpir.
                    0x03 => {
                        porta_n = n;
                        porta[..n].copy_from_slice(&ruta[..n]);
                        pintar_estado(&p, &caja, "copiado", TEXTO_TENUE);
                    }
                    0x16 => {
                        if porta_n > 0 && n + porta_n <= RUTA_MAX {
                            // Hueco del tamaño del pegado, y meterlo.
                            let mut k = n;
                            while k > cur {
                                ruta[k + porta_n - 1] = ruta[k - 1];
                                k -= 1;
                            }
                            ruta[cur..cur + porta_n].copy_from_slice(&porta[..porta_n]);
                            cur += porta_n;
                            n += porta_n;
                            repintar_campo = true;
                        }
                    }
                    // Ctrl+U — borra la línea. Ctrl+L — borra la salida.
                    // Los mismos que el shell de Ring 0, porque los dedos ya
                    // los tienen y un atajo que cambia entre dos ventanas del
                    // mismo sistema es peor que no tenerlo.
                    0x15 => {
                        n = 0;
                        cur = 0;
                        repintar_campo = true;
                    }
                    0x0C => {
                        salida.limpiar();
                        repintar_campo = true;
                    }
                    // FLECHA ARRIBA / ABAJO — el historial. Llegan por la misma
                    // cola que las letras, con bytes del rango C1 (0x80..0x9F)
                    // que no tienen glifo: el driver los eligió justo para que
                    // no puedan confundirse con texto.
                    // Ctrl+ARRIBA copia, Ctrl+ABAJO pega. Lo mismo que
                    // Ctrl+C / Ctrl+V, con las flechas — porque los dedos que
                    // ya andan por el historial no tienen que irse a buscar
                    // otra tecla para copiar lo que acaban de recuperar.
                    0x80 if ctrl => {
                        porta_n = n;
                        porta[..n].copy_from_slice(&ruta[..n]);
                        pintar_estado(&p, &caja, "copiado", TEXTO_TENUE);
                    }
                    0x81 if ctrl => {
                        if porta_n > 0 && n + porta_n <= RUTA_MAX {
                            let mut k = n;
                            while k > cur {
                                ruta[k + porta_n - 1] = ruta[k - 1];
                                k -= 1;
                            }
                            ruta[cur..cur + porta_n].copy_from_slice(&porta[..porta_n]);
                            cur += porta_n;
                            n += porta_n;
                            repintar_campo = true;
                        }
                    }
                    0x80 => {
                        if let Some(k) = historial.atras(&mut ruta) {
                            n = k;
                            cur = k;
                            repintar_campo = true;
                        }
                    }
                    0x81 => {
                        if let Some(k) = historial.adelante(&mut ruta) {
                            n = k;
                            cur = k;
                            repintar_campo = true;
                        }
                    }
                    // IZQUIERDA / DERECHA — mover el cursor.
                    0x82 => {
                        if cur > 0 { cur -= 1; repintar_campo = true; }
                    }
                    0x83 => {
                        if cur < n { cur += 1; repintar_campo = true; }
                    }
                    // INICIO / FIN.
                    0x84 => { cur = 0; repintar_campo = true; }
                    0x85 => { cur = n; repintar_campo = true; }
                    // ── Los atajos de edicion de linea ──
                    //
                    // Los de toda la vida en una consola: Ctrl+A al principio,
                    // Ctrl+E al final, Ctrl+K corta hasta el final, Ctrl+W
                    // borra la palabra de atras. Van ADEMAS de Inicio/Fin, que
                    // ya estaban: los dedos que vienen de un terminal buscan
                    // estos, y los que vienen de Windows buscan aquellos.
                    // Atender a los dos cuesta cuatro lineas.
                    0x01 => { cur = 0; repintar_campo = true; }
                    0x05 => { cur = n; repintar_campo = true; }
                    // Ctrl+K: tirar lo que hay del cursor al final.
                    0x0B => {
                        n = cur;
                        repintar_campo = true;
                    }
                    // Ctrl+W: borrar la palabra de atras. Primero se comen los
                    // espacios y luego las letras, que es lo que espera
                    // cualquiera que lo haya usado — si no, borrar tras un
                    // espacio no haria nada.
                    0x17 => {
                        let mut k = cur;
                        while k > 0 && ruta[k - 1] == b' ' { k -= 1; }
                        while k > 0 && ruta[k - 1] != b' ' { k -= 1; }
                        let quitados = cur - k;
                        if quitados > 0 {
                            let mut i = cur;
                            while i < n {
                                ruta[i - quitados] = ruta[i];
                                i += 1;
                            }
                            n -= quitados;
                            cur = k;
                            repintar_campo = true;
                        }
                    }
                    // SUPRIMIR — borra HACIA ADELANTE, al reves que el
                    // retroceso. Son dos teclas porque son dos intenciones.
                    0x86 => {
                        if cur < n {
                            let mut k = cur + 1;
                            while k < n { ruta[k - 1] = ruta[k]; k += 1; }
                            n -= 1;
                            repintar_campo = true;
                        }
                    }
                    // El resto de navegación (PgUp/PgDn) se ignora, pero
                    // EXPLÍCITAMENTE: dejarlas caer al comodín las dibujaría
                    // como basura.
                    0x87..=0x9F => {}
                    // Todo lo demás imprimible, incluido el Latin-1 alto: la
                    // `ñ` llega como 0xF1 y la fuente la tiene.
                    c if c >= 0x20 => {
                        if n < RUTA_MAX {
                            // Hueco en el cursor y meter ahi: escribir en
                            // medio de una linea es lo normal, no un caso raro.
                            let mut k = n;
                            while k > cur {
                                ruta[k] = ruta[k - 1];
                                k -= 1;
                            }
                            ruta[cur] = c;
                            cur += 1;
                            n += 1;
                            repintar_campo = true;
                        }
                    }
                    _ => {}
                }
            }

            // ── Ratón ──
            let pos = e.puntero();
            // ── Los botones de la calculadora ──
            let boton = pos.botones != 0;
            if calc.visible && boton && !boton_antes && !calc.esperando {
                if let Some(t) = calc_caja.tecla_en(pos.x, pos.y) {
                    match t {
                        b'C' => calc.limpiar(),
                        b'+' => calc.operador(1),
                        b'-' => calc.operador(2),
                        b'*' => calc.operador(3),
                        b'/' => calc.operador(4),
                        b'=' => {
                            if calc.op != 0 && calc.guardado_n > 0 && calc.n > 0 {
                                // Lanzar el MOTOR y darle los tres datos por su
                                // consola. Aqui es donde la cara deja de saber
                                // de aritmetica y empieza a saber COBOL.
                                let cap = salida_cap.as_ref().map(|c| c.cap).unwrap_or(0);
                                if bmo::ejecutar_en(b"apps/calcgui.bex", cap).is_ok() {
                                    if let Some(cc) = salida_cap.as_ref() {
                                        cc.escribir(&calc.guardado[..calc.guardado_n]);
                                        cc.escribir(b"\n");
                                        cc.escribir(&[b'0' + calc.op]);
                                        cc.escribir(b"\n");
                                        cc.escribir(&calc.entrada[..calc.n]);
                                        cc.escribir(b"\n");
                                    }
                                    calc.esperando = true;
                                    resp_n = 0;
                                } else {
                                    pintar_estado(&p, &caja, "falta apps/calcgui.bex", TEXTO_MAL);
                                }
                            }
                        }
                        d => calc.meter(d),
                    }
                    pintar_calc(&p, &calc_caja, &calc);
                }
            }
            boton_antes = boton;
            if pos.x != ax || pos.y != ay {
                if ax != u32::MAX && borrar_cursor(&p, &caja, visible, ax, ay) {
                    // El cursor pasó por encima de la caja: la escena restauró
                    // los rectángulos, pero no las letras — ni las del campo ni
                    // las de la salida.
                    repintar_campo = true;
                    salida.sucia = true;
                }
                dibujar_cursor(&p, pos.x, pos.y);
                ax = pos.x;
                ay = pos.y;
            }

            // El pulsómetro. Se satura a propósito: interesa "late / no late",
            // no el valor exacto, y una barra que se sale de la pantalla no
            // dice nada que no diga una llena.
            let ev = e.eventos().min(PULSO_ANCHO as u64) as u32;
            if ev != pulso_previo {
                p.rect(PULSO_X, PULSO_Y, ev, PULSO_ALTO, ACENTO);
                pulso_previo = ev;
            }
            // Los botones, encima del marco: pulsar debería verse aunque el
            // movimiento no llegue. Son dos preguntas distintas al mismo HID.
            let col = if pos.botones != 0 { 0x00FF_FFFF } else { FONDO };
            p.rect(PULSO_X + PULSO_ANCHO + 16, PULSO_Y, PULSO_ALTO, PULSO_ALTO, col);
        }

        // ── Drenar la salida de los hijos ──
        //
        // Con tope por fotograma. Un programa que escupe sin parar podría
        // quedarse con el bucle entero y congelar el cursor: es preferible que
        // la salida vaya un poco por detrás a que el escritorio deje de
        // responder. Lo que no se lea ahora sigue en el anillo del kernel.
        if let Some(c) = salida_cap.as_ref() {
            let mut buf = [0u8; 8];
            let mut vueltas = 0;
            while vueltas < 64 {
                let leidos = c.leer(&mut buf);
                if leidos == 0 {
                    break;
                }
                if calc.esperando {
                    // Todo lo que escriba el motor es la respuesta: el
                    // programa no imprime prompts a proposito.
                    for &b in &buf[..leidos] {
                        if b == b'\n' {
                            if resp_n > 0 {
                                calc.entrada = [0; 20];
                                let k = resp_n.min(calc.entrada.len());
                                calc.entrada[..k].copy_from_slice(&resp[..k]);
                                calc.n = k;
                                calc.guardado_n = 0;
                                calc.op = 0;
                                calc.esperando = false;
                                pintar_calc(&p, &calc_caja, &calc);
                            }
                        } else if resp_n < resp.len() && b >= 0x20 {
                            resp[resp_n] = b;
                            resp_n += 1;
                        }
                    }
                } else {
                    salida.texto(&buf[..leidos]);
                }
                vueltas += 1;
            }
        }
        if salida.sucia {
            // Se pinta sólo si se ve; el contenido sigue acumulándose oculto,
            // así que al invocar la ventana está todo lo que pasó mientras.
            if visible {
                pintar_salida(&p, &caja, &salida);
            }
            salida.sucia = false;
        }

        // El parpadeo del cursor de escritura. Sólo repinta cuando cambia de
        // estado — repintar el campo cada vuelta sería reescribir la ruta
        // miles de veces por segundo para que se vea igual.
        //
        // ★ El contador se REINICIA con cada tecla (ver el manejador). Antes
        // era `vueltas % PARPADEO`, un reloj que corría solo: si te ponías a
        // escribir justo cuando tocaba apagarlo, el cursor desaparecía a mitad
        // de la palabra y no volvía hasta la siguiente vuelta entera. Un
        // cursor que se esconde mientras escribes es lo contrario de lo que
        // un cursor existe para decir.
        desde_tecla = desde_tecla.wrapping_add(1);
        if desde_tecla >= PARPADEO {
            desde_tecla = 0;
            caret = !caret;
            repintar_campo = true;
        }
        if repintar_campo && visible {
            pintar_campo(&p, &caja, &ruta[..n], cur, caret);
        }

        bmo::ceder();
    }
}

/// Un pánico aquí no puede tumbar nada más que a este proceso: lo dice y sale
/// por la puerta normal. El kernel revoca sus capabilities —incluidas la
/// pantalla y la entrada— y sigue vivo.
#[panic_handler]
fn panico(_info: &core::panic::PanicInfo) -> ! {
    bmo::consola("panico en el compositor\n");
    bmo::salir()
}
