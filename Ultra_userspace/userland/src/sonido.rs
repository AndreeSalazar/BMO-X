//! El SONIDO: `KIND_AUDIO` visto desde Ring 3.
//!
//! Lo que hay al otro lado es un CONTRATO, no un motor de audio: el derecho a
//! hacer ruido, exclusivo, que se recupera solo cuando su dueno muere. El
//! driver de HD Audio es otra cosa y todavia no existe -- ver
//! `docs/PLAN_DOOM.md`, fase 5.
//!
//! Por eso lo unico que suena hoy es el altavoz del PC, y [`Sonido::aparatos`]
//! lo dice en vez de que haya que suponerlo. **Preguntar y no suponer** es todo
//! el motivo de que esa operacion exista: el dia que haya HDA, el mismo binario
//! se entera sin recompilarse.

use crate::*;

/// El sonido, cedido a este proceso.
///
/// Exclusivo como la pantalla: un solo proceso lo tiene a la vez. Dos duenos
/// escribiendo en el mismo aparato no es mezclar, es ruido -- y mezclar es
/// trabajo de Ring 3, igual que componer ventanas.
pub struct Sonido {
    pub cap: u64,
}

impl Sonido {
    /// Reclamarlo. `None` si ya lo tiene otro proceso.
    pub fn claim() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_AUDIO_CLAIM, 0, 0, 0).valor()?;
        Some(Self { cap })
    }

    /// Soltarlo y seguir vivo. Consume el `Sonido`, igual que `Pantalla::release`.
    ///
    /// Va desde el primer dia por lo que costo que faltara en la pantalla: sin
    /// esto, el primer programa que pite se queda el altavoz hasta que muera.
    ///
    /// Devuelve `false` si no era el dueno, en vez de fingir que lo solto.
    pub fn release(self) -> bool {
        invoke(CURRENT_TASK, OP_AUDIO_RELEASE, 0, 0, 0).valor().is_some()
    }

    /// Que aparatos hay: mascara de [`DEVICE_SPEAKER`] y [`DEVICE_HDA`].
    ///
    /// [!] Un bit puesto dice que **hay camino**, no que se vaya a oir algo. El
    /// puerto del altavoz existe en todo x86; el zumbador fisico, no -- muchas
    /// placas modernas traen el cabezal SPKR sin nada conectado, y desde aqui
    /// no hay forma de saberlo.
    pub fn aparatos(&self) -> u64 {
        invoke(self.cap, AUDIO_OP_DEVICES, 0, 0, 0).valor().unwrap_or(0)
    }

    /// Pitar. Devuelve los milisegundos que de verdad sono.
    ///
    /// [!] **BLOQUEA.** Mientras dura, este nucleo no hace otra cosa: el altavoz
    /// del PC no tiene interrupcion que avise de que el tono acabo. El kernel
    /// recorta a [`AUDIO_MAX_MS`], asi que pedir mas no cuelga la maquina --
    /// pero tampoco suena mas.
    pub fn pitar(&self, freq_hz: u32, ms: u32) -> u64 {
        invoke(self.cap, AUDIO_OP_BEEP, freq_hz as u64, ms as u64, 0)
            .valor()
            .unwrap_or(0)
    }

    /// Volumen de 0 a 100. Devuelve el que quedo puesto.
    ///
    /// En el altavoz del PC esto no es un fundido: son **dos escalones**, porque
    /// el volumen se consigue cambiando el modo del PIT --pulsos estrechos
    /// suenan mas flojo que una onda cuadrada al 50%-- y no hay mas modos.
    pub fn volumen(&self, v: u8) -> u64 {
        invoke(self.cap, AUDIO_OP_VOLUME, v as u64, 0, 0).valor().unwrap_or(0)
    }

    /// Callar ahora mismo.
    pub fn callar(&self) {
        let _ = invoke(self.cap, AUDIO_OP_SILENCE, 0, 0, 0);
    }
}
