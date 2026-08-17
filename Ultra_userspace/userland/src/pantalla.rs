//! La PANTALLA: framebuffer, doble bufer, dibujo y letras.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

// -- La pantalla ---------------------------------------------------------

/// La pantalla, ya mapeada en este proceso.
///
/// No hay un `dibujar()` que cruce el anillo, y no lo va a haber: el
/// framebuffer **es memoria de este proceso**. `lienzo` es un puntero de verdad
/// y escribir en el es un `mov`. Ese es el trato entero de `KIND_FRAMEBUFFER`
/// -- el kernel contesta cuatro preguntas al arrancar y despues se aparta.
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
/// === Lo sucio, y por que una caja y no la pantalla entera ===
///
/// Volcar 8 MB por fotograma contradiria la regla que ya estaba escrita aqui:
/// *lo que se repinta en un bucle es el DANO, no la pantalla*. Asi que se lleva
/// la **caja envolvente** de todo lo escrito desde el ultimo volcado y solo se
/// copia eso.
///
/// El precio, dicho: una caja **no** son varias regiones. Tocar la esquina de
/// arriba y la de abajo da una caja que las contiene a las dos, o sea casi todo.
/// Es el caso peor y sigue siendo mucho mejor que volcar siempre entero; si
/// algun dia se nota, lo que toca es una lista corta de rectangulos, no
/// abandonar la caja.
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
    /// Caja envolvente de lo escrito desde el ultimo volcado, `(x0,y0,x1,y1)`
    /// con `x1`/`y1` exclusivos. Vacia cuando `x0 >= x1`.
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
    /// Quien llama decide si lo dice por la consola. Aqui no se decide eso.
    pub fn activar_doble_bufer(&mut self) -> bool {
        if self.lienzo != self.panel {
            return true; // ya esta
        }
        // El lienzo tiene el MISMO stride que el panel, no el mismo ancho: asi
        // el indice `y*stride + x` vale para los dos y no hay dos aritmeticas
        // que mantener en paralelo. Que sobren unos pixeles por fila es mas
        // barato que una segunda forma de calcular la misma direccion.
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

    /// **Apunta que esta region ha cambiado.** Sin esto, lo pintado se queda en
    /// el lienzo y no llega nunca al panel.
    ///
    /// Las primitivas de dibujo de aqui lo hacen solas. Es publico porque
    /// [`Self::punto_sin_comprobar`] no marca --es el camino caliente y no va a
    /// llevar esto dentro--, asi que quien la use tiene que marcar el.
    #[inline]
    pub fn marcar(&self, x: u32, y: u32, ancho: u32, alto: u32) {
        if ancho == 0 || alto == 0 {
            return;
        }
        let nx1 = (x + ancho).min(self.ancho);
        let ny1 = (y + alto).min(self.alto);
        if x >= nx1 || y >= ny1 {
            return;
        }
        // ** VARIAS CAJAS Y NO UNA. Ver `crate::sin_gpu::sucio`.
        //
        // [!] Y esa carpeta se llama asi por algo: todo esto **desaparece con el
        // page flip**. Trocear la copia es trabajo que la CPU hace porque no hay
        // quien mueva la direccion del escaner; no es como deberia quedarse.
        //
        // Con una sola, dos cambios en esquinas opuestas --el cursor donde
        // estaba y donde esta-- unian a la pantalla ENTERA: 384 pixeles reales
        // convertidos en 2.073.600 copiados, cada fotograma, a memoria
        // write-combining. Eso era a la vez la lentitud y el parpadeo.
        let mut s = self.sucio.get();
        s.marcar((x, y, nx1, ny1));
        self.sucio.set(s);
    }

    /// Un pixel, sin comprobar nada. Es el camino caliente de un compositor y
    /// no va a llevar una rama dentro.
    ///
    /// # Safety
    /// `x < stride`, `y < alto`, y **quien llame tiene que marcar la region**
    /// con [`Self::marcar`] o lo pintado no llegara al panel. Las primitivas de
    /// aqui lo hacen; de fuera no lo llama nadie.
    #[inline(always)]
    pub unsafe fn punto_sin_comprobar(&self, x: u32, y: u32, color: u32) {
        self.lienzo
            .add((y as usize) * (self.stride as usize) + x as usize)
            .write_volatile(color);
    }

    /// Un pixel, comprobando. Fuera de la pantalla no hace nada.
    #[inline]
    pub fn punto(&self, x: u32, y: u32, color: u32) {
        if x < self.ancho && y < self.alto {
            unsafe { self.punto_sin_comprobar(x, y, color) };
            self.marcar(x, y, 1, 1);
        }
    }

    /// Rellenar la pantalla entera.
    ///
    /// Solo para el primer pintado. Repetirlo por fotograma seria recorrer
    /// varios MB de memoria sin cache: un pase de diapositivas. Lo que se
    /// repinta en un bucle es el DANO, no la pantalla.
    pub fn limpiar(&self, color: u32) {
        // * El tope es el MENOR de los dos, y esto no es prudencia de mas.
        //
        // `pixeles()` mide el area que mapeo el KERNEL, que puede ser mas
        // grande que `stride x alto` (redondeos, padding del firmware). El
        // lienzo mide exactamente `stride x alto`. Recorrer el primero
        // escribiendo en el segundo se sale del bloque de `KIND_MEMORIA` y pisa
        // lo que haya detras -- y como el bloque se pidio contiguo y el
        // asignador da lo siguiente que encuentre, "lo que haya detras" es
        // memoria de alguien.
        //
        // El minimo protege en los dos sentidos, que es la razon de que sea un
        // minimo y no un caso especial.
        let n = self.pixeles().min((self.stride as usize) * (self.alto as usize));
        for i in 0..n {
            unsafe { self.lienzo.add(i).write_volatile(color) };
        }
        self.marcar(0, 0, self.ancho, self.alto);
    }

    /// **Empuja a la pantalla lo que se acaba de pintar.**
    ///
    /// * Esto es la otra mitad del write-combining, y sin ella el WC no es una
    /// optimizacion: es un bug.
    ///
    /// Con memoria WC el CPU **acumula** las escrituras en un bufer y las suelta
    /// cuando se llena o cuando algo le obliga. Eso es lo que hace que pintar
    /// sea rapido -- y tambien lo que hace que lo pintado **no llegue** al panel
    /// si el fotograma acaba con el bufer a medias. El escaner de video lee la
    /// memoria, no el bufer.
    ///
    /// El sintoma, dicho por quien lo sufrio: *"cuando muevo el raton tengo que
    /// apuntar bien para que me pinte las escrituras"*. No era el raton: era que
    /// mover el raton genera mas escrituras, el bufer se llenaba, y al vaciarse
    /// aparecia de golpe el texto que se habia tecleado antes.
    ///
    /// `sfence` ordena: nada de lo de despues se ve antes que lo de antes. Es
    /// una instruccion, se hace **una vez por fotograma**, y convierte el WC en
    /// lo que promete.
    ///
    /// * Con doble bufer esto **ademas vuelca**: primero la copia del lienzo al
    /// panel, despues la barrera. Ese orden es el unico que sirve -- la barrera
    /// tiene que cerrar las escrituras del volcado, no las de antes.
    #[inline]
    pub fn vaciar(&self) {
        self.volcar();
        unsafe { core::arch::asm!("sfence", options(nostack, preserves_flags)) };
    }

    /// **Copia al panel lo sucio del lienzo**, y deja la caja vacia.
    ///
    /// [!!] **ESTA FUNCION ENTERA ES PROVISIONAL.** Con un driver de pantalla no
    /// se copia nada: se cambia la direccion que lee el escaner de video y ya
    /// esta (page flip). Es el escalon 8 de `docs/identidad/LA_RAM.md`, y su bloqueante
    /// es que tras `ExitBootServices` el GOP no existe.
    ///
    /// Mover pixeles con el CPU **es trabajo de la GPU hecho por quien no
    /// toca**. Funciona, y eso no lo convierte en la forma correcta.
    ///
    /// Sin doble bufer no hay nada que copiar: lo pintado ya esta en el panel.
    /// Igual se limpia la caja, porque llevarla puesta sin volcar seria mentir
    /// sobre lo que queda pendiente.
    pub fn volcar(&self) {
        let sucias = self.sucio.replace(crate::sin_gpu::sucio::Sucias::nueva());
        if self.lienzo == self.panel || sucias.vacia() {
            return;
        }
        let stride = self.stride as usize;
        for &(x0, y0, x1, y1) in sucias.cajas() {
        let ancho = (x1 - x0) as usize;
        let mut fila = y0 as usize;
        while fila < y1 as usize {
            let off = fila * stride + x0 as usize;
            // * De DOS pixeles por escritura cuando se puede.
            //
            // El panel es write-combining: el CPU junta escrituras seguidas en
            // rafagas de 64 bytes, y le cuesta menos juntar ocho de 8 bytes que
            // dieciseis de 4. Es la misma cantidad de datos con la mitad de
            // operaciones, y no necesita SSE ni alineacion especial mas alla de
            // la que ya tiene un framebuffer (base de pagina, pixeles de 4 B).
            //
            // El pixel suelto del final se copia como siempre. Un bucle que
            // "casi" cubre el ancho es peor que uno que lo cubre.
            let mut i = 0usize;
            unsafe {
                while i + 1 < ancho {
                    let a = self.lienzo.add(off + i).read() as u64;
                    let b = self.lienzo.add(off + i + 1).read() as u64;
                    (self.panel.add(off + i) as *mut u64).write_volatile(a | (b << 32));
                    i += 2;
                }
                while i < ancho {
                    let v = self.lienzo.add(off + i).read();
                    self.panel.add(off + i).write_volatile(v);
                    i += 1;
                }
            }
            fila += 1;
        }
        }

        // La cuenta, para poder contestar "hace falta una GPU?" con un numero
        // en vez de con una intuicion.
        // La cuenta sale de las cajas y no de un acumulador a mano: es el mismo
        // numero por construccion, y un acumulador que se pueda olvidar en una
        // rama es una estadistica que miente despacio.
        let bytes = sucias.pixeles() * 4;
        let v = self.volcado.get();
        // Las cajas se apuntan solo cuando este fotograma ES el peor: guardar
        // el maximo de las dos cosas por separado daria una pareja que nunca
        // ocurrio, y un numero que no paso no explica nada.
        let peor_ahora = bytes > v.peor;
        self.volcado.set(Volcado {
            fotogramas: v.fotogramas + 1,
            bytes: v.bytes + bytes,
            peor: v.peor.max(bytes),
            cajas: if peor_ahora { sucias.cajas().len() as u32 } else { v.cajas },
            modo: v.modo,
        });
    }

    /// Lo que lleva costado el volcado. Ver [`Volcado`] y [`Volcador`].
    pub fn volcado(&self) -> Volcado {
        let mut v = self.volcado.get();
        v.modo = if self.lienzo == self.panel { Volcador::Ninguno } else { Volcador::Directo };
        v
    }

    /// **Asegura que lo escrito se puede LEER.** Llamar antes de [`Self::read`].
    ///
    /// * Existe por el Ep. 25, y hace dos cosas distintas segun donde se dibuje:
    ///
    /// - **Con doble bufer**: nada. El lienzo es RAM normal y cacheada, asi que
    ///   una lectura ve lo que se acaba de escribir. El problema no existe.
    /// - **Sin doble bufer**: `sfence`. Se esta leyendo memoria WC, y una
    ///   lectura de WC **no esta ordenada** contra las escrituras pendientes en
    ///   el bufer: sin barrera devuelve la pantalla de hace un fotograma.
    ///
    /// Que sea un no-op en el camino bueno es justo la gracia: el doble bufer no
    /// arregla el ghosting, lo hace **imposible**.
    #[inline]
    pub fn sincronizar_lectura(&self) {
        if self.lienzo == self.panel {
            unsafe { core::arch::asm!("sfence", options(nostack, preserves_flags)) };
        }
    }

    /// Que hay AHORA en un pixel. Fuera de la pantalla, negro.
    ///
    /// * Se lee del LIENZO, que es donde se ha dibujado. Eso es lo que permite
    /// dibujar el cursor del raton **encima de cualquier cosa**: se guarda lo
    /// que habia debajo y se devuelve al moverlo. Sin esto hay que preguntarle a
    /// un modelo de la escena que deberia haber, y ese modelo se queda corto en
    /// cuanto aparece una ventana que no conoce: el cursor deja agujeros con el
    /// color del fondo por donde pasa.
    ///
    /// Sin doble bufer esto lee memoria de video, que es cara y ademas exige
    /// [`Self::sincronizar_lectura`] antes. Con doble bufer es RAM normal.
    #[inline]
    pub fn read(&self, x: u32, y: u32) -> u32 {
        if x >= self.ancho || y >= self.alto {
            return 0;
        }
        unsafe {
            self.lienzo
                .add((y as usize) * (self.stride as usize) + x as usize)
                .read_volatile()
        }
    }

    /// Un rectangulo, recortado a la pantalla. Es la unica primitiva de dibujo
    /// que hay, y con ella se hace un escritorio entero: fondo, barra,
    /// ventanas, bordes. Lo demas son estas mismas llamadas puestas en orden.
    pub fn rect(&self, x: u32, y: u32, ancho: u32, alto: u32, color: u32) {
        let x1 = (x.saturating_add(ancho)).min(self.ancho);
        let y1 = (y.saturating_add(alto)).min(self.alto);
        // Se marca UNA vez, con las medidas ya recortadas, en vez de un pixel
        // por vuelta: un rectangulo de pantalla completa son millones de
        // llamadas a `marcar` que darian exactamente la misma caja.
        if x < x1 && y < y1 {
            self.marcar(x, y, x1 - x, y1 - y);
        }
        let mut fila = y;
        while fila < y1 {
            let mut col = x;
            while col < x1 {
                unsafe { self.punto_sin_comprobar(col, fila, color) };
                col += 1;
            }
            fila += 1;
        }
    }
}

