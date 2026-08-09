//! **La AUTOPSIA de un fallo de Ring 3.** Lo que el kernel guarda cuando mata
//! una tarea, para que se pueda leer despues y mandar.
//!
//! # Por que existe, y por que aqui
//!
//! El aislamiento de faults ya funcionaba: una tarea de Ring 3 revienta, el
//! kernel le quita las capabilities, la marca muerta y **BMO sigue vivo**. Eso
//! se ve en CABINA como una linea roja:
//!
//! ```text
//!   FAULT ring3: fault en CPL3: tarea eliminada, BMO sigue vivo =4000105
//! ```
//!
//! Una linea. Con el `rip` y nada mas. Y eso alcanza para saber QUE paso y no
//! para saber DONDE: falta el vector, el codigo de error, la direccion que se
//! toco, la pila, y sobre todo **de que programa se trataba**.
//!
//! Sin esas cinco cosas, un fallo en la maquina del dueno no se puede mandar a
//! nadie: se cuenta de memoria, y contar un fallo de memoria es como se pierden
//! los fallos. Con ellas, la maquina redacta su propio informe.
//!
//! Esto es lo que el README llama el "meta" del metakernel, y esta es su forma
//! mas literal: **el sistema deja escrito lo que le paso a el mismo.**
//!
//! # La regla que decide el diseno: aqui NO se toca el disco
//!
//! La tentacion es escribir el informe a un fichero desde el propio manejador
//! de faults. No se hace, y el motivo no es prudencia general:
//!
//! * Se corre **dentro de un fault**, con la pila del kernel y sin saber que
//!   estado dejo el que fallo. Escribir a disco ahi es entrar en el driver de
//!   AHCI, que tiene esperas y estado propio.
//! * Y el fallo **puede ser del disco**. Un informe que necesita el subsistema
//!   que acaba de caerse no es un informe: es un segundo fallo encima del
//!   primero, y del que no queda nada escrito.
//!
//! Asi que el kernel **captura en RAM** --que es barato, acotado y no puede
//! fallar-- y quien lo persiste es Ring 3, que esta vivo, tiene la capability
//! de escribir y puede permitirse tardar. Es la misma division que el resto del
//! sistema: el kernel CONTESTA, no actua por cuenta de nadie.

use crate::ring0::plat::timer;

/// Cuantas autopsias se guardan. Cuatro porque un fallo que se repite lo hace
/// en rafaga --el mismo programa relanzado tres veces-- y lo que interesa es
/// tener la primera Y la ultima: si son iguales es determinista, y si no, algo
/// del entorno cambio entre medias.
const CUANTAS: usize = 4;
/// Renglones por informe.
const RENGLONES: usize = 9;
/// Ancho de cada renglon. El de la ventana de datos, para que quepa sin cortar.
const ANCHO: usize = 72;

struct Autopsia {
    texto: [[u8; ANCHO]; RENGLONES],
    largo: [u8; RENGLONES],
    usados: u8,
}

static mut ANILLO: [Autopsia; CUANTAS] = [const {
    Autopsia {
        texto: [[0; ANCHO]; RENGLONES],
        largo: [0; RENGLONES],
        usados: 0,
    }
}; CUANTAS];
static mut ESCRIBE: usize = 0;
/// Cuantas van desde el arranque. **No se reinicia**: es el numero que Ring 3
/// compara para saber si hay una nueva sin tener que leerla entera.
static mut TOTAL: u32 = 0;
/// Guarda contra reentrada: un fault dentro del manejador de faults no puede
/// volver a entrar aqui a medio escribir.
static mut DENTRO: bool = false;
/// **Recursos que un muerto dejo sin devolver, acumulados.** Tiene que ser
/// CERO, y por eso vale: es el kernel comprobandose a si mismo. Sube en `info`
/// al lado de los choques de cerrojo, que son la misma clase de numero.
static mut FUGAS_TOTAL: u32 = 0;

