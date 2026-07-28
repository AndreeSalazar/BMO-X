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

const CAJA_ANCHO: u32 = 560;
const CAJA_ALTO: u32 = 140;
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
            estado_y: y + CAJA_ALTO - 30,
        }
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
fn color_escena(c: &Caja, x: u32, y: u32) -> u32 {
    if y < BARRA_ALTO {
        // La marca de referencia dentro de la barra.
        if x >= 16 && x < 32 && y >= 14 && y < 30 {
            return ACENTO;
        }
        return BARRA;
    }
    if c.contiene(x, y) {
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
fn borrar_cursor(p: &bmo::Pantalla, c: &Caja, x: u32, y: u32) -> bool {
    let mut toco = false;
    for fila in 0..CUR_ALTO as u32 {
        for col in 0..CUR_ANCHO as u32 {
            let (px, py) = (x + col, y + fila);
            if c.contiene(px, py) {
                toco = true;
            }
            p.punto(px, py, color_escena(c, px, py));
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
        "Escribe la ruta de un .bex y pulsa Enter.",
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
    pintar_campo(&p, &caja, &ruta[..n], true);
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

    loop {
        vueltas = vueltas.wrapping_add(1);
        let mut repintar_campo = false;

        if let Some(e) = entrada.as_ref() {
            // ── Teclado ──
            //
            // Se drena hasta vaciar, no una tecla por fotograma: escribiendo
            // rápido llegan varias entre vuelta y vuelta, y quedarse con una
            // sería perder letras de forma que parecería un teclado malo.
            while let Some(c) = e.tecla() {
                match c {
                    b'\r' | b'\n' => {
                        if n == 0 {
                            pintar_estado(&p, &caja, "escribe una ruta primero", TEXTO_TENUE);
                        } else {
                            match bmo::ejecutar(&ruta[..n]) {
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
                if ax != u32::MAX && borrar_cursor(&p, &caja, ax, ay) {
                    // El cursor pasó por encima de la caja: la escena restauró
                    // los rectángulos, pero no las letras.
                    repintar_campo = true;
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

        // El parpadeo del cursor de escritura. Sólo repinta cuando cambia de
        // estado — repintar el campo cada vuelta sería reescribir la ruta
        // miles de veces por segundo para que se vea igual.
        if vueltas % PARPADEO == 0 {
            caret = !caret;
            repintar_campo = true;
        }
        if repintar_campo {
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
