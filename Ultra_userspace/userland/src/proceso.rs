//! Lanzar un programa y hablar con su consola.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

// ── Lanzar un programa ──────────────────────────────────────────────────

/// Lanza el `.bex` que hay en `ruta`. Devuelve el tid, o el código del fallo.
///
/// La ruta viaja en trozos de 8 bytes, igual que la consola y por la misma
/// razón: los argumentos van en registros y el kernel no tiene todavía forma de
/// leer un puntero de Ring 3 con seguridad. Ver `TASK_OP_RUTA` en el kernel.
///
/// El gate de firma de ESTRATOS se aplica al otro lado: un binario sin firma
/// buena no se ejecuta por mucho que se pida desde aquí.
pub fn ejecutar(ruta: &[u8]) -> Result<u64, u32> {
    ejecutar_en(ruta, 0)
}

/// Igual, pero la salida del hijo va a `consola` en vez de al panel del kernel.
///
/// Esto es lo que separa un lanzador de un TERMINAL: sin la consola el programa
/// se ejecuta y su salida cae donde no la ves; con ella, aterriza en un anillo
/// que tú drenas. `consola = 0` es el comportamiento de siempre.
pub fn ejecutar_en(ruta: &[u8], consola: u64) -> Result<u64, u32> {
    for trozo in ruta.chunks(8) {
        let mut w = [0u8; 8];
        w[..trozo.len()].copy_from_slice(trozo);
        invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
    }
    let st = invoke(CURRENT_TASK, OP_EJECUTAR, 0, consola, 0);
    if st.ok() {
        Ok(st.value)
    } else {
        Err(st.code)
    }
}

// ── La consola ──────────────────────────────────────────────────────────

/// La salida de los programas que este proceso lance.
///
/// Cierra la última asimetría del sistema: la pantalla y la entrada ya eran
/// capabilities, y la consola era un global fijo — por eso un terminal no podía
/// leer lo que imprimía su propio hijo. Ahora la crea quien va a leerla.
pub struct Consola {
    pub cap: u64,
}

impl Consola {
    pub fn crear() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_CONSOLA_CREAR, 0, 0, 0).valor()?;
        Some(Self { cap })
    }

    /// Hasta **7** bytes de salida. Devuelve cuántos son válidos; `0` = no hay
    /// nada. **No bloquea**, igual que `tecla()` y por la misma razón: un
    /// terminal tiene bucle de fotograma y dormirse aquí congelaría el cursor.
    ///
    /// Siete y no ocho porque el octavo lleva el contador — ver
    /// `CONSOLA_OP_LEER` en el kernel.
    pub fn leer(&self, dst: &mut [u8; 8]) -> usize {
        let v = invoke(self.cap, CONSOLA_OP_LEER, 0, 0, 0).value;
        *dst = v.to_le_bytes();
        let n = (v >> 56) as usize;
        dst[7] = 0;
        n.min(7)
    }

    /// Mete texto en la ENTRADA de esta consola: lo que el terminal teclea
    /// PARA su hijo. El otro sentido del canal.
    pub fn escribir(&self, texto: &[u8]) {
        for trozo in texto.chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            invoke(self.cap, CONSOLA_OP_ESCRIBIR, u64::from_le_bytes(w), 0, 0);
        }
    }

    /// ¿Hay un proceso vivo escribiendo aquí? El terminal lo pregunta para
    /// saber si lo que se teclea es un comando suyo o entrada para el hijo.
    pub fn hay_hijo(&self) -> bool {
        invoke(self.cap, CONSOLA_OP_HAY_HIJO, 0, 0, 0).value != 0
    }

    /// Bytes descartados por anillo lleno. Un terminal que va lento tiene
    /// derecho a saber que está perdiendo salida en vez de creerse completo.
    pub fn perdidos(&self) -> u64 {
        invoke(self.cap, CONSOLA_OP_PERDIDOS, 0, 0, 0).value
    }
}

/// Los códigos que devuelve `ejecutar`. Son pocos a propósito: un proceso no
/// necesita saber de FAT32, pero sí distinguir las tres cosas que le hacen
/// cambiar de conducta.
/// Los motivos por los que no se pudo abrir o crear un archivo.
///
/// Son varios y no uno porque cada uno manda a hacer algo DISTINTO: crear la
/// carpeta, acortar el nombre, o mirar si te equivocaste de sitio. Un unico
/// "no se pudo" manda a buscar donde no es.
pub const ERROR_ARCH_SIN_HUECO: u32 = 27;
pub const ERROR_ARCH_NO_ESTA: u32 = 28;
pub const ERROR_ARCH_GRANDE: u32 = 29;
pub const ERROR_ARCH_NOMBRE: u32 = 30;
pub const ERROR_ARCH_SOLO_LECTURA: u32 = 31;
pub const ERROR_ARCH_CARPETA: u32 = 32;
pub const ERROR_ARCH_ES_CARPETA: u32 = 33;

pub const ERROR_NO_ESTA: u32 = 20;
pub const ERROR_GATE: u32 = 21;
pub const ERROR_OCUPADO: u32 = 22;
pub const ERROR_NO_ADMITIDO: u32 = 23;

/// Lee de la consola de ESTE proceso — lo que el terminal le manda.
///
/// Hasta 7 bytes; `0` = no hay nada. **No bloquea**: un programa que espera
/// entrada cede el turno entre intentos en vez de quedarse con el CPU.
///
/// Es la pareja de `consola()`: se escribe por una y se escucha por la otra,
/// sobre el mismo objeto. Es lo que permite un `ACCEPT` en un proceso que no
/// tiene —ni debe tener— la capability del teclado.
pub fn leer_consola(dst: &mut [u8; 8]) -> usize {
    let v = invoke(CURRENT_TASK, OP_CONSOLE_READ, 0, 0, 0).value;
    *dst = v.to_le_bytes();
    let n = (v >> 56) as usize;
    dst[7] = 0;
    n.min(7)
}

