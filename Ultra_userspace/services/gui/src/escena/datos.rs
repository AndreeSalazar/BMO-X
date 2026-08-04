//! **La consola de DATOS** — F12, el centro de control de ESTRATOS.
//!
//! ═══ Por qué una ventana aparte y no otro comando ═══
//!
//! La caja de `Ejecutar` es de una línea: escribes una ruta y algo corre. Eso
//! sirve para lanzar; no sirve para **mirar un almacén**. Un volumen tiene
//! estado —generación, ocupación, identidad, nivel— que se lee de un vistazo o
//! no se lee, y ponerlo detrás de un comando obliga a teclearlo cada vez que
//! quieres saber si algo cambió.
//!
//! ═══ Por qué F12 y no un Ctrl+algo ═══
//!
//! ★ **Una tecla de función no produce carácter en NINGUNA distribución.** No
//! puede chocar con escribir, y eso es lo único que importa en un atajo del
//! sistema. `Ctrl+Alt` ya lo tiene la ventana de Ejecutar **y es AltGr en
//! español** —lo que da `@ # [ ] \ | €`—, así que ese atajo lleva una danza
//! entera para no romper el teclado: dispara al SOLTAR, y sólo si no llegó
//! ningún carácter mientras estaban pulsados. Encadenar otro combo encima
//! empeoraría justo lo que costó arreglar.
//!
//! Hasta hoy las teclas de función llegaban al kernel y morían ahí: la
//! distribución no las resolvía a ningún byte. El hueco estaba limpio.
//!
//! ═══ Lo que ENSEÑA, y lo que todavía no hace ═══
//!
//! Enseña. Y dice, en alto, que todavía no escribe.
//!
//! La máquina de estados de la transacción existe y está probada
//! (`bmo_estratos::escritura`, 12 tests), pero **nadie la ha cableado al
//! dispositivo**: no hay `write` ni `FLUSH CACHE`. Esta ventana lo pone en
//! pantalla en vez de ofrecer un botón que no hace nada — en un almacén, una
//! promesa de escritura que no ocurre es como se pierde el trabajo de alguien.

//! ═══ ★ LAS DOS CARAS (spec del dueño, CUMPLIDA el 2026-08-03) ═══
//!
//! `[numeros]` contesta *"¿cómo está el almacén?"* — generación, espacio,
//! identidad, nivel. `[nodos]` contesta *"¿qué hay dentro?"*. Son preguntas
//! distintas y por eso son dos pestañas y no una pantalla: meter un árbol entre
//! la generación y la ocupación deja las dos ilegibles. `TAB` cambia.
//!
//! La referencia que puso el dueño era buena y concreta: **un grafo tipo n8n** —
//! cajas con título y nombre, unidas por líneas, con color por clase. No una
//! lista con sangrías.
//!
//! El porqué es el de siempre en este proyecto: **ESTRATOS no es un árbol de
//! carpetas, es un grafo de objetos** (nodos, atributos, flujos, estratos) que
//! se apuntan entre sí y **nunca se sobreescriben**. Dibujarlo como una lista
//! indentada obliga a imaginarse las aristas; dibujarlo como lo que es se
//! entiende sin explicación.
//!
//! Los cuatro puntos, y dónde quedó cada uno:
//!
//! 1. ✅ Exponer a Ring 3 lo que el kernel ya sabía leer. Es el **cursor** de
//!    `ring0/fsys/estratos.rs`, dos operaciones de la superficie.
//! 2. ✅ Un color por clase, y el mismo en toda la ventana — `color_clase`.
//! 3. ✅ Caja con **título** (qué es) y **nombre** (cuál es), que es lo que el
//!    dueño pidió: *"con títulos y nombres para facilitar"*.
//! 4. ◐ Teclado sí: flechas, `ENTRAR` baja, `RETROCESO` sube, `RePág`/`AvPág`
//!    de cinco en cinco. **Con el ratón, sólo arrastrar la ventana**: pulsar una
//!    caja para seleccionarla todavía no está.
//!
//! ═══ Lo que sigue sin hacer, dicho ═══
//!
//! - **Las versiones no se ven.** Cada commit deja los nodos viejos en pie, y
//!   eso en un grafo *se vería* — es la historia del volumen dibujada. Hoy el
//!   cursor sólo llega al estrato más reciente.
//! - **Escribir sigue sin existir.** La máquina de estados de la transacción
//!   está probada y `sellar()` ya commitea, pero crear un objeto es otra cosa.
//!   Esta ventana lo dice en alto en vez de ofrecer un botón que no hace nada:
//!   en un almacén, una promesa de escritura que no ocurre es como se pierde el
//!   trabajo de alguien.

