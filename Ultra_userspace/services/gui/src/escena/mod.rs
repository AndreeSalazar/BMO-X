//! **Lo que se PINTA.**
//!
//! Colores, geometria y las primitivas de dibujo de la ventana. Aqui no se
//! interpreta nada: un modulo de esta carpeta no sabe que es un comando.

use bmo_userland as bmo;

pub(crate) mod calc;
pub(crate) mod conmutador;
pub(crate) mod datos;
pub(crate) mod cursor;
/// La ENTRADA a Ring 3: lo que se ve cuando el userspace toma la máquina.
pub(crate) mod entrada;
/// La consola del KERNEL (F11): lo que dice Ring 0, leído desde Ring 3. No da
/// privilegio, da vista — ver la cabecera del módulo.
pub(crate) mod klog;
pub(crate) mod salida;


// ── La escena ───────────────────────────────────────────────────────────

pub(crate) const FONDO: u32 = 0x0014_1C2B;
pub(crate) const BARRA: u32 = 0x0028_3448;
pub(crate) const ACENTO: u32 = 0x004C_9BE8;

pub(crate) const BARRA_ALTO: u32 = 44;

/// Los seis parches de medida, con sus valores EXACTOS. No son decorativos:
/// cada uno responde una pregunta distinta sobre el formato.
pub(crate) const MEDIDA: [u32; 6] = [
    0x00FF_0000, // ¿rojo o azul? -> orden de canales
    0x0000_FF00, // verde: el canal de en medio no cambia con el orden
    0x0000_00FF, // el complementario del primero
    0x00FF_FFFF, // blanco: el techo
    0x0080_8080, // gris medio: la mitad
    0x0020_2020, // casi negro: si esto sale claro, no es orden, es intensidad
];
pub(crate) const MEDIDA_LADO: u32 = 56;
pub(crate) const MEDIDA_Y: u32 = BARRA_ALTO + 24;
pub(crate) const MEDIDA_X: u32 = 24;

/// Pulsómetro del ratón: cuántos reportes HID han llegado. Quieto = el ratón no
/// llega; creciendo = late. Ahora que hay fuente podría escribirse el número,
/// pero una barra se lee de un vistazo desde el otro lado del cuarto, que es
/// desde donde se mira una máquina que está arrancando.
pub(crate) const PULSO_X: u32 = 24;
pub(crate) const PULSO_Y: u32 = MEDIDA_Y + MEDIDA_LADO + 32;
pub(crate) const PULSO_ANCHO: u32 = 240;
pub(crate) const PULSO_ALTO: u32 = 14;

// ── La caja ─────────────────────────────────────────────────────────────

pub(crate) const CAJA_ANCHO: u32 = 760;
pub(crate) const CAJA_ALTO: u32 = 428;

/// La rejilla de SALIDA: lo que imprimen los programas que se lanzan desde
/// aquí. Antes no existía y no era un olvido — **no había dónde leerlo**:
/// `OP_CONSOLE_WRITE` iba siempre al panel del kernel, así que un terminal de
/// Ring 3 no podía ver lo que escribía su propio hijo. Con `KIND_CONSOLE` la
/// salida tiene dueño, y el dueño es este proceso.
pub(crate) const SAL_COLS: usize = 88;
pub(crate) const SAL_ROWS: usize = 16;
/// Cuántas filas se GUARDAN, aunque sólo se vean [`SAL_ROWS`].
///
/// ★ Antes lo que salía por arriba se perdía para siempre: `desplazar` movía
/// las filas y la de arriba se tiraba. Un `ls` largo o la salida de un batch se
/// iban sin que hubiera forma de volver a mirarlas — y eso en una máquina donde
/// depurar es hacer una foto de la pantalla duele el doble.
///
/// 200 filas de 88 columnas son 17 KiB. La pantalla es de 8 MiB.
pub(crate) const SAL_HIST: usize = 200;
pub(crate) const SAL_TEXTO: u32 = 0x00C8_D8E8;
pub(crate) const SAL_ECO: u32 = 0x0079_C4F2;
pub(crate) const CAJA_FONDO: u32 = 0x001E_2A40;
pub(crate) const CAJA_BORDE: u32 = 0x004C_9BE8;
pub(crate) const CAMPO_FONDO: u32 = 0x000C_1220;
pub(crate) const TEXTO: u32 = 0x00E6_EDF6;
pub(crate) const TEXTO_TENUE: u32 = 0x008A_9BB4;
pub(crate) const TEXTO_MAL: u32 = 0x00FF_8A7A;
pub(crate) const TEXTO_BIEN: u32 = 0x007E_E787;

/// Cuántos bytes de ruta caben. Es el mismo tope que el renglón del kernel
/// (`RUTA_MAX`), y no por casualidad: escribir más de lo que el otro lado puede
/// aceptar sería dejar que la ruta se corte en silencio a mitad de camino.
pub(crate) const RUTA_MAX: usize = 128;

/// Geometría de la caja, ya resuelta contra el tamaño real del panel.
pub(crate) struct Caja {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) campo_x: u32,
    pub(crate) campo_y: u32,
    pub(crate) campo_ancho: u32,
    pub(crate) campo_alto: u32,
    pub(crate) texto_x: u32,
    pub(crate) texto_y: u32,
    pub(crate) estado_y: u32,
    pub(crate) salida_x: u32,
    pub(crate) salida_y: u32,
}

