//! El log del kernel, la memoria que se pide, y las salidas de texto.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

pub fn klog_texto(n: u64, dst: &mut [u8]) -> usize {
    let mut escritos = 0usize;
    let mut trozo = 0u64;
    while escritos < dst.len() {
        let w = invoke(CURRENT_TASK, OP_KLOG_TEXTO, n, trozo, 0).value;
        if w == 0 {
            break;
        }
        for k in 0..8 {
            let b = ((w >> (k * 8)) & 0xFF) as u8;
            if b == 0 || escritos >= dst.len() {
                return escritos;
            }
            dst[escritos] = b;
            escritos += 1;
        }
        trozo += 1;
    }
    escritos
}

/// **Un bloque de memoria pedido al kernel.**
///
/// * Esto NO es un `malloc` y no lo pretende. Es memoria entregada entera:
/// pides una vez, te dan un bloque contiguo, y **no hay forma de devolverlo**
/// -- vive hasta que el proceso muere.
///
/// El asignador se escribe ENCIMA, aqui en Ring 3, con la politica que quiera
/// cada uno. Esa es la razon de que el kernel no traiga uno: un `malloc`
/// general dentro del kernel seria escribir una politica que el programa de al
/// lado no usa, y encima cobrarsela con un syscall por llamada.
///
/// El caso que lo decidio: DOOM pide ~8 MiB una vez al arrancar y se los
/// administra el con su `Z_Zone`. Para eso, esto es exactamente lo que hace
/// falta y ni un byte mas.
pub struct Memoria {
    cap: u64,
    base: u64,
    bytes: u64,
}

impl Memoria {
    /// Pide `bytes`. `None` si no hay RAM contigua, si pasa del tope por
    /// peticion (64 MiB) o si este proceso ya gasto sus cuatro peticiones.
    pub fn request(bytes: u64) -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_MEMORIA_PEDIR, bytes, 0, 0).valor()?;
        let base = invoke(cap, MEM_OP_BASE, 0, 0, 0).valor()?;
        Some(Self { cap, base, bytes })
    }

    /// La direccion del primer byte.
    ///
    /// Esta MAPEADO: a partir de aqui se escribe con `mov` y el kernel no se
    /// entera de nada. Un syscall por byte seria justo lo contrario de
    /// entregar memoria.
    pub fn base(&self) -> *mut u8 {
        self.base as *mut u8
    }

    /// Lo que se pidio.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Lo que el kernel dice que lleva entregado a este proceso -- que puede ser
    /// mas que `bytes()` si se pidio varias veces, y siempre esta redondeado a
    /// paginas enteras.
    pub fn entregado(&self) -> u64 {
        invoke(self.cap, MEM_OP_BYTES, 0, 0, 0).value
    }
}

/// Un campo de TEXTO en `dst`. Devuelve cuantos bytes se escribieron.
///
/// Viaja de 8 en 8 con el cero como final, igual que la ruta de `ejecutar`: en
/// esta superficie no hay punteros de Ring 3 hacia el kernel.
pub fn info_texto(campo: u64, dst: &mut [u8]) -> usize {
    let mut n = 0usize;
    let mut trozo = 0u64;
    while n < dst.len() {
        let w = invoke(CURRENT_TASK, OP_INFO_TEXTO, campo, trozo, 0).value;
        if w == 0 {
            break;
        }
        for k in 0..8 {
            let b = ((w >> (k * 8)) & 0xFF) as u8;
            if b == 0 || n >= dst.len() {
                return n;
            }
            dst[n] = b;
            n += 1;
        }
        trozo += 1;
    }
    n
}

/// Reiniciar la maquina. No vuelve.
///
/// Reiniciar es tocar puertos de E/S (`0xCF9`, el 8042) y Ring 3 no puede
/// hacerlo: se le pide al kernel, que ya tenia el reinicio de tres pasos para
/// su propio shell. Si volviera --no deberia-- se cede el turno en vez de seguir
/// como si nada, por la misma razon que en [`salir`].
pub fn reiniciar() -> ! {
    invoke(CURRENT_TASK, OP_REINICIAR, 0, 0, 0);
    loop {
        yield_screen();
    }
}

/// Escribir en la consola del kernel.
///
/// La puerta admite 8 bytes empaquetados en little-endian por llamada, con el
/// cero como final. Es deliberadamente pobre: es la consola de arranque, no la
/// salida de nadie en serio. Cuando el compositor exista, el terminal sera un
/// proceso Ring 3 y esto quedara para lo que es -- decir "estoy vivo" antes de
/// que haya con que decirlo.
pub fn consola(texto: &str) {
    for trozo in texto.as_bytes().chunks(8) {
        let mut w = [0u8; 8];
        w[..trozo.len()].copy_from_slice(trozo);
        invoke(CURRENT_TASK, OP_CONSOLE_WRITE, u64::from_le_bytes(w), 0, 0);
    }
}

