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

use super::*;
use crate::texto::decimal;

pub(crate) const DATOS_ANCHO: u32 = 640;
pub(crate) const DATOS_ALTO: u32 = 330;

const DATOS_FONDO: u32 = 0x0012_2418;
const DATOS_BORDE: u32 = 0x0037_C871;
const DATOS_TITULO: u32 = 0x008F_F0B8;

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

/// Dónde va la ventana. Empieza centrada y **se puede arrastrar**.
pub(crate) struct CajaDatos {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) ancho: u32,
    pub(crate) alto: u32,
    /// Qué se está enseñando: los números o el árbol. Ver [`Vista`].
    pub(crate) vista: Vista,
    /// Qué hijo está señalado en la vista de nodos.
    pub(crate) sel: usize,
    /// Primer hijo visible: la lista es más larga que la ventana.
    pub(crate) desde: usize,
    /// Si se está arrastrando, dónde se agarró DENTRO de la ventana.
    ///
    /// Se guarda el agarre y no la posición del ratón porque si no la ventana
    /// pega un salto al empezar a arrastrar: se colocaría con su esquina bajo
    /// el puntero en vez de quedarse donde la cogiste.
    arrastre: Option<(u32, u32)>,
}

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

/// Alto de la barra de título. Es la zona por la que se arrastra — como en
/// cualquier ventana, y por el mismo motivo: si se arrastrara desde cualquier
/// parte, no se podría pulsar nada de dentro.
const TITULO_ALTO: u32 = 26;

impl CajaDatos {
    pub(crate) fn nueva(p: &bmo::Pantalla) -> Self {
        let ancho = DATOS_ANCHO.min(p.ancho.saturating_sub(40));
        let alto = DATOS_ALTO.min(p.alto.saturating_sub(40));
        Self {
            x: (p.ancho.saturating_sub(ancho)) / 2,
            y: (p.alto.saturating_sub(alto)) / 2,
            ancho,
            alto,
            vista: Vista::Numeros,
            sel: 0,
            desde: 0,
            arrastre: None,
        }
    }

    /// ¿Este píxel cae dentro? Lo necesita el borrado para saber qué repintar.
    pub(crate) fn contiene(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.ancho && py >= self.y && py < self.y + self.alto
    }

    /// ¿Cae en la barra de título, o sea en el asa?
    pub(crate) fn en_el_asa(&self, px: u32, py: u32) -> bool {
        self.contiene(px, py) && py < self.y + TITULO_ALTO
    }

    /// Empieza a arrastrar desde `(px, py)`. Sólo agarra por el asa.
    pub(crate) fn agarrar(&mut self, px: u32, py: u32) -> bool {
        if !self.en_el_asa(px, py) {
            return false;
        }
        self.arrastre = Some((px - self.x, py - self.y));
        true
    }

    pub(crate) fn soltar(&mut self) {
        self.arrastre = None;
    }

    pub(crate) fn arrastrando(&self) -> bool {
        self.arrastre.is_some()
    }

    /// Lleva la ventana bajo el puntero. Devuelve `true` si se movió de verdad
    /// — mover cero píxeles no vale un repintado.
    ///
    /// Se topa contra los bordes del panel dejando el asa siempre dentro: una
    /// ventana arrastrada fuera de la pantalla no se puede volver a coger, y
    /// entonces la única salida es cerrarla a ciegas con F12.
    pub(crate) fn arrastrar_a(&mut self, p: &bmo::Pantalla, px: u32, py: u32) -> bool {
        let Some((ax, ay)) = self.arrastre else { return false };
        let nx = px.saturating_sub(ax).min(p.ancho.saturating_sub(self.ancho));
        let ny = py.saturating_sub(ay).min(p.alto.saturating_sub(self.alto));
        if nx == self.x && ny == self.y {
            return false;
        }
        self.x = nx;
        self.y = ny;
        true
    }

