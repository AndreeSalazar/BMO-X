//! La PANTALLA: framebuffer, doble bufer, dibujo y letras.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

// ── La pantalla ─────────────────────────────────────────────────────────

/// La pantalla, ya mapeada en este proceso.
///
/// No hay un `dibujar()` que cruce el anillo, y no lo va a haber: el
/// framebuffer **es memoria de este proceso**. `lienzo` es un puntero de verdad
/// y escribir en él es un `mov`. Ése es el trato entero de `KIND_FRAMEBUFFER`
/// — el kernel contesta cuatro preguntas al arrancar y después se aparta.
///
/// ═══ ★ EL DOBLE BÚFER ═══
///
/// Se dibuja en **`lienzo`** y se vuelca a **`panel`**. Sin doble búfer los dos
/// punteros son el mismo y todo funciona como siempre; con él, `lienzo` es RAM
/// normal pedida con [`Memoria`] y `panel` es la memoria de vídeo.
///
/// **Por qué**, en orden de lo que más dolía:
///
/// 1. **Mata el ghosting por construcción.** El framebuffer está en
///    write-combining, y leer memoria WC no devuelve lo que acabas de escribir
///    (Ep. 25 de `BITACORA.md`). El *save-under* del cursor es una lectura, y
///    era la única del programa. Leyendo del lienzo el problema **no existe**:
///    es RAM normal, cacheada y coherente consigo misma. Un `sfence` bien puesto
///    lo arregla; esto lo hace imposible.
/// 2. **Mata el tearing.** El escáner de vídeo ya no ve un fotograma a medio
///    pintar: ve el anterior hasta que llega el volcado.
/// 3. **Pintar es más rápido.** Escribir en RAM cacheada no se parece a escribir
///    en memoria de vídeo, ni con WC. El coste se paga una vez, en el volcado, y
///    en la forma que al bus le gusta: ráfagas seguidas.
/// 4. **Es el prerequisito de las superficies.** El día que una ventana sea de
///    otro proceso, lo que ese proceso pinta va a un búfer y alguien lo compone.
///    Esto es esa pieza, con un solo cliente todavía.
///
/// **Y sólo es posible desde que existe `KIND_MEMORIA`**: hasta entonces un
/// proceso recibía su imagen y 64 KiB de pila, y un búfer de pantalla son ~8 MB.
///
/// ═══ Lo sucio, y por qué una caja y no la pantalla entera ═══
///
/// Volcar 8 MB por fotograma contradiría la regla que ya estaba escrita aquí:
/// *lo que se repinta en un bucle es el DAÑO, no la pantalla*. Así que se lleva
/// la **caja envolvente** de todo lo escrito desde el último volcado y sólo se
/// copia eso.
///
/// El precio, dicho: una caja **no** son varias regiones. Tocar la esquina de
/// arriba y la de abajo da una caja que las contiene a las dos, o sea casi todo.
/// Es el caso peor y sigue siendo mucho mejor que volcar siempre entero; si
/// algún día se nota, lo que toca es una lista corta de rectángulos, no
/// abandonar la caja.
pub struct Pantalla {
    /// Handle de la capability. Hace falta para preguntarle cosas.
    pub cap: u64,
    /// **Donde se DIBUJA.** Con doble búfer es RAM normal; sin él, el panel.
    pub lienzo: *mut u32,
    /// **El framebuffer de verdad.** Igual que `lienzo` si no hay doble búfer.
    pub panel: *mut u32,
    pub ancho: u32,
    pub alto: u32,
    /// En PÍXELES, no en bytes: es el mismo número que usa el kernel.
    pub stride: u32,
    pub formato: u32,
    pub bytes: u64,
    /// Caja envolvente de lo escrito desde el último volcado, `(x0,y0,x1,y1)`
    /// con `x1`/`y1` exclusivos. Vacía cuando `x0 >= x1`.
    ///
    /// Es una `Cell` porque dibujar toma `&self` en todo el compositor y
    /// cambiarlo a `&mut self` obligaría a reescribir cada llamada para ganar
    /// nada: esto es un programa de un solo hilo y `Cell` es exactamente la
    /// herramienta para eso.
    sucio: core::cell::Cell<(u32, u32, u32, u32)>,
    /// Lo que ha costado mover píxeles. Ver [`Volcado`]: es el número que
    /// decide si una GPU compra algo o sólo cuesta un año.
    volcado: core::cell::Cell<Volcado>,
}

