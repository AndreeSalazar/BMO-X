//! **La SUPERFICIE de una app, pegada dentro de un marco.**
//!
//! === El cambio de modelo, dicho desde este lado ===
//!
//! Hasta ahora, lanzar un programa que pinta era `lend_screen`: el
//! escritorio SOLTABA la pantalla y el hijo la tomaba entera. Mientras el hijo
//! vivia, aqui no habia nadie -- no habia caja, habia relevo.
//!
//! Una superficie le da la vuelta. La app pide memoria, dibuja ahi, y **se la
//! ofrece** (`MEM_OP_OFRECER`, con el tid que le da `TASK_OP_MI_PADRE`). El
//! DIRECTOR la toma una vez, la mapea en su espacio, y a partir de ese momento
//! **lee pixeles de otro proceso con un `mov`**: cero copias por el camino del
//! kernel y cero syscalls por fotograma para traerlos.
//!
//! ** La pantalla no cambia de dueno ni una vez.
//!
//! === Los tres numeros que este modulo NO se cree ===
//!
//! La cabecera `BSUP` la escribe **la app**, o sea otro proceso, o sea que
//! `width`, `height` y `stride` son datos de fuera. Una app que declare
//! `4000 x 4000` dentro de un bloque de 1 MiB --por un fallo o a proposito-- se
//! lleva por delante al compositor: leeriamos fuera del prestamo y el fallo de
//! pagina lo cobra el DIRECTOR, no ella.
//!
//! Por eso [`Header::read`] comprueba que **lo que la cabecera declara cabe en
//! los bytes que el kernel dijo que presto**, y si no cuadra la superficie no
//! existe. Es la unica frontera de confianza del modulo, y va toda en una
//! funcion a proposito.
//!
//! === La secuencia, y por que no hay cerrojo ===
//!
//! Se lee mientras la app escribe: dos procesos sobre la misma memoria y nadie
//! los para. La regla entera cabe en una linea -- **la app sube `sequence`
//! cuando el dibujo esta entero, y aqui solo se repinta cuando el numero es
//! distinto del ultimo que se pego**. Un fotograma a medias no cambia el numero,
//! asi que no se pinta, y el peor caso es ensenar el anterior un fotograma mas.
//!
//! No es un cerrojo **y no debe serlo**: un cerrojo entre dos procesos deja al
//! compositor esperando a una app colgada, y entonces una app rota se lleva el
//! escritorio -- que es justo lo que este diseno existe para impedir.

use bmo_userland as bmo;

use super::chrome::Chrome;
use super::*;

/// `"BSUP"` en little-endian. El mismo numero que escribe `<bmo/surface.h>`.
const MAGIC: u32 = 0x5055_5342;
/// Lo que ocupa la cabecera antes del primer pixel.
const HEADER_TAG: u64 = 32;
/// BGRA de 32 bits, el mismo del framebuffer: se compone COPIANDO y no
/// convirtiendo. Cualquier otro formato se rechaza en vez de convertirse -- una
/// conversion por pixel y por fotograma en el proceso que menos puede
/// permitirsela no es soporte, es una promesa que se paga en cada vuelta.
const BGRA32: u32 = 0;

/// Cuantas apps pueden tener caja a la vez.
///
/// Cuatro, y el numero no es arbitrario: el kernel tiene 16 ranuras de prestamo
/// y hay que dejar sitio a lo que no son ventanas. Cuando se llene se dice --
/// una superficie que no entra no se toma, y su app se queda esperando en vez de
/// tumbar a otra.
pub(crate) const MAX: usize = 4;

/// Lo que la app declara de si misma, **ya comprobado contra el prestamo**.
#[derive(Clone, Copy)]
pub(crate) struct Header {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stride: u32,
    pub(crate) sequence: u32,
}

/// Un `u32` de la cabecera. `volatile` porque **lo escribe otro proceso**: sin
/// eso el compilador puede leerlo una vez y quedarse con el valor para siempre,
/// que es exactamente el fallo de "la ventana no se actualiza nunca".
fn campo(base: u64, i: u64) -> u32 {
    unsafe { core::ptr::read_volatile((base + i * 4) as *const u32) }
}

