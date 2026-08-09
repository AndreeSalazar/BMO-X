//! **Lo que se PINTA.**
//!
//! Colores, geometria y las primitivas de dibujo de la ventana. Aqui no se
//! interpreta nada: un modulo de esta carpeta no sabe que es un comando.

use bmo_userland as bmo;

/// **La ventana de CABINA** (F11): lo que el kernel ve, con su gravedad y en
/// su color. Sustituye a la del klog, que era texto plano sin severidad.
pub(crate) mod cabina;
pub(crate) mod calc;
pub(crate) mod conmutador;
pub(crate) mod datos;
pub(crate) mod cursor;
/// La ENTRADA a Ring 3: lo que se ve cuando el userspace toma la maquina.
pub(crate) mod entrada;
/// El LOGO, en dos mascaras de 1 bit. Generado por `docs/arte/gato_a_mascara.py`.
pub(crate) mod gato;
/// El MARCO compartido: geometria, arrastre, estirar, maximizar y los tres
/// botones. Lo que toda ventana tiene y ninguna deberia escribir dos veces.
pub(crate) mod marco;
pub(crate) mod salida;
/// **La ventana del SONIDO** (F10). Reclama `KIND_AUDIO` al abrirse y lo
/// DEVUELVE al cerrarse -- ver la cabecera del modulo: es lo unico que impide
/// que el escritorio deje mudos a todos los programas que lanza.
pub(crate) mod sonido;


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
pub(crate) const FONDO_ARRIBA: u32 = 0x001B_2233;
pub(crate) const FONDO_ABAJO: u32 = 0x000C_0F17;
/// El color de referencia cuando hace falta uno solo (bordes de mezcla).
pub(crate) const FONDO: u32 = 0x0014_1A28;
/// La barra de arriba. Mas oscura que el escritorio a proposito: una barra de
/// sistema se lee como un borde de la pantalla, no como una ventana.
pub(crate) const BARRA: u32 = 0x000F_131D;
/// El pelo de luz bajo la barra. Un borde entero seria una raya; esto separa.
pub(crate) const BARRA_LINEA: u32 = 0x0026_2F42;
pub(crate) const ACENTO: u32 = 0x0060_A5FA;

pub(crate) const BARRA_ALTO: u32 = 40;

// -- Esquinas redondeadas ------------------------------------------------
//
// No hay primitiva de circulo ni la va a haber. Un cuarto de circulo son ocho
// sangrias, y una tabla de ocho numeros es mas honesta que una raiz cuadrada en
// coma flotante que este sistema no tiene.

pub(crate) const RADIO: u32 = 8;
/// Cuanto se mete cada fila del extremo. Es un cuarto de circulo de radio 8
/// tabulado: `CURVA[0]` es la fila del borde y `CURVA[7]` ya no se mete.
const CURVA: [u32; RADIO as usize] = [8, 5, 3, 2, 1, 1, 0, 0];

/// Esta `(x, y)` DENTRO de un rectangulo de esquinas redondeadas?
///
/// La necesita el borrado tanto como el pintado: si el modelo de la escena
/// creyera que la ventana es cuadrada, al cerrarla quedarian cuatro pellizcos
/// del color de la ventana en las esquinas. Un redondeo que solo sabe pintar
/// deja basura al desaparecer.
pub(crate) fn dentro_redondeado(x: u32, y: u32, rx: u32, ry: u32, w: u32, h: u32) -> bool {
    if x < rx || x >= rx + w || y < ry || y >= ry + h {
        return false;
    }
    let dy = y - ry;
    let desde_borde = if dy < RADIO {
        Some(dy)
    } else if dy >= h - RADIO {
        Some(h - 1 - dy)
    } else {
        None
    };
    match desde_borde {
        None => true,
        Some(i) => {
            let s = CURVA[i as usize];
            x >= rx + s && x < rx + w - s
        }
    }
}