impl Pantalla {
    /// Reclamarla. Sólo un proceso puede tenerla a la vez; el kernel deja de
    /// dibujar mientras dure, y la recupera solo si este proceso muere.
    pub fn reclamar() -> Option<Self> {
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
            sucio: core::cell::Cell::new(VACIO),
            volcado: core::cell::Cell::new(Volcado {
                fotogramas: 0,
                bytes: 0,
                peor: 0,
                modo: Volcador::Ninguno,
            }),
        })
    }

    /// **Pide el búfer de fondo y empieza a dibujar en él.**
    ///
    /// Devuelve `false` si no lo consigue, y entonces **no pasa nada**: se
    /// sigue dibujando directamente en el panel, que es lo que se hacía antes.
    /// Eso no es un adorno defensivo — el bloque son ~8 MB de RAM **contigua en
    /// físico**, y si la memoria está fragmentada el kernel lo rechaza con su
    /// motivo. Un compositor que se cayera por no conseguir una optimización
    /// sería peor que uno sin la optimización.
    ///
    /// Quien llama decide si lo dice por la consola. Aquí no se decide eso.
    pub fn activar_doble_bufer(&mut self) -> bool {
        if self.lienzo != self.panel {
            return true; // ya está
        }
        // El lienzo tiene el MISMO stride que el panel, no el mismo ancho: así
        // el índice `y*stride + x` vale para los dos y no hay dos aritméticas
        // que mantener en paralelo. Que sobren unos píxeles por fila es más
        // barato que una segunda forma de calcular la misma dirección.
        let bytes = (self.stride as u64) * (self.alto as u64) * 4;
        let Some(m) = Memoria::pedir(bytes) else {
            return false;
        };
        self.lienzo = m.base() as *mut u32;
        // Lo que hay en el panel ahora mismo no está en el lienzo: hasta el
        // primer volcado completo, los dos no dicen lo mismo. Se marca la
        // pantalla entera para que el primer `vaciar` los iguale.
        self.marcar(0, 0, self.ancho, self.alto);
        true
    }

    /// ¿Se está dibujando fuera de la pantalla de vídeo?
    pub fn tiene_doble_bufer(&self) -> bool {
        self.lienzo != self.panel
    }

    /// Píxeles que caben en el área mapeada.
    #[inline]
    pub fn pixeles(&self) -> usize {
        (self.bytes / 4) as usize
    }

    /// **Apunta que esta región ha cambiado.** Sin esto, lo pintado se queda en
    /// el lienzo y no llega nunca al panel.
    ///
    /// Las primitivas de dibujo de aquí lo hacen solas. Es público porque
    /// [`Self::punto_sin_comprobar`] no marca —es el camino caliente y no va a
    /// llevar esto dentro—, así que quien la use tiene que marcar él.
    #[inline]
    pub fn marcar(&self, x: u32, y: u32, ancho: u32, alto: u32) {
        if ancho == 0 || alto == 0 {
            return;
        }
        let (x0, y0, x1, y1) = self.sucio.get();
        let nx1 = (x + ancho).min(self.ancho);
        let ny1 = (y + alto).min(self.alto);
        if x >= nx1 || y >= ny1 {
            return;
        }
        self.sucio.set(if x0 >= x1 {
            (x, y, nx1, ny1)
        } else {
            (x0.min(x), y0.min(y), x1.max(nx1), y1.max(ny1))
        });
    }

    /// Un píxel, sin comprobar nada. Es el camino caliente de un compositor y
    /// no va a llevar una rama dentro.
    ///
    /// # Safety
    /// `x < stride`, `y < alto`, y **quien llame tiene que marcar la región**
    /// con [`Self::marcar`] o lo pintado no llegará al panel. Las primitivas de
    /// aquí lo hacen; de fuera no lo llama nadie.
    #[inline(always)]
    pub unsafe fn punto_sin_comprobar(&self, x: u32, y: u32, color: u32) {
        self.lienzo
            .add((y as usize) * (self.stride as usize) + x as usize)
            .write_volatile(color);
    }

    /// Un píxel, comprobando. Fuera de la pantalla no hace nada.
    #[inline]
    pub fn punto(&self, x: u32, y: u32, color: u32) {
        if x < self.ancho && y < self.alto {
            unsafe { self.punto_sin_comprobar(x, y, color) };
            self.marcar(x, y, 1, 1);
        }
    }

    /// Rellenar la pantalla entera.
    ///
    /// Sólo para el primer pintado. Repetirlo por fotograma sería recorrer
    /// varios MB de memoria sin caché: un pase de diapositivas. Lo que se
    /// repinta en un bucle es el DAÑO, no la pantalla.
    pub fn limpiar(&self, color: u32) {
        // ★ El tope es el MENOR de los dos, y esto no es prudencia de más.
        //
        // `pixeles()` mide el área que mapeó el KERNEL, que puede ser más
        // grande que `stride × alto` (redondeos, padding del firmware). El
        // lienzo mide exactamente `stride × alto`. Recorrer el primero
        // escribiendo en el segundo se sale del bloque de `KIND_MEMORIA` y pisa
        // lo que haya detrás — y como el bloque se pidió contiguo y el
        // asignador da lo siguiente que encuentre, "lo que haya detrás" es
        // memoria de alguien.
        //
        // El mínimo protege en los dos sentidos, que es la razón de que sea un
        // mínimo y no un caso especial.
        let n = self.pixeles().min((self.stride as usize) * (self.alto as usize));
        for i in 0..n {
            unsafe { self.lienzo.add(i).write_volatile(color) };
        }
        self.marcar(0, 0, self.ancho, self.alto);
    }

    /// **Empuja a la pantalla lo que se acaba de pintar.**
    ///
    /// ★ Esto es la otra mitad del write-combining, y sin ella el WC no es una
    /// optimización: es un bug.
    ///
    /// Con memoria WC el CPU **acumula** las escrituras en un búfer y las suelta
    /// cuando se llena o cuando algo le obliga. Eso es lo que hace que pintar
    /// sea rápido — y también lo que hace que lo pintado **no llegue** al panel
    /// si el fotograma acaba con el búfer a medias. El escáner de vídeo lee la
    /// memoria, no el búfer.
    ///
    /// El síntoma, dicho por quien lo sufrió: *"cuando muevo el ratón tengo que
    /// apuntar bien para que me pinte las escrituras"*. No era el ratón: era que
    /// mover el ratón genera más escrituras, el búfer se llenaba, y al vaciarse
    /// aparecía de golpe el texto que se había tecleado antes.
    ///
    /// `sfence` ordena: nada de lo de después se ve antes que lo de antes. Es
    /// una instrucción, se hace **una vez por fotograma**, y convierte el WC en
    /// lo que promete.
    ///
    /// ★ Con doble búfer esto **además vuelca**: primero la copia del lienzo al
    /// panel, después la barrera. Ese orden es el único que sirve — la barrera
    /// tiene que cerrar las escrituras del volcado, no las de antes.
    #[inline]
    pub fn vaciar(&self) {
        self.volcar();
        unsafe { core::arch::asm!("sfence", options(nostack, preserves_flags)) };
    }

    /// **Copia al panel lo sucio del lienzo**, y deja la caja vacía.
    ///
    /// Sin doble búfer no hay nada que copiar: lo pintado ya está en el panel.
    /// Igual se limpia la caja, porque llevarla puesta sin volcar sería mentir
    /// sobre lo que queda pendiente.
    pub fn volcar(&self) {
        let (x0, y0, x1, y1) = self.sucio.replace(VACIO);
        if self.lienzo == self.panel || x0 >= x1 || y0 >= y1 {
            return;
        }
        let stride = self.stride as usize;
        let ancho = (x1 - x0) as usize;
        let mut fila = y0 as usize;
        while fila < y1 as usize {
            let off = fila * stride + x0 as usize;
            // ★ De DOS píxeles por escritura cuando se puede.
            //
            // El panel es write-combining: el CPU junta escrituras seguidas en
            // ráfagas de 64 bytes, y le cuesta menos juntar ocho de 8 bytes que
            // dieciséis de 4. Es la misma cantidad de datos con la mitad de
            // operaciones, y no necesita SSE ni alineación especial más allá de
            // la que ya tiene un framebuffer (base de página, píxeles de 4 B).
            //
            // El píxel suelto del final se copia como siempre. Un bucle que
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

        // La cuenta, para poder contestar "¿hace falta una GPU?" con un número
        // en vez de con una intuición.
        let bytes = (ancho as u64) * ((y1 - y0) as u64) * 4;
        let v = self.volcado.get();
        self.volcado.set(Volcado {
            fotogramas: v.fotogramas + 1,
            bytes: v.bytes + bytes,
            peor: v.peor.max(bytes),
            modo: v.modo,
        });
    }

    /// Lo que lleva costado el volcado. Ver [`Volcado`] y [`Volcador`].
    pub fn volcado(&self) -> Volcado {
        let mut v = self.volcado.get();
        v.modo = if self.lienzo == self.panel { Volcador::Ninguno } else { Volcador::Directo };
        v
    }

    /// **Asegura que lo escrito se puede LEER.** Llamar antes de [`Self::leer`].
    ///
    /// ★ Existe por el Ep. 25, y hace dos cosas distintas según dónde se dibuje:
    ///
    /// - **Con doble búfer**: nada. El lienzo es RAM normal y cacheada, así que
    ///   una lectura ve lo que se acaba de escribir. El problema no existe.
    /// - **Sin doble búfer**: `sfence`. Se está leyendo memoria WC, y una
    ///   lectura de WC **no está ordenada** contra las escrituras pendientes en
    ///   el búfer: sin barrera devuelve la pantalla de hace un fotograma.
    ///
    /// Que sea un no-op en el camino bueno es justo la gracia: el doble búfer no
    /// arregla el ghosting, lo hace **imposible**.
    #[inline]
    pub fn sincronizar_lectura(&self) {
        if self.lienzo == self.panel {
            unsafe { core::arch::asm!("sfence", options(nostack, preserves_flags)) };
        }
    }

    /// Qué hay AHORA en un píxel. Fuera de la pantalla, negro.
    ///
    /// ★ Se lee del LIENZO, que es donde se ha dibujado. Eso es lo que permite
    /// dibujar el cursor del ratón **encima de cualquier cosa**: se guarda lo
    /// que había debajo y se devuelve al moverlo. Sin esto hay que preguntarle a
    /// un modelo de la escena qué debería haber, y ese modelo se queda corto en
    /// cuanto aparece una ventana que no conoce: el cursor deja agujeros con el
    /// color del fondo por donde pasa.
    ///
    /// Sin doble búfer esto lee memoria de vídeo, que es cara y además exige
    /// [`Self::sincronizar_lectura`] antes. Con doble búfer es RAM normal.
    #[inline]
    pub fn leer(&self, x: u32, y: u32) -> u32 {
        if x >= self.ancho || y >= self.alto {
            return 0;
        }
        unsafe {
            self.lienzo
                .add((y as usize) * (self.stride as usize) + x as usize)
                .read_volatile()
        }
    }

    /// Un rectángulo, recortado a la pantalla. Es la única primitiva de dibujo
    /// que hay, y con ella se hace un escritorio entero: fondo, barra,
    /// ventanas, bordes. Lo demás son estas mismas llamadas puestas en orden.
    pub fn rect(&self, x: u32, y: u32, ancho: u32, alto: u32, color: u32) {
        let x1 = (x.saturating_add(ancho)).min(self.ancho);
        let y1 = (y.saturating_add(alto)).min(self.alto);
        // Se marca UNA vez, con las medidas ya recortadas, en vez de un píxel
        // por vuelta: un rectángulo de pantalla completa son millones de
        // llamadas a `marcar` que darían exactamente la misma caja.
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

/// La caja vacía: `x0 >= x1`, así que no hay nada que volcar.
const VACIO: (u32, u32, u32, u32) = (u32::MAX, u32::MAX, 0, 0);

/// **Cómo llegan los píxeles del lienzo al panel.**
///
/// ═══ ★ Ésta es la costura donde entra una GPU ═══
///
/// La idea que la motivó era partir el compositor en `gui_CPU.bex` y
/// `gui_GPU.bex`. Eso sería **bifurcar antes de que exista la segunda
/// implementación**: cada arreglo habría que hacerlo dos veces, que es
/// exactamente el problema que resolvió `refactor(abi): la disposición de
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
/// Por eso el contrato es esto y no un trait `Lienzo` entero: `punto` está en
/// el camino caliente y meterle una llamada indirecta costaría en cada píxel
/// para no ganar nada. `volcar` se llama **una vez por fotograma**, así que
/// aquí una rama no se nota — y es donde está el coste de verdad.
///
/// ═══ Y antes de comprar una tarjeta, MEDIR ═══
///
/// La caja de sucio ya evita casi todo el trabajo: escribir una letra vuelca
/// unos pocos KiB, no la pantalla. Ver [`Pantalla::volcado`] — el número va
/// primero, la tarjeta después.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Volcador {
    /// No hay doble búfer: lo pintado ya está en el panel. No hay nada que
    /// mover, y decirlo con su nombre vale más que un `if` suelto.
    Ninguno,
    /// Copia con escrituras normales, de 8 en 8 bytes cuando se puede.
    ///
    /// Es lo que hay hoy y lo que corre en el Ryzen. Dos píxeles por
    /// escritura: el framebuffer es write-combining y agrupa mejor cuantas
    /// menos escrituras sueltas reciba.
    Directo,
}

/// Lo que ha costado el volcado, para poder **perfilar antes de comprar nada**.
///
/// No es telemetría de adorno: la pregunta "¿hace falta una GPU?" sólo se puede
/// contestar con estos dos números. Si los bytes por fotograma son pocos, una
/// tarjeta no compra nada y cuesta un año de trabajo.
#[derive(Clone, Copy)]
pub struct Volcado {
    /// Fotogramas con algo que volcar. Los que no cambian nada no cuentan:
    /// promediar con ellos escondería el caso caro.
    pub fotogramas: u64,
    /// Bytes movidos del lienzo al panel, en total.
    pub bytes: u64,
    /// El fotograma más caro visto. **El peor caso importa más que la media**:
    /// un tirón se nota, y una media buena lo esconde.
    pub peor: u64,
    pub modo: Volcador,
}

// ── Las letras ──────────────────────────────────────────────────────────

/// La fuente 8x16 de BMO, la MISMA que pinta el kernel.
///
/// ## Por qué está copiada aquí y no se pide por syscall
///
/// La alternativa era una operación `DIBUJAR_TEXTO` sobre el framebuffer, y
/// eso es exactamente la línea que `KIND_FRAMEBUFFER` existe para no cruzar: el
/// kernel contesta cuatro preguntas y se aparta. Si Ring 0 dibujara letras
/// tendría que saber de tipografía, de kerning y de colores — decisiones de
/// aspecto, ninguna suya. Es el mismo argumento que dejó el cursor en Ring 3.
///
/// Así que aquí hay 4 KiB de tabla duplicada. Sale del mismo generador
/// (`toolchain/tools/fontgen`), así que no son dos fuentes que puedan
/// divergir: son dos copias de una, y regenerar actualiza las dos.
static FONT16: [[u8; 16]; 120] = include!("font16_data.rs");
static FONT_EXTRA: [u8; 25] = include!("font16_extra.rs");
const ASCII_GLYPHS: usize = 95;

/// Ancho y alto de un glifo, en píxeles. El avance horizontal ES el ancho: la
/// fuente ya trae su propio espaciado dentro del mapa de bits.
pub const GLIFO_ANCHO: u32 = 8;
pub const GLIFO_ALTO: u32 = 16;

/// Byte → índice de glifo. ASCII directo; para el español (ñ, á, ¿...) se
/// busca el byte Latin-1 en la tabla de extras.
///
/// **Latin-1 y no UTF-8, igual que en Ring 0.** Un carácter es UN byte, así el
/// teclado, la caja y la fuente hablan el mismo idioma sin decodificador de por
/// medio. Un `&str` de Rust es UTF-8, así que una `ñ` escrita en el código
/// fuente llega como dos bytes y no se dibuja: para eso están `texto_bytes` y
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
    /// Un carácter. Sólo pinta los píxeles encendidos: el fondo se respeta,
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

    /// Un carácter AMPLIADO por un entero: cada píxel del glifo pasa a ser un
    /// cuadrado de `escala`.
    ///
    /// Entero y con `rect`, no interpolado: ampliar por 4 un glifo de 8x16 da
    /// bloques limpios de 32x64, y esa estética es la que tiene esta máquina.
    /// Una interpolación pediría coma flotante, un buffer intermedio y un gusto
    /// que no es el de aquí — y con la misma fuente que ya está cargada.
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

    /// Un `&str` ampliado. Devuelve la x donde acabó.
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

    /// Una tira de bytes Latin-1. Devuelve la x donde acabó, para encadenar.
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