    /// Cuántas cajas de hijo caben de una vez en la vista de nodos.
    fn caben(&self) -> usize {
        let util = self.alto.saturating_sub(TITULO_ALTO + 40);
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

const CAJA_NODO_ANCHO: u32 = 190;
const CAJA_NODO_ALTO: u32 = 34;
const CAJA_NODO_HUECO: u32 = 10;

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
    tipo: u64,
    nombre: &[u8],
    señalada: bool,
) {
    let (titulo, color) = color_clase(tipo);
    let borde = if señalada { 0x00FF_FFFF } else { color };
    p.rect(x, y, CAJA_NODO_ANCHO, CAJA_NODO_ALTO, borde);
    p.rect(x + 1, y + 1, CAJA_NODO_ANCHO - 2, CAJA_NODO_ALTO - 2, DATOS_FONDO);
    // La pestaña de color a la izquierda: se ve la clase de un vistazo aunque
    // el título quede tapado por un nombre largo.
    p.rect(x + 1, y + 1, 3, CAJA_NODO_ALTO - 2, color);
    p.texto(x + 9, y + 4, titulo, color);
    let mut n = nombre.len();
    // Recortar por el final y no por el principio: los nombres de un volumen
    // se distinguen por delante (`maestro.bex`, `movim.txt`), no por detrás.
    let cabe = ((CAJA_NODO_ANCHO - 14) / bmo::GLIFO_ANCHO) as usize;
    if n > cabe {
        n = cabe;
    }
    p.texto_bytes(x + 9, y + 4 + bmo::GLIFO_ALTO + 2, &nombre[..n], TEXTO);
}

/// La vista de NODOS: el nodo actual a la izquierda y sus hijos a la derecha,
/// unidos por una espina y sus ramas.
fn pintar_nodos(p: &bmo::Pantalla, c: &CajaDatos) {
    let tx = c.x + 16;
    let mut ty = c.y + TITULO_ALTO + 6;

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

    // ── El nodo actual, a la izquierda ──
    let padre_y = ty + 4;
    let nombre_actual: &[u8] = if hondo == 0 { b"/" } else { b"(aqui)" };
    caja_nodo(p, tx, padre_y, bmo::estratos::tipo(), nombre_actual, false);

    if cuantos == 0 {
        p.texto(tx, padre_y + CAJA_NODO_ALTO + 14, "sin hijos.", TEXTO_TENUE);
        return;
    }

    // ── La espina y las ramas ──
    //
    // Sin primitiva de línea: un rectángulo de un píxel de ancho ES una línea,
    // y para un grafo de codos —que es como pinta n8n— no hace falta más.
    let espina_x = tx + CAJA_NODO_ANCHO + 20;
    let hijos_x = espina_x + 24;
    let caben = c.caben();
    let ultimo = (c.desde + caben).min(cuantos);

    let primera_y = ty + 4;
    let mut hy = primera_y;
    for i in c.desde..ultimo {
        let centro = hy + CAJA_NODO_ALTO / 2;
        // La rama, de la espina a la caja.
        p.rect(espina_x, centro, hijos_x - espina_x, 1, DATOS_BORDE);
        let tipo = bmo::estratos::hijo_tipo(i as u64);
        let mut nombre = [0u8; 64];
        let n = bmo::estratos::hijo_nombre(i as u64, &mut nombre);
        caja_nodo(p, hijos_x, hy, tipo, &nombre[..n], i == c.sel);
        hy += CAJA_NODO_ALTO + CAJA_NODO_HUECO;
    }
    // La espina vertical, del centro de la primera rama al de la última.
    let arriba = primera_y + CAJA_NODO_ALTO / 2;
    let abajo = hy - CAJA_NODO_ALTO - CAJA_NODO_HUECO + CAJA_NODO_ALTO / 2;
    p.rect(espina_x, arriba, 1, abajo.saturating_sub(arriba) + 1, DATOS_BORDE);
    // Y el tramo que sale del padre hasta la espina, a su altura.
    let salida_y = padre_y + CAJA_NODO_ALTO / 2;
    p.rect(tx + CAJA_NODO_ANCHO, salida_y, espina_x - (tx + CAJA_NODO_ANCHO), 1, DATOS_BORDE);
    if salida_y != arriba {
        let (a, b) = if salida_y < arriba { (salida_y, arriba) } else { (arriba, salida_y) };
        p.rect(espina_x, a, 1, b - a + 1, DATOS_BORDE);
    }

    // Si la lista no cabe entera, decirlo con números y no con puntos
    // suspensivos: "3-8 de 40" se lee; "..." no dice cuánto falta.
    if cuantos > caben {
        let mut b = [0u8; 10];
        let y = c.y + c.alto - TITULO_ALTO - bmo::GLIFO_ALTO;
        let n = decimal(c.desde as u64 + 1, &mut b);
        let x = p.texto_bytes(tx, y, &b[..n], TEXTO_TENUE);
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
    p.rect(c.x, c.y, c.ancho, c.alto, DATOS_BORDE);
    p.rect(c.x + 2, c.y + 2, c.ancho - 4, c.alto - 4, DATOS_FONDO);

    // ── La barra de título: el asa, y las dos pestañas ──
    let tx = c.x + 16;
    p.texto(tx, c.y + 8, "ESTRATOS", DATOS_TITULO);
    let px = tx + 10 * bmo::GLIFO_ANCHO;
    let (c1, c2) = match c.vista {
        Vista::Numeros => (DATOS_TITULO, TEXTO_TENUE),
        Vista::Nodos => (TEXTO_TENUE, DATOS_TITULO),
    };
    let px = p.texto(px, c.y + 8, "[numeros]", c1);
    p.texto(px + bmo::GLIFO_ANCHO, c.y + 8, "[nodos]", c2);
    p.texto(
        c.x + c.ancho - 22 * bmo::GLIFO_ANCHO,
        c.y + 8,
        "TAB cambia  arrastra",
        TEXTO_TENUE,
    );
    p.rect(c.x + 2, c.y + TITULO_ALTO - 2, c.ancho - 4, 1, DATOS_BORDE);

    if c.vista == Vista::Nodos {
        pintar_nodos(p, c);
        let y = c.y + c.alto - bmo::GLIFO_ALTO - 8;
        p.texto(tx, y, "flechas mueven   ENTRAR baja   RETROCESO sube   F12 cierra", TEXTO_TENUE);
        return;
    }

    let mut ty = c.y + TITULO_ALTO + 6;

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