/// La sangria de la fila `i` de una esquina. Para quien redondee a mano una
/// barra de titulo: tiene que usar LA MISMA curva que su ventana o asomara.
pub(crate) fn curva(i: u32) -> u32 {
    CURVA[(i as usize).min(CURVA.len() - 1)]
}

/// Un rectangulo con las esquinas comidas. Diecisiete `rect` y ya.
pub(crate) fn rect_redondeado(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, color: u32) {
    if w <= 2 * RADIO || h <= 2 * RADIO {
        p.rect(x, y, w, h, color);
        return;
    }
    for i in 0..RADIO {
        let s = CURVA[i as usize];
        p.rect(x + s, y + i, w - 2 * s, 1, color);
        p.rect(x + s, y + h - 1 - i, w - 2 * s, 1, color);
    }
    p.rect(x, y + RADIO, w, h - 2 * RADIO, color);
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
/// Por eso viven aqui y no dentro de `sombra`: quien dibuja y quien borra
/// **leen la misma constante**. Dos numeros que tienen que cuadrar y viven en
/// dos sitios son dos numeros que un dia no cuadran.
pub(crate) const SOMBRA_DER: u32 = 8;
pub(crate) const SOMBRA_ABAJO: u32 = 10;

pub(crate) fn sombra(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32) {
    const LEJOS: u32 = 0x000A_0D14;
    const CERCA: u32 = 0x0006_0810;
    // Las medidas salen de las constantes de arriba: el borde derecho cae en
    // `x + w + SOMBRA_DER` y el de abajo en `y + h + SOMBRA_ABAJO`.
    rect_redondeado(p, x + 2, y + 4, w + SOMBRA_DER - 2, h + SOMBRA_ABAJO - 4, LEJOS);
    rect_redondeado(p, x + 3, y + 5, w + SOMBRA_DER - 5, h + SOMBRA_ABAJO - 7, CERCA);
}

/// El color del escritorio en la fila `y`. El degradado, dicho una sola vez.
///
/// Vive aqui y no en quien pinta porque lo consultan DOS: el que dibuja el
/// fondo y el que lo restaura al cerrar una ventana. Dos copias de un degradado
/// es una franja que no cuadra justo donde estaba la ventana.
pub(crate) fn fondo_en(y: u32, alto: u32) -> u32 {
    if alto == 0 {
        return FONDO;
    }
    // La interpolacion va en los DOS sentidos. Con `saturating_sub` a secas,
    // un canal que baja de arriba a abajo daria cero y el degradado se comeria
    // el color: aqui baja siempre, asi que ese error habria dejado la pantalla
    // de un solo tono y nadie habria sabido por que.
    let mezcla = |a: u32, b: u32, desp: u32| -> u32 {
        let (ca, cb) = ((a >> desp) & 0xFF, (b >> desp) & 0xFF);
        let t = y.min(alto);
        let c = if cb >= ca {
            ca + (cb - ca) * t / alto
        } else {
            ca - (ca - cb) * t / alto
        };
        c << desp
    };
    mezcla(FONDO_ARRIBA, FONDO_ABAJO, 16)
        | mezcla(FONDO_ARRIBA, FONDO_ABAJO, 8)
        | mezcla(FONDO_ARRIBA, FONDO_ABAJO, 0)
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

pub(crate) const FICHA_ALTO: u32 = 24;
pub(crate) const FICHA_ANCHO: u32 = 128;
/// Donde empiezan, dejando sitio al logotipo de la izquierda.
pub(crate) const FICHA_X: u32 = 120;

/// El rectangulo de la ficha numero `i`.
pub(crate) fn ficha_caja(i: u32) -> (u32, u32, u32, u32) {
    (FICHA_X + i * (FICHA_ANCHO + 8), (BARRA_ALTO - FICHA_ALTO) / 2, FICHA_ANCHO, FICHA_ALTO)
}

/// Sobre que ficha esta el puntero, si sobre alguna. `cuantas` es cuantas hay.
pub(crate) fn ficha_en(px: u32, py: u32, cuantas: u32) -> Option<u32> {
    for i in 0..cuantas {
        let (x, y, w, h) = ficha_caja(i);
        if px >= x && px < x + w && py >= y && py < y + h {
            return Some(i);
        }
    }
    None
}

/// Pinta una ficha. `color` es el de su ventana -- el mismo idioma que el punto
/// de su barra de titulo, para que se sepa cual es sin leerla.
pub(crate) fn pintar_ficha(
    p: &bmo::Pantalla,
    i: u32,
    name: &str,
    color: u32,
    activa: bool,
    minimizada: bool,
) {
    let (x, y, w, h) = ficha_caja(i);
    // Tres estados y tres aspectos. Dos que se vieran igual serian dos que no
    // se pueden distinguir de un vistazo, que es para lo que esta la barra.
    let fondo = if activa { 0x001F_2838 } else { BARRA };
    p.rect(x, y, w, h, fondo);
    if activa {
        // La activa lleva su subrayado, como las pestanas de Datos. Mismo
        // idioma en toda la pantalla.
        p.rect(x, y + h - 2, w, 2, color);
    }
    // El punto de color se apaga si esta minimizada: es la senal de "esta ahi
    // pero no se ve", y se lee sin texto.
    let punto = if minimizada { TEXTO_TENUE } else { color };
    p.rect(x + 8, y + (h - 8) / 2, 8, 8, punto);
    let tinta = if minimizada { TEXTO_TENUE } else { TEXTO };
    let cabe = ((w - 26) / bmo::GLIFO_ANCHO) as usize;
    let n = name.len().min(cabe);
    p.texto(x + 22, y + (h - bmo::GLIFO_ALTO) / 2, &name[..n], tinta);
}

/// Pinta el escritorio entero: degradado y barra.
pub(crate) fn pintar_fondo(p: &bmo::Pantalla) {
    // ** PRIMERO se limpia el lienzo ENTERO, y esto no es de mas.
    //
    // El lienzo del doble bufer son ~8 MiB de `KIND_MEMORIA` **sin
    // inicializar**: lo que no se pinte encima es basura de RAM, y el volcado
    // la lleva a la pantalla tal cual. `limpiar` recorre `stride x alto`
    // pixeles de forma LINEAL --el relleno de cada fila incluido--; un `rect` de
    // ancho completo solo cubre `0..ancho` y deja fuera lo que haya entre
    // `ancho` y `stride`.
    //
    // Aqui vivia el fallo que salio en las fotos del 2026-08-04: se cambio el
    // `p.limpiar(FONDO)` de siempre por las bandas del degradado, y con el se
    // perdio la unica garantia de que el lienzo estuviera entero escrito. El
    // resultado eran bloques palidos y barras verticales alrededor de las
    // ventanas -- memoria de otro, dibujada.
    //
    // La leccion, que es la de siempre: **una optimizacion que sustituye a algo
    // hereda sus responsabilidades**, no solo su resultado visible.
    p.limpiar(FONDO_ABAJO);

    // De ocho en ocho filas. A un pixel serian mil `rect` para una diferencia
    // que no se ve; a treinta y dos se notarian los escalones.
    let mut y = 0;
    while y < p.alto {
        let alto = 8.min(p.alto - y);
        p.rect(0, y, p.ancho, alto, fondo_en(y, p.alto));
        y += alto;
    }
    p.rect(0, 0, p.ancho, BARRA_ALTO, BARRA);
    p.rect(0, BARRA_ALTO - 1, p.ancho, 1, BARRA_LINEA);
}

// -- La caja -------------------------------------------------------------

pub(crate) const CAJA_ANCHO: u32 = 760;
pub(crate) const CAJA_ALTO: u32 = 428;

/// Alto de la barra de titulo de la caja. Antes era un `26` suelto repetido en
/// cuatro sitios; ahora se llama por su nombre, que es lo que impide que el
/// modelo de la escena y el que pinta se separen dos pixeles y nadie sepa por
/// que queda una raya.
pub(crate) const TITULO_ALTO: u32 = 28;

/// La rejilla de SALIDA: lo que imprimen los programas que se lanzan desde
/// aqui. Antes no existia y no era un olvido -- **no habia donde leerlo**:
/// `OP_CONSOLE_WRITE` iba siempre al panel del kernel, asi que un terminal de
/// Ring 3 no podia ver lo que escribia su propio hijo. Con `KIND_CONSOLE` la
/// salida tiene dueno, y el dueno es este proceso.
pub(crate) const SAL_COLS: usize = 88;
pub(crate) const SAL_ROWS: usize = 16;
/// Cuantas filas se GUARDAN, aunque solo se vean [`SAL_ROWS`].
///
/// * Antes lo que salia por arriba se perdia para siempre: `desplazar` movia
/// las filas y la de arriba se tiraba. Un `ls` largo o la salida de un batch se
/// iban sin que hubiera forma de volver a mirarlas -- y eso en una maquina donde
/// depurar es hacer una foto de la pantalla duele el doble.
///
/// 200 filas de 88 columnas son 17 KiB. La pantalla es de 8 MiB.
pub(crate) const SAL_HIST: usize = 200;
pub(crate) const SAL_TEXTO: u32 = 0x00C5_CEDC;
pub(crate) const SAL_ECO: u32 = 0x0060_A5FA;
/// El cuerpo de una ventana. Mas claro que el escritorio: es lo que la pone
/// delante sin necesidad de dibujarle un marco grueso.
pub(crate) const CAJA_FONDO: u32 = 0x001E_2534;
/// El borde. **Discreto a proposito**: era el mismo azul del acento, o sea un
/// marco de neon alrededor de todo. Un borde grita cuando deberia susurrar --
/// lo que separa la ventana del fondo es la sombra y el salto de tono, no una
/// raya de color.
pub(crate) const CAJA_BORDE: u32 = 0x0033_3D52;
/// La barra de titulo: un peldano MAS claro que el cuerpo.
pub(crate) const CAJA_TITULO: u32 = 0x0027_3040;
/// Los campos donde se escribe van hacia abajo, no hacia arriba: un hueco se
/// lee como hundido y ahi es donde se mete texto.
pub(crate) const CAMPO_FONDO: u32 = 0x0016_1C28;
pub(crate) const TEXTO: u32 = 0x00E6_EDF6;
pub(crate) const TEXTO_TENUE: u32 = 0x008A_9BB4;
pub(crate) const TEXTO_MAL: u32 = 0x00FF_8A7A;
pub(crate) const TEXTO_BIEN: u32 = 0x007E_E787;

/// Cuantos bytes de ruta caben. Es el mismo tope que el renglon del kernel
/// (`RUTA_MAX`), y no por casualidad: escribir mas de lo que el otro lado puede
/// aceptar seria dejar que la ruta se corte en silencio a mitad de camino.
pub(crate) const RUTA_MAX: usize = 128;

/// Geometria de la caja, ya resuelta contra el tamano real del panel.
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
        // donde el ojo la busca. (Antes habia ademas una tira de parches de
        // medida que esquivar; se quito el 2026-08-04 -- ver el escritorio.)
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
            // fondo es ahora la salida, y un mensaje de error a veinte lineas
            // de distancia de la linea que lo causo no lo lee nadie.
            estado_y: campo_y + campo_alto + 10,
            salida_x: x + 18,
            salida_y: campo_y + campo_alto + 40,
        }
    }

    /// Alto de la rejilla de salida, en pixeles.
    pub(crate) fn salida_alto(&self) -> u32 {
        SAL_ROWS as u32 * bmo::GLIFO_ALTO
    }

    /// Cuantos caracteres caben en el campo. El resto se recorta al pintar --
    /// nunca al guardar: lo que no se ve sigue estando en la ruta.
    pub(crate) fn visibles(&self) -> usize {
        ((self.campo_ancho - 12) / bmo::GLIFO_ANCHO) as usize
    }

    pub(crate) fn contiene(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + CAJA_ANCHO && y >= self.y && y < self.y + CAJA_ALTO
    }

    /// Este pixel cae DENTRO del campo donde se escribe?
    ///
    /// Lo usa el puntero para cambiar a la barra de texto. Es la misma cuenta
    /// que ya hacia `color_escena` para saber que color devolver, dicha una vez
    /// y con nombre: dos copias de la misma geometria se separan en cuanto
    /// alguien mueve el campo dos pixeles.
    pub(crate) fn en_campo(&self, x: u32, y: u32) -> bool {
        x >= self.campo_x
            && x < self.campo_x + self.campo_ancho
            && y >= self.campo_y
            && y < self.campo_y + self.campo_alto
    }
}

