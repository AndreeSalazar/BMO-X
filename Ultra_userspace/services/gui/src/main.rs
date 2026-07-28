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
fn pintar_caja(p: &bmo::Pantalla, c: &Caja) {
    p.rect(c.x, c.y, CAJA_ANCHO, CAJA_ALTO, CAJA_BORDE);
    p.rect(c.x + 2, c.y + 2, CAJA_ANCHO - 4, CAJA_ALTO - 4, CAJA_FONDO);
    p.texto(c.x + 18, c.y + 16, "Ejecutar", TEXTO);
    p.texto(
        c.x + 18,
        c.y + 34,
        "Escribe la ruta de un .bex y pulsa Enter.   Ctrl+Alt esconde/invoca.",
        TEXTO_TENUE,
    );
    p.rect(c.campo_x, c.campo_y, c.campo_ancho, c.campo_alto, CAMPO_FONDO);
}

/// El contenido del campo: la ruta y el cursor de escritura.
///
/// Repinta el fondo del campo entero antes de escribir. Es un rectángulo de
/// unos 500x28 px —nada— y evita el clásico de borrar un carácter y que quede
/// medio glifo del anterior porque el nuevo es más estrecho.
fn pintar_campo(p: &bmo::Pantalla, c: &Caja, ruta: &[u8], caret: bool) {
    p.rect(c.campo_x, c.campo_y, c.campo_ancho, c.campo_alto, CAMPO_FONDO);

    // Si la ruta no cabe, se ve el FINAL: es donde está el cursor y donde uno
    // mira mientras escribe. Ver el principio de una ruta que ya no estás
    // tocando no ayuda a nadie.
    let cabe = c.visibles();
    let visible = if ruta.len() > cabe {
        &ruta[ruta.len() - cabe..]
    } else {
        ruta
    };
    let fin = p.texto_bytes(c.texto_x, c.texto_y, visible, TEXTO);

    if caret {
        p.rect(fin, c.texto_y, 2, bmo::GLIFO_ALTO, ACENTO);
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
struct Salida {
    celdas: [[u8; SAL_COLS]; SAL_ROWS],
    fila: usize,
    col: usize,
    /// Hay algo nuevo que pintar. Repintar la rejilla entera cada fotograma
    /// serían 88x16 glifos por vuelta sobre memoria de vídeo sin caché.
    sucia: bool,
}

impl Salida {
    fn nueva() -> Self {
        Self { celdas: [[b' '; SAL_COLS]; SAL_ROWS], fila: 0, col: 0, sucia: true }
    }

    /// Sube todo una línea y deja la última en blanco.
    fn desplazar(&mut self) {
        for f in 1..SAL_ROWS {
            self.celdas[f - 1] = self.celdas[f];
        }
        self.celdas[SAL_ROWS - 1] = [b' '; SAL_COLS];
    }

    fn salto(&mut self) {
        self.col = 0;
        if self.fila + 1 >= SAL_ROWS {
            self.desplazar();
        } else {
            self.fila += 1;
        }
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
        self.fila = 0;
        self.col = 0;
        self.sucia = true;
    }
}

// ── La línea de comandos ────────────────────────────────────────────────

/// Qué pidió el usuario. Se separa del bucle porque la decisión "esto es un
/// comando o es una ruta" merece leerse de un vistazo.
enum Orden<'a> {
    Nada,
    Lanzar(&'a [u8]),
    Limpiar,
    Ayuda,
    /// `ls [ruta]` — qué hay en el disco. Antes esto no podía existir: no
    /// había capability de directorio, así que había que saberse los nombres
    /// de memoria y teclearlos enteros.
    Listar(&'a [u8]),
    /// Una palabra suelta que no parece una ruta. `reboot`, `ls`, `dir`...
    Desconocida,
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

fn parece_ruta(t: &[u8]) -> bool {
    t.iter().any(|&c| c == b'/' || c == b'\\' || c == b'.')
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
        b"run" | b"RUN" => {
            if resto.is_empty() { Orden::Ayuda } else { Orden::Lanzar(resto) }
        }
        b"clear" | b"cls" => Orden::Limpiar,
        b"ls" | b"dir" => Orden::Listar(resto),
        b"help" | b"ayuda" | b"?" => Orden::Ayuda,
        _ if parece_ruta(linea) => Orden::Lanzar(linea),
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
        // La última línea escrita en otro color: es la que acabas de provocar.
        let color = if f == s.fila { SAL_ECO } else { SAL_TEXTO };
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
    if salida_cap.is_none() {
        salida.texto(b"sin consola: la salida de los programas ira al panel del kernel\n");
    }
    pintar_campo(&p, &caja, &ruta[..n], true);
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
            let combo = m & bmo::MOD_CTRL != 0 && m & bmo::MOD_ALT != 0;
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
                match c {
                    b'\r' | b'\n' => {
                        // Eco SIEMPRE, también de lo que no se entiende: un
                        // terminal que se traga lo que escribiste deja al
                        // usuario sin saber qué llegó.
                        salida.texto(b"> ");
                        salida.texto(&ruta[..n]);
                        salida.byte(b'\n');
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
                            Orden::Limpiar => {
                                salida.limpiar();
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Ayuda => {
                                salida.texto(b"  <ruta>       lanza un .bex   (apps/COBOL.bex)\n");
                                salida.texto(b"  run <ruta>   lo mismo, como en el shell de Ring 0\n");
                                salida.texto(b"  clear / cls  limpia esta salida\n");
                                salida.texto(b"  help         esto\n");
                                salida.texto(b"  Ctrl+Alt     esconde o invoca esta ventana\n");
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
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
                        repintar_campo = true;
                    }
                    // Retroceso.
                    0x08 | 0x7F => {
                        if n > 0 {
                            n -= 1;
                            repintar_campo = true;
                        }
                    }
                    // Escape: borrar la línea entera, igual que en el Win+R.
                    0x1B => {
                        n = 0;
                        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                        repintar_campo = true;
                    }
                    // Las teclas de navegación viajan por la misma cola con
                    // bytes del rango C1 (0x80..0x9F), que no tienen glifo.
                    // Esta caja no tiene cursor que mover dentro de la línea,
                    // así que se ignoran — pero explícitamente, para que no se
                    // dibujen como basura.
                    0x80..=0x9F => {}
                    // Todo lo demás imprimible, incluido el Latin-1 alto: la
                    // `ñ` llega como 0xF1 y la fuente la tiene.
                    c if c >= 0x20 => {
                        if n < RUTA_MAX {
                            ruta[n] = c;
                            n += 1;
                            repintar_campo = true;
                        }
                    }
                    _ => {}
                }
            }

            // ── Ratón ──
            let pos = e.puntero();
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
                salida.texto(&buf[..leidos]);
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
        if vueltas % PARPADEO == 0 {
            caret = !caret;
            repintar_campo = true;
        }
        if repintar_campo && visible {
            pintar_campo(&p, &caja, &ruta[..n], caret);
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
