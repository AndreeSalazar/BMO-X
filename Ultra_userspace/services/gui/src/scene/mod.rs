//! **Lo que se PINTA.**
//!
//! Colores, geometria y las primitivas de dibujo de la ventana. Aqui no se
//! interpreta nada: un modulo de esta carpeta no sabe que es un comando.

use bmo_userland as bmo;

/// **La ventana de CABINA** (F11): lo que el kernel ve, con su gravedad y en
/// su color. Sustituye a la del klog, que era texto plano sin severidad.
/// F7 y F8: lo que la maquina esta haciendo AHORA, cada uno en su ventana.
pub(crate) mod vitals;
pub(crate) mod cabina;
pub(crate) mod calc;
pub(crate) mod switcher;
pub(crate) mod data;
pub(crate) mod cursor;
/// La ENTRADA a Ring 3: lo que se ve cuando el userspace toma la maquina.
pub(crate) mod splash;
/// El LOGO, en dos mascaras de 1 bit. Generado por `docs/arte/gato_a_mascara.py`.
pub(crate) mod gato;
/// El MARCO compartido: geometria, arrastre, estirar, maximizar y los tres
/// botones. Lo que toda ventana tiene y ninguna deberia escribir dos veces.
pub(crate) mod launcher;
pub(crate) mod chrome;
pub(crate) mod output;
/// **La luz del bus USB en la barra**: si el teclado se muere, se ve sin abrir
/// nada. E6 de `docs/EL_TECLADO_EXIGE.md`.
pub(crate) mod testigo;
/// **La ventana del SONIDO** (F10). Reclama `KIND_AUDIO` al abrirse y lo
/// DEVUELVE al cerrarse -- ver la cabecera del modulo: es lo unico que impide
/// que el escritorio deje mudos a todos los programas que lanza.
pub(crate) mod sound;
/// **La SUPERFICIE de una app**: memoria que otro proceso dibuja y el DIRECTOR
/// pega dentro de un marco. Es lo que convierte "prestar la pantalla entera" en
/// "tener una ventana".
pub(crate) mod surface;


// -- La escena -----------------------------------------------------------

// === La paleta ===========================================================
//
// * Se rehizo entera el 2026-08-04. La de antes venia del bring-up: colores
// escogidos para VERSE, no para mirarse una hora seguida. Ahora es un gris
// azulado oscuro con un acento y poco mas, que es lo que hacen tanto Windows 11
// como cualquier escritorio de Linux moderno -- y por el mismo motivo: en una
// pantalla con ventanas, el color es para SEPARAR planos, no para decorar.
//
// La regla que ordena la paleta: **cuanto mas cerca de ti, mas claro.** El
// escritorio es lo mas oscuro, la ventana esta por encima, y su barra de titulo
// por encima de ella. El ojo lee la profundidad sin que nadie se la explique.

/// El fondo del escritorio, arriba y abajo. Es un DEGRADADO, no un color.
///
/// Cuesta una franja de `rect` cada ocho filas --nada-- y quita de golpe el
/// aspecto de "pantalla de arranque": un plano liso enorme es lo que hace que
/// algo parezca un panel de diagnostico y no un escritorio.
pub(crate) const BG_TOP: u32 = 0x001B_2233;
pub(crate) const BG_BOTTOM: u32 = 0x000C_0F17;
/// El color de referencia cuando hace falta uno solo (bordes de mezcla).
pub(crate) const BG: u32 = 0x0014_1A28;
/// La barra de arriba. Mas oscura que el escritorio a proposito: una barra de
/// sistema se lee como un borde de la pantalla, no como una ventana.
pub(crate) const TASKBAR: u32 = 0x000F_131D;
/// El pelo de luz bajo la barra. Un borde entero seria una raya; esto separa.
pub(crate) const TASKBAR_LINE: u32 = 0x0026_2F42;
pub(crate) const ACCENT: u32 = 0x0060_A5FA;

pub(crate) const TASKBAR_H: u32 = 40;

// -- Esquinas redondeadas ------------------------------------------------
//
// No hay primitiva de circulo ni la va a haber. Un cuarto de circulo son ocho
// sangrias, y una tabla de ocho numeros es mas honesta que una raiz cuadrada en
// coma flotante que este sistema no tiene.

pub(crate) const RADIUS: u32 = 8;
/// Cuanto se mete cada fila del extremo. Es un cuarto de circulo de radio 8
/// tabulado: `CURVE_TABLE[0]` es la fila del borde y `CURVE_TABLE[7]` ya no se mete.
const CURVE_TABLE: [u32; RADIUS as usize] = [8, 5, 3, 2, 1, 1, 0, 0];

