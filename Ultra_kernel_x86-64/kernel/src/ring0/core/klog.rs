//! **El log del kernel, GUARDADO** — para que Ring 3 pueda leerlo.
//!
//! ═══ El hueco que tapa ═══
//!
//! Hasta ahora las líneas del `KERNEL LOG` se pintaban **directamente en el
//! framebuffer** y no se guardaban en ninguna parte: la única copia era la
//! imagen en pantalla. Mientras el panel del kernel fue lo único que había, eso
//! bastaba. Dejó de bastar el día que el escritorio pasó a ser el arranque —
//! desde que el compositor reclama la pantalla, **el panel del kernel no se
//! pinta**, y con él desaparece el relato entero de cómo arrancó la máquina.
//!
//! Y no es un problema teórico: es exactamente lo que bloqueó una sesión de
//! depuración. La línea que decidía entre dos culpables —`doble bufer:
//! pintando fuera de la pantalla` o `SIN doble bufer`— se escribía en un sitio
//! que ya nadie podía mirar. Un dato que existe y no se puede leer no está.
//!
//! ═══ Lo que NO es ═══
//!
//! **Esto no da privilegio, da VISTA.** Ring 3 no pasa a ejecutar nada en Ring
//! 0: pide una línea de texto por su número y recibe bytes. Es la misma forma
//! que `TASK_OP_INFO` —el kernel contesta preguntas y no cede poder— y es a
//! propósito: en un sistema de capabilities, *ver* y *poder* son cosas
//! separadas, y confundirlas es como se acaba con un "modo administrador".
//!
//! Tampoco sustituye a [`crate::ring0::cabina`]. CABINA es el **narrador**:
//! eventos con severidad, capa y valor, pensados para un cockpit. Esto es la
//! **transcripción**: el texto tal cual salió, en orden, incluido todo lo que
//! los drivers escupen y que nunca fue un evento. Dos cosas distintas con dos
//! usos distintos; la primera se lee de un vistazo y la segunda se lee cuando
//! algo ya ha fallado.
//!
//! ═══ El tamaño, y por qué ese ═══
//!
//! 64 líneas de 96 bytes son 6 KiB de `.bss`. El arranque completo escupe
//! bastante más que 64 líneas, así que **el anillo tira las viejas** — y eso es
//! lo correcto para lo que sirve: cuando algo falla, lo que importa es lo
//! último que pasó. El contador total no se pierde, así que se puede saber
//! cuántas se cayeron por el borde.

/// Cuántas líneas se guardan. Ver la cabecera.
pub const LINEAS: usize = 64;
/// Cuánto mide una línea. El mismo `KEEP` que usa la detección de repetidas en
/// `phase::dash_log`, que es de donde viene el texto.
pub const ANCHO: usize = 96;

static mut TEXTO: [[u8; ANCHO]; LINEAS] = [[0; ANCHO]; LINEAS];
static mut LARGO: [u8; LINEAS] = [0; LINEAS];
/// Dónde va la próxima.
static mut ESCRIBE: usize = 0;
/// Cuántas se han guardado desde el arranque. **No baja.** Restarle [`LINEAS`]
/// da cuántas se cayeron por el borde del anillo.
static mut TOTAL: u64 = 0;

/// Guarda una línea. La llama `phase::dash_log`, que es quien ya tiene el texto
/// en la mano justo antes de pintarlo.
///
/// Se guarda **lo mismo que se pinta**, y eso importa: un log que guarda una
/// versión distinta de la que se ve es un log en el que no se puede confiar
/// para comparar una foto con lo leído.
pub fn guardar(msg: &str) {
    escribir(msg.as_bytes());
}

