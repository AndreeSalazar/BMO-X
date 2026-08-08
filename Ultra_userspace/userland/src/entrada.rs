//! La ENTRADA: el puntero, las teclas y la rueda.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

// -- La entrada ----------------------------------------------------------

/// Donde esta el puntero y que botones tiene pulsados.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Punto {
    pub x: u32,
    pub y: u32,
    pub botones: u8,
}

/// La entrada --raton **y teclado**-- cedida a este proceso.
///
/// El kernel lee el HID --transferencias xHCI, endpoints, reintentos-- porque
/// eso es tocar hardware y todavia no hay otro sitio donde pueda vivir. Lo que
/// entrega son coordenadas ya recortadas al panel, una mascara de botones y los
/// bytes que van saliendo del teclado.
///
/// **El cursor no sale de aqui, y las letras tampoco.** Su forma, su color y su
/// contorno son decisiones de aspecto, y ninguna de esas tiene nada que hacer
/// en Ring 0.
///
/// * Reclamarla es EXCLUSIVO y tiene consecuencia: mientras este proceso la
/// tenga, el shell de Ring 0 deja de leer el teclado fisico. No es un reparto,
/// es una cesion -- si los dos leyeran la misma cola se repartirian las letras.
pub struct Entrada {
    pub cap: u64,
}

impl Entrada {
    pub fn reclamar() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_INPUT_CLAIM, 0, 0, 0).valor()?;
        Some(Self { cap })
    }

    /// * **Soltarla y seguir vivo.** Consume la `Entrada`, igual que
    /// `Pantalla::soltar`.
    ///
    /// Existe por un fallo concreto: el escritorio aprendio a prestar la PANTALLA
    /// y se quedo la ENTRADA, asi que el programa al que se la prestaba pintaba
    /// perfectamente y **no podia leer su propia tecla de salida**. Se quedaba
    /// dentro para siempre y la maquina sin teclado.
    ///
    /// **Ceder la pantalla sin ceder la entrada no es prestar: es dejar a alguien
    /// pintando en una habitacion cerrada.** Las dos van juntas.
    ///
    /// Devuelve `false` si no era el dueno, en vez de fingir que la solto.
    pub fn soltar(self) -> bool {
        invoke(CURRENT_TASK, OP_ENTRADA_SOLTAR, 0, 0, 0).valor().is_some()
    }

    /// Una llamada por fotograma: los tres datos vienen empaquetados.
    pub fn puntero(&self) -> Punto {
        let v = invoke(self.cap, INPUT_OP_PUNTERO, 0, 0, 0).value;
        Punto {
            x: (v >> 32) as u32,
            y: ((v >> 16) & 0xFFFF) as u32,
            botones: (v & 0xFF) as u8,
        }
    }

    /// Cuantos reportes HID se han visto. Distingue "el raton no se mueve" de
    /// "el raton no llega": si esto no sube, el problema esta en el USB.
    pub fn eventos(&self) -> u64 {
        invoke(self.cap, INPUT_OP_EVENTOS, 0, 0, 0).value
    }

    /// Las vueltas de rueda desde la ultima vez. Positivo = hacia arriba.
    ///
    /// **Consume**: dos llamadas seguidas sin girar dan cero la segunda. Asi el
    /// llamante no tiene que guardar el valor anterior y restar -- que es donde
    /// se cuela el scroll que se mueve solo.
    pub fn rueda(&self) -> i32 {
        invoke(self.cap, INPUT_OP_RUEDA, 0, 0, 0).value as i32
    }

    /// Que modificadores estan pulsados AHORA. No consume nada: es estado.
    ///
    /// Existe porque `tecla()` da un byte ya resuelto y hay combinaciones que
    /// no producen caracter -- `Ctrl+Alt` a secas no es ninguna letra.
    ///
    /// * En la distribucion espanola `Ctrl+Alt` **es** `AltGr`: lo que produce
    /// `@`, `#`, `[`, `]`, `\`, `|` y `EUR`. Un atajo que dispare al PULSARLOS
    /// rompe escribir todo eso. Si lo usas como atajo, dispara al SOLTAR y solo
    /// si no llego ningun caracter mientras estaban pulsados.
    pub fn modificadores(&self) -> u8 {
        invoke(self.cap, INPUT_OP_MODIFICADORES, 0, 0, 0).value as u8
    }

    /// La siguiente tecla, si hay alguna. **No bloquea**: devuelve `None`
    /// cuando no hay nada esperando.
    ///
    /// No bloquea a proposito. Un compositor tiene un bucle de fotograma; si
    /// se durmiera en el teclado, el cursor se congelaria entre tecla y tecla --
    /// el raton dejaria de moverse mientras nadie escribe, que es exactamente
    /// al reves de lo que uno quiere.
    ///
    /// El byte es **Latin-1**: la `n` llega como `0xF1`, que es justo el indice
    /// que entiende la fuente. Sin decodificador de por medio.
    pub fn tecla(&self) -> Option<u8> {
        let v = invoke(self.cap, INPUT_OP_TECLA, 0, 0, 0).value;
        if v & 0x100 != 0 {
            Some((v & 0xFF) as u8)
        } else {
            None
        }
    }
}