impl Header {
    /// Lee y **valida**. `None` si esto no es una superficie que se pueda pegar.
    ///
    /// `bytes` es lo que dijo el KERNEL que se presto, y es el unico numero de
    /// aqui en el que se puede confiar: todo lo demas lo escribio la app.
    pub(crate) fn read(base: u64, bytes: u64) -> Option<Header> {
        if bytes < HEADER_TAG || campo(base, 0) != MAGIC || campo(base, 4) != BGRA32 {
            return None;
        }
        let (width, height, stride) = (campo(base, 1), campo(base, 2), campo(base, 3));
        if width == 0 || height == 0 || stride < width {
            return None;
        }
        // ** LA COMPROBACION QUE IMPIDE QUE UNA APP TUMBE AL ESCRITORIO.
        //
        // En `u64` y no en `u32`: `stride * height * 4` con numeros grandes se
        // desborda en 32 bits y da un total PEQUENO, o sea que la comprobacion
        // pasaria justo en el caso que tiene que parar.
        let necesita = HEADER_TAG + stride as u64 * height as u64 * 4;
        if necesita > bytes {
            return None;
        }
        Some(Header { width, height, stride, sequence: campo(base, 5) })
    }
}

/// Una app en su caja.
pub(crate) struct Surface {
    pub(crate) chrome: Chrome,
    /// El handle del prestamo: lo unico que distingue este de otro para
    /// preguntar por su dueno o para devolverlo.
    handle: u64,
    /// Donde quedo mapeado, en MI espacio.
    base: u64,
    /// Lo que el KERNEL dijo que presto. El tope de todo lo que se lee.
    bytes: u64,
    /// Quien la ofrecio. Es el titulo de la ventana y la identidad de la app.
    pub(crate) tid: u32,
    /// La ultima secuencia que se PEGO. Empieza al reves de la primera lectura
    /// para que el primer fotograma se pinte siempre.
    stuck: u32,
    /// El aspecto de la ultima vez, para saber si hay que repintar el cromo.
    width: u32,
    height: u32,
}

impl Surface {
    /// Toma una superficie ofrecida y le da su caja. `None` si lo ofrecido no es
    /// una superficie -- entonces no era para esto, y no se toca.
    fn new(p: &bmo::Pantalla, handle: u64, base: u64, bytes: u64, tid: u32) -> Option<Self> {
        let cab = Header::read(base, bytes)?;
        let chrome = Chrome::for_content(p, cab.width, cab.height);
        Some(Surface {
            chrome,
            handle,
            base,
            bytes,
            tid,
            // Cualquier valor distinto del que hay: asi el primer `compose`
            // pinta sin tener que llevar ademas un "es la primera vez".
            stuck: cab.sequence.wrapping_sub(1),
            // A cero para que el primer `moved` diga que si: asi el cromo lo
            // pinta el mismo camino que lo repinta al mover, y no hay un
            // "primera vez" que alguien pueda olvidarse de llamar.
            width: 0,
            height: 0,
        })
    }

    /// Donde empieza el interior, dentro del marco.
    fn inner(&self) -> (u32, u32) {
        (self.chrome.x + 1, self.chrome.y + TITLE_H)
    }

    /// **Lo que de verdad se esta viendo del interior**, en coordenadas de
    /// pantalla y ya recortado contra el marco Y contra el lienzo.
    ///
    /// Es el MISMO recorte que hace [`Surface::compose`], y esta escrito una
    /// sola vez a proposito: si el golpe se recortara distinto que los pixeles,
    /// habria un borde donde se ve una cosa y se pulsa otra. Ese es el tipo de
    /// fallo que no da error -- da un numero.
    fn visible(&self, p: &bmo::Pantalla, cab: &Header) -> bmo_golpe::Visible {
        let (x, y) = self.inner();
        let gap_w = self.chrome.width.saturating_sub(2);
        let gap_h = self.chrome.height.saturating_sub(TITLE_H + 1);
        bmo_golpe::Visible {
            x,
            y,
            ancho: cab.width.min(gap_w).min(p.ancho.saturating_sub(x)),
            alto: cab.height.min(gap_h).min(p.alto.saturating_sub(y)),
        }
    }

