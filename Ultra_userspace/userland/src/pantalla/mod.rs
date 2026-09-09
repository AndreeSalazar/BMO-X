//! La PANTALLA: framebuffer, doble bufer, dibujo y letras.
//!
//! # EL LETRERO DE LA CARPETA -- y el corte lo eligio el cuello de botella
//!
//! Esto era `pantalla.rs`, 623 lineas, y se partio en carriles el **2026-09-09**
//! mientras se buscaba por que un fotograma costaba lo que costaba. El corte no
//! salio de contar lineas: salio de que **las tres mitades tienen tres precios
//! distintos si se equivocan**, y estaban en el mismo fichero sin decirlo.
//!
//! ```text
//!    roja.rs      MOVER PIXELES     escribe memoria cruda con la cuenta en un
//!                                   registro -> [cuesta] MAQUINA
//!    amarilla.rs  LA CONTABILIDAD   que cambio y lo que costo. No mueve nada,
//!                                   pero le dice al rojo CUANTO -> APARATO
//!    verde.rs     LAS LETRAS        `punto` encima de `punto`. Si falla, se ve
//!                                   -> NADA
//! ```
//!
//! *** Y la prueba de que el corte es el bueno estaba escrita **dentro del
//! propio fichero desde agosto**: la cabecera de `Volcador` ya decia que el
//! compositor tiene tres capas --POLITICA, DIBUJO, VOLCADO-- y que *"solo en el
//! volcado una GPU cambia algo"*. Los carriles son esas capas con su semaforo
//! puesto. El fichero llevaba meses describiendo su propio corte.
//!
//! ## Que se queda aqui, y por que
//!
//! La estructura, el reclamo de la capability y `punto`. Son lo que las tres
//! mitades comparten: sin los punteros no hay nada que mover, contar ni pintar.
//! Un modulo hijo de Rust ve lo privado de su padre, asi que `sucio` y `volcado`
//! siguen siendo privados y los tres carriles los alcanzan sin abrirlos al
//! resto del mundo.
//!
//! [!] `punto_sin_comprobar` se queda aqui **a proposito**, aunque escriba
//! memoria cruda: es el camino caliente, no lleva cuenta ninguna --escribe UN
//! pixel-- y meterlo en el rojo obligaria a que el verde cruzara de carril en
//! cada glifo. Lo que lo hace peligroso no es lo que escribe: es que **no
//! marca**, y eso es un contrato, que es por lo que va en su `# Safety`.

use crate::*;

mod amarilla;
mod roja;
mod verde;

pub use amarilla::{Volcado, Volcador};
pub use verde::{GLIFO_ALTO, GLIFO_ANCHO};

// -- La pantalla ---------------------------------------------------------

/// La pantalla, ya mapeada en este proceso.
///
/// No hay un `dibujar()` que cruce el anillo, y no lo va a haber: el
/// framebuffer **es memoria de este proceso**. `lienzo` es un puntero de verdad
/// y escribir en el es un `mov`. Ese es el trato entero de `KIND_FRAMEBUFFER`
/// -- el kernel contesta cuatro preguntas al arrancar y despues se aparta.
///
/// ** Y eso ya es lo que otros sistemas llaman *kernel bypass*: la puerta se
/// paga UNA vez, al reclamar, y despues no hay puerta. Lo que queda de coste no
/// es el sistema operativo: es el bus hasta la VRAM. Ver `roja.rs`.
///
/// === * EL DOBLE BUFER ===
///
/// Se dibuja en **`lienzo`** y se vuelca a **`panel`**. Sin doble bufer los dos
/// punteros son el mismo y todo funciona como siempre; con el, `lienzo` es RAM
/// normal pedida con [`Memoria`] y `panel` es la memoria de video.
///
/// **Por que**, en orden de lo que mas dolia:
///
/// 1. **Mata el ghosting por construccion.** El framebuffer esta en
///    write-combining, y leer memoria WC no devuelve lo que acabas de escribir
///    (Ep. 25 de `BITACORA.md`). El *save-under* del cursor es una lectura, y
///    era la unica del programa. Leyendo del lienzo el problema **no existe**:
///    es RAM normal, cacheada y coherente consigo misma. Un `sfence` bien puesto
///    lo arregla; esto lo hace imposible.
/// 2. **Mata el tearing.** El escaner de video ya no ve un fotograma a medio
///    pintar: ve el anterior hasta que llega el volcado.
/// 3. **Pintar es mas rapido.** Escribir en RAM cacheada no se parece a escribir
///    en memoria de video, ni con WC. El coste se paga una vez, en el volcado, y
///    en la forma que al bus le gusta: rafagas seguidas.
/// 4. **Es el prerequisito de las superficies.** El dia que una ventana sea de
///    otro proceso, lo que ese proceso pinta va a un bufer y alguien lo compone.
///    Esto es esa pieza, con un solo cliente todavia.
///
/// **Y solo es posible desde que existe `KIND_MEMORIA`**: hasta entonces un
/// proceso recibia su imagen y 64 KiB de pila, y un bufer de pantalla son ~8 MB.
///
/// === Lo sucio, y por que varias cajas y no la pantalla entera ===
///
/// Volcar 8 MB por fotograma contradiria la regla que ya estaba escrita aqui:
/// *lo que se repinta en un bucle es el DANO, no la pantalla*. Asi que se llevan
/// hasta ocho cajas de lo escrito desde el ultimo volcado y solo se copia eso.
/// Ver [`crate::sin_gpu::sucio`], y `amarilla.rs` para la contabilidad.
pub struct Pantalla {
    /// Handle de la capability. Hace falta para preguntarle cosas.
    pub cap: u64,
    /// **Donde se DIBUJA.** Con doble bufer es RAM normal; sin el, el panel.
    pub lienzo: *mut u32,
    /// **El framebuffer de verdad.** Igual que `lienzo` si no hay doble bufer.
    pub panel: *mut u32,
    pub ancho: u32,
    pub alto: u32,
    /// En PIXELES, no en bytes: es el mismo numero que usa el kernel.
    pub stride: u32,
    pub formato: u32,
    pub bytes: u64,
    /// Las regiones escritas desde el ultimo volcado.
    ///
    /// Es una `Cell` porque dibujar toma `&self` en todo el compositor y
    /// cambiarlo a `&mut self` obligaria a reescribir cada llamada para ganar
    /// nada: esto es un programa de un solo hilo y `Cell` es exactamente la
    /// herramienta para eso.
    sucio: core::cell::Cell<crate::sin_gpu::sucio::Sucias>,
    /// Lo que ha costado mover pixeles. Ver [`Volcado`]: es el numero que
    /// decide si una GPU compra algo o solo cuesta un ano.
    volcado: core::cell::Cell<Volcado>,
}