/// La caja vacia: `x0 >= x1`, asi que no hay nada que volcar.

/// **Como llegan los pixeles del lienzo al panel.**
///
/// === * Esta es la costura donde entra una GPU ===
///
/// La idea que la motivo era partir el compositor en `gui_CPU.bex` y
/// `gui_GPU.bex`. Eso seria **bifurcar antes de que exista la segunda
/// implementacion**: cada arreglo habria que hacerlo dos veces, que es
/// exactamente el problema que resolvio `refactor(abi): la disposicion de
/// agregados estaba escrita TRES veces`.
///
/// El corte correcto es por CAPA, y hay tres:
///
/// ```text
///   POLITICA     que ventana existe, donde va, quien tiene el foco
///                -> no cambia NUNCA con una GPU. Vive en el compositor.
///   DIBUJO       llenar el lienzo: punto, rect, texto
///                -> es CPU SIEMPRE. Una app que pinta su superficie en RAM
///                   la pinta con el CPU, tenga GPU o no.
///   VOLCADO      mover el rectangulo sucio del lienzo al panel
///                -> AQUI, y solo aqui, una GPU cambia algo.
/// ```
///
/// Por eso el contrato es esto y no un trait `Lienzo` entero: `punto` esta en
/// el camino caliente y meterle una llamada indirecta costaria en cada pixel
/// para no ganar nada. `volcar` se llama **una vez por fotograma**, asi que
/// aqui una rama no se nota -- y es donde esta el coste de verdad.
///
/// === Y antes de comprar una tarjeta, MEDIR ===
///
/// La caja de sucio ya evita casi todo el trabajo: escribir una letra vuelca
/// unos pocos KiB, no la pantalla. Ver [`Pantalla::volcado`] -- el numero va
/// primero, la tarjeta despues.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Volcador {
    /// No hay doble bufer: lo pintado ya esta en el panel. No hay nada que
    /// mover, y decirlo con su nombre vale mas que un `if` suelto.
    Ninguno,
    /// Copia con escrituras normales, de 8 en 8 bytes cuando se puede.
    ///
    /// Es lo que hay hoy y lo que corre en el Ryzen. Dos pixeles por
    /// escritura: el framebuffer es write-combining y agrupa mejor cuantas
    /// menos escrituras sueltas reciba.
    Directo,
}