/// Igual, pero con la HORA delante: `[  1234ms] usb: puerto listo`.
///
/// ★★ Existe porque el arranque de BMO-X **no estaba cronometrado en ninguna
/// parte**. Se sabía que tarda "unos diez segundos" y no se sabía en qué, y
/// optimizar sin ese número es mover cosas a ver si suena distinto. Lo primero
/// que hay que poder descartar es que esos segundos sean del firmware de la
/// placa —el POST de una A320M se come diez él solo— en cuyo caso aquí no hay
/// nada que arreglar.
///
/// Con esto, **una sola foto de F11 dice dónde se van**, línea por línea, sin
/// añadir un solo cronómetro: `timer::ticks()` ya corría.
///
/// El campo son seis cifras a la derecha —hasta 999999 ms, dieciséis minutos—
/// y por delante para que las columnas cuadren y el ojo compare dos líneas sin
/// leerlas. Un número alineado se resta de un vistazo.
pub fn guardar_con_hora(ticks: u64, msg: &str) {
    let mut linea = [0u8; ANCHO];
    let mut n = 0usize;
    linea[n] = b'[';
    n += 1;

    let ms = ticks;
    let mut cifras = [0u8; 8];
    let mut c = 0usize;
    let mut v = ms;
    // El cero necesita su cifra: un bucle que divide no la produce.
    if v == 0 {
        cifras[0] = b'0';
        c = 1;
    }
    while v > 0 && c < cifras.len() {
        cifras[c] = b'0' + (v % 10) as u8;
        v /= 10;
        c += 1;
    }
    // Relleno a seis para que las columnas cuadren, y si se pasa de seis se
    // ensancha sola: cortar un número por la izquierda lo convierte en otro.
    let ancho_num = if c < 6 { 6 } else { c };
    for _ in c..ancho_num {
        linea[n] = b' ';
        n += 1;
    }
    for i in (0..c).rev() {
        linea[n] = cifras[i];
        n += 1;
    }
    for &b in b"ms] " {
        linea[n] = b;
        n += 1;
    }
    let cabe = ANCHO - n;
    let b = msg.as_bytes();
    let k = b.len().min(cabe);
    linea[n..n + k].copy_from_slice(&b[..k]);
    escribir(&linea[..n + k]);
}

fn escribir(b: &[u8]) {
    let n = b.len().min(ANCHO);
    unsafe {
        let t = &mut *core::ptr::addr_of_mut!(TEXTO);
        let l = &mut *core::ptr::addr_of_mut!(LARGO);
        t[ESCRIBE][..n].copy_from_slice(&b[..n]);
        l[ESCRIBE] = n as u8;
        ESCRIBE = (ESCRIBE + 1) % LINEAS;
        TOTAL = TOTAL.wrapping_add(1);
    }
}

/// Cuántas líneas se pueden leer ahora mismo (nunca más de [`LINEAS`]).
pub fn disponibles() -> u64 {
    unsafe { TOTAL.min(LINEAS as u64) }
}

/// Cuántas se han escrito desde el arranque, incluidas las que ya no están.
pub fn total() -> u64 {
    unsafe { TOTAL }
}

/// **Ocho bytes de la línea `n`**, empaquetados en little-endian.
///
/// `n = 0` es **la más reciente** y hacia arriba se va hacia atrás en el
/// tiempo. Es el mismo criterio que `cabina::event_back`, y no es capricho:
/// quien lee un log quiere lo último primero, y numerar desde el principio
/// obligaría a saber cuántas hay antes de poder pedir ninguna.
///
/// `trozo` cuenta de 8 en 8. Fuera del texto devuelve 0, y **el cero es el
/// final** — igual que en `informe::texto` y por el mismo motivo: pasar un
/// puntero de Ring 3 obligaría al kernel a validar el rango entero contra el
/// espacio del llamante, y esa infraestructura no existe.
///
/// ★ Ojo a la consecuencia: una línea que contenga un byte cero se corta ahí.
/// No pasa, porque estas líneas son texto ASCII de `&str`, pero está dicho.
pub fn texto(n: u64, trozo: u64) -> u64 {
    let hay = disponibles();
    if n >= hay {
        return 0;
    }
    unsafe {
        // De "la n-ésima hacia atrás" al índice del anillo.
        let idx = (ESCRIBE + LINEAS - 1 - (n as usize % LINEAS)) % LINEAS;
        let t = &*core::ptr::addr_of!(TEXTO);
        let l = &*core::ptr::addr_of!(LARGO);
        let largo = l[idx] as usize;
        let base = (trozo as usize).saturating_mul(8);
        let mut w = [0u8; 8];
        for i in 0..8 {
            match t[idx].get(base + i) {
                Some(&c) if base + i < largo => w[i] = c,
                _ => break,
            }
        }
        u64::from_le_bytes(w)
    }
}