use bmo_userland as bmo;

use super::marco::Marco;
use super::*;
use crate::texto::decimal;

// La ventana de Datos es VERDE porque es ESTRATOS, y eso se queda: el color
// dice de qué ventana estás hablando antes de leer su título. Lo que cambia es
// el tono — el verde de antes era de rotulador, escogido para verse en una foto
// de una pantalla que a lo mejor ni arrancaba.
const DATOS_FONDO: u32 = 0x0013_1C18;
const DATOS_TITULO_FONDO: u32 = 0x001B_2622;
/// El borde, discreto. Lo que separa la ventana del fondo es la sombra.
const DATOS_BORDE: u32 = 0x002C_4038;
/// Y el acento verde, que sí puede ser vivo: es una línea, no un marco.
const DATOS_TITULO: u32 = 0x0034_D399;

/// Los cuatro niveles de `bmo_estratos::espacio`, con su color.
///
/// El orden es el del ABI (`INFO_ES_NIVEL`), no uno inventado aquí: si
/// divergieran, el panel pintaría en verde un volumen en solo lectura.
fn nivel_texto(n: u64) -> (&'static str, u32) {
    match n {
        0 => ("holgado", TEXTO_BIEN),
        1 => ("AVISO: por encima del 70%", 0x00F0_D070),
        2 => ("FAULT: por encima del 85%", TEXTO_MAL),
        _ => ("SOLO LECTURA: por encima del 95%", TEXTO_MAL),
    }
}

/// La ventana de Datos: **un marco y lo que hay dentro**.
///
/// Todo lo de mover, estirar, maximizar y los tres botones vive en
/// [`super::marco::Marco`] y no aquí. Lo que queda en esta estructura es lo
/// único que de verdad es de ESTRATOS: qué se está enseñando y por dónde va la
/// vista del árbol.
pub(crate) struct CajaDatos {
    pub(crate) marco: Marco,
    /// Qué se está enseñando: los números o el árbol. Ver [`Vista`].
    pub(crate) vista: Vista,
    /// Qué hijo está señalado en la vista de nodos.
    pub(crate) sel: usize,
    /// Primer hijo visible: la lista es más larga que la ventana.
    pub(crate) desde: usize,
}

/// Lo que se puede encoger sin que deje de servir. Por debajo de esto el grafo
/// no cabe y los números se cortan — una ventana que se puede dejar inservible
/// con el ratón es una trampa, no una libertad.
pub(crate) const DATOS_MIN_ANCHO: u32 = 460;
pub(crate) const DATOS_MIN_ALTO: u32 = 260;
/// Y el tamaño con el que nace, **en tantos por ciento de la pantalla**. Un
/// `640 x 330` en píxeles es correcto en una pantalla y en ninguna otra.
const DATOS_PCT_ANCHO: u32 = 46;
const DATOS_PCT_ALTO: u32 = 44;

/// Las dos caras de esta ventana.
///
/// La de números contesta *¿cómo está el almacén?* y la de nodos *¿qué hay
/// dentro?*. Son preguntas distintas y por eso no se mezclan en una pantalla:
/// meter un árbol entre la generación y la ocupación deja las dos ilegibles.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Vista {
    Numeros,
    Nodos,
}

// El alto de la barra de título —que es el asa— sale de `super::TITULO_ALTO`:
// el mismo que la caja de Ejecutar. Dos ventanas del mismo sistema con barras
// de distinta altura se ven como dos programas de distinta época.

impl CajaDatos {
    pub(crate) fn nueva(p: &bmo::Pantalla) -> Self {
        Self {
            marco: Marco::nuevo(
                p,
                DATOS_PCT_ANCHO,
                DATOS_PCT_ALTO,
                DATOS_MIN_ANCHO,
                DATOS_MIN_ALTO,
            ),
            vista: Vista::Numeros,
            sel: 0,
            desde: 0,
        }
    }