/// Esta `(x, y)` DENTRO de un rectangulo de esquinas redondeadas?
///
/// La necesita el borrado tanto como el pintado: si el modelo de la escena
/// creyera que la ventana es cuadrada, al cerrarla quedarian cuatro pellizcos
/// del color de la ventana en las esquinas. Un redondeo que solo sabe pintar
/// deja basura al desaparecer.
pub(crate) fn inside_rounded(x: u32, y: u32, rx: u32, ry: u32, w: u32, h: u32) -> bool {
    if x < rx || x >= rx + w || y < ry || y >= ry + h {
        return false;
    }
    let dy = y - ry;
    let from_edge = if dy < RADIUS {
        Some(dy)
    } else if dy >= h - RADIUS {
        Some(h - 1 - dy)
    } else {
        None
    };
    match from_edge {
        None => true,
        Some(i) => {
            let s = CURVE_TABLE[i as usize];
            x >= rx + s && x < rx + w - s
        }
    }
}

/// La sangria de la fila `i` de una esquina. Para quien redondee a mano una
/// barra de titulo: tiene que usar LA MISMA curva que su ventana o asomara.
pub(crate) fn curve(i: u32) -> u32 {
    CURVE_TABLE[(i as usize).min(CURVE_TABLE.len() - 1)]
}

/// Un rectangulo con las esquinas comidas. Diecisiete `rect` y ya.
pub(crate) fn rounded_rect(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, color: u32) {
    if w <= 2 * RADIUS || h <= 2 * RADIUS {
        p.rect(x, y, w, h, color);
        return;
    }
    for i in 0..RADIUS {
        let s = CURVE_TABLE[i as usize];
        p.rect(x + s, y + i, w - 2 * s, 1, color);
        p.rect(x + s, y + h - 1 - i, w - 2 * s, 1, color);
    }
    p.rect(x, y + RADIUS, w, h - 2 * RADIUS, color);
}

/// La sombra de una ventana: **dos capas**, no una.
///
/// Sin canal alfa no hay difuminado, pero dos anillos de oscuridad distinta
/// enganan bastante bien al ojo -- que es lo unico que se le pide a una sombra.
/// Una sola capa se ve como lo que es: un rectangulo negro detras.
/// ** Cuanto SOBRESALE la sombra de su ventana, por la derecha y por abajo.
///
/// No son numeros decorativos: **son los que tiene que borrar quien quite la
/// ventana**. Y ahi estuvo el fallo que se vio en el Ryzen el 2026-08-04 --
/// cerrar una ventana dejaba una huella en forma de L, porque la sombra se
/// pintaba 8 px a la derecha y 10 abajo y el borrado solo cubria el rectangulo
/// de la ventana. Ocho por diez pixeles de un tono mas oscuro, que en una foto
/// parecen una raya y en la pantalla parecen suciedad.
///
/// Por eso viven aqui y no dentro de `shadow`: quien dibuja y quien borra
/// **leen la misma constante**. Dos numeros que tienen que cuadrar y viven en
/// dos sitios son dos numeros que un dia no cuadran.
pub(crate) const SHADOW_RIGHT: u32 = 8;
pub(crate) const SHADOW_BOTTOM: u32 = 10;

pub(crate) fn shadow(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32) {
    const FAR: u32 = 0x000A_0D14;
    const NEAR: u32 = 0x0006_0810;
    // Las medidas salen de las constantes de arriba: el borde derecho cae en
    // `x + w + SHADOW_RIGHT` y el de abajo en `y + h + SHADOW_BOTTOM`.
    rounded_rect(p, x + 2, y + 4, w + SHADOW_RIGHT - 2, h + SHADOW_BOTTOM - 4, FAR);
    rounded_rect(p, x + 3, y + 5, w + SHADOW_RIGHT - 5, h + SHADOW_BOTTOM - 7, NEAR);
}

/// El color del escritorio en la fila `y`. El degradado, dicho una sola vez.
///
/// Vive aqui y no en quien pinta porque lo consultan DOS: el que dibuja el
/// fondo y el que lo restaura al cerrar una ventana. Dos copias de un degradado
/// es una franja que no cuadra justo donde estaba la ventana.
pub(crate) fn background_at(y: u32, height: u32) -> u32 {
    if height == 0 {
        return BG;
    }
    // La interpolacion va en los DOS sentidos. Con `saturating_sub` a secas,
    // un canal que baja de arriba a abajo daria cero y el degradado se comeria
    // el color: aqui baja siempre, asi que ese error habria dejado la pantalla
    // de un solo tono y nadie habria sabido por que.
    let mezcla = |a: u32, b: u32, desp: u32| -> u32 {
        let (ca, cb) = ((a >> desp) & 0xFF, (b >> desp) & 0xFF);
        let t = y.min(height);
        let c = if cb >= ca {
            ca + (cb - ca) * t / height
        } else {
            ca - (ca - cb) * t / height
        };
        c << desp
    };
    mezcla(BG_TOP, BG_BOTTOM, 16)
        | mezcla(BG_TOP, BG_BOTTOM, 8)
        | mezcla(BG_TOP, BG_BOTTOM, 0)
}

