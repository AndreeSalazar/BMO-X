//! **EL LIENZO DE RING 0** -- donde caen los pixeles, y nada mas.
//!
//! ## Que se llevo de `splash.rs`, y por que era lo primero
//!
//! Este fichero es el escalon de abajo del monolito: las cuatro funciones que
//! de verdad tocan el framebuffer (`put_pix`, `fill_rect`, `draw_rect_outline`
//! y la barrera) mas la mezcla de colores. Estaban en medio de mil quinientas
//! lineas que tambien sabian de tipografia, de la CABINA y del guion de la
//! intro, asi que **nadie las miraba como una pieza** -- y una de ellas estaba
//! mal.
//!
//! ## ** EL FALLO QUE VIVIA AQUI
//!
//! No en estas funciones: en la GUARDA que cada quien se escribia antes de
//! llamarlas. `pintar_escena` decia:
//!
//! ```text
//!    if cw > 0 && ch > 0 && x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
//!        fill_rect(x as u32, y as u32, cw as u32, ch as u32, c);
//!    }
//! ```
//!
//! Que es "comprobar los limites" y es **descartar**: si una esquina se sale,
//! el rectangulo entero se tira. Y la ciudad emite rectangulos que empiezan
//! fuera a proposito --el sello del marco se estira pasado el borde para que la
//! deriva de la camara no abra una rendija--, asi que el rectangulo que existe
//! para tapar el borde era el primero en desaparecer.
//!
//! Medido sobre el video del arranque del 2026-08-15, a 1920x1080: **2.625 de
//! los 8.775 rectangulos de cada fotograma se tiraban enteros** y 149.376
//! pixeles --el 7,2% de la pantalla-- no se escribian jamas. La franja muerta de
//! 191 px pegada al borde izquierdo se ve a simple vista en la grabacion.
//!
//! El previsualizador del anfitrion no podia cazarlo porque tenia SU regla, y
//! la suya recortaba. Dos implementaciones de la misma decision es una que esta
//! mal sin que nadie sepa cual.
//!
//! Ahora la decision no esta aqui: esta en `bmo_dibujo::Lienzo`, escrita una vez
//! para las tres orillas. [`Pantalla`] solo contesta **donde acaba la pantalla**
//! y **como se escribe un pixel**; el recorte se lo hacen.
//!
//! ## Por que `Pantalla` no vacia el buffer en cada rectangulo
//!
//! El framebuffer de UEFI esta mapeado como *write-combining*: las escrituras
//! se agrupan en el buffer WC y **no llegan a la VRAM hasta que una barrera lo
//! drena**. `fill_rect` hace esa barrera al final de cada rectangulo, que es lo
//! correcto para dibujar una cosa suelta -- el panel de arranque, una linea del
//! log, la pantalla de fallo.
//!
//! Para un fotograma de la intro es lo contrario de lo correcto: son unos 8.800
//! rectangulos, o sea 8.800 `mfence` para ensenar una sola imagen. Por eso
//! [`Pantalla`] escribe sin barrera y la barrera se pide una vez, en
//! [`Pantalla::presentar`]. Es tambien el sitio donde entraria el volcado desde
//! una superficie en RAM el dia que la intro deje de pintar sobre la pantalla
//! que se esta escaneando.

use bmo_dibujo::{Color, Lienzo, Recorte};

// ?????? Barrera de memoria ??????????????????????????????????????????????????
//
// The GOP framebuffer is typically mapped as WC (write-combining)
// by UEFI. WC stores are batched into the WC buffer and NOT
// guaranteed to reach VRAM until a full memory barrier flushes
// the buffer. `sfence` only orders `movnti` non-temporal stores;
// for normal WC writes, `mfence` is required. Without `mfence`,
// the display hardware sees the old contents (black) for an
// unpredictable amount of time, and the screen appears blank.

#[inline]
pub(crate) fn wc_flush() {
    // `mfence` is the correct barrier for WC memory:
    // it serializes all load/store instructions AND drains
    // the WC buffer before any subsequent loads or stores.
    unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }
}

// ?????? Dibujo suelto: una cosa, y se ve ya ?????????????????????????????????
//
// Estas son las de siempre y siguen aqui porque siguen teniendo su sitio: el
// panel de arranque, la CABINA y la pantalla de fallo pintan **de una en una** y
// quieren que se vea al momento. Lo que ya no hacen es sostener la intro.