    // Los atajos de siempre, para no escribir `.marco.` en cada uso. Son
    // reenvíos y nada más: la lógica vive en `Marco` y aquí no se repite.
    pub(crate) fn x(&self) -> u32 { self.marco.x }
    pub(crate) fn y(&self) -> u32 { self.marco.y }
    pub(crate) fn ancho(&self) -> u32 { self.marco.ancho }
    pub(crate) fn alto(&self) -> u32 { self.marco.alto }

    /// ¿Este píxel cae dentro? Lo necesita el borrado para saber qué repintar.
    pub(crate) fn contiene(&self, px: u32, py: u32) -> bool {
        self.marco.contiene(px, py)
    }

    /// Tras cambiar de tamaño, la selección puede haber quedado fuera de lo que
    /// se pinta. Se recoloca la ventana de scroll — si no, encoger dejaría el
    /// cursor señalando una caja que ya no está en pantalla.
    pub(crate) fn recolocar(&mut self) {
        let caben = self.caben();
        if self.sel >= self.desde + caben {
            self.desde = self.sel + 1 - caben;
        }
    }

    /// Cuántas cajas de hijo caben de una vez en la vista de nodos.
    fn caben(&self) -> usize {
        let util = self.marco.alto.saturating_sub(TITULO_ALTO + 56);
        (util / (CAJA_NODO_ALTO + CAJA_NODO_HUECO)).max(1) as usize
    }

    /// Mueve la selección y arrastra la ventana de scroll con ella.
    pub(crate) fn mover_sel(&mut self, delta: i32, cuantos: usize) {
        if cuantos == 0 {
            self.sel = 0;
            self.desde = 0;
            return;
        }
        let tope = cuantos - 1;
        self.sel = (self.sel as i32 + delta).clamp(0, tope as i32) as usize;
        let caben = self.caben();
        if self.sel < self.desde {
            self.desde = self.sel;
        } else if self.sel >= self.desde + caben {
            self.desde = self.sel + 1 - caben;
        }
    }

    /// Vuelve al principio de la lista. Se llama al cambiar de nodo: dejar la
    /// selección donde estaba haría que entrar en un directorio de dos hijos
    /// señalara al séptimo, que no existe.
    pub(crate) fn al_principio(&mut self) {
        self.sel = 0;
        self.desde = 0;
    }
}

// ── El GRAFO ────────────────────────────────────────────────────────────────
//
// ★ La spec del dueño, cumplida: **un grafo tipo n8n** — cajas con título y
// nombre, unidas por líneas, con color por clase. No una lista con sangrías.
//
// El porqué es el de siempre en este proyecto: **ESTRATOS no es un árbol de
// carpetas, es un grafo de objetos** que se apuntan entre sí y nunca se
// sobreescriben. Dibujarlo como una lista indentada obliga a imaginarse las
// aristas; dibujarlo como lo que es se entiende sin explicación.

/// Ancho MÍNIMO de una caja. El de verdad sale del ancho de la ventana: al
/// estirarla, las cajas crecen y caben nombres más largos. Una caja de tamaño
/// fijo dentro de una ventana que se estira deja un desierto a la derecha.
const CAJA_NODO_MIN: u32 = 170;
const CAJA_NODO_ALTO: u32 = 40;
const CAJA_NODO_HUECO: u32 = 12;
const SOMBRA_NODO: u32 = 0x000B_100E;
/// Las aristas del grafo. Más claras que el borde de la ventana **a propósito**:
/// son lo que hay que seguir con la vista, y una línea del mismo tono que el
/// marco se pierde entre los marcos.
const DATOS_ARISTA: u32 = 0x0045_6B5C;
/// El cuerpo de una caja del grafo: un peldaño por encima de la ventana, que es
/// la misma regla que separa la ventana del escritorio.
const CAJA_NODO_FONDO: u32 = 0x001B_2622;
/// Y la señalada, otro peldaño más. La profundidad se lee sola.
const CAJA_NODO_SEL: u32 = 0x0024_332C;