// -- Las FICHAS de la barra ----------------------------------------------
//
// ** Sin esto, minimizar es un boton de "desaparece para siempre".
//
// Una ventana minimizada sigue abierta --conserva su sitio, su tamano y lo que
// estuvieras mirando-- pero no se ve. Si no hay donde encontrarla, ese estado no
// se distingue de haberla cerrado, y el boton miente sobre lo que hace.
//
// La barra de arriba lleva una ficha por ventana abierta: la activa realzada,
// la minimizada apagada. Un clic la trae. Es lo que hacen Windows 11 y GNOME, y
// es lo que convierte tres ventanas sueltas en un escritorio.

pub(crate) const CHIP_H: u32 = 24;
pub(crate) const CHIP_W: u32 = 128;
/// Donde empiezan, dejando sitio al logotipo de la izquierda.
pub(crate) const FICHA_X: u32 = 120;

/// El rectangulo de la ficha numero `i`.
pub(crate) fn chip_box(i: u32) -> (u32, u32, u32, u32) {
    (FICHA_X + i * (CHIP_W + 8), (TASKBAR_H - CHIP_H) / 2, CHIP_W, CHIP_H)
}

/// Sobre que ficha esta el puntero, si sobre alguna. `count` es cuantas hay.
pub(crate) fn chip_at(px: u32, py: u32, count: u32) -> Option<u32> {
    for i in 0..count {
        let (x, y, w, h) = chip_box(i);
        if px >= x && px < x + w && py >= y && py < y + h {
            return Some(i);
        }
    }
    None
}

/// Pinta una ficha. `color` es el de su ventana -- el mismo idioma que el punto
/// de su barra de titulo, para que se sepa cual es sin leerla.
pub(crate) fn paint_chip(
    p: &bmo::Pantalla,
    i: u32,
    name: &str,
    color: u32,
    active: bool,
    minimized: bool,
) {
    let (x, y, w, h) = chip_box(i);
    // Tres estados y tres aspectos. Dos que se vieran igual serian dos que no
    // se pueden distinguir de un vistazo, que es para lo que esta la barra.
    let fondo = if active { 0x001F_2838 } else { TASKBAR };
    p.rect(x, y, w, h, fondo);
    if active {
        // La activa lleva su subrayado, como las pestanas de Datos. Mismo
        // idioma en toda la pantalla.
        p.rect(x, y + h - 2, w, 2, color);
    }
    // El punto de color se apaga si esta minimizada: es la senal de "esta ahi
    // pero no se ve", y se lee sin texto.
    let punto = if minimized { INK_DIM } else { color };
    p.rect(x + 8, y + (h - 8) / 2, 8, 8, punto);
    let ink = if minimized { INK_DIM } else { INK };
    let fits = ((w - 26) / bmo::GLIFO_ANCHO) as usize;
    let n = name.len().min(fits);
    p.texto(x + 22, y + (h - bmo::GLIFO_ALTO) / 2, &name[..n], ink);
}

/// Pinta el escritorio entero: degradado y barra.
pub(crate) fn paint_background(p: &bmo::Pantalla) {
    // ** PRIMERO se limpia el lienzo ENTERO, y esto no es de mas.
    //
    // El lienzo del doble bufer son ~8 MiB de `KIND_MEMORIA` **sin
    // inicializar**: lo que no se pinte encima es basura de RAM, y el volcado
    // la lleva a la pantalla tal cual. `clear` recorre `stride x height`
    // pixeles de forma LINEAL --el relleno de cada fila incluido--; un `rect` de
    // ancho completo solo cubre `0..width` y deja fuera lo que haya entre
    // `width` y `stride`.
    //
    // Aqui vivia el fallo que salio en las fotos del 2026-08-04: se cambio el
    // `p.clear(BG)` de siempre por las bandas del degradado, y con el se
    // perdio la unica garantia de que el lienzo estuviera entero escrito. El
    // resultado eran bloques palidos y barras verticales alrededor de las
    // ventanas -- memoria de otro, dibujada.
    //
    // La leccion, que es la de siempre: **una optimizacion que sustituye a algo
    // hereda sus responsabilidades**, no solo su resultado visible.
    p.limpiar(BG_BOTTOM);

    // De ocho en ocho filas. A un pixel serian mil `rect` para una diferencia
    // que no se ve; a treinta y dos se notarian los escalones.
    let mut y = 0;
    while y < p.alto {
        let height = 8.min(p.alto - y);
        p.rect(0, y, p.ancho, height, background_at(y, p.alto));
        y += height;
    }
    p.rect(0, 0, p.ancho, TASKBAR_H, TASKBAR);
    p.rect(0, TASKBAR_H - 1, p.ancho, 1, TASKBAR_LINE);
}