    /// **De quien es este golpe y en que pixel suyo cae.** `None` si no es de
    /// esta superficie.
    ///
    /// La resta vive en `bmo-golpe` y no aqui: alli se puede PROBAR (L7b). Lo
    /// de este lado es solo juntar los dos hechos que la resta necesita -- la
    /// caja visible y lo que la app declaro.
    pub(crate) fn golpe(&self, p: &bmo::Pantalla, px: u32, py: u32) -> Option<(u32, u32)> {
        if self.chrome.minimized {
            return None;
        }
        let cab = Header::read(self.base, self.bytes)?;
        let d = bmo_golpe::Declarada { ancho: cab.width, alto: cab.height };
        bmo_golpe::traducir(self.visible(p, &cab), d, px, py)
    }

    /// **Pega los pixeles.** Solo si la secuencia cambio; `true` si pinto.
    ///
    /// Se recorta contra el marco Y contra la pantalla: una ventana arrastrada
    /// medio fuera del panel no puede escribir mas alla del lienzo, y el marco
    /// puede ser mas pequeno que la superficie si el usuario lo encogio.
    pub(crate) fn compose(&mut self, p: &bmo::Pantalla) -> bool {
        let Some(cab) = Header::read(self.base, self.bytes) else {
            return false;
        };
        if cab.sequence == self.stuck {
            return false;
        }
        // Lo que cabe: el hueco del marco, lo que mide la superficie, y lo que
        // queda de pantalla. El menor de los tres, y ninguno se puede saltar.
        // ** Sale de `visible()`, que es el MISMO recorte que usa el golpe: dos
        // copias de esta cuenta serian un borde donde se ve una cosa y se pulsa
        // otra, y eso no da error, da un numero.
        let v = self.visible(p, &cab);
        let (x0, y0) = (v.x, v.y);
        let (width, height) = (v.ancho, v.alto);
        if width == 0 || height == 0 {
            self.stuck = cab.sequence;
            return false;
        }

        for row in 0..height {
            let src = self.base + HEADER_TAG + (row as u64 * cab.stride as u64) * 4;
            for col in 0..width {
                let px = unsafe { core::ptr::read_volatile((src + col as u64 * 4) as *const u32) };
                // `punto_sin_comprobar` y una sola marca al final: el recorte ya
                // esta hecho arriba, y marcar pixel a pixel serian cientos de
                // miles de llamadas para acabar en la misma caja.
                unsafe { p.punto_sin_comprobar(x0 + col, y0 + row, px) };
            }
        }
        p.marcar(x0, y0, width, height);
        // Se apunta DESPUES de pegar. Al reves, un fotograma que se quedara a
        // medias por un recorte se daria por pintado y no volveria a intentarse.
        self.stuck = cab.sequence;
        true
    }

    /// El cromo de la ventana: el marco con sus tres botones y el titulo.
    ///
    /// La superficie NO se repinta aqui: quien llama hace `compose` despues, y
    /// el orden importa -- el cromo pinta el cuerpo entero y borraria los
    /// pixeles de la app si fuera al reves.
    pub(crate) fn paint_chrome(&self, p: &bmo::Pantalla) {
        self.chrome.paint_chrome(p, BOX_EDGE, BOX_BG, BOX_TITLE, ACCENT);
        p.rect(self.chrome.x + 10, self.chrome.y + 10, 8, 8, ACCENT);
        // El titulo es el TID, porque es lo unico que el DIRECTOR sabe de esta
        // app con certeza: el nombre lo pondria quien la lanzo, y lanzar y
        // componer son dos cosas distintas. Ver el paso 3 del plan.
        let mut n = [0u8; 12];
        let length = tid_text(self.tid, &mut n);
        p.texto_bytes(self.chrome.x + 26, self.chrome.y + 7, &n[..length], INK);
    }

    /// Ha cambiado de tamano o de sitio? Entonces hay que repintar el cromo y
    /// devolverle al escritorio lo que la ventana deje de tapar.
    fn moved(&mut self) -> bool {
        let cambio = self.width != self.chrome.width || self.height != self.chrome.height;
        self.width = self.chrome.width;
        self.height = self.chrome.height;
        cambio
    }

    /// **Fuerza a repegar los pixeles en la siguiente vuelta**, aunque la app no
    /// haya tocado la secuencia. Hace falta cuando algo tapo la ventana: la app
    /// no tiene por que enterarse de que le pasaron algo por encima.
    fn mark_dirty(&mut self) {
        self.stuck = self.stuck.wrapping_sub(1);
    }