/// **Un color por clase, y el mismo en toda la ventana.** Es el punto 2 de la
/// spec: si el verde significara una cosa en el padre y otra en los hijos, el
/// color dejaría de informar y sólo decoraría.
fn color_clase(tipo: u64) -> (&'static str, u32) {
    match tipo {
        bmo::estratos::DIRECTORIO => ("directorio", 0x0057_C8F0),
        bmo::estratos::ARCHIVO => ("archivo", 0x007E_E787),
        _ => ("ilegible", TEXTO_MAL),
    }
}

/// Una caja con **título** (qué es) y **nombre** (cuál es). Punto 3 de la spec.
///
/// El título va arriba y en el color de la clase; el nombre, debajo y en
/// blanco. Al revés se leería el nombre y habría que buscar el tipo, que es lo
/// contrario de para qué está el color.
fn caja_nodo(
    p: &bmo::Pantalla,
    x: u32,
    y: u32,
    ancho: u32,
    tipo: u64,
    nombre: &[u8],
    señalada: bool,
) {
    let (titulo, color) = color_clase(tipo);
    // La señalada lleva el borde del acento y un cuerpo un punto más claro. Un
    // borde blanco a secas se lee como "esto está roto"; el realce de una
    // selección tiene que ser el color del sistema, no una alarma.
    let (borde, cuerpo) = if señalada {
        (color, CAJA_NODO_SEL)
    } else {
        (DATOS_BORDE, CAJA_NODO_FONDO)
    };
    // Sombra propia. Es lo que separa las cajas del fondo de la ventana y lo
    // que hace que un grafo parezca un grafo y no una lista con marcos.
    rect_redondeado(p, x + 2, y + 3, ancho, CAJA_NODO_ALTO, SOMBRA_NODO);
    rect_redondeado(p, x, y, ancho, CAJA_NODO_ALTO, borde);
    rect_redondeado(p, x + 1, y + 1, ancho - 2, CAJA_NODO_ALTO - 2, cuerpo);

    // ★ El PUNTO de clase, no una pestaña lateral.
    //
    // La barra pegada al borde peleaba con la curva de la esquina y se veía
    // como un defecto. Un punto delante del título es el mismo idioma que usan
    // la barra del sistema y las dos ventanas: se lee la clase de un vistazo y
    // no depende de que el título quepa.
    p.rect(x + 11, y + 7, 7, 7, color);
    p.texto(x + 24, y + 5, titulo, color);

    // El nombre, en su propia línea y en blanco. El título dice QUÉ es y el
    // nombre CUÁL es; ponerlos del mismo color obliga a leer los dos para
    // saber cuál es cuál.
    let cabe = ((ancho.saturating_sub(28)) / bmo::GLIFO_ANCHO) as usize;
    // Recortar por el final y no por el principio: los nombres de un volumen
    // se distinguen por delante (`maestro.bex`, `movim.txt`), no por detrás.
    let n = nombre.len().min(cabe);
    p.texto_bytes(x + 24, y + 5 + bmo::GLIFO_ALTO + 3, &nombre[..n], TEXTO);
    // Si no cupo entero se dice con un punto, no cortando a lo bruto: una
    // ventana estrecha no puede hacer que dos archivos parezcan el mismo.
    if n < nombre.len() {
        p.texto(
            x + 24 + n as u32 * bmo::GLIFO_ANCHO,
            y + 5 + bmo::GLIFO_ALTO + 3,
            "~",
            TEXTO_TENUE,
        );
    }
}