// -- La caja -------------------------------------------------------------

/// El tamano de la terminal: **por defecto Y minimo a la vez**.
///
/// === Por que el minimo es el tamano de siempre ===
///
/// Debajo de esto la rejilla de 88x16 no cabe, y una ventana que se puede
/// encoger hasta dejar su propio contenido fuera es una trampa, no una
/// libertad -- la misma frase que ya justifica `min_w`/`min_h` en el marco
/// compartido. Con el minimo puesto aqui, estirar solo puede AGRANDAR, y una
/// rejilla que sobra sitio es un caso que no rompe nada.
pub(crate) const BOX_W: u32 = 760;
pub(crate) const BOX_H: u32 = 428;

/// La fraccion de pantalla que pide al abrirse. En 1920x1080 da 768x432, o sea
/// practicamente el tamano de siempre; en una pantalla mayor se aprovecha, que
/// es justo lo que un tamano en pixeles no sabe hacer.
const RUN_PCT_W: u32 = 40;
const RUN_PCT_H: u32 = 40;

/// Alto de la barra de titulo de la caja. Antes era un `26` suelto repetido en
/// cuatro sitios; ahora se llama por su nombre, que es lo que impide que el
/// modelo de la escena y el que pinta se separen dos pixeles y nadie sepa por
/// que queda una raya.
pub(crate) const TITLE_H: u32 = 28;

/// La rejilla de SALIDA: lo que imprimen los programas que se lanzan desde
/// aqui. Antes no existia y no era un olvido -- **no habia donde leerlo**:
/// `OP_CONSOLE_WRITE` iba siempre al panel del kernel, asi que un terminal de
/// Ring 3 no podia ver lo que escribia su propio hijo. Con `KIND_CONSOLE` la
/// salida tiene dueno, y el dueno es este proceso.
pub(crate) const OUT_COLS: usize = 88;
/// **El TOPE de filas visibles**, no las que se ven siempre.
///
/// Desde que la terminal se estira, las que se ven de verdad las cuenta
/// [`RunBox::out_rows`] a partir del alto. Este numero es el techo, y subio de
/// 16 a 32 el 2026-08-16 para que MAXIMIZAR sirva de algo: con el tope en 16,
/// una ventana del alto de la pantalla ensenaba exactamente el mismo texto que
/// una pequena y dejaba el resto en negro -- o sea que el boton estaba pero no
/// pagaba. El historial guardado sigue siendo [`OUT_HIST`].
pub(crate) const OUT_ROWS: usize = 32;
/// Cuantas filas se GUARDAN, aunque solo se vean [`OUT_ROWS`].
///
/// * Antes lo que salia por arriba se perdia para siempre: `scroll` movia
/// las filas y la de arriba se tiraba. Un `ls` largo o la salida de un batch se
/// iban sin que hubiera forma de volver a mirarlas -- y eso en una maquina donde
/// depurar es hacer una foto de la pantalla duele el doble.
///
/// 200 filas de 88 columnas son 17 KiB. La pantalla es de 8 MiB.
pub(crate) const OUT_HIST: usize = 200;
pub(crate) const OUT_TEXT: u32 = 0x00C5_CEDC;
pub(crate) const OUT_ECHO: u32 = 0x0060_A5FA;
/// El cuerpo de una ventana. Mas claro que el escritorio: es lo que la pone
/// delante sin necesidad de dibujarle un marco grueso.
pub(crate) const BOX_BG: u32 = 0x001E_2534;
/// El borde. **Discreto a proposito**: era el mismo azul del acento, o sea un
/// marco de neon alrededor de todo. Un borde grita cuando deberia susurrar --
/// lo que separa la ventana del fondo es la sombra y el salto de tono, no una
/// raya de color.
pub(crate) const BOX_EDGE: u32 = 0x0033_3D52;
/// La barra de titulo: un peldano MAS claro que el cuerpo.
pub(crate) const BOX_TITLE: u32 = 0x0027_3040;
/// Los campos donde se escribe van hacia abajo, no hacia arriba: un hueco se
/// lee como hundido y ahi es donde se mete texto.
pub(crate) const FIELD_BG: u32 = 0x0016_1C28;
pub(crate) const INK: u32 = 0x00E6_EDF6;
pub(crate) const INK_DIM: u32 = 0x008A_9BB4;
pub(crate) const INK_BAD: u32 = 0x00FF_8A7A;
pub(crate) const INK_OK: u32 = 0x007E_E787;