/// Un renglon en construccion. Sin `format!` ni asignaciones: esto corre en un
/// manejador de faults, donde el asignador puede ser justo lo que se rompio.
struct Renglon {
    b: [u8; ANCHO],
    n: usize,
}

impl Renglon {
    fn nuevo() -> Self {
        Self { b: [0; ANCHO], n: 0 }
    }
    fn s(&mut self, t: &str) {
        for &c in t.as_bytes() {
            if self.n < ANCHO {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    fn bytes(&mut self, t: &[u8]) {
        for &c in t {
            if c == 0 {
                break;
            }
            if self.n < ANCHO {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    /// Hexadecimal con `0x` y sin ceros de mas. Un `rip` con doce ceros
    /// delante es doce caracteres que no dicen nada y una linea que no cabe.
    fn hex(&mut self, v: u64) {
        self.s("0x");
        let mut visto = false;
        for i in (0..16).rev() {
            let d = ((v >> (i * 4)) & 0xF) as u8;
            if d != 0 {
                visto = true;
            }
            if visto || i == 0 {
                self.b[self.n.min(ANCHO - 1)] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
                if self.n < ANCHO {
                    self.n += 1;
                }
            }
        }
    }
    fn dec(&mut self, mut v: u64) {
        let mut cifras = [0u8; 20];
        let mut c = 0;
        if v == 0 {
            cifras[0] = b'0';
            c = 1;
        }
        while v > 0 && c < 20 {
            cifras[c] = b'0' + (v % 10) as u8;
            v /= 10;
            c += 1;
        }
        for i in (0..c).rev() {
            if self.n < ANCHO {
                self.b[self.n] = cifras[i];
                self.n += 1;
            }
        }
    }
}

/// El nombre de la excepcion. Un numero de vector no se lo sabe nadie de
/// memoria, y la diferencia entre un `#PF` y un `#GP` es la primera pregunta
/// que se hace quien lee el informe.
fn nombre_vector(v: u64) -> &'static str {
    match v {
        0 => "#DE division por cero",
        6 => "#UD instruccion invalida",
        8 => "#DF doble falta",
        11 => "#NP segmento ausente",
        12 => "#SS fallo de pila",
        13 => "#GP proteccion general",
        14 => "#PF fallo de pagina",
        16 => "#MF error x87",
        19 => "#XM error SSE",
        _ => "excepcion",
    }
}

/// Lo que el codigo de error de un `#PF` significa, en palabras. Son cuatro
/// bits y cada uno cambia el sitio donde hay que mirar.
fn causa_pf(err: u64, r: &mut Renglon) {
    r.s(if err & 1 == 0 { "pagina NO PRESENTE" } else { "violacion de permisos" });
    r.s(if err & 2 == 0 { ", leyendo" } else { ", escribiendo" });
    if err & 4 != 0 {
        r.s(", desde Ring 3");
    }
    if err & 16 != 0 {
        r.s(", buscando INSTRUCCIONES");
    }
}

/// **Guarda la autopsia.** Se llama desde el manejador de faults.
///
/// No devuelve nada y no puede fallar: si no cabe, se corta. Un informe a
/// medias sigue diciendo el vector y el `rip`, que es lo primero que se mira.
#[allow(clippy::too_many_arguments)]
pub fn registrar(
    vector: u64,
    error: u64,
    rip: u64,
    cr2: u64,
    rsp: u64,
    pid: u32,
    tid: u32,
) {
    unsafe {
        if DENTRO {
            return;
        }
        DENTRO = true;
    }

    let mut renglones: [Renglon; RENGLONES] = [
        Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(),
        Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(),
        Renglon::nuevo(),
    ];

    renglones[0].s("== FALLO EN RING 3 #");
    renglones[0].dec(unsafe { TOTAL } as u64 + 1);
    renglones[0].s("  t=");
    renglones[0].dec(timer::ticks());
    renglones[0].s("ms ==");

    renglones[1].s("programa  ");
    // El nombre del `.bex` que se lanzo. Es el dato que convierte "fallo algo"
    // en "fallo ESTO", y es justo el que la linea de CABINA no llevaba.
    let mut visto = false;
    for r in crate::ring0::task::proc::programs() {
        if r.pid == pid {
            renglones[1].s(r.name);
            visto = true;
            break;
        }
    }
    if !visto {
        renglones[1].s("(desconocido)");
    }
    renglones[1].s("   pid ");
    renglones[1].dec(pid as u64);
    renglones[1].s(" tid ");
    renglones[1].dec(tid as u64);

    renglones[2].s("causa     ");
    renglones[2].s(nombre_vector(vector));
    renglones[2].s("  (vector ");
    renglones[2].dec(vector);
    renglones[2].s(")");

    renglones[3].s("codigo    ");
    renglones[3].hex(error);
    if vector == 14 {
        renglones[3].s("  ");
        let (a, b) = renglones.split_at_mut(4);
        let _ = b;
        causa_pf(error, &mut a[3]);
    }

    renglones[4].s("rip       ");
    renglones[4].hex(rip);
    renglones[4].s("   la instruccion que fallo");

    renglones[5].s("direccion ");
    renglones[5].hex(cr2);
    if vector == 14 {
        renglones[5].s("   lo que se intento tocar");
    }

    renglones[6].s("rsp       ");
    renglones[6].hex(rsp);

    // * Y LO QUE EL PROCESO DIJO ANTES DE MORIR.
    //
    // `uconsole` guarda las ultimas lineas que escribio cada proceso, y esa es
    // la unica pista sobre QUE ESTABA HACIENDO. El resto del informe dice donde
    // se rompio la maquina; esta linea dice por donde iba el programa.
    renglones[7].s("ultimo    ");
    if crate::ring0::uconsole::hubo_palabras(pid) {
        // `ultimas_palabras` entrega las que haya, de la mas vieja a la mas
        // nueva. Se queda la ULTIMA: es la que dice hasta donde llego.
        let mut ultima: [u8; ANCHO] = [0; ANCHO];
        let mut largo = 0usize;
        crate::ring0::uconsole::ultimas_palabras(pid, |l| {
            let b = l.as_bytes();
            let n = b.len().min(ANCHO);
            ultima[..n].copy_from_slice(&b[..n]);
            largo = n;
        });
        renglones[7].bytes(&ultima[..largo]);
    } else {
        renglones[7].s("(no escribio nada)");
    }

    // ** Y LA COMPROBACION DE QUE EL KERNEL RECUPERO LO SUYO.
    //
    // `revoke_all` corre ANTES de esto y hace su trabajo. Pero eso es lo que el
    // codigo DICE que hace, y hasta hoy **nadie miraba si funciono**.
    //
    // Una fuga de ranuras no da error: da un sistema que un dia no puede abrir
    // un directorio mas, sin nada que lo relacione con el proceso que murio
    // hace una hora. `AVANCES.md` la lleva abierta desde el 02-08 -- ranuras de
    // directorio que solo se liberan al morir, con un cliente (el escritorio)
    // que no muere nunca.
    //
    // Esta linea la convierte en un numero. Es el escalon 1 de
    // `docs/PLAN_AUTOCURACION.md`, y su regla es la de siempre: **tiene que
    // decir CERO**, y si no lo dice, dice QUE falto.
    let caps = crate::ring0::obj::cap::vivas_de(pid);
    let dirs = crate::ring0::obj::directorio::pendientes_de(pid);
    let archs = crate::ring0::obj::archivo::pendientes_de(pid);
    let pantalla = crate::ring0::obj::fb::owner() == Some(pid);
    // El sonido entra en la cuenta desde el dia que existe la capability, y no
    // hubo que tocar nada mas: un aparato exclusivo que se recupera al morir es
    // exactamente la forma que este recuento ya sabia comprobar.
    let sonido = crate::ring0::obj::audio::owner() == Some(pid);
    let fugas = caps + dirs + archs + pantalla as u32 + sonido as u32;

    renglones[8].s("recursos  ");
    if fugas == 0 {
        renglones[8].s("todo devuelto");
    } else {
        renglones[8].s("*** SIN DEVOLVER:");
        if caps > 0 {
            renglones[8].s(" caps=");
            renglones[8].dec(caps as u64);
        }
        if dirs > 0 {
            renglones[8].s(" directorios=");
            renglones[8].dec(dirs as u64);
        }
        if archs > 0 {
            renglones[8].s(" archivos=");
            renglones[8].dec(archs as u64);
        }
        if pantalla {
            renglones[8].s(" LA PANTALLA");
        }
        if sonido {
            renglones[8].s(" EL SONIDO");
        }
        // Tambien a CABINA: una fuga es un fallo del KERNEL, no del programa
        // que murio, y merece su linea roja aunque nadie abra la autopsia.
        crate::ring0::cabina::warn("autopsia", "el muerto dejo recursos sin devolver", fugas as u64);
    }
    unsafe {
        FUGAS_TOTAL = FUGAS_TOTAL.wrapping_add(fugas);
    }

    unsafe {
        let anillo = &mut *core::ptr::addr_of_mut!(ANILLO);
        let a = &mut anillo[ESCRIBE];
        for i in 0..RENGLONES {
            let n = renglones[i].n.min(ANCHO);
            a.texto[i][..n].copy_from_slice(&renglones[i].b[..n]);
            a.largo[i] = n as u8;
        }
        a.usados = RENGLONES as u8;
        ESCRIBE = (ESCRIBE + 1) % CUANTAS;
        TOTAL = TOTAL.wrapping_add(1);
        DENTRO = false;
    }
}

/// Cuantos fallos van desde el arranque. **Ring 3 mira este numero** para saber
/// si hay uno nuevo sin leer el informe entero: si cambio, hay autopsia nueva.
pub fn total() -> u64 {
    unsafe { TOTAL as u64 }
}

/// Recursos que los muertos dejaron sin devolver desde el arranque.
///
/// **Tiene que ser CERO.** Un numero distinto no acusa al programa que murio:
/// acusa al kernel, que dijo haberlo recuperado todo y no lo hizo.
pub fn fugas() -> u64 {
    unsafe { FUGAS_TOTAL as u64 }
}

/// Cuantos informes se pueden leer ahora.
pub fn disponibles() -> u64 {
    unsafe { (TOTAL as usize).min(CUANTAS) as u64 }
}

/// Cuantos renglones tiene el informe `n` (`0` = el mas reciente).
pub fn renglones(n: u64) -> u64 {
    if n >= disponibles() {
        return 0;
    }
    unsafe {
        let idx = (ESCRIBE + CUANTAS - 1 - (n as usize % CUANTAS)) % CUANTAS;
        let anillo = &*core::ptr::addr_of!(ANILLO);
        anillo[idx].usados as u64
    }
}

/// **Ocho bytes del renglon `fila` del informe `n`**, empaquetados.
///
/// Mismo contrato que `klog::texto` y por el mismo motivo: pasar un puntero de
/// Ring 3 obligaria al kernel a validar el rango contra el espacio del
/// llamante, y esa infraestructura no existe. El cero es el final.
pub fn texto(n: u64, fila: u64, trozo: u64) -> u64 {
    if n >= disponibles() || fila as usize >= RENGLONES {
        return 0;
    }
    unsafe {
        let idx = (ESCRIBE + CUANTAS - 1 - (n as usize % CUANTAS)) % CUANTAS;
        let anillo = &*core::ptr::addr_of!(ANILLO);
        let a = &anillo[idx];
        let largo = a.largo[fila as usize] as usize;
        let base = (trozo as usize).saturating_mul(8);
        let mut w = [0u8; 8];
        for i in 0..8 {
            match a.texto[fila as usize].get(base + i) {
                Some(&c) if base + i < largo => w[i] = c,
                _ => break,
            }
        }
        u64::from_le_bytes(w)
    }
}
