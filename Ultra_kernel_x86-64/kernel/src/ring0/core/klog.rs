//! **El log del kernel, GUARDADO** -- para que Ring 3 pueda leerlo.
//!
//! === El hueco que tapa ===
//!
//! Hasta ahora las lineas del `KERNEL LOG` se pintaban **directamente en el
//! framebuffer** y no se guardaban en ninguna parte: la unica copia era la
//! imagen en pantalla. Mientras el panel del kernel fue lo unico que habia, eso
//! bastaba. Dejo de bastar el dia que el escritorio paso a ser el arranque --
//! desde que el compositor reclama la pantalla, **el panel del kernel no se
//! pinta**, y con el desaparece el relato entero de como arranco la maquina.
//!
//! Y no es un problema teorico: es exactamente lo que bloqueo una sesion de
//! depuracion. La linea que decidia entre dos culpables --`doble bufer:
//! pintando fuera de la pantalla` o `SIN doble bufer`-- se escribia en un sitio
//! que ya nadie podia mirar. Un dato que existe y no se puede leer no esta.
//!
//! === Lo que NO es ===
//!
//! **Esto no da privilegio, da VISTA.** Ring 3 no pasa a ejecutar nada en Ring
//! 0: pide una linea de texto por su numero y recibe bytes. Es la misma forma
//! que `TASK_OP_INFO` --el kernel contesta preguntas y no cede poder-- y es a
//! proposito: en un sistema de capabilities, *ver* y *poder* son cosas
//! separadas, y confundirlas es como se acaba con un "modo administrador".
//!
//! Tampoco sustituye a [`crate::ring0::cabina`]. CABINA es el **narrador**:
//! eventos con severidad, capa y valor, pensados para un cockpit. Esto es la
//! **transcripcion**: el texto tal cual salio, en orden, incluido todo lo que
//! los drivers escupen y que nunca fue un evento. Dos cosas distintas con dos
//! usos distintos; la primera se lee de un vistazo y la segunda se lee cuando
//! algo ya ha fallado.
//!
//! === El tamano, y por que ese ===
//!
//! 64 lineas de 96 bytes son 6 KiB de `.bss`. El arranque completo escupe
//! bastante mas que 64 lineas, asi que **el anillo tira las viejas** -- y eso es
//! lo correcto para lo que sirve: cuando algo falla, lo que importa es lo
//! ultimo que paso. El contador total no se pierde, asi que se puede saber
//! cuantas se cayeron por el borde.

/// Cuantas lineas se guardan. Ver la cabecera.
pub const LINEAS: usize = 64;
/// Cuanto mide una linea. El mismo `KEEP` que usa la deteccion de repetidas en
/// `phase::dash_log`, que es de donde viene el texto.
pub const ANCHO: usize = 96;

static mut TEXTO: [[u8; ANCHO]; LINEAS] = [[0; ANCHO]; LINEAS];
static mut LARGO: [u8; LINEAS] = [0; LINEAS];
/// Donde va la proxima.
static mut WRITES: usize = 0;
/// Cuantas se han guardado desde el arranque. **No baja.** Restarle [`LINEAS`]
/// da cuantas se cayeron por el borde del anillo.
static mut TOTAL: u64 = 0;

/// Guarda una linea. La llama `phase::dash_log`, que es quien ya tiene el texto
/// en la mano justo antes de pintarlo.
///
/// Se guarda **lo mismo que se pinta**, y eso importa: un log que guarda una
/// version distinta de la que se ve es un log en el que no se puede confiar
/// para comparar una foto con lo leido.
pub fn save(msg: &str) {
    write(msg.as_bytes());
}

/// Igual, pero con la HORA delante: `[  1234ms] usb: puerto listo`.
///
/// ** Existe porque el arranque de BMO-X **no estaba cronometrado en ninguna
/// parte**. Se sabia que tarda "unos diez segundos" y no se sabia en que, y
/// optimizar sin ese numero es mover cosas a ver si suena distinto. Lo primero
/// que hay que poder descartar es que esos segundos sean del firmware de la
/// placa --el POST de una A320M se come diez el solo-- en cuyo caso aqui no hay
/// nada que arreglar.
///
/// Con esto, **una sola foto de F11 dice donde se van**, linea por linea, sin
/// anadir un solo cronometro: `timer::ticks()` ya corria.
///
/// El campo son seis cifras a la derecha --hasta 999999 ms, dieciseis minutos--
/// y por delante para que las columnas cuadren y el ojo compare dos lineas sin
/// leerlas. Un numero alineado se resta de un vistazo.
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
    // ensancha sola: cortar un numero por la izquierda lo convierte en otro.
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
    write(&linea[..n + k]);
}

fn write(b: &[u8]) {
    let n = b.len().min(ANCHO);
    unsafe {
        let t = &mut *core::ptr::addr_of_mut!(TEXTO);
        let l = &mut *core::ptr::addr_of_mut!(LARGO);
        t[WRITES][..n].copy_from_slice(&b[..n]);
        l[WRITES] = n as u8;
        WRITES = (WRITES + 1) % LINEAS;
        TOTAL = TOTAL.wrapping_add(1);
    }
}

/// Cuantas lineas se pueden leer ahora mismo (nunca mas de [`LINEAS`]).
pub fn disponibles() -> u64 {
    unsafe { TOTAL.min(LINEAS as u64) }
}

/// Cuantas se han escrito desde el arranque, incluidas las que ya no estan.
pub fn total() -> u64 {
    unsafe { TOTAL }
}

/// **Ocho bytes de la linea `n`**, empaquetados en little-endian.
///
/// `n = 0` es **la mas reciente** y hacia arriba se va hacia atras en el
/// tiempo. Es el mismo criterio que `cabina::event_back`, y no es capricho:
/// quien lee un log quiere lo ultimo primero, y numerar desde el principio
/// obligaria a saber cuantas hay antes de poder pedir ninguna.
///
/// `trozo` cuenta de 8 en 8. Fuera del texto devuelve 0, y **el cero es el
/// final** -- igual que en `report::texto` y por el mismo motivo: pasar un
/// puntero de Ring 3 obligaria al kernel a validar el rango entero contra el
/// espacio del llamante, y esa infraestructura no existe.
///
/// * Ojo a la consecuencia: una linea que contenga un byte cero se corta ahi.
/// No pasa, porque estas lineas son texto ASCII de `&str`, pero esta dicho.
pub fn texto(n: u64, trozo: u64) -> u64 {
    let hay = disponibles();
    if n >= hay {
        return 0;
    }
    unsafe {
        // De "la n-esima hacia atras" al indice del anillo.
        let idx = (WRITES + LINEAS - 1 - (n as usize % LINEAS)) % LINEAS;
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