/// Cuantos bytes de ruta caben. Es el mismo tope que el renglon del kernel
/// (`PATH_MAX`), y no por casualidad: escribir mas de lo que el otro lado puede
/// aceptar seria dejar que la ruta se corte en silencio a mitad de camino.
pub(crate) const PATH_MAX: usize = 128;

/// Geometria de la caja, ya resuelta contra el tamano real del panel.
///
/// === ** LA TERMINAL ES UNA VENTANA DE VERDAD (2026-08-16) ===
///
/// Hasta hoy era **lo unico del escritorio clavado a la pantalla**: CABINA,
/// datos, sonido, las constantes y las superficies de las apps llevaban
/// `Chrome` --arrastre, estirar, maximizar, botones-- y la caja donde de verdad
/// se trabaja no. Su barra de titulo era decorativa: se pintaba y no se podia
/// agarrar.
///
/// Eddi: *"me gustaria que sea movible... para mejorar MAS la HUD"*. Tenia
/// razon, y la cabecera de `chrome.rs` ya habia escrito el criterio: **existe
/// para que la cuarta ventana salga gratis**. Esta es la quinta y sale por el
/// mismo sitio; no hay un gestor de ventanas nuevo aqui, hay un `Chrome` mas.
///
/// Los campos de posicion siguen existiendo como ESPEJO del marco --se
/// recalculan en [`RunBox::relayout`]-- y no como la verdad. Asi los ciento y
/// pico sitios que ya leian `c.field_x` siguen leyendo lo mismo, y solo hay un
/// lugar donde la geometria se decide.
pub(crate) struct RunBox {
    /// El marco compartido: donde esta, cuanto mide, y quien la arrastra.
    pub(crate) chrome: chrome::Chrome,
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) field_x: u32,
    pub(crate) field_y: u32,
    pub(crate) field_w: u32,
    pub(crate) field_h: u32,
    pub(crate) texto_x: u32,
    pub(crate) texto_y: u32,
    pub(crate) status_y: u32,
    pub(crate) out_x: u32,
    pub(crate) out_y: u32,
}

impl RunBox {
    pub(crate) fn new(p: &bmo::Pantalla) -> Self {
        let mut c = Self {
            chrome: chrome::Chrome::new(p, RUN_PCT_W, RUN_PCT_H, BOX_W, BOX_H).sin_cerrar(),
            x: 0,
            y: 0,
            field_x: 0,
            field_y: 0,
            field_w: 0,
            field_h: 0,
            texto_x: 0,
            texto_y: 0,
            status_y: 0,
            out_x: 0,
            out_y: 0,
        };
        c.relayout();
        c
    }

    /// **Todo lo que se deduce de donde esta el marco.** Se llama despues de
    /// CADA cambio de geometria -- mover, estirar, maximizar, restaurar.
    ///
    /// Existe para que la respuesta a *"donde esta el campo de texto"* tenga un
    /// solo autor. La leccion esta escrita dos veces en este mismo fichero: la
    /// primera version del `TITLE_H` era un `26` suelto repetido en cuatro
    /// sitios, y `on_field` nacio porque el puntero y el pintor hacian la misma
    /// cuenta por su cuenta. Con la ventana quieta eso solo costaba rayas; con
    /// la ventana en movimiento, dos copias de la geometria son dos ventanas.
    pub(crate) fn relayout(&mut self) {
        self.x = self.chrome.x;
        self.y = self.chrome.y;
        self.field_x = self.x + 18;
        self.field_y = self.y + 54;
        self.field_w = self.chrome.width.saturating_sub(36);
        self.field_h = 28;
        self.texto_x = self.field_x + 6;
        self.texto_y = self.field_y + 6;
        // El estado va JUSTO debajo del campo, no al fondo de la caja: el
        // fondo es ahora la salida, y un mensaje de error a veinte lineas
        // de distancia de la linea que lo causo no lo lee nadie.
        self.status_y = self.field_y + self.field_h + 10;
        self.out_x = self.x + 18;
        self.out_y = self.field_y + self.field_h + 40;
    }

    /// Lo que mide la ventana AHORA. No es `BOX_W`: eso es el minimo.
    pub(crate) fn w(&self) -> u32 {
        self.chrome.width
    }

    pub(crate) fn h(&self) -> u32 {
        self.chrome.height
    }

