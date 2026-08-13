//! **El teclado y el raton**: `INPUT_OP_*`, las teclas con nombre y los
//! modificadores.
//!
//! Vive aparte porque es lo unico del contrato que describe **hechos fisicos**
//! --una tecla que baja, un boton, una rueda-- y no operaciones. Las constantes
//! `TECLA_*` no son ordenes: son el vocabulario con el que Ring 3 nombra lo que
//! el usuario hizo.
//!
//! [!] `INPUT_OP_TECLA` contesta `0x100 | byte`, no el byte pelado. Comparar el
//! valor entero contra `27` compara **283** contra 27 -- y eso, en el
//! raycaster, dejo la maquina de rehen porque el ESC nunca coincidia.

/// Donde esta el puntero y que botones tiene: `(x << 32) | (y << 16) | botones`.
/// Ya viene recortado al panel: el kernel es quien sabe de que tamano es.
pub const INPUT_OP_PUNTERO: u64 = 0x01;

/// Cuantos informes HID se han visto desde el arranque. Distingue "el raton no
/// se mueve" de "el raton no llega": si esto no sube, el problema esta en el USB.
pub const INPUT_OP_EVENTOS: u64 = 0x02;

/// La siguiente tecla: `0x100 | byte`, o `0` si no hay ninguna esperando.
/// **No bloquea.** El byte es Latin-1 ya resuelto (la `n` es `0xF1`).
pub const INPUT_OP_TECLA: u64 = 0x03;

/// Mascara de modificadores pulsados AHORA. Es estado, no consume nada.
pub const INPUT_OP_MODIFICADORES: u64 = 0x04;

/// Las muescas de rueda **desde la ultima vez**, como `i32` en complemento a
/// dos dentro del `u64`. Positivo = hacia arriba.
///
/// * **Consume**: dos lecturas seguidas sin girar dan cero la segunda. Devolver
/// un acumulado desde el arranque obligaria a cada llamante a guardar el
/// anterior y restar, y el primero que lo olvide tiene un scroll que se mueve
/// solo.
pub const INPUT_OP_RUEDA: u64 = 0x05;

/// La siguiente tecla CRUDA: scancode Set 1 + pulsada o soltada.
///
/// `0` si no hay. Si hay: bit 8 = hay evento, bit 9 = pulsada, bits 0..7 = el
/// scancode. **Consume.**
///
/// Es la otra cara de [`INPUT_OP_TECLA`], no su sustituta: aquella entrega el
/// CARACTER que la tecla produjo --resuelto por la distribucion, listo para
/// pintar-- y esta la TECLA que fue. Un caracter no tiene "soltar", y sin
/// soltar un juego no puede saber que sigue pulsado; ademas Shift, Ctrl y Alt
/// no producen caracter, asi que por aquella puerta no salen.
///
/// El kernel ya tenia las dos caras (`bmo_uhid::teclado` compara informes boot
/// consecutivos): se perdian al cruzar a Ring 3.
pub const INPUT_OP_EVENTO_TECLA: u64 = 0x06;

/// Bits de la mascara de [`INPUT_OP_MODIFICADORES`].
pub const MOD_SHIFT: u8 = 1 << 0;

pub const MOD_CTRL: u8 = 1 << 1;

pub const MOD_ALT: u8 = 1 << 2;

pub const MOD_ALTGR: u8 = 1 << 3;

pub const MOD_CAPS: u8 = 1 << 4;

/// Las teclas sin glifo, en el rango C1 (0x80..0x9F) que eligio el driver.
///
/// No son ASCII y no lo pretenden: son bytes que ninguna distribucion produce
/// como caracter, asi que un programa puede distinguirlas de lo que se escribe
/// sin un segundo canal.
/// Son los mismos bytes que `ring0::dev::keyboard::KEY_*`, y esa igualdad es
/// el contrato: si divergen, un programa lee flechas donde hay paginas.
pub const TECLA_ARRIBA: u8 = 0x80;

pub const TECLA_ABAJO: u8 = 0x81;

pub const TECLA_IZQUIERDA: u8 = 0x82;

pub const TECLA_DERECHA: u8 = 0x83;

pub const TECLA_INICIO: u8 = 0x84;

pub const TECLA_FIN: u8 = 0x85;

pub const TECLA_SUPR: u8 = 0x86;

pub const TECLA_REPAG: u8 = 0x87;

pub const TECLA_AVPAG: u8 = 0x88;

/// Las teclas de funcion, detras de la navegacion en el mismo rango C1.
///
/// * Son el sitio correcto para un atajo del sistema porque **no producen
/// caracter en ninguna distribucion**: no pueden chocar con escribir. Una
/// combinacion con `Ctrl+Alt` si puede -- en espanol `Ctrl+Alt` *es* AltGr.
pub const TECLA_F1: u8 = 0x89;

pub const TECLA_F2: u8 = 0x8A;

pub const TECLA_F3: u8 = 0x8B;

pub const TECLA_F4: u8 = 0x8C;

pub const TECLA_F5: u8 = 0x8D;

pub const TECLA_F6: u8 = 0x8E;

pub const TECLA_F7: u8 = 0x8F;

pub const TECLA_F8: u8 = 0x90;

pub const TECLA_F9: u8 = 0x91;

pub const TECLA_F10: u8 = 0x92;

pub const TECLA_F11: u8 = 0x93;

pub const TECLA_F12: u8 = 0x94;