pub(crate) fn put_pix(x: u32, y: u32, color: u32) {
    let fb = unsafe { crate::info::FB_ADDR as *mut u32 };
    let st  = unsafe { crate::info::FB_STRIDE as usize };
    let h   = unsafe { crate::info::FB_HEIGHT };
    if y < h && (x as usize) < st {
        unsafe {
            fb.add((y as usize) * st + (x as usize)).write_volatile(color);
        }
    }
}

pub(crate) fn fill_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    let fb = unsafe { crate::info::FB_ADDR as *mut u32 };
    let st = unsafe { crate::info::FB_STRIDE as usize };
    let alto = unsafe { crate::info::FB_HEIGHT };
    if fb.is_null() { return; }
    let mut any = false;
    for dy in 0..h {
        let py = y + dy;
        if py >= alto { break; }
        for dx in 0..w {
            let px = x + dx;
            if (px as usize) >= st { break; }
            unsafe { fb.add((py as usize) * st + (px as usize)).write_volatile(color); }
            any = true;
        }
    }
    if any { wc_flush(); }
}

pub(crate) fn draw_rect_outline(x: u32, y: u32, w: u32, h: u32, color: u32) {
    if w == 0 || h == 0 { return; }
    for dx in 0..w {
        put_pix(x + dx, y, color);
        put_pix(x + dx, y + h - 1, color);
    }
    for dy in 0..h {
        put_pix(x, y + dy, color);
        put_pix(x + w - 1, y + dy, color);
    }
    wc_flush();
}

// ?????? El lienzo de la pantalla ????????????????????????????????????????????

/// **La pantalla de Ring 0, como lienzo.**
///
/// Se coge una vez por fotograma con [`Pantalla::actual`], se le pinta encima
/// con los metodos de [`Lienzo`] --que aceptan coordenadas negativas y las
/// recortan-- y se cierra con [`Pantalla::presentar`].
///
/// [!] **El recorte es el ancho VISIBLE, no el `stride`.** Son distintos: el
/// framebuffer puede tener filas mas largas que la pantalla, y esos pixeles de
/// mas no se ven. `fill_rect` recorta contra el `stride` --escribe en el relleno
/// y no pasa nada-- pero un lienzo que dijera que mide el `stride` estaria
/// mintiendo sobre donde acaba la imagen, y quien pinte centrado pintaria
/// descentrado.
pub(crate) struct Pantalla {
    fb: *mut u32,
    stride: usize,
    w: i32,
    h: i32,
}

impl Pantalla {
    /// El framebuffer que haya ahora mismo, o `None` si todavia no hay.
    ///
    /// Se pregunta cada vez y no se guarda: el modo de video se fija en el
    /// arranque, pero guardar un puntero al framebuffer en un `static` es
    /// exactamente la clase de copia que sobrevive a un cambio de modo y pinta
    /// en una direccion que ya no es de nadie.
    pub(crate) fn actual() -> Option<Pantalla> {
        let fb = unsafe { crate::info::FB_ADDR as *mut u32 };
        let stride = unsafe { crate::info::FB_STRIDE as usize };
        let w = unsafe { crate::info::FB_WIDTH } as i32;
        let h = unsafe { crate::info::FB_HEIGHT } as i32;
        if fb.is_null() || w <= 0 || h <= 0 || stride == 0 {
            return None;
        }
        Some(Pantalla { fb, stride, w, h })
    }

    /// **Ensena lo pintado.** Una barrera por fotograma, no una por rectangulo.
    ///
    /// Mientras esto no se llame, lo escrito puede estar todavia en el buffer
    /// de escritura combinada y no haber llegado a la VRAM.
    pub(crate) fn presentar(&self) {
        wc_flush();
    }
}

// ?????? La superficie en RAM ????????????????????????????????????????????????