    /// **Cuantas filas de salida caben de verdad**, sabiendo el alto.
    ///
    /// Se calcula y no se fija, por el mismo motivo que `cabina::visible_rows`:
    /// una cuenta fija en una ventana que se estira deja filas pintadas FUERA
    /// del marco --encima del escritorio, sin nada que las borre-- o un hueco
    /// muerto dentro. El tope sigue siendo [`OUT_ROWS`] porque por encima de eso
    /// no hay mas historial que ensenar de golpe.
    ///
    /// El `24` del final es el pie donde viven los atajos: sin reservarlo, la
    /// ultima fila de texto se comeria esa linea al agrandar.
    pub(crate) fn out_rows(&self) -> usize {
        let fondo = self.y + self.h().saturating_sub(24);
        let alto = fondo.saturating_sub(self.out_y);
        ((alto / bmo::GLIFO_ALTO) as usize).min(OUT_ROWS)
    }

    /// Alto de la rejilla de salida, en pixeles.
    pub(crate) fn out_h(&self) -> u32 {
        self.out_rows() as u32 * bmo::GLIFO_ALTO
    }

    /// Cuantos caracteres caben en el campo. El resto se recorta al pintar --
    /// nunca al guardar: lo que no se ve sigue estando en la ruta.
    pub(crate) fn visibles(&self) -> usize {
        ((self.field_w - 12) / bmo::GLIFO_ANCHO) as usize
    }

    pub(crate) fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + self.w() && y >= self.y && y < self.y + self.h()
    }

    /// Este pixel cae DENTRO del campo donde se escribe?
    ///
    /// Lo usa el puntero para cambiar a la barra de texto. Es la misma cuenta
    /// que ya hacia `scene_color` para saber que color devolver, dicha una vez
    /// y con nombre: dos copias de la misma geometria se separan en cuanto
    /// alguien mueve el campo dos pixeles.
    pub(crate) fn on_field(&self, x: u32, y: u32) -> bool {
        x >= self.field_x
            && x < self.field_x + self.field_w
            && y >= self.field_y
            && y < self.field_y + self.field_h
    }
}

/// Que color le toca a un pixel segun la escena. Es el modelo entero del
/// escritorio, y es lo que permite borrar el cursor sin repintarlo todo: para
/// restaurar una zona basta con volver a preguntar que habia ahi.
///
/// Sabe de rectangulos, no de letras. Por eso `borrar_cursor` avisa cuando ha
/// pasado por encima de la caja: el texto hay que volver a escribirlo.
pub(crate) fn scene_color(c: &RunBox, visible: bool, x: u32, y: u32, height: u32) -> u32 {
    if y < TASKBAR_H {
        if y == TASKBAR_H - 1 {
            return TASKBAR_LINE;
        }
        // La marca de referencia dentro de la barra.
        if x >= 16 && x < 30 && y >= 13 && y < 27 {
            return ACCENT;
        }
        return TASKBAR;
    }
    // * Se pregunta por el rectangulo REDONDEADO y no por `contains`. Si el
    // modelo creyera que la caja es cuadrada, al taparla y destaparla quedarian
    // cuatro pellizcos de su color en las esquinas -- un redondeo que solo sabe
    // pintar deja basura al desaparecer.
    if visible && inside_rounded(x, y, c.x, c.y, c.w(), c.h()) {
        let on_edge = !inside_rounded(x, y, c.x + 1, c.y + 1, c.w() - 2, c.h() - 2);
        if on_edge {
            return BOX_EDGE;
        }
        // El acento va en `TITLE_H - 1`, que es donde lo pone `paint_chrome`.
        // Si este modelo dijera otra fila, destapar la caja dejaria la raya
        // corrida un pixel -- y esa clase de diferencia es justo por lo que la
        // barra de titulo dejo de pintarse aqui a mano.
        if y == c.y + TITLE_H - 1 {
            return ACCENT;
        }
        if y < c.y + TITLE_H {
            return BOX_TITLE;
        }
        if x >= c.field_x
            && x < c.field_x + c.field_w
            && y >= c.field_y
            && y < c.field_y + c.field_h
        {
            return FIELD_BG;
        }
        return BOX_BG;
    }
    background_at(y, height)
}


// -- Pintar la caja ------------------------------------------------------

