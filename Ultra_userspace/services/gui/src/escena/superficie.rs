//! **La SUPERFICIE de una app, pegada dentro de un marco.**
//!
//! === El cambio de modelo, dicho desde este lado ===
//!
//! Hasta ahora, lanzar un programa que pinta era `prestar_pantalla`: el
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
//! `ancho`, `alto` y `stride` son datos de fuera. Una app que declare
//! `4000 x 4000` dentro de un bloque de 1 MiB --por un fallo o a proposito-- se
//! lleva por delante al compositor: leeriamos fuera del prestamo y el fallo de
//! pagina lo cobra el DIRECTOR, no ella.
//!
//! Por eso [`Cabecera::leer`] comprueba que **lo que la cabecera declara cabe en
//! los bytes que el kernel dijo que presto**, y si no cuadra la superficie no
//! existe. Es la unica frontera de confianza del modulo, y va toda en una
//! funcion a proposito.
//!
//! === La secuencia, y por que no hay cerrojo ===
//!
//! Se lee mientras la app escribe: dos procesos sobre la misma memoria y nadie
//! los para. La regla entera cabe en una linea -- **la app sube `secuencia`
//! cuando el dibujo esta entero, y aqui solo se repinta cuando el numero es
//! distinto del ultimo que se pego**. Un fotograma a medias no cambia el numero,
//! asi que no se pinta, y el peor caso es ensenar el anterior un fotograma mas.
//!
//! No es un cerrojo **y no debe serlo**: un cerrojo entre dos procesos deja al
//! compositor esperando a una app colgada, y entonces una app rota se lleva el
//! escritorio -- que es justo lo que este diseno existe para impedir.

use bmo_userland as bmo;

use super::marco::Marco;
use super::*;

/// `"BSUP"` en little-endian. El mismo numero que escribe `<bmo/superficie.h>`.
const MAGIC: u32 = 0x5055_5342;
/// Lo que ocupa la cabecera antes del primer pixel.
const CABECERA: u64 = 32;
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
pub(crate) struct Cabecera {
    pub(crate) ancho: u32,
    pub(crate) alto: u32,
    pub(crate) stride: u32,
    pub(crate) secuencia: u32,
}

/// Un `u32` de la cabecera. `volatile` porque **lo escribe otro proceso**: sin
/// eso el compilador puede leerlo una vez y quedarse con el valor para siempre,
/// que es exactamente el fallo de "la ventana no se actualiza nunca".
fn campo(base: u64, i: u64) -> u32 {
    unsafe { core::ptr::read_volatile((base + i * 4) as *const u32) }
}

impl Cabecera {
    /// Lee y **valida**. `None` si esto no es una superficie que se pueda pegar.
    ///
    /// `bytes` es lo que dijo el KERNEL que se presto, y es el unico numero de
    /// aqui en el que se puede confiar: todo lo demas lo escribio la app.
    pub(crate) fn leer(base: u64, bytes: u64) -> Option<Cabecera> {
        if bytes < CABECERA || campo(base, 0) != MAGIC || campo(base, 4) != BGRA32 {
            return None;
        }
        let (ancho, alto, stride) = (campo(base, 1), campo(base, 2), campo(base, 3));
        if ancho == 0 || alto == 0 || stride < ancho {
            return None;
        }
        // ** LA COMPROBACION QUE IMPIDE QUE UNA APP TUMBE AL ESCRITORIO.
        //
        // En `u64` y no en `u32`: `stride * alto * 4` con numeros grandes se
        // desborda en 32 bits y da un total PEQUENO, o sea que la comprobacion
        // pasaria justo en el caso que tiene que parar.
        let necesita = CABECERA + stride as u64 * alto as u64 * 4;
        if necesita > bytes {
            return None;
        }
        Some(Cabecera { ancho, alto, stride, secuencia: campo(base, 5) })
    }
}

/// Una app en su caja.
pub(crate) struct Superficie {
    pub(crate) marco: Marco,
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
    pegada: u32,
    /// El aspecto de la ultima vez, para saber si hay que repintar el cromo.
    ancho: u32,
    alto: u32,
}

impl Superficie {
    /// Toma una superficie ofrecida y le da su caja. `None` si lo ofrecido no es
    /// una superficie -- entonces no era para esto, y no se toca.
    fn nueva(p: &bmo::Pantalla, handle: u64, base: u64, bytes: u64, tid: u32) -> Option<Self> {
        let cab = Cabecera::leer(base, bytes)?;
        let marco = Marco::para_contenido(p, cab.ancho, cab.alto);
        Some(Superficie {
            marco,
            handle,
            base,
            bytes,
            tid,
            // Cualquier valor distinto del que hay: asi el primer `componer`
            // pinta sin tener que llevar ademas un "es la primera vez".
            pegada: cab.secuencia.wrapping_sub(1),
            // A cero para que el primer `movida` diga que si: asi el cromo lo
            // pinta el mismo camino que lo repinta al mover, y no hay un
            // "primera vez" que alguien pueda olvidarse de llamar.
            ancho: 0,
            alto: 0,
        })
    }