/// Que color le toca a un pixel segun la escena. Es el modelo entero del
/// escritorio, y es lo que permite borrar el cursor sin repintarlo todo: para
/// restaurar una zona basta con volver a preguntar que habia ahi.
///
/// Sabe de rectangulos, no de letras. Por eso `borrar_cursor` avisa cuando ha
/// pasado por encima de la caja: el texto hay que volver a escribirlo.
pub(crate) fn color_escena(c: &Caja, visible: bool, x: u32, y: u32, alto: u32) -> u32 {
    if y < BARRA_ALTO {
        if y == BARRA_ALTO - 1 {
            return BARRA_LINEA;
        }
        // La marca de referencia dentro de la barra.
        if x >= 16 && x < 30 && y >= 13 && y < 27 {
            return ACENTO;
        }
        return BARRA;
    }
    // * Se pregunta por el rectangulo REDONDEADO y no por `contiene`. Si el
    // modelo creyera que la caja es cuadrada, al taparla y destaparla quedarian
    // cuatro pellizcos de su color en las esquinas -- un redondeo que solo sabe
    // pintar deja basura al desaparecer.
    if visible && dentro_redondeado(x, y, c.x, c.y, CAJA_ANCHO, CAJA_ALTO) {
        let en_borde = !dentro_redondeado(x, y, c.x + 1, c.y + 1, CAJA_ANCHO - 2, CAJA_ALTO - 2);
        if en_borde {
            return CAJA_BORDE;
        }
        if y < c.y + TITULO_ALTO {
            return CAJA_TITULO;
        }
        if y == c.y + TITULO_ALTO {
            return ACENTO;
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
    fondo_en(y, alto)
}


// -- Pintar la caja ------------------------------------------------------

/// El marco entero. Se pinta UNA vez; despues solo se repinta el campo.
/// La caja, con algo de forma.
///
/// Era un rectangulo con un borde de 2 px y el titulo escrito encima del mismo
/// fondo: plano, y con las cuatro esquinas en pico. Cinco cosas lo arreglan sin
/// salir de `rect` y `texto`, que es todo lo que tiene esta pantalla:
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
pub(crate) fn pintar_caja(p: &bmo::Pantalla, c: &Caja) {
    // 1. La sombra, primero y en dos capas.
    sombra(p, c.x, c.y, CAJA_ANCHO, CAJA_ALTO);

    // El marco y el relleno, ya redondos. Las esquinas no hay que biselarlas a
    // mano: `rect_redondeado` no pinta lo que sobra, asi que por ahi se sigue
    // viendo el escritorio.
    rect_redondeado(p, c.x, c.y, CAJA_ANCHO, CAJA_ALTO, CAJA_BORDE);
    rect_redondeado(p, c.x + 1, c.y + 1, CAJA_ANCHO - 2, CAJA_ALTO - 2, CAJA_FONDO);

    // 2 y 3. La barra de titulo y su acento. La barra tambien va redondeada
    // por arriba, o asomaria por fuera de las esquinas de la ventana.
    for i in 0..RADIO {
        let s = CURVA[i as usize];
        p.rect(c.x + s, c.y + 1 + i, CAJA_ANCHO - 2 * s, 1, CAJA_TITULO);
    }
    p.rect(c.x + 1, c.y + 1 + RADIO, CAJA_ANCHO - 2, TITULO_ALTO - 1 - RADIO, CAJA_TITULO);
    p.rect(c.x + 1, c.y + TITULO_ALTO, CAJA_ANCHO - 2, 1, ACENTO);

    // El punto de la izquierda: el mismo lenguaje que la marca de la barra de
    // arriba. Dos sitios, un solo idioma.
    p.rect(c.x + 16, c.y + 10, 8, 8, ACENTO);
    p.texto(c.x + 32, c.y + 7, "Ejecutar", TEXTO);
    p.texto(c.x + 32 + 10 * bmo::GLIFO_ANCHO, c.y + 7, "BMO-X", TEXTO_TENUE);

    p.texto(
        c.x + 18,
        c.y + 36,
        "ruta de un .bex y Enter.  info / cpu / mem / perf / ls / lee / guarda / smp / reboot.",
        TEXTO_TENUE,
    );
    // * Las dos ventanas del sistema, DICHAS. Un atajo que no esta escrito en
    // ninguna parte es un atajo que solo conoce quien lo programo -- y F11 existe
    // precisamente para los dias en que esta caja no responde.
    //
    // Va al PIE de la caja, no debajo de la pista: ahi lo puse primero y
    // `campo_y` es exactamente `y + 54`, asi que el marco del campo lo pintaba
    // encima y la linea no se veia. Se cazo en la foto -- el texto se emitia y
    // desaparecia en la instruccion siguiente.
    p.texto(
        c.x + 18,
        c.y + CAJA_ALTO - 22,
        "F11 kernel (Ring 0)   F12 datos (ESTRATOS)   ESC cierra",
        TEXTO_TENUE,
    );

    // 5. El campo. **El acento va SOLO en la linea de abajo**, no rodeandolo.
    //
    // Un marco entero del color del sistema alrededor de la caja de texto es lo
    // que hacia que pareciera un cuadro de dialogo de hace treinta anos: el
    // acento pasa a ser un marco y deja de senalar. Una raya bajo el campo dice
    // "aqui se escribe" con un cuarto de la tinta -- es lo que hacen Windows 11
    // y todos los escritorios de Linux modernos, y por este motivo.
    p.rect(c.campo_x - 1, c.campo_y - 1, c.campo_ancho + 2, c.campo_alto + 2, CAJA_BORDE);
    p.rect(c.campo_x, c.campo_y, c.campo_ancho, c.campo_alto, CAMPO_FONDO);
    p.rect(c.campo_x, c.campo_y + c.campo_alto, c.campo_ancho, 2, ACENTO);
}

/// El contenido del campo: la ruta y el cursor de escritura.
///
/// Repinta el fondo del campo entero antes de escribir. Es un rectangulo de
/// unos 500x28 px --nada-- y evita el clasico de borrar un caracter y que quede
/// medio glifo del anterior porque el nuevo es mas estrecho.
pub(crate) fn pintar_campo(p: &bmo::Pantalla, c: &Caja, ruta: &[u8], cur: usize, caret: bool) {
    p.rect(c.campo_x, c.campo_y, c.campo_ancho, c.campo_alto, CAMPO_FONDO);

    // La ventana visible se calcula alrededor del CURSOR, no del final.
    //
    // Antes se ensenaba siempre la cola, que valia mientras solo se podia
    // escribir al final. Con el cursor moviendose, eso deja de valer: si te
    // vas al principio de una ruta larga, el cursor se sale por la izquierda y
    // editas a ciegas. La regla es sencilla y es la de cualquier editor --
    // **el cursor SIEMPRE se ve**, y la ventana se desplaza lo minimo para
    // que asi sea.
    let cabe = c.visibles();
    let desde = if ruta.len() <= cabe {
        0
    } else if cur >= cabe {
        // El cursor se salio por la derecha: pegarlo al borde derecho.
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


/// Borra la caja devolviendo cada pixel a lo que la escena dice que hay
/// debajo. Es el precio de que la ventana se pueda invocar y esconder.
///
/// Recorre el rectangulo entero -- unos 325k pixeles sobre memoria de video sin
/// cache, que no es gratis. Pero pasa UNA vez por pulsacion de atajo, no por
/// fotograma, y la alternativa (guardar lo que habia debajo) seria un buffer de
/// 1,3 MB en un proceso con 64 KiB de pila.
pub(crate) fn borrar_caja(p: &bmo::Pantalla, c: &Caja) {
    // Su sombra tambien, por el mismo motivo que en `borrar_ventana`:
    // esconder con Ctrl+Alt dejaba la misma huella en L.
    for fila in 0..CAJA_ALTO + SOMBRA_ABAJO {
        for col in 0..CAJA_ANCHO + SOMBRA_DER {
            let (x, y) = (c.x + col, c.y + fila);
            p.punto(x, y, color_escena(c, false, x, y, p.alto));
        }
    }
}

/// Borra la consola de datos devolviendo cada pixel a lo que hay debajo.
///
/// * `visible` es si la caja de Ejecutar esta abierta, y hace falta: la consola
/// de datos se pinta ENCIMA de ella. `color_escena` sabe devolver el color de
/// la caja cuando el pixel cae dentro, asi que pasarle `false` aqui dejaria un
/// agujero con el fondo del escritorio en medio de la ventana de abajo.
///
/// Quien llama repinta despues el texto de la caja: esto devuelve el fondo, no
/// las letras.
/// * Toma un RECTANGULO y no una ventana concreta.
///
/// Era `borrar_datos(&datos::CajaDatos)`, atado al tipo de una ventana -- y con
/// eso, anadir la segunda ventana obligaba a copiar la funcion. Lo que esto
/// hace no depende de que ventana se cierra: devuelve el fondo de un area.
pub(crate) fn borrar_ventana(
    p: &bmo::Pantalla,
    c: &Caja,
    x0: u32,
    y0: u32,
    ancho: u32,
    alto: u32,
    visible: bool,
) {
    // * Se borra la ventana **Y SU SOMBRA**. Sin esto, cerrar deja una huella
    // en forma de L abajo a la derecha: los pixeles que la sombra pinto fuera
    // del rectangulo no los cubre nadie. Se vio en el Ryzen y es el motivo de
    // que `SOMBRA_DER`/`SOMBRA_ABAJO` sean constantes compartidas.
    //
    // `punto` recorta solo contra el panel, asi que pasarse por la derecha o
    // por abajo no hay que comprobarlo aqui.
    let alto = alto + SOMBRA_ABAJO;
    let ancho = ancho + SOMBRA_DER;
    for fila in 0..alto {
        for col in 0..ancho {
            let (x, y) = (x0 + col, y0 + fila);
            p.punto(x, y, color_escena(c, visible, x, y, p.alto));
        }
    }
}

pub(crate) fn pintar_estado(p: &bmo::Pantalla, c: &Caja, msg: &str, color: u32) {
    // Ancho fijo de limpieza: el mensaje anterior puede ser mas largo que el
    // nuevo, y media frase vieja detras de una nueva es peor que ninguna.
    p.rect(c.x + 18, c.estado_y, CAJA_ANCHO - 36, bmo::GLIFO_ALTO, CAJA_FONDO);
    p.texto(c.x + 18, c.estado_y, msg, color);
}