/// El marco entero. Se pinta UNA vez; despues solo se repinta el campo.
/// La caja, con algo de forma.
///
/// Era un rectangulo con un borde de 2 px y el titulo escrito encima del mismo
/// fondo: plano, y con las cuatro esquinas en pico. Cinco cosas lo arreglan sin
/// salir de `rect` y `text`, que es todo lo que tiene esta pantalla:
///
/// 1. **Sombra** desplazada -- un rectangulo oscuro detras. Es lo que despega la
///    caja del fondo y lo que mas se nota en una foto.
/// 2. **Barra de titulo** con su propio fondo, en vez de texto suelto.
/// 3. **Linea de acento** bajo la barra: separa sin dibujar un borde entero.
/// 4. **Esquinas biseladas** -- se repinta el color de fondo en el pixel de cada
///    esquina. Cuatro rectangulos de 1x1 y deja de parecer un cuadro de dialogo
///    de hace treinta anos.
/// 5. El campo de entrada con **marco propio** y un `>` de aviso, para que se
///    vea que ahi se escribe.
#[inline(never)]
pub(crate) fn paint_run_box(p: &bmo::Pantalla, c: &RunBox) {
    // 1, 2 y 3. Sombra, marco redondeado, barra de titulo, acento, **los
    // botones y el asa de la esquina** -- todo del MARCO COMPARTIDO.
    //
    // ** Esto eran diecisiete lineas calcadas de `chrome.rs` con una diferencia
    // de un pixel en el acento, que es exactamente la forma en que dos ventanas
    // del mismo escritorio acaban comportandose distinto sin que nadie lo
    // decida. Ahora la terminal se pinta como CABINA, como datos y como sonido,
    // y lo que se arregle en el marco le llega sola.
    c.chrome.paint_chrome(p, BOX_EDGE, BOX_BG, BOX_TITLE, ACCENT);

    // El punto de la izquierda: el mismo lenguaje que la marca de la barra de
    // arriba. Dos sitios, un solo idioma.
    p.rect(c.x + 16, c.y + 10, 8, 8, ACCENT);
    p.texto(c.x + 32, c.y + 7, "Ejecutar", INK);
    p.texto(c.x + 32 + 10 * bmo::GLIFO_ANCHO, c.y + 7, "BMO-X", INK_DIM);

    p.texto(
        c.x + 18,
        c.y + 36,
        // * `estratos` FALTABA AQUI, y por eso nadie sabia sellar.
        //
        // La orden existe, se parsea, escribe en el disco y **no estaba escrita
        // en ninguna parte que se vea**: solo salia en el volcado de `ayuda`, al
        // que se llega tecleando algo mal. El dueno la pidio por su nombre el
        // 2026-08-13 -- *"no se como lo sello"*-- teniendola delante.
        //
        // Es el patron 33 otra vez, girado: un mensaje que existe y sale por un
        // canal que nadie mira. Una funcion que no se anuncia no es una funcion
        // discreta: es una funcion que no esta.
        "ruta de un .bex y Enter.  info / cpu / mem / perf / ls / lee / guarda / smp / sella / reboot.",
        INK_DIM,
    );
    // * Las dos ventanas del sistema, DICHAS. Un atajo que no esta escrito en
    // ninguna parte es un atajo que solo conoce quien lo programo -- y F11 existe
    // precisamente para los dias en que esta caja no responde.
    //
    // Va al PIE de la caja, no debajo de la pista: ahi lo puse primero y
    // `field_y` es exactamente `y + 54`, asi que el marco del campo lo pintaba
    // encima y la linea no se veia. Se cazo en la foto -- el texto se emitia y
    // desaparecia en la instruccion siguiente.
    p.texto(
        c.x + 18,
        c.y + c.h() - 22,
        "F11 kernel  F12 datos  ESC cierra   |   arrastra la barra  Alt+flechas mueve  Ctrl+Alt esconde",
        INK_DIM,
    );

    // 5. El campo. **El acento va SOLO en la linea de abajo**, no rodeandolo.
    //
    // Un marco entero del color del sistema alrededor de la caja de texto es lo
    // que hacia que pareciera un cuadro de dialogo de hace treinta anos: el
    // acento pasa a ser un marco y deja de senalar. Una raya bajo el campo dice
    // "aqui se escribe" con un cuarto de la tinta -- es lo que hacen Windows 11
    // y todos los escritorios de Linux modernos, y por este motivo.
    p.rect(c.field_x - 1, c.field_y - 1, c.field_w + 2, c.field_h + 2, BOX_EDGE);
    p.rect(c.field_x, c.field_y, c.field_w, c.field_h, FIELD_BG);
    p.rect(c.field_x, c.field_y + c.field_h, c.field_w, 2, ACCENT);
}