/// La vista de NODOS: el nodo actual a la izquierda y sus hijos a la derecha,
/// unidos por una espina y sus ramas.
fn pintar_nodos(p: &bmo::Pantalla, c: &CajaDatos) {
    let tx = c.marco.x + 16;
    let mut ty = c.marco.y + TITULO_ALTO + 6;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", TEXTO_MAL);
        return;
    }
    if !bmo::estratos::a_la_raiz() && bmo::estratos::tipo() == bmo::estratos::NADA {
        p.texto(tx, ty, "el volumen monta pero no tiene raiz legible.", TEXTO_MAL);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "el motivo esta en F11.", TEXTO_TENUE);
        return;
    }

    let hondo = bmo::estratos::hondo();
    let cuantos = bmo::estratos::hijos() as usize;

    // La miga de pan: a qué profundidad estamos. Sin esto, dos directorios con
    // los mismos nombres dentro se ven idénticos.
    {
        let mut b = [0u8; 10];
        let x = p.texto(tx, ty, "profundidad ", TEXTO_TENUE);
        let n = decimal(hondo, &mut b);
        let x = p.texto_bytes(x, ty, &b[..n], TEXTO);
        let x = p.texto(x, ty, "   hijos ", TEXTO_TENUE);
        let n = decimal(cuantos as u64, &mut b);
        let x = p.texto_bytes(x, ty, &b[..n], TEXTO);
        if bmo::estratos::truncado() {
            // Se DICE. Un listado recortado en silencio se ve igual que un
            // directorio con pocos archivos, y esa confusión cuesta horas.
            p.texto(x, ty, "   (RECORTADO: no cabian todos)", TEXTO_MAL);
        }
    }
    ty += bmo::GLIFO_ALTO + 10;

    // ── ★ EL REPARTO DEL ANCHO ──
    //
    // Las cajas ya no miden lo mismo pase lo que pase: el ancho útil se parte
    // entre las dos columnas y el canal de las ramas. Estirar la ventana hace
    // que quepan nombres más largos, que es para lo que uno la estira. Una caja
    // de tamaño fijo dentro de una ventana elástica deja un desierto a la
    // derecha y sigue cortando los nombres.
    const CANAL: u32 = 44; // lo que ocupan la espina y sus codos
    let util = c.marco.ancho.saturating_sub(32);
    let ancho_caja = ((util.saturating_sub(CANAL)) / 2).max(CAJA_NODO_MIN);

    // ── El nodo actual, a la izquierda ──
    let padre_y = ty + 4;
    let nombre_actual: &[u8] = if hondo == 0 { b"/" } else { b"(aqui)" };
    caja_nodo(p, tx, padre_y, ancho_caja, bmo::estratos::tipo(), nombre_actual, false);

    if cuantos == 0 {
        p.texto(tx, padre_y + CAJA_NODO_ALTO + 16, "esta vacio.", TEXTO_TENUE);
        return;
    }

    // ── La espina y las ramas ──
    //
    // Sin primitiva de línea: un rectángulo de un píxel de ancho ES una línea,
    // y para un grafo de codos —que es como pinta n8n— no hace falta más.
    let espina_x = tx + ancho_caja + CANAL / 2;
    let hijos_x = tx + ancho_caja + CANAL;
    let caben = c.caben();
    let ultimo = (c.desde + caben).min(cuantos);

    let primera_y = ty + 4;
    let mut hy = primera_y;
    for i in c.desde..ultimo {
        let centro = hy + CAJA_NODO_ALTO / 2;
        // La rama, de la espina a la caja. **Dos píxeles de grueso**: a uno
        // solo, una línea horizontal sobre un fondo oscuro casi no se ve, y
        // entonces las cajas parecen sueltas en vez de colgadas de un padre.
        p.rect(espina_x, centro, hijos_x - espina_x, 2, DATOS_ARISTA);
        // El punto de enganche en la caja: cierra la arista en vez de dejarla
        // chocando contra un borde. Es lo que hace que se lea como un grafo.
        p.rect(hijos_x - 4, centro - 2, 5, 5, DATOS_ARISTA);
        let tipo = bmo::estratos::hijo_tipo(i as u64);
        let mut nombre = [0u8; 64];
        let n = bmo::estratos::hijo_nombre(i as u64, &mut nombre);
        caja_nodo(p, hijos_x, hy, ancho_caja, tipo, &nombre[..n], i == c.sel);
        hy += CAJA_NODO_ALTO + CAJA_NODO_HUECO;
    }
    // La espina vertical, del centro de la primera rama al de la última.
    let arriba = primera_y + CAJA_NODO_ALTO / 2;
    let abajo = hy - CAJA_NODO_ALTO - CAJA_NODO_HUECO + CAJA_NODO_ALTO / 2;
    p.rect(espina_x, arriba, 2, abajo.saturating_sub(arriba) + 2, DATOS_ARISTA);
    // Y el tramo que sale del padre hasta la espina, a su altura.
    let salida_y = padre_y + CAJA_NODO_ALTO / 2;
    p.rect(tx + ancho_caja, salida_y, espina_x - (tx + ancho_caja), 2, DATOS_ARISTA);
    p.rect(tx + ancho_caja - 1, salida_y - 2, 5, 5, DATOS_ARISTA);
    if salida_y != arriba {
        let (a, b) = if salida_y < arriba { (salida_y, arriba) } else { (arriba, salida_y) };
        p.rect(espina_x, a, 2, b - a + 2, DATOS_ARISTA);
    }

    // Si la lista no cabe entera, decirlo con números y no con puntos
    // suspensivos: "3-8 de 40" se lee; "..." no dice cuánto falta.
    if cuantos > caben {
        let mut b = [0u8; 10];
        let y = c.marco.y + c.marco.alto - TITULO_ALTO - bmo::GLIFO_ALTO;
        let n = decimal(c.desde as u64 + 1, &mut b);
        let x = p.texto_bytes(hijos_x, y, &b[..n], TEXTO_TENUE);
        let x = p.texto(x, y, "-", TEXTO_TENUE);
        let n = decimal(ultimo as u64, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO_TENUE);
        let x = p.texto(x, y, " de ", TEXTO_TENUE);
        let n = decimal(cuantos as u64, &mut b);
        p.texto_bytes(x, y, &b[..n], TEXTO_TENUE);
    }
}