/// **Una pantalla entera en memoria normal, para volcarla de una vez.**
///
/// ## Que compra, y no es solo el desgarro
///
/// La intro pintaba **directamente sobre el framebuffer que el monitor esta
/// escaneando**. Con unos 8.800 rectangulos por fotograma y sin sincronizar con
/// nada, lo que el panel ensena a mitad de camino es una mezcla de dos
/// fotogramas: el desgarro que se ve en el video del 2026-08-15 entre el
/// segundo 5,0 y el 5,4.
///
/// Pero el motivo de peso es otro y es aritmetica, no gusto:
///
/// ```text
///   pintando en el framebuffer   11,5 MB/fotograma a 300 MB/s  =  38,2 ms
///   pintando en RAM y volcando    8,3 MB/fotograma a 300 MB/s  =  27,6 ms
///                               + 11,5 MB a velocidad de RAM   ~   1,2 ms
/// ```
///
/// Los 11,5 MB salen de `bmo-vista-ciudad --bin presupuesto` a 1920x1080: la
/// escena escribe 1,38 veces la pantalla porque el algoritmo del pintor pinta
/// cielo debajo de las torres. **Ese sobredibujo se paga hoy a precio de
/// framebuffer**, que es la memoria mas lenta de la maquina. Con una superficie
/// se paga a precio de RAM, y al framebuffer va cada pixel exactamente una vez.
///
/// O sea que volcar no es el coste: volcar es lo que **quita** un coste mayor.
/// Diez milisegundos por fotograma, casi una cuarta parte.
///
/// [!] Los numeros de arriba salen de los 300 MB/s medidos en DOOM. Lo que
/// falta es medirlo con la intro de verdad en el Ryzen, y para eso esta
/// `[perf]` -- ver `bmo-pendiente-hardware`.
///
/// ## Y lo que ABRE, que es lo que mas va a durar
///
/// Media escena esta escrita esquivando una regla: *"no se puede leer el
/// framebuffer"*. Es cierta --leer de memoria write-combining va lentisimo, y
/// ya costo caro en el blit de DOOM-- y de ella salen el aura opaca, el gato
/// que se enciende con color en vez de con alfa, y el destello que es una caja
/// en vez de un resplandor. **Sobre RAM esa regla no existe**: leer un pixel de
/// aqui es leer memoria normal. El alfa de verdad es el escalon 4 del
/// rasterizador, y esto es lo que lo hace posible.
///
/// ## Si no hay memoria, no pasa nada
///
/// [`Superficie::nueva`] devuelve `None` y quien llama pinta sobre la pantalla
/// como toda la vida. Una intro con desgarro es mucho mejor que un arranque que
/// no arranca porque no habia ocho megas contiguos.
pub(crate) struct Superficie {
    px: *mut u32,
    w: i32,
    h: i32,
    /// Base FISICA, para devolver los marcos. Guardarla es obligatorio: el
    /// puntero de arriba es la direccion del physmap, no la que el asignador
    /// entiende.
    base: u64,
    marcos: u64,
}

impl Superficie {
    /// Reserva una superficie de `w x h`. `None` si no hay sitio.
    ///
    /// Los marcos son **contiguos** a proposito: la superficie se recorre
    /// linealmente, y el physmap solo garantiza que `base + n*PAGE` sea el
    /// marco `n` si se pidieron juntos. Con `alloc_frame` en bucle saldria bien
    /// en QEMU y mal en un mapa de memoria real con huecos, que es la peor
    /// forma de estar mal.
    pub(crate) fn nueva(w: i32, h: i32) -> Option<Superficie> {
        if w <= 0 || h <= 0 {
            return None;
        }
        let bytes = (w as u64) * (h as u64) * 4;
        let marcos = bytes.div_ceil(crate::ring0::mm::PAGE);
        let base = crate::ring0::mm::phys::alloc_frames_contig(marcos)?;
        let px = crate::ring0::mm::phys_to_virt(base) as *mut u32;
        Some(Superficie { px, w, h, base, marcos })
    }

    /// **Vuelca la superficie a la pantalla y la ensena.** Una copia, una
    /// barrera.
    ///
    /// Va fila a fila y no de un tiron porque el `stride` del framebuffer puede
    /// ser mayor que el ancho visible: copiar seguido correria la imagen un
    /// poco mas en cada fila y la escena saldria inclinada.
    pub(crate) fn volcar(&self) {
        let fb = unsafe { crate::info::FB_ADDR as *mut u32 };
        let stride = unsafe { crate::info::FB_STRIDE as usize };
        let alto = unsafe { crate::info::FB_HEIGHT } as i32;
        let ancho = unsafe { crate::info::FB_WIDTH } as i32;
        if fb.is_null() {
            return;
        }
        let filas = self.h.min(alto);
        let cols = (self.w.min(ancho)) as usize;
        for y in 0..filas {
            let origen = unsafe { self.px.add((y as usize) * self.w as usize) };
            let destino = unsafe { fb.add((y as usize) * stride) };
            for x in 0..cols {
                unsafe { destino.add(x).write_volatile(origen.add(x).read()) };
            }
        }
        wc_flush();
    }