/// El contenido del campo: la ruta y el cursor de escritura.
///
/// Repinta el fondo del campo entero antes de escribir. Es un rectangulo de
/// unos 500x28 px --nada-- y evita el clasico de borrar un caracter y que quede
/// medio glifo del anterior porque el nuevo es mas estrecho.
#[inline(never)]
pub(crate) fn paint_field(p: &bmo::Pantalla, c: &RunBox, path: &[u8], cur: usize, caret: bool) {
    p.rect(c.field_x, c.field_y, c.field_w, c.field_h, FIELD_BG);

    // La ventana visible se calcula alrededor del CURSOR, no del final.
    //
    // Antes se ensenaba siempre la cola, que valia mientras solo se podia
    // escribir al final. Con el cursor moviendose, eso deja de valer: si te
    // vas al principio de una ruta larga, el cursor se sale por la izquierda y
    // editas a ciegas. La regla es sencilla y es la de cualquier editor --
    // **el cursor SIEMPRE se ve**, y la ventana se desplaza lo minimo para
    // que asi sea.
    let fits = c.visibles();
    let from = if path.len() <= fits {
        0
    } else if cur >= fits {
        // El cursor se salio por la derecha: pegarlo al borde derecho.
        (cur + 1).saturating_sub(fits).min(path.len() - fits)
    } else {
        0
    };
    let to = (from + fits).min(path.len());
    p.texto_bytes(c.texto_x, c.texto_y, &path[from..to], INK);

    if caret {
        let col = cur.saturating_sub(from) as u32;
        p.rect(
            c.texto_x + col * bmo::GLIFO_ANCHO,
            c.texto_y,
            2,
            bmo::GLIFO_ALTO,
            ACCENT,
        );
    }
}


/// Borra la caja devolviendo cada pixel a lo que la escena dice que hay
/// debajo. Es el precio de que la ventana se pueda invocar y esconder.
///
/// Recorre el rectangulo entero -- unos 325k pixeles sobre memoria de video sin
/// cache, que no es gratis. Pero pasa UNA vez por pulsacion de atajo, no por
/// fotograma, y la alternativa (guardar lo que habia debajo) seria un buffer de
/// 1,3 MB en un proceso con 64 KiB de pila.
pub(crate) fn erase_box(p: &bmo::Pantalla, c: &RunBox) {
    // Su sombra tambien, por el mismo motivo que en `erase_window`:
    // esconder con Ctrl+Alt dejaba la misma huella en L.
    for row in 0..c.h() + SHADOW_BOTTOM {
        for col in 0..c.w() + SHADOW_RIGHT {
            let (x, y) = (c.x + col, c.y + row);
            p.punto(x, y, scene_color(c, false, x, y, p.alto));
        }
    }
}

/// Borra la consola de datos devolviendo cada pixel a lo que hay debajo.
///
/// * `visible` es si la caja de Ejecutar esta abierta, y hace falta: la consola
/// de datos se pinta ENCIMA de ella. `scene_color` sabe devolver el color de
/// la caja cuando el pixel cae dentro, asi que pasarle `false` aqui dejaria un
/// agujero con el fondo del escritorio en medio de la ventana de abajo.
///
/// Quien llama repinta despues el texto de la caja: esto devuelve el fondo, no
/// las letras.
/// * Toma un RECTANGULO y no una ventana concreta.
///
/// Era `borrar_datos(&data::DataWindow)`, atado al tipo de una ventana -- y con
/// eso, anadir la segunda ventana obligaba a copiar la funcion. Lo que esto
/// hace no depende de que ventana se cierra: devuelve el fondo de un area.
pub(crate) fn erase_window(
    p: &bmo::Pantalla,
    c: &RunBox,
    x0: u32,
    y0: u32,
    width: u32,
    height: u32,
    visible: bool,
) {
    // * Se borra la ventana **Y SU SOMBRA**. Sin esto, cerrar deja una huella
    // en forma de L abajo a la derecha: los pixeles que la sombra pinto fuera
    // del rectangulo no los cubre nadie. Se vio en el Ryzen y es el motivo de
    // que `SHADOW_RIGHT`/`SHADOW_BOTTOM` sean constantes compartidas.
    //
    // `punto` recorta solo contra el panel, asi que pasarse por la derecha o
    // por abajo no hay que comprobarlo aqui.
    let height = height + SHADOW_BOTTOM;
    let width = width + SHADOW_RIGHT;
    for row in 0..height {
        for col in 0..width {
            let (x, y) = (x0 + col, y0 + row);
            p.punto(x, y, scene_color(c, visible, x, y, p.alto));
        }
    }
}

#[inline(never)]
pub(crate) fn paint_status(p: &bmo::Pantalla, c: &RunBox, msg: &str, color: u32) {
    // Ancho fijo de limpieza: el mensaje anterior puede ser mas largo que el
    // nuevo, y media frase vieja detras de una nueva es peor que ninguna.
    p.rect(c.x + 18, c.status_y, c.w() - 36, bmo::GLIFO_ALTO, BOX_BG);
    p.texto(c.x + 18, c.status_y, msg, color);
}

