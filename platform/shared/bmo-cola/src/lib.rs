//! **LA COLA DE UN PRODUCTOR Y UN CONSUMIDOR** -- que pueden estar en nucleos
//! distintos, y por eso ninguno toca el indice del otro.
//!
//! [carril]  AMARILLO  no toca hardware: dos indices atomicos y un array. Es
//!                     peligrosa de USAR MAL: dos productores a la vez la rompen
//! [consumo] NADA      corre cuando alguien empuja o saca
//!
//! [cuesta]  DATO -- si se llena, se pierde UN evento (una tecla, un
//!           caracter) y se cuenta. No se pierde el sistema ni el aparato:
//!           se pierde lo que ese evento decia (L6e)
//!
//! [riesgo]  SILENCIO -- una cola que se llena no avisa a nadie por si sola:
//!           cuenta `perdidos` y sigue. Si nadie mira ese numero, una tecla
//!           que no llego y una tecla que no se pulso se ven igual (L6f)
//!
//! # *** POR QUE EXISTE (2026-09-18): el bus USB quiere su propio nucleo
//!
//! El paso A0 de `docs/plan/PLAN_EL_BUS_APARTE.md`. El hilo del bus escribe
//! las teclas y los caracteres en dos colas de `static mut` con un indice de
//! lectura y otro de escritura, y el escritorio las lee desde un syscall.
//! Hoy es el MISMO nucleo y nunca se pisan. El dia que el bus viva en otro
//! nucleo, esas colas son una carrera -- y la cola cruda era peor: cuando se
//! llenaba, **el productor movia el indice del consumidor** para tirar lo mas
//! viejo. Con dos nucleos eso es escribir el indice que el otro esta leyendo.
//!
//! # El contrato, que es lo unico que la hace segura
//!
//! ```text
//!    UN productor    llama a `empujar`. Solo el toca `escribe`.
//!    UN consumidor   llama a `sacar` y `vaciar`. Solo el toca `lee`.
//!    lleno           se tira LO NUEVO y se cuenta. El productor no puede
//!                    tirar lo viejo: lo viejo es del consumidor.
//! ```
//!
//! Los dos indices son atomicos con Release al escribir y Acquire al leer: el
//! dato se escribe ANTES de que el indice lo anuncie, y el otro nucleo lo ve
//! entero o no lo ve. Sin cerrojo, sin `cli`, sin esperar a nadie.
//!
//! ** Y "se tira lo nuevo" es un cambio de politica a proposito. La cola cruda
//! de teclas tiraba lo viejo con un argumento bueno --un `soltar` viejo que se
//! pierde deja una tecla pegada--, pero tirar lo viejo TAMBIEN tira soltares
//! viejos: las dos politicas pierden lo mismo cuando se llenan, y solo una se
//! puede hacer desde otro nucleo. La respuesta correcta a una cola que se
//! llena no es elegir que tirar: es que `perdidos` sea cero, y para eso hay 64
//! sitios y un consumidor que drena por fotograma.

#![no_std]

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// La cola: `N` sitios, de los que se usan `N - 1` (uno queda libre para
/// distinguir llena de vacia sin un contador compartido).
pub struct Cola<T: Copy, const N: usize> {
    datos: UnsafeCell<[T; N]>,
    /// Lo mueve SOLO el consumidor.
    lee: AtomicUsize,
    /// Lo mueve SOLO el productor.
    escribe: AtomicUsize,
    /// Lo que no cupo. Lo suma el productor; lo lee quien quiera.
    perdidos: AtomicU32,
}

// Un productor y un consumidor en nucleos distintos es exactamente para lo
// que esta hecha: los indices son atomicos y cada lado escribe solo el suyo.
// Lo que NO cubre es dos productores, y eso es un contrato de quien la usa.
unsafe impl<T: Copy + Send, const N: usize> Sync for Cola<T, N> {}

impl<T: Copy, const N: usize> Cola<T, N> {
    /// Una cola vacia. `vacio` es el valor con el que se rellena el array:
    /// nunca se entrega, solo hace falta para construir en un `static`.
    pub const fn nueva(vacio: T) -> Self {
        Self {
            datos: UnsafeCell::new([vacio; N]),
            lee: AtomicUsize::new(0),
            escribe: AtomicUsize::new(0),
            perdidos: AtomicU32::new(0),
        }
    }