impl Caja {
    pub(crate) fn nueva(ancho: u32, alto: u32) -> Self {
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
    pub(crate) fn salida_alto(&self) -> u32 {
        SAL_ROWS as u32 * bmo::GLIFO_ALTO
    }

    /// Cuántos caracteres caben en el campo. El resto se recorta al pintar —
    /// nunca al guardar: lo que no se ve sigue estando en la ruta.
    pub(crate) fn visibles(&self) -> usize {
        ((self.campo_ancho - 12) / bmo::GLIFO_ANCHO) as usize
    }

    pub(crate) fn contiene(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + CAJA_ANCHO && y >= self.y && y < self.y + CAJA_ALTO
    }

    /// ¿Este píxel cae DENTRO del campo donde se escribe?
    ///
    /// Lo usa el puntero para cambiar a la barra de texto. Es la misma cuenta
    /// que ya hacía `color_escena` para saber qué color devolver, dicha una vez
    /// y con nombre: dos copias de la misma geometría se separan en cuanto
    /// alguien mueve el campo dos píxeles.
    pub(crate) fn en_campo(&self, x: u32, y: u32) -> bool {
        x >= self.campo_x
            && x < self.campo_x + self.campo_ancho
            && y >= self.campo_y
            && y < self.campo_y + self.campo_alto
    }
}

/// Qué color le toca a un píxel según la escena. Es el modelo entero del
/// escritorio, y es lo que permite borrar el cursor sin repintarlo todo: para
/// restaurar una zona basta con volver a preguntar qué había ahí.
///
/// Sabe de rectángulos, no de letras. Por eso `borrar_cursor` avisa cuando ha
/// pasado por encima de la caja: el texto hay que volver a escribirlo.
pub(crate) fn color_escena(c: &Caja, visible: bool, x: u32, y: u32) -> u32 {
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
pub(crate) fn pintar_caja(p: &bmo::Pantalla, c: &Caja) {
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
        "ruta de un .bex y Enter.  info / cpu / mem / perf / ls / lee / guarda / reboot.",
        TEXTO_TENUE,
    );
    // ★ Las dos ventanas del sistema, DICHAS. Un atajo que no está escrito en
    // ninguna parte es un atajo que sólo conoce quien lo programó — y F11 existe
    // precisamente para los días en que esta caja no responde.
    //
    // Va al PIE de la caja, no debajo de la pista: ahí lo puse primero y
    // `campo_y` es exactamente `y + 54`, así que el marco del campo lo pintaba
    // encima y la línea no se veía. Se cazó en la foto — el texto se emitía y
    // desaparecía en la instrucción siguiente.
    p.texto(
        c.x + 18,
        c.y + CAJA_ALTO - 22,
        "F11 kernel (Ring 0)   F12 datos (ESTRATOS)   ESC cierra",
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
pub(crate) fn pintar_campo(p: &bmo::Pantalla, c: &Caja, ruta: &[u8], cur: usize, caret: bool) {
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


/// Borra la caja devolviendo cada píxel a lo que la escena dice que hay
/// debajo. Es el precio de que la ventana se pueda invocar y esconder.
///
/// Recorre el rectángulo entero — unos 325k píxeles sobre memoria de vídeo sin
/// caché, que no es gratis. Pero pasa UNA vez por pulsación de atajo, no por
/// fotograma, y la alternativa (guardar lo que había debajo) sería un buffer de
/// 1,3 MB en un proceso con 64 KiB de pila.
pub(crate) fn borrar_caja(p: &bmo::Pantalla, c: &Caja) {
    for fila in 0..CAJA_ALTO {
        for col in 0..CAJA_ANCHO {
            let (x, y) = (c.x + col, c.y + fila);
            p.punto(x, y, color_escena(c, false, x, y));
        }
    }
}

/// Borra la consola de datos devolviendo cada píxel a lo que hay debajo.
///
/// ★ `visible` es si la caja de Ejecutar está abierta, y hace falta: la consola
/// de datos se pinta ENCIMA de ella. `color_escena` sabe devolver el color de
/// la caja cuando el píxel cae dentro, así que pasarle `false` aquí dejaría un
/// agujero con el fondo del escritorio en medio de la ventana de abajo.
///
/// Quien llama repinta después el texto de la caja: esto devuelve el fondo, no
/// las letras.
/// ★ Toma un RECTÁNGULO y no una ventana concreta.
///
/// Era `borrar_datos(&datos::CajaDatos)`, atado al tipo de una ventana — y con
/// eso, añadir la segunda ventana obligaba a copiar la función. Lo que esto
/// hace no depende de qué ventana se cierra: devuelve el fondo de un área.
pub(crate) fn borrar_ventana(
    p: &bmo::Pantalla,
    c: &Caja,
    x0: u32,
    y0: u32,
    ancho: u32,
    alto: u32,
    visible: bool,
) {
    for fila in 0..alto {
        for col in 0..ancho {
            let (x, y) = (x0 + col, y0 + fila);
            p.punto(x, y, color_escena(c, visible, x, y));
        }
    }
}

pub(crate) fn pintar_estado(p: &bmo::Pantalla, c: &Caja, msg: &str, color: u32) {
    // Ancho fijo de limpieza: el mensaje anterior puede ser más largo que el
    // nuevo, y media frase vieja detrás de una nueva es peor que ninguna.
    p.rect(c.x + 18, c.estado_y, CAJA_ANCHO - 36, bmo::GLIFO_ALTO, CAJA_FONDO);
    p.texto(c.x + 18, c.estado_y, msg, color);
}