    /// Devuelve los marcos. Sin esto, cada arranque se queda ocho megas.
    pub(crate) fn liberar(self) {
        for n in 0..self.marcos {
            crate::ring0::mm::phys::free_frame(self.base + n * crate::ring0::mm::PAGE);
        }
    }
}

impl Lienzo for Superficie {
    fn recorte(&self) -> Recorte {
        Recorte::nuevo(0, 0, self.w, self.h)
    }

    /// Escritura normal, no `write_volatile`: esto es RAM y el compilador puede
    /// agrupar y vectorizar todo lo que quiera. Es la mitad de la ventaja.
    fn rect_dentro(&mut self, r: Recorte, color: Color) {
        for y in r.y0..r.y1 {
            let fila = (y as usize) * self.w as usize;
            for x in r.x0..r.x1 {
                unsafe { *self.px.add(fila + x as usize) = color };
            }
        }
    }
}

/// **Un lienzo que apaga hacia negro lo que le pasa por encima.**
///
/// Es el fundido final de la intro --*"el gato toma el control y todo se vuelve
/// negro"*-- convertido en una capa en vez de en una multiplicacion repetida en
/// cada sitio que pinta.
///
/// # Por que asi y no como estaba
///
/// Estaba escrito dentro del callback de la ciudad: `mezcla(color, NEGRO,
/// f.negro, 255)` justo antes de rellenar. Y estaba tambien en el aura, y en el
/// gato, y en el kanji, y en el titulo, y en el subtitulo, y en las dos reglas
/// -- **ocho sitios repitiendo la misma cuenta**, que es ocho sitios donde se
/// puede olvidar. En el aura, de hecho, se olvidaba a medias: el fondo se
/// apagaba y el tinte no.
///
/// Envolviendo el lienzo, quien dibuja no se entera de que hay un fundido y el
/// fundido no se entera de que hay una ciudad. Y se pueden apilar: manana un
/// tinte, un flash o un recorte de ventana son otra capa igual.
pub(crate) struct Apagado<'a, L: Lienzo + ?Sized> {
    dentro: &'a mut L,
    /// Cuanto negro por encima, 0..255. Con `0` no hace nada.
    negro: u32,
}

impl<'a, L: Lienzo + ?Sized> Apagado<'a, L> {
    pub(crate) fn nuevo(dentro: &'a mut L, negro: u32) -> Self {
        Apagado { dentro, negro }
    }
}

impl<L: Lienzo + ?Sized> Lienzo for Apagado<'_, L> {
    fn recorte(&self) -> Recorte {
        self.dentro.recorte()
    }

    fn rect_dentro(&mut self, r: Recorte, color: Color) {
        // El recorte ya lo hizo el trait contra `recorte()`, que es el del
        // lienzo de dentro: pasarlo directo a `rect_dentro` no se salta nada.
        let c = if self.negro == 0 {
            color
        } else {
            bmo_dibujo::mezclar(NEGRO, color, self.negro, 255)
        };
        self.dentro.rect_dentro(r, c);
    }
}

/// Negro puro. El final del fundido.
const NEGRO: Color = 0xFF000000;

impl Lienzo for Pantalla {
    fn recorte(&self) -> Recorte {
        Recorte::nuevo(0, 0, self.w, self.h)
    }

    /// Escribe el rectangulo. **Sin comprobar nada, y eso es correcto.**
    ///
    /// Llega garantizado no vacio y dentro de `recorte()`, que es la pantalla
    /// visible. De ahi que los `as usize` no puedan desbordar y que no haya ni
    /// un `if`: toda la decision se tomo arriba, una vez, en `bmo-dibujo`.
    fn rect_dentro(&mut self, r: Recorte, color: Color) {
        for y in r.y0..r.y1 {
            let fila = (y as usize) * self.stride;
            for x in r.x0..r.x1 {
                unsafe { self.fb.add(fila + x as usize).write_volatile(color) };
            }
        }
    }
}