/// Lo que ha costado el volcado, para poder **perfilar antes de comprar nada**.
///
/// No es telemetria de adorno: la pregunta "hace falta una GPU?" solo se puede
/// contestar con estos dos numeros. Si los bytes por fotograma son pocos, una
/// tarjeta no compra nada y cuesta un ano de trabajo.
#[derive(Clone, Copy)]
pub struct Volcado {
    /// Fotogramas con algo que volcar. Los que no cambian nada no cuentan:
    /// promediar con ellos esconderia el caso caro.
    pub fotogramas: u64,
    /// Bytes movidos del lienzo al panel, en total.
    pub bytes: u64,
    /// El fotograma mas caro visto. **El peor caso importa mas que la media**:
    /// un tiron se nota, y una media buena lo esconde.
    pub peor: u64,
    /// ** CAJAS SUCIAS DEL PEOR FOTOGRAMA, y es el numero que dice si el
    /// arreglo del 2026-08-12 sirvio de algo.
    ///
    /// Con la caja unica de antes esto valdria SIEMPRE 1, y `peor` seria la
    /// pantalla entera en cuanto dos cosas cambiaran lejos. Si en metal sale
    /// `cajas 2` o `3` con un `peor` pequeno, el troceado esta trabajando. Si
    /// sale `cajas 1` con un `peor` de 8 MB, degenero -- y entonces el
    /// sospechoso es `COSTE_DE_UNA_CAJA`, no el volcado.
    pub cajas: u32,
    pub modo: Volcador,
}

