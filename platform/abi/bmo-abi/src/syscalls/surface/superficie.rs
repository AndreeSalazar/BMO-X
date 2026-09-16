//! **La FORMA de una superficie y de su buzon**: lo que una app escribe en su
//! bloque para que el DIRECTOR lo componga, y por donde le llegan las teclas.
//!
//! [carril]  VERDE   son numeros de un contrato entre dos procesos de Ring 3;
//!                   el kernel no los lee (presta bytes y se aparta)
//! [cuesta]  DATO    un numero distinto en dos lectores es una ventana que no
//!                   sale, o una tecla leida de la ranura de al lado
//! [riesgo]  ESPEJO  este contrato tiene TRES lectores en TRES lenguajes, y
//!                   cada uno lleva su copia
//!
//! # Por que esto entra en el ABI el 2026-09-16
//!
//! Hasta hoy estos numeros vivian en DOS copias escritas a mano: C
//! (`tables/bmo/superficie/roja.h` y `amarilla.h`, los `BMO_SUP_*`) y Rust
//! (`director/src/scene/surface.rs`: `MAGIC`, `HEADER_TAG`, `BUZON_TAG`;
//! `desktop/keys/app.rs`: `CARACTER = 1 << 62`), cada una con un comentario que
//! decia *"el mismo numero que..."*. Un comentario no es un juez. Y el port de
//! la superficie a INTI (N0 de `docs/plan/PLAN_NAVEGAR.md`) iba a ser la
//! TERCERA copia.
//!
//! ** La regla de la casa es la que ya siguen las operaciones: el numero vive
//! UNA vez aqui, cada lenguaje lleva su copia con SU nombre, y un guardian las
//! compara en cada build:
//!
//! ```text
//!    C       BMO_SUP_*  en REX               R13 (`contrato_rex`, el espejo)
//!    Rust    SUP_*      en bmo-userland      R4  (`contrato`, el userland)
//!    INTI    sup_*      en modulos.toml      `tests/espejo_del_kernel.rs`
//! ```
//!
//! Asi C e INTI COOPERAN sin enlazarse: no comparten codigo --no pueden, tienen
//! convenciones de llamada distintas--, comparten el contrato, y el contrato
//! tiene juez.
//!
//! # La cabecera: 32 bytes, ocho campos de `u32` (lo que escribe `roja.h`)
//!
//! ```text
//!    [0] +0   SUP_MAGIC ("BSUP")     [1] +4   ancho
//!    [2] +8   alto                   [3] +12  stride (= ancho: sin relleno)
//!    [4] +16  formato (SUP_BGRA32)   [5] +20  secuencia: la app la SUBE al
//!                                             acabar el dibujo, nunca antes
//!    [6] +24  buzon: donde empieza   [7] +28  ranuras del buzon
//!             dentro del bloque (0 = no hay)
//!    +32  los pixeles, BGRA de 32 bits, `ancho * alto * 4`; el buzon va DETRAS
//! ```
//!
//! # El buzon: 16 bytes de cabecera y `n` ranuras de 8
//!
//! ```text
//!    +0   cabeza (u32)   +4  cola (u32)   +8  estado (u64): el puntero, el
//!                                             bit 24 (SUP_TOMADA) y el byte 2
//!                                             (la VISTA)
//!    +16  ranura 0, ranura 1, ... : un evento crudo por ranura
//! ```
//!
//! Un evento lleva en sus tres bits altos QUE es: raton (bit 63), una LETRA ya
//! cocinada por el kernel (bit 62) o un cambio de ventana (bit 61). Sin bit
//! alto es un scancode. Los tres son EXCLUYENTES y el DIRECTOR los escribe; la
//! app solo los lee.

/// `"BSUP"` en little-endian, en el primer `u32` del bloque. Sin el, el bloque
/// no es una superficie y el DIRECTOR lo devuelve sin componer.
pub const SUP_MAGIC: u64 = 0x5055_5342;
/// Lo que ocupa la cabecera antes del primer pixel.
pub const SUP_CABECERA: u64 = 32;
/// El unico formato: BGRA de 32 bits, el del framebuffer. Se compone COPIANDO.
pub const SUP_BGRA32: u64 = 0;
/// Indice (en `u32`) del campo `secuencia` dentro de la cabecera.
pub const SUP_CAMPO_SECUENCIA: u64 = 5;

/// Lo que ocupa el buzon antes de la primera ranura: cabeza, cola y estado.
pub const SUP_BUZON_CABECERA: u64 = 16;
/// Lo que mide una ranura: un evento crudo, el mismo `u64` de `INPUT_OP_*`.
pub const SUP_BUZON_RANURA: u64 = 8;

/// Bit 63: el evento es del raton (`x`, `y`, botones).
pub const SUP_EV_RATON: u64 = 0x8000_0000_0000_0000;
/// Bit 62: el evento es una LETRA (Latin-1) ya cocinada por el kernel.
pub const SUP_EV_CARACTER: u64 = 0x4000_0000_0000_0000;
/// Bit 61: la ventana cambio de estado (`SUP_ESTADO_*`) o de tamano.
pub const SUP_EV_CONFIGURE: u64 = 0x2000_0000_0000_0000;

/// `SUP_EV_CONFIGURE`: es una ventana normal.
pub const SUP_ESTADO_VENTANA: u64 = 0;
/// `SUP_EV_CONFIGURE`: maximizada.
pub const SUP_ESTADO_MAXIMIZADA: u64 = 1;
/// `SUP_EV_CONFIGURE`: a pantalla completa (sin borde; los pixeles son los mismos).
pub const SUP_ESTADO_COMPLETA: u64 = 2;
/// Bit 24 del estado del buzon: el DIRECTOR ya TOMO esta superficie.
pub const SUP_TOMADA: u64 = 0x0100_0000;

/// Byte 2 del estado del buzon, la VISTA: se ve.
pub const SUP_VISTA_SE_VE: u64 = 0;
/// La VISTA: minimizada. La app puede saltarse el dibujo (R-APP8).
pub const SUP_VISTA_MINIMIZADA: u64 = 1;
/// La VISTA: fuera del panel.
pub const SUP_VISTA_FUERA: u64 = 2;
/// La VISTA: la pantalla esta prestada a otro (pantalla exclusiva).
pub const SUP_VISTA_PRESTADA: u64 = 3;
/// La VISTA: tapada entera por otra ventana.
pub const SUP_VISTA_TAPADA: u64 = 4;