    /// **El productor mete uno.** `false` si no cabia: se tira ESTE y se
    /// cuenta.
    pub fn empujar(&self, v: T) -> bool {
        let e = self.escribe.load(Ordering::Relaxed);
        let siguiente = (e + 1) % N;
        if siguiente == self.lee.load(Ordering::Acquire) {
            self.perdidos.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        // El dato primero, el indice despues (Release): quien vea el indice
        // nuevo ve el dato entero.
        unsafe { (*self.datos.get())[e] = v };
        self.escribe.store(siguiente, Ordering::Release);
        true
    }

    /// **El consumidor saca el mas viejo**, o `None` si no hay.
    pub fn sacar(&self) -> Option<T> {
        let l = self.lee.load(Ordering::Relaxed);
        if l == self.escribe.load(Ordering::Acquire) {
            return None;
        }
        let v = unsafe { (*self.datos.get())[l] };
        self.lee.store((l + 1) % N, Ordering::Release);
        Some(v)
    }

    /// **El consumidor tira todo lo que hay** y dice cuantos eran.
    pub fn vaciar(&self) -> usize {
        let l = self.lee.load(Ordering::Relaxed);
        let e = self.escribe.load(Ordering::Acquire);
        let habia = if e >= l { e - l } else { N - l + e };
        self.lee.store(e, Ordering::Release);
        habia
    }

    /// Cuantos hay ahora mismo (una foto: puede cambiar al instante).
    pub fn cuantos(&self) -> usize {
        let l = self.lee.load(Ordering::Acquire);
        let e = self.escribe.load(Ordering::Acquire);
        if e >= l { e - l } else { N - l + e }
    }

    /// Cuantos no cupieron desde el arranque. Tiene que ser cero.
    pub fn perdidos(&self) -> u32 {
        self.perdidos.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sale_en_el_orden_en_que_entro() {
        let c: Cola<u16, 4> = Cola::nueva(0);
        assert!(c.empujar(1));
        assert!(c.empujar(2));
        assert!(c.empujar(3));
        assert_eq!(c.cuantos(), 3);
        assert_eq!(c.sacar(), Some(1));
        assert_eq!(c.sacar(), Some(2));
        assert_eq!(c.sacar(), Some(3));
        assert_eq!(c.sacar(), None);
    }

    /// *** LLENA: se tira LO NUEVO y se cuenta. Lo viejo es del consumidor.
    #[test]
    fn llena_tira_lo_nuevo_y_lo_cuenta() {
        let c: Cola<u16, 4> = Cola::nueva(0);
        assert!(c.empujar(1));
        assert!(c.empujar(2));
        assert!(c.empujar(3));
        assert!(!c.empujar(4));
        assert_eq!(c.perdidos(), 1);
        assert_eq!(c.sacar(), Some(1));
        // Ya hay sitio otra vez.
        assert!(c.empujar(5));
        assert_eq!(c.sacar(), Some(2));
        assert_eq!(c.sacar(), Some(3));
        assert_eq!(c.sacar(), Some(5));
    }

    #[test]
    fn da_la_vuelta_sin_perder_ni_repetir() {
        let c: Cola<u32, 8> = Cola::nueva(0);
        let mut n = 0u32;
        for vuelta in 0..100 {
            for _ in 0..5 {
                assert!(c.empujar(n), "vuelta {vuelta}");
                n += 1;
            }
            let mut esperado = n - 5;
            while let Some(v) = c.sacar() {
                assert_eq!(v, esperado);
                esperado += 1;
            }
            assert_eq!(esperado, n);
        }
        assert_eq!(c.perdidos(), 0);
    }

    #[test]
    fn vaciar_dice_cuantos_habia() {
        let c: Cola<u8, 8> = Cola::nueva(0);
        assert_eq!(c.vaciar(), 0);
        for i in 0..6 {
            c.empujar(i);
        }
        // Que de la vuelta al array antes de vaciar.
        for _ in 0..4 {
            c.sacar();
        }
        for i in 0..5 {
            c.empujar(i);
        }
        assert_eq!(c.vaciar(), 7);
        assert_eq!(c.sacar(), None);
    }

    /// El uso real: un hilo empuja, otro saca, y no se pierde ni se repite
    /// nada. En el anfitrion son dos hilos; en la maquina seran dos nucleos.
    #[test]
    fn un_productor_y_un_consumidor_en_hilos_distintos() {
        extern crate std;
        static COLA: Cola<u32, 64> = Cola::nueva(0);
        let total = 20_000u32;
        let productor = std::thread::spawn(move || {
            let mut i = 0;
            let mut rechazos = 0u32;
            while i < total {
                if COLA.empujar(i) {
                    i += 1;
                } else {
                    rechazos += 1;
                    std::thread::yield_now();
                }
            }
            rechazos
        });
        let consumidor = std::thread::spawn(move || {
            let mut esperado = 0;
            while esperado < total {
                if let Some(v) = COLA.sacar() {
                    assert_eq!(v, esperado);
                    esperado += 1;
                } else {
                    std::thread::yield_now();
                }
            }
            esperado
        });
        // Cada rechazo se conto: `perdidos` es exactamente lo que el
        // productor tuvo que reintentar, ni uno mas.
        let rechazos = productor.join().unwrap();
        assert_eq!(consumidor.join().unwrap(), total);
        assert_eq!(COLA.perdidos(), rechazos);
    }
}