// -- Las letras ----------------------------------------------------------

/// La fuente 8x16 de BMO, la MISMA que pinta el kernel.
///
/// ## Por que esta copiada aqui y no se pide por syscall
///
/// La alternativa era una operacion `DIBUJAR_TEXTO` sobre el framebuffer, y
/// eso es exactamente la linea que `KIND_FRAMEBUFFER` existe para no cruzar: el
/// kernel contesta cuatro preguntas y se aparta. Si Ring 0 dibujara letras
/// tendria que saber de tipografia, de kerning y de colores -- decisiones de
/// aspecto, ninguna suya. Es el mismo argumento que dejo el cursor en Ring 3.
///
/// Asi que aqui hay 4 KiB de tabla duplicada. Sale del mismo generador
/// (`toolchain/tools/fontgen`), asi que no son dos fuentes que puedan
/// divergir: son dos copias de una, y regenerar actualiza las dos.
static FONT16: [[u8; 16]; 120] = include!("font16_data.rs");
static FONT_EXTRA: [u8; 25] = include!("font16_extra.rs");
const ASCII_GLYPHS: usize = 95;

/// Ancho y alto de un glifo, en pixeles. El avance horizontal ES el ancho: la
/// fuente ya trae su propio espaciado dentro del mapa de bits.
pub const GLIFO_ANCHO: u32 = 8;
pub const GLIFO_ALTO: u32 = 16;