    /// Donde empieza el interior, dentro del marco.
    fn interior(&self) -> (u32, u32) {
        (self.marco.x + 1, self.marco.y + TITULO_ALTO)
    }

    /// **Pega los pixeles.** Solo si la secuencia cambio; `true` si pinto.
    ///
    /// Se recorta contra el marco Y contra la pantalla: una ventana arrastrada
    /// medio fuera del panel no puede escribir mas alla del lienzo, y el marco
    /// puede ser mas pequeno que la superficie si el usuario lo encogio.
    pub(crate) fn componer(&mut self, p: &bmo::Pantalla) -> bool {
        let Some(cab) = Cabecera::leer(self.base, self.bytes) else {
            return false;
        };
        if cab.secuencia == self.pegada {
            return false;
        }
        let (x0, y0) = self.interior();
        // Lo que cabe: el hueco del marco, lo que mide la superficie, y lo que
        // queda de pantalla. El menor de los tres, y ninguno se puede saltar.
        let hueco_ancho = self.marco.ancho.saturating_sub(2);
        let hueco_alto = self.marco.alto.saturating_sub(TITULO_ALTO + 1);
        let ancho = cab.ancho.min(hueco_ancho).min(p.ancho.saturating_sub(x0));
        let alto = cab.alto.min(hueco_alto).min(p.alto.saturating_sub(y0));
        if ancho == 0 || alto == 0 {
            self.pegada = cab.secuencia;
            return false;
        }

        for fila in 0..alto {
            let origen = self.base + CABECERA + (fila as u64 * cab.stride as u64) * 4;
            for col in 0..ancho {
                let px = unsafe { core::ptr::read_volatile((origen + col as u64 * 4) as *const u32) };
                // `punto_sin_comprobar` y una sola marca al final: el recorte ya
                // esta hecho arriba, y marcar pixel a pixel serian cientos de
                // miles de llamadas para acabar en la misma caja.
                unsafe { p.punto_sin_comprobar(x0 + col, y0 + fila, px) };
            }
        }
        p.marcar(x0, y0, ancho, alto);
        // Se apunta DESPUES de pegar. Al reves, un fotograma que se quedara a
        // medias por un recorte se daria por pintado y no volveria a intentarse.
        self.pegada = cab.secuencia;
        true
    }

    /// El cromo de la ventana: el marco con sus tres botones y el titulo.
    ///
    /// La superficie NO se repinta aqui: quien llama hace `componer` despues, y
    /// el orden importa -- el cromo pinta el cuerpo entero y borraria los
    /// pixeles de la app si fuera al reves.
    pub(crate) fn pintar_cromo(&self, p: &bmo::Pantalla) {
        self.marco.pintar_cromo(p, CAJA_BORDE, CAJA_FONDO, CAJA_TITULO, ACENTO);
        p.rect(self.marco.x + 10, self.marco.y + 10, 8, 8, ACENTO);
        // El titulo es el TID, porque es lo unico que el DIRECTOR sabe de esta
        // app con certeza: el nombre lo pondria quien la lanzo, y lanzar y
        // componer son dos cosas distintas. Ver el paso 3 del plan.
        let mut n = [0u8; 12];
        let largo = tid_texto(self.tid, &mut n);
        p.texto_bytes(self.marco.x + 26, self.marco.y + 7, &n[..largo], TEXTO);
    }

    /// Ha cambiado de tamano o de sitio? Entonces hay que repintar el cromo y
    /// devolverle al escritorio lo que la ventana deje de tapar.
    fn movida(&mut self) -> bool {
        let cambio = self.ancho != self.marco.ancho || self.alto != self.marco.alto;
        self.ancho = self.marco.ancho;
        self.alto = self.marco.alto;
        cambio
    }

    /// **Fuerza a repegar los pixeles en la siguiente vuelta**, aunque la app no
    /// haya tocado la secuencia. Hace falta cuando algo tapo la ventana: la app
    /// no tiene por que enterarse de que le pasaron algo por encima.
    fn ensuciar(&mut self) {
        self.pegada = self.pegada.wrapping_sub(1);
    }

    /// Igual, **y el cromo tambien**. Es lo que hay que llamar cuando se ha
    /// borrado un trozo de escritorio encima: `borrar_ventana` devuelve el
    /// FONDO, asi que un marco que estuviera ahi se lo ha llevado por delante y
    /// repintar solo los pixeles dejaria una ventana sin borde ni botones.
    pub(crate) fn repintar_todo(&mut self) {
        self.ancho = 0;
        self.alto = 0;
        self.ensuciar();
    }

    /// Sigue viva la app que presto esto?
    pub(crate) fn viva(&self) -> bool {
        bmo::prestado_dueno(self.handle) != 0
    }