    /// Igual, **y el cromo tambien**. Es lo que hay que llamar cuando se ha
    /// borrado un trozo de escritorio encima: `erase_window` devuelve el
    /// FONDO, asi que un marco que estuviera ahi se lo ha llevado por delante y
    /// repintar solo los pixeles dejaria una ventana sin borde ni botones.
    pub(crate) fn repaint_all(&mut self) {
        self.width = 0;
        self.height = 0;
        self.mark_dirty();
    }

    /// Sigue viva la app que presto esto?
    pub(crate) fn alive(&self) -> bool {
        bmo::prestado_dueno(self.handle) != 0
    }

    /// Devuelve el prestamo al kernel. **Despues de esto `base` no se toca**:
    /// esas paginas ya no estan mapeadas.
    fn soltar(&self) {
        bmo::soltar_prestado(self.handle);
    }
}

/// `tid 7` en bytes, sin `alloc` y sin formato.
fn tid_text(tid: u32, dst: &mut [u8; 12]) -> usize {
    dst[..4].copy_from_slice(b"tid ");
    let mut n = 4;
    let mut d = [0u8; 10];
    let mut k = 0;
    let mut v = tid;
    loop {
        d[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    while k > 0 {
        k -= 1;
        dst[n] = d[k];
        n += 1;
    }
    n
}

/// **La mesa del DIRECTOR**: las apps que tienen caja ahora mismo.
pub(crate) struct Table {
    sup: [Option<Surface>; MAX],
}

impl Table {
    pub(crate) fn new() -> Self {
        Table { sup: [None, None, None, None] }
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Surface> {
        self.sup.iter_mut().filter_map(|s| s.as_mut())
    }

    /// **De que superficie es este golpe, y en que pixel suyo.**
    ///
    /// Paso 2c.1 de `docs/plan/PLAN_DIRECTOR.md`, y su prueba es literalmente
    /// poder contestar *"este clic es de la superficie 2, en su pixel
    /// (81, 210)"* **sin que la app exista todavia**: aqui no se manda nada a
    /// nadie, solo se traduce.
    ///
    /// [!] El orden es el de la mesa, que hoy es el de llegada. Cuando dos cajas
    /// se solapen habra que preguntarle al foco quien esta delante -- y eso ya
    /// tiene dueno (`bmo_input::foco`, paso 2c.3), asi que no se inventa aqui
    /// una segunda politica que luego habria que reconciliar.
    // Todavia no lo llama nadie, y eso es el plan y no un olvido: 2c.1 se
    // entrega SOLA para que su fallo no se confunda con el del transporte.
    // Quien lo llame es el paso 2c.3, cuando el foco diga de quien es la tecla.
    #[allow(dead_code)]
    pub(crate) fn golpe(&self, p: &bmo::Pantalla, px: u32, py: u32) -> Option<(usize, u32, u32)> {
        for (i, s) in self.sup.iter().enumerate() {
            let Some(s) = s.as_ref() else { continue };
            if let Some((lx, ly)) = s.golpe(p, px, py) {
                return Some((i, lx, ly));
            }
        }
        None
    }

    /// **Recoge lo que alguien haya ofrecido.** Devuelve `true` si nacio una
    /// ventana.
    ///
    /// Se llama una vez por fotograma y cuesta un syscall que casi siempre dice
    /// que no. Es el precio de no tener que avisar: una app ofrece cuando
    /// quiere, y el DIRECTOR se entera mirando.
    ///
    /// ** Se toma **una por vuelta** a proposito. Tomar en bucle hasta que el
    /// kernel diga que no deja al compositor mapeando memoria ajena dentro de un
    /// fotograma sin tope, y un programa que ofrezca en bucle podria estirar esa
    /// vuelta hasta que se note.
    /// Devuelve **en que hueco** nacio la ventana, no solo que nacio: sin ese
    /// numero el escritorio no puede nombrarla al foco, y una app sin nombre
    /// es una app que Alt+Tab no ve.
    pub(crate) fn collect(&mut self, p: &bmo::Pantalla) -> Option<usize> {
        let Some(gap) = self.sup.iter().position(|s| s.is_none()) else {
            // Sin sitio no se toma. Dejarla ofrecida es lo correcto: la app
            // sigue esperando y la recogemos cuando se cierre una ventana, en
            // vez de tomarla para no poder ensenarla.
            return None;
        };
        let Some((handle, base, bytes)) = bmo::tomar_prestado_de() else {
            return None;
        };
        let tid = bmo::prestado_dueno(handle);
        match Surface::new(p, handle, base, bytes, tid) {
            Some(s) => {
                self.sup[gap] = Some(s);
                Some(gap)
            }
            None => {
                // Lo ofrecido no es una superficie. Se devuelve en vez de
                // quedarselo: retener memoria ajena que no sabemos leer es
                // gastarle una ranura del kernel a quien nos la presto.
                bmo::soltar_prestado(handle);
                None
            }
        }
    }

    /// **Hay algo nuevo que pegar?** Mira las secuencias sin pintar nada.
    ///
    /// Existe porque el fotograma se decide ANTES de pintarlo: el cursor del
    /// raton se quita al principio y se pone al final, y si un fotograma en el
    /// que solo cambio una superficie no se contara como "va a pintar", la app
    /// dibujaria **encima del cursor** y el puntero desapareceria bajo su
    /// ventana. Cuesta una lectura por ventana.
    pub(crate) fn has_new(&self) -> bool {
        self.sup.iter().flatten().any(|s| {
            !s.chrome.minimized
                && Header::read(s.base, s.bytes).is_some_and(|c| c.sequence != s.stuck)
        })
    }

    /// **Retira las ventanas cuya app murio** y devuelve cuantas, dejando sus
    /// rectangulos en `cajas`.
    ///
    /// Va separado de [`Self::compose`] porque lo que hay que hacer con el
    /// hueco --devolverle al escritorio los pixeles que la ventana tapaba-- no
    /// se puede decidir aqui: este modulo no sabe que habia debajo, y aprenderlo
    /// seria meterle el modelo de la escena entero.
    ///
    /// ** Y se pregunta cada fotograma porque una app muerta deja la secuencia
    /// CONGELADA, que es indistinguible de una app pensando. Sin esto, la
    /// ventana de un programa que ya no existe se quedaria en pantalla con su
    /// ultimo fotograma y sus tres botones, como si fuera a responder.
    pub(crate) fn reap_dead(&mut self, cajas: &mut [(u32, u32, u32, u32); MAX]) -> usize {
        let mut n = 0;
        for gap in self.sup.iter_mut() {
            let dead_one = match gap.as_ref() {
                Some(s) if !s.alive() => {
                    cajas[n] = (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
                    s.soltar();
                    true
                }
                _ => false,
            };
            if dead_one {
                *gap = None;
                n += 1;
            }
        }
        n
    }

    /// **Compone.** `true` si pinto algo.
    pub(crate) fn compose(&mut self, p: &bmo::Pantalla) -> bool {
        let mut painted = false;
        for s in self.iter_mut() {
            if s.chrome.minimized {
                continue;
            }
            if s.moved() {
                s.paint_chrome(p);
                s.mark_dirty();
                painted = true;
            }
            painted |= s.compose(p);
        }
        painted
    }

    /// Cierra la ventana `i` y devuelve su rectangulo, para que quien llama
    /// sepa que trozo de escritorio hay que repintar.
    ///
    /// * Cerrar aqui **no mata a la app**: le quita la caja. Matarla seria
    /// autoridad sobre un proceso ajeno, y eso es el paso 3 del plan -- se hace
    /// con el handle que devolvio lanzarla, no por ser el DIRECTOR. Mientras
    /// tanto, la app deja de tener donde pintar y lo sabe: su prestamo
    /// desaparece.
    pub(crate) fn close(&mut self, i: usize) -> Option<(u32, u32, u32, u32)> {
        let s = self.sup.get_mut(i)?.take()?;
        let run_box = (s.chrome.x, s.chrome.y, s.chrome.width, s.chrome.height);
        s.soltar();
        Some(run_box)
    }

    /// Sobre que ventana esta el puntero, de arriba a abajo.
    pub(crate) fn at(&self, px: u32, py: u32) -> Option<usize> {
        self.sup
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.chrome.contains(px, py)))
    }

    pub(crate) fn get_mut(&mut self, i: usize) -> Option<&mut Surface> {
        self.sup.get_mut(i)?.as_mut()
    }
}