impl Pantalla {
    /// Reclamarla. Solo un proceso puede tenerla a la vez; el kernel deja de
    /// dibujar mientras dure, y la recupera solo si este proceso muere.
    pub fn claim() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_FRAMEBUFFER_CLAIM, 0, 0, 0).valor()?;
        let base = invoke(cap, FB_OP_BASE, 0, 0, 0).valor()?;
        let dims = invoke(cap, FB_OP_DIMS, 0, 0, 0).valor()?;
        let stride = invoke(cap, FB_OP_STRIDE, 0, 0, 0).valor()?;
        let bytes = invoke(cap, FB_OP_BYTES, 0, 0, 0).valor()?;
        Some(Self {
            cap,
            lienzo: base as *mut u32,
            panel: base as *mut u32,
            ancho: (dims >> 32) as u32,
            alto: dims as u32,
            stride: (stride >> 32) as u32,
            formato: stride as u32,
            bytes,
            sucio: core::cell::Cell::new(crate::sin_gpu::sucio::Sucias::nueva()),
            volcado: core::cell::Cell::new(Volcado {
                fotogramas: 0,
                bytes: 0,
                peor: 0,
                ultimo: 0,
                cajas: 0,
                modo: Volcador::Ninguno,
            }),
        })
    }

    /// * **Soltarla y seguir vivo.** Consume la `Pantalla`, que es el punto.
    ///
    /// Tras esto el kernel vuelve a tener la pantalla, las paginas del
    /// framebuffer **se desmapean de este proceso** y el handle se revoca. Que
    /// tome `self` por valor no es estilo: si devolviera `&self`, quedaria una
    /// `Pantalla` en manos del programa con un puntero a memoria ya desmapeada,
    /// y el primer pixel que escribiera seria un fallo de pagina. Aqui el
    /// sistema de tipos hace de guardia.
    ///
    /// Para recuperarla, [`Pantalla::claim`] otra vez -- y hay que **repintar
    /// entero**: mientras no era suya pudo pintar otro.
    ///
    /// Devuelve `false` si no era el dueno, en vez de fingir que la solto.
    pub fn release(self) -> bool {
        invoke(CURRENT_TASK, OP_PANTALLA_SOLTAR, 0, 0, 0).valor().is_some()
    }

    /// **Pide el bufer de fondo y empieza a dibujar en el.**
    ///
    /// Devuelve `false` si no lo consigue, y entonces **no pasa nada**: se
    /// sigue dibujando directamente en el panel, que es lo que se hacia antes.
    /// Eso no es un adorno defensivo -- el bloque son ~8 MB de RAM **contigua en
    /// fisico**, y si la memoria esta fragmentada el kernel lo rechaza con su
    /// motivo. Un compositor que se cayera por no conseguir una optimizacion
    /// seria peor que uno sin la optimizacion.
    ///
    /// [!] Y "no pasa nada" es MENTIRA a medias, que es justo lo que costo el
    /// 09-09: sin lienzo, cada `rect` y cada glifo van directos a la VRAM por
    /// PCIe y `read()` LEE de la VRAM. Cien veces mas caro. Por eso el modo se
    /// pinta en la barra --`SIN LIENZO`-- y no solo por la consola del arranque,
    /// que se va y la tapa el escritorio.
    ///
    /// Quien llama decide si lo dice por la consola. Aqui no se decide eso.
    pub fn activar_doble_bufer(&mut self) -> bool {
        if self.lienzo != self.panel {
            return true; // ya esta
        }
        // El lienzo tiene el MISMO stride que el panel, no el mismo ancho: asi
        // el indice `y*stride + x` vale para los dos y no hay dos aritmeticas
        // que mantener en paralelo. Que sobren unos pixeles por fila es mas
        // barato que una segunda forma de calcular la misma direccion.
        //
        // ** Y desde el 09-09 compra una segunda cosa: con el mismo stride, una
        // caja que ocupa la pantalla entera es UN solo tramo contiguo en los dos
        // lados, y el volcado completo se hace con una unica instruccion. Con
        // dos strides distintos habria que ir fila a fila siempre.
        let bytes = (self.stride as u64) * (self.alto as u64) * 4;
        let Some(m) = Memoria::request(bytes) else {
            return false;
        };
        self.lienzo = m.base() as *mut u32;
        // Lo que hay en el panel ahora mismo no esta en el lienzo: hasta el
        // primer volcado completo, los dos no dicen lo mismo. Se marca la
        // pantalla entera para que el primer `vaciar` los iguale.
        self.marcar(0, 0, self.ancho, self.alto);
        true
    }

    /// Se esta dibujando fuera de la pantalla de video?
    pub fn tiene_doble_bufer(&self) -> bool {
        self.lienzo != self.panel
    }

    /// Pixeles que caben en el area mapeada.
    #[inline]
    pub fn pixeles(&self) -> usize {
        (self.bytes / 4) as usize
    }

    /// Un pixel, sin comprobar nada. Es el camino caliente de un compositor y
    /// no va a llevar una rama dentro.
    ///
    /// # Safety
    /// `x < stride`, `y < alto`, y **quien llame tiene que marcar la region**
    /// con [`Pantalla::marcar`] o lo pintado no llegara al panel. Las primitivas
    /// de aqui lo hacen; de fuera no lo llama nadie.
    #[inline(always)]
    pub unsafe fn punto_sin_comprobar(&self, x: u32, y: u32, color: u32) {
        unsafe {
            self.lienzo
                .add((y as usize) * (self.stride as usize) + x as usize)
                .write_volatile(color)
        };
    }

    /// Un pixel, comprobando. Fuera de la pantalla no hace nada.
    #[inline]
    pub fn punto(&self, x: u32, y: u32, color: u32) {
        if x < self.ancho && y < self.alto {
            unsafe { self.punto_sin_comprobar(x, y, color) };
            self.marcar(x, y, 1, 1);
        }
    }

    /// **Un pixel comprobado, pero SIN marcar**: quien llama ya marco la caja.
    ///
    /// === *** EL SEGUNDO CUELLO DE BOTELLA, y no estaba en los pixeles ===
    ///
    /// `marcar` no es barato, y la razon es su tamano: `Sucias` son **136
    /// bytes** --ocho cajas de 16 mas la cuenta-- y vive en una `Cell`, asi que
    /// cada llamada hace `get()` y `set()`: **272 bytes copiados por pixel**.
    ///
    /// `rect` ya lo sabia y marca UNA vez con las medidas recortadas. `glifo`
    /// no: llamaba a `punto` por cada bit encendido de la fuente, y eso sale a
    ///
    /// ```text
    ///    la barra de tareas, un fotograma      ~110 letras, ~4.950 pixeles
    ///    lo que esos pixeles PINTAN                     19,3 KiB
    ///    lo que su contabilidad COPIA                1.315,0 KiB
    ///    ---------------------------------------------------------
    ///    razon papeleo / trabajo                          68 a 1
    /// ```
    ///
    /// ** Sesenta y ocho veces mas memoria movida para apuntar el trabajo que
    /// para hacerlo, y todo dentro del carril VERDE -- el que dice *"aqui no
    /// puede pasar nada malo"*. Y era verdad: no pasaba nada malo. Solo costaba.
    ///
    /// Con esto un glifo marca **una caja de 8x16** y pinta sus bits sueltos.
    ///
    /// [!] Sigue COMPROBANDO los limites, y eso es a proposito: lo caro era el
    /// papeleo, no la comparacion. Quitar tambien el recorte habria movido a
    /// `verde.rs` de `[cuesta] NADA` a `MAQUINA` para ahorrar dos `cmp` -- y un
    /// carril no cambia de color por dos instrucciones.
    #[inline]
    pub fn punto_ya_marcado(&self, x: u32, y: u32, color: u32) {
        if x < self.ancho && y < self.alto {
            unsafe { self.punto_sin_comprobar(x, y, color) };
        }
    }
}