    /// Devuelve el prestamo al kernel. **Despues de esto `base` no se toca**:
    /// esas paginas ya no estan mapeadas.
    fn soltar(&self) {
        bmo::soltar_prestado(self.handle);
    }
}

/// `tid 7` en bytes, sin `alloc` y sin formato.
fn tid_texto(tid: u32, dst: &mut [u8; 12]) -> usize {
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
pub(crate) struct Mesa {
    sup: [Option<Superficie>; MAX],
}

impl Mesa {
    pub(crate) fn nueva() -> Self {
        Mesa { sup: [None, None, None, None] }
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Superficie> {
        self.sup.iter_mut().filter_map(|s| s.as_mut())
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
    pub(crate) fn recoger(&mut self, p: &bmo::Pantalla) -> bool {
        let Some(hueco) = self.sup.iter().position(|s| s.is_none()) else {
            // Sin sitio no se toma. Dejarla ofrecida es lo correcto: la app
            // sigue esperando y la recogemos cuando se cierre una ventana, en
            // vez de tomarla para no poder ensenarla.
            return false;
        };
        let Some((handle, base, bytes)) = bmo::tomar_prestado_de() else {
            return false;
        };
        let tid = bmo::prestado_dueno(handle);
        match Superficie::nueva(p, handle, base, bytes, tid) {
            Some(s) => {
                self.sup[hueco] = Some(s);
                true
            }
            None => {
                // Lo ofrecido no es una superficie. Se devuelve en vez de
                // quedarselo: retener memoria ajena que no sabemos leer es
                // gastarle una ranura del kernel a quien nos la presto.
                bmo::soltar_prestado(handle);
                false
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
    pub(crate) fn hay_nuevo(&self) -> bool {
        self.sup.iter().flatten().any(|s| {
            !s.marco.minimizada
                && Cabecera::leer(s.base, s.bytes).is_some_and(|c| c.secuencia != s.pegada)
        })
    }

    /// **Retira las ventanas cuya app murio** y devuelve cuantas, dejando sus
    /// rectangulos en `cajas`.
    ///
    /// Va separado de [`Self::componer`] porque lo que hay que hacer con el
    /// hueco --devolverle al escritorio los pixeles que la ventana tapaba-- no
    /// se puede decidir aqui: este modulo no sabe que habia debajo, y aprenderlo
    /// seria meterle el modelo de la escena entero.
    ///
    /// ** Y se pregunta cada fotograma porque una app muerta deja la secuencia
    /// CONGELADA, que es indistinguible de una app pensando. Sin esto, la
    /// ventana de un programa que ya no existe se quedaria en pantalla con su
    /// ultimo fotograma y sus tres botones, como si fuera a responder.
    pub(crate) fn retirar_difuntas(&mut self, cajas: &mut [(u32, u32, u32, u32); MAX]) -> usize {
        let mut n = 0;
        for hueco in self.sup.iter_mut() {
            let difunta = match hueco.as_ref() {
                Some(s) if !s.viva() => {
                    cajas[n] = (s.marco.x, s.marco.y, s.marco.ancho, s.marco.alto);
                    s.soltar();
                    true
                }
                _ => false,
            };
            if difunta {
                *hueco = None;
                n += 1;
            }
        }
        n
    }

    /// **Compone.** `true` si pinto algo.
    pub(crate) fn componer(&mut self, p: &bmo::Pantalla) -> bool {
        let mut pinto = false;
        for s in self.iter_mut() {
            if s.marco.minimizada {
                continue;
            }
            if s.movida() {
                s.pintar_cromo(p);
                s.ensuciar();
                pinto = true;
            }
            pinto |= s.componer(p);
        }
        pinto
    }

    /// Cierra la ventana `i` y devuelve su rectangulo, para que quien llama
    /// sepa que trozo de escritorio hay que repintar.
    ///
    /// * Cerrar aqui **no mata a la app**: le quita la caja. Matarla seria
    /// autoridad sobre un proceso ajeno, y eso es el paso 3 del plan -- se hace
    /// con el handle que devolvio lanzarla, no por ser el DIRECTOR. Mientras
    /// tanto, la app deja de tener donde pintar y lo sabe: su prestamo
    /// desaparece.
    pub(crate) fn cerrar(&mut self, i: usize) -> Option<(u32, u32, u32, u32)> {
        let s = self.sup.get_mut(i)?.take()?;
        let caja = (s.marco.x, s.marco.y, s.marco.ancho, s.marco.alto);
        s.soltar();
        Some(caja)
    }

    /// Sobre que ventana esta el puntero, de arriba a abajo.
    pub(crate) fn en(&self, px: u32, py: u32) -> Option<usize> {
        self.sup
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.marco.contiene(px, py)))
    }

    pub(crate) fn get_mut(&mut self, i: usize) -> Option<&mut Superficie> {
        self.sup.get_mut(i)?.as_mut()
    }
}
