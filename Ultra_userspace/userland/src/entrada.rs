//! La ENTRADA: el puntero, las teclas y la rueda.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

// ── La entrada ──────────────────────────────────────────────────────────

/// Dónde está el puntero y qué botones tiene pulsados.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Punto {
    pub x: u32,
    pub y: u32,
    pub botones: u8,
}

/// La entrada —ratón **y teclado**— cedida a este proceso.
///
/// El kernel lee el HID —transferencias xHCI, endpoints, reintentos— porque
/// eso es tocar hardware y todavía no hay otro sitio donde pueda vivir. Lo que
/// entrega son coordenadas ya recortadas al panel, una máscara de botones y los
/// bytes que van saliendo del teclado.
///
/// **El cursor no sale de aquí, y las letras tampoco.** Su forma, su color y su
/// contorno son decisiones de aspecto, y ninguna de ésas tiene nada que hacer
/// en Ring 0.
///
/// ★ Reclamarla es EXCLUSIVO y tiene consecuencia: mientras este proceso la
/// tenga, el shell de Ring 0 deja de leer el teclado físico. No es un reparto,
/// es una cesión — si los dos leyeran la misma cola se repartirían las letras.
pub struct Entrada {
    pub cap: u64,
}

impl Entrada {
    pub fn reclamar() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_INPUT_CLAIM, 0, 0, 0).valor()?;
        Some(Self { cap })
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

    /// Cuántos reportes HID se han visto. Distingue "el ratón no se mueve" de
    /// "el ratón no llega": si esto no sube, el problema está en el USB.
    pub fn eventos(&self) -> u64 {
        invoke(self.cap, INPUT_OP_EVENTOS, 0, 0, 0).value
    }

    /// Las vueltas de rueda desde la última vez. Positivo = hacia arriba.
    ///
    /// **Consume**: dos llamadas seguidas sin girar dan cero la segunda. Así el
    /// llamante no tiene que guardar el valor anterior y restar — que es donde
    /// se cuela el scroll que se mueve solo.
    pub fn rueda(&self) -> i32 {
        invoke(self.cap, INPUT_OP_RUEDA, 0, 0, 0).value as i32
    }

    /// Qué modificadores están pulsados AHORA. No consume nada: es estado.
    ///
    /// Existe porque `tecla()` da un byte ya resuelto y hay combinaciones que
    /// no producen carácter — `Ctrl+Alt` a secas no es ninguna letra.
    ///
    /// ★ En la distribución española `Ctrl+Alt` **es** `AltGr`: lo que produce
    /// `@`, `#`, `[`, `]`, `\`, `|` y `€`. Un atajo que dispare al PULSARLOS
    /// rompe escribir todo eso. Si lo usas como atajo, dispara al SOLTAR y sólo
    /// si no llegó ningún carácter mientras estaban pulsados.
    pub fn modificadores(&self) -> u8 {
        invoke(self.cap, INPUT_OP_MODIFICADORES, 0, 0, 0).value as u8
    }

    /// La siguiente tecla, si hay alguna. **No bloquea**: devuelve `None`
    /// cuando no hay nada esperando.
    ///
    /// No bloquea a propósito. Un compositor tiene un bucle de fotograma; si
    /// se durmiera en el teclado, el cursor se congelaría entre tecla y tecla —
    /// el ratón dejaría de moverse mientras nadie escribe, que es exactamente
    /// al revés de lo que uno quiere.
    ///
    /// El byte es **Latin-1**: la `ñ` llega como `0xF1`, que es justo el índice
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