/// Pinta la consola de datos entera.
///
/// Se redibuja completa en cada invocación y no por fotograma: los números de
/// un volumen no cambian solos mientras nadie escriba, y repintar 200k píxeles
/// sobre memoria de vídeo sin caché sesenta veces por segundo para enseñar los
/// mismos dígitos es tirar el fotograma.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &CajaDatos) {
    if c.marco.minimizada {
        return;
    }
    // ★ El cromo entero —sombra, borde, cuerpo, barra, los tres botones y el
    // asa de la esquina— lo pinta el MARCO. Aquí sólo van los colores, que sí
    // son de esta ventana: el verde dice ESTRATOS antes de que nadie lea el
    // título.
    c.marco.pintar_cromo(p, DATOS_BORDE, DATOS_FONDO, DATOS_TITULO_FONDO, DATOS_TITULO);

    let tx = c.marco.x + 16;
    p.rect(tx, c.marco.y + 9, 8, 8, DATOS_TITULO);
    let px = p.texto(tx + 16, c.marco.y + 8, "ESTRATOS", TEXTO);
    let px = px + 2 * bmo::GLIFO_ANCHO;
    // Las pestañas: la activa lleva su subrayado. Un corchete pintado de otro
    // color se pierde en una foto; una línea debajo no.
    let (c1, c2) = match c.vista {
        Vista::Numeros => (TEXTO, TEXTO_TENUE),
        Vista::Nodos => (TEXTO_TENUE, TEXTO),
    };
    let fin1 = p.texto(px, c.marco.y + 8, "numeros", c1);
    let px2 = fin1 + 2 * bmo::GLIFO_ANCHO;
    let fin2 = p.texto(px2, c.marco.y + 8, "nodos", c2);
    let (sx, sw) = match c.vista {
        Vista::Numeros => (px, fin1 - px),
        Vista::Nodos => (px2, fin2 - px2),
    };
    p.rect(sx, c.marco.y + 8 + bmo::GLIFO_ALTO + 2, sw, 2, DATOS_TITULO);

    if c.vista == Vista::Nodos {
        pintar_nodos(p, c);
        let y = c.marco.y + c.marco.alto - bmo::GLIFO_ALTO - 8;
        p.texto(tx, y, "flechas mueven   ENTRAR baja   RETROCESO sube   F12 cierra", TEXTO_TENUE);
        return;
    }

    let mut ty = c.marco.y + TITULO_ALTO + 6;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", TEXTO_MAL);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "se formatea desde el anfitrion con estratos-fmt.", TEXTO_TENUE);
        return;
    }

    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let usados = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);
    let nivel = bmo::info(bmo::INFO_ES_NIVEL);

    let mut fila = |etiqueta: &str, y: &mut u32, pinta: &dyn Fn(u32, u32)| {
        p.texto(tx, *y, etiqueta, TEXTO_TENUE);
        pinta(tx + 13 * bmo::GLIFO_ANCHO, *y);
        *y += bmo::GLIFO_ALTO + 3;
    };

    fila("generacion", &mut ty, &|x, y| {
        let g = bmo::info(bmo::INFO_ES_GENERACION);
        let mut b = [0u8; 10];
        let n = decimal(g, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "  transacciones desde el formateo", TEXTO_TENUE);
    });

    fila("espacio", &mut ty, &|x, y| {
        let x = magnitud(p, x, y, usados * tam, TEXTO);
        let x = p.texto(x, y, " de ", TEXTO_TENUE);
        let x = magnitud(p, x, y, bloques * tam, TEXTO);
        let pct = if bloques == 0 { 0 } else { usados * 100 / bloques };
        let x = p.texto(x, y, "   ", TEXTO_TENUE);
        let mut b = [0u8; 10];
        let n = decimal(pct, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "%", TEXTO);
    });

    fila("estado", &mut ty, &|x, y| {
        let (t, color) = nivel_texto(nivel);
        p.texto(x, y, t, color);
    });

    fila("identidad", &mut ty, &|x, y| {
        if bmo::info(bmo::INFO_ES_IDENTIDAD) != 0 {
            p.texto(x, y, "nacio en ESTE disco", TEXTO_BIEN);
        } else {
            p.texto(x, y, "NO nacio aqui: clonado? no se escribira", TEXTO_MAL);
        }
    });

    // ★ Cuantas VERSIONES mas caben. Es lo que de verdad contesta "¿cuando
    // hara falta el recolector?" — un porcentaje no lo dice, y la respuesta
    // con 414 GiB son millones.
    fila("caben", &mut ty, &|x, y| {
        let libres = bloques.saturating_sub(usados);
        let por_obj = (20 * 1024u64).div_ceil(tam).max(1);
        let mut b = [0u8; 10];
        let n = decimal(libres / por_obj, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "  objetos mas de 20 KiB", TEXTO_TENUE);
    });

    ty += 8;
    // ── La verdad sobre la escritura ──
    if bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0 {
        p.texto(tx, ty, "escritura: ABIERTA", TEXTO_BIEN);
    } else {
        p.texto(tx, ty, "escritura: CERRADA", TEXTO_MAL);
        ty += bmo::GLIFO_ALTO + 3;
        p.texto(tx, ty, "  la transaccion existe y esta probada (12 tests),", TEXTO_TENUE);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  pero nadie la ha cableado al dispositivo todavia:", TEXTO_TENUE);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  falta el write y el FLUSH CACHE de verdad.", TEXTO_TENUE);
    }

    ty += bmo::GLIFO_ALTO + 10;
    p.texto(tx, ty, "F12 o ESC cierran.   TAB ensena el ARBOL de nodos.", TEXTO_TENUE);
    ty += bmo::GLIFO_ALTO + 2;
    // ★ Decirlo aquí evita el susto: con esta ventana delante el teclado es
    // SUYO, así que teclear no escribe en la caja de abajo. Antes sí escribía
    // —en una ventana tapada, sin verlo—, y eso era el fallo.
    p.texto(tx, ty, "mientras este abierta, el teclado es de esta ventana.", TEXTO_TENUE);
}

/// Un número de bytes con su unidad. Devuelve la x donde acabó.
///
/// Sin coma flotante: la parte fraccionaria sale de multiplicar el resto por
/// cien antes de dividir. Es la misma cuenta que hace el panel del kernel, y
/// está aquí duplicada a propósito — cruzar el anillo para formatear un número
/// sería exactamente lo que un library OS no hace.
fn magnitud(p: &bmo::Pantalla, x: u32, y: u32, bytes: u64, color: u32) -> u32 {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    let (unidad, div) = if bytes >= G {
        ("GiB", G)
    } else if bytes >= M {
        ("MiB", M)
    } else if bytes >= K {
        ("KiB", K)
    } else {
        ("B", 1)
    };
    let mut b = [0u8; 10];
    let n = decimal(bytes / div, &mut b);
    let mut x = p.texto_bytes(x, y, &b[..n], color);
    if div > 1 {
        let frac = (bytes % div) * 100 / div;
        x = p.texto(x, y, ".", color);
        if frac < 10 {
            x = p.texto(x, y, "0", color);
        }
        let n = decimal(frac, &mut b);
        x = p.texto_bytes(x, y, &b[..n], color);
    }
    x = p.texto(x, y, " ", color);
    p.texto(x, y, unidad, color)
}