/// Byte -> indice de glifo. ASCII directo; para el espanol (n, a, ...) se
/// busca el byte Latin-1 en la tabla de extras.
///
/// **Latin-1 y no UTF-8, igual que en Ring 0.** Un caracter es UN byte, asi el
/// teclado, la caja y la fuente hablan el mismo idioma sin decodificador de por
/// medio. Un `&str` de Rust es UTF-8, asi que una `n` escrita en el codigo
/// fuente llega como dos bytes y no se dibuja: para eso estan `texto_bytes` y
/// el hecho de que lo que se teclea ya viene en Latin-1 del kernel.
fn indice_glifo(c: u8) -> Option<usize> {
    if (32..=126).contains(&c) {
        return Some(c as usize - 32);
    }
    let mut i = 0;
    while i < FONT_EXTRA.len() {
        if FONT_EXTRA[i] == c {
            return Some(ASCII_GLYPHS + i);
        }
        i += 1;
    }
    None
}

impl Pantalla {
    /// Un caracter. Solo pinta los pixeles encendidos: el fondo se respeta,
    /// que es lo que permite escribir encima de lo que ya hay sin recuadros.
    pub fn glifo(&self, x: u32, y: u32, c: u8, color: u32) {
        let idx = match indice_glifo(c) {
            Some(i) => i,
            None => return,
        };
        let g = &FONT16[idx];
        for (fila, &bits) in g.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            for col in 0..8u32 {
                if bits & (0x80 >> col) != 0 {
                    self.punto(x + col, y + fila as u32, color);
                }
            }
        }
    }

    /// Un caracter AMPLIADO por un entero: cada pixel del glifo pasa a ser un
    /// cuadrado de `escala`.
    ///
    /// Entero y con `rect`, no interpolado: ampliar por 4 un glifo de 8x16 da
    /// bloques limpios de 32x64, y esa estetica es la que tiene esta maquina.
    /// Una interpolacion pediria coma flotante, un buffer intermedio y un gusto
    /// que no es el de aqui -- y con la misma fuente que ya esta cargada.
    pub fn glifo_escala(&self, x: u32, y: u32, c: u8, color: u32, escala: u32) {
        if escala <= 1 {
            self.glifo(x, y, c, color);
            return;
        }
        let idx = match indice_glifo(c) {
            Some(i) => i,
            None => return,
        };
        let g = &FONT16[idx];
        for (fila, &bits) in g.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            for col in 0..8u32 {
                if bits & (0x80 >> col) != 0 {
                    self.rect(x + col * escala, y + fila as u32 * escala, escala, escala, color);
                }
            }
        }
    }

    /// Un `&str` ampliado. Devuelve la x donde acabo.
    pub fn texto_escala(&self, x: u32, y: u32, s: &str, color: u32, escala: u32) -> u32 {
        let mut cx = x;
        for &c in s.as_bytes() {
            self.glifo_escala(cx, y, c, color, escala);
            cx += GLIFO_ANCHO * escala;
        }
        cx
    }

    /// Lo que ocupa un texto ampliado, para poder centrarlo sin adivinar.
    pub fn ancho_escala(s: &str, escala: u32) -> u32 {
        s.len() as u32 * GLIFO_ANCHO * escala
    }

    /// Una tira de bytes Latin-1. Devuelve la x donde acabo, para encadenar.
    pub fn texto_bytes(&self, x: u32, y: u32, s: &[u8], color: u32) -> u32 {
        let mut cx = x;
        for &c in s {
            self.glifo(cx, y, c, color);
            cx += GLIFO_ANCHO;
        }
        cx
    }

    /// Un `&str`. Los bytes que no sean ASCII se saltan en vez de salir como
    /// basura: un literal con acentos viene en UTF-8 y esta fuente es Latin-1.
    pub fn texto(&self, x: u32, y: u32, s: &str, color: u32) -> u32 {
        self.texto_bytes(x, y, s.as_bytes(), color)
    }
}

