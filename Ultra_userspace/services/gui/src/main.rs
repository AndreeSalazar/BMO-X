//! **El compositor de BMO.** El proceso Ring 3 que es dueño de la pantalla.
//!
//! Durante meses esta crate fue `pub fn init() {}`. No por dejadez: no existía
//! forma de convertir un crate de Rust en algo que BMO pudiera admitir, y no
//! existía manera de que un proceso Ring 3 tocara el framebuffer. Las dos
//! cosas existen ya —`bex-link` y `KIND_FRAMEBUFFER`— así que esto pasa a ser
//! un programa de verdad.
//!
//! ## Lo que hace, y lo que deliberadamente no hace
//!
//! Reclama la pantalla, la pinta y **se queda vivo**. Nada más, todavía. No
//! hay ventanas, ni ratón, ni clientes: eso viene cuando el compositor sepa
//! atender llamadas por un endpoint, que es el mecanismo que ya funciona.
//!
//! Lo que sí demuestra, que es lo que importaba:
//!
//! - un binario de Rust compilado a `.bex` corre en Ring 3;
//! - los píxeles se escriben con `mov`, sin un solo syscall por píxel;
//! - el kernel se calla mientras haya dueño y recupera la pantalla si el
//!   dueño muere.
//!
//! ## Por qué no termina
//!
//! Si saliera, `revoke_all` le quitaría la pantalla, el kernel la recuperaría
//! y repintaría su panel encima: no se vería nada. Un escritorio es un proceso
//! que VIVE. Y de paso es la prueba de estrés honesta del cambio de contexto —
//! entra y sale del CPU miles de veces por segundo, que es exactamente el
//! perfil que reventaba anoche.

#![no_std]
#![no_main]

use bmo_userland as bmo;

// XRGB-8888. En esta máquina el GOP entrega BGR; estos tonos son grises
// azulados, no primarios, así que se ven bien en los dos órdenes. Cuando el
// compositor mire `pantalla.formato` de verdad, esto se corrige solo.
const FONDO: u32 = 0x0014_1C2B;
const BARRA: u32 = 0x0028_3448;
const ACENTO: u32 = 0x004C_9BE8;

/// Alto de la barra superior, en píxeles.
const BARRA_ALTO: u32 = 44;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // El aviso va ANTES de reclamar: en cuanto la cesión se consuma, el kernel
    // deja de dibujar y nada de lo que se imprima después llega al panel. Ésta
    // es la última línea que se ve.
    bmo::consola("reclamo la pantalla\n");

    let Some(p) = bmo::Pantalla::reclamar() else {
        bmo::consola("sin pantalla que reclamar\n");
        bmo::salir()
    };

    p.limpiar(FONDO);
    p.rect(0, 0, p.ancho, BARRA_ALTO, BARRA);
    // Una marca a la izquierda de la barra: sirve para ver de un vistazo que
    // el origen y el stride son los que creemos. Si sale desplazada o
    // escalonada, el stride no es el que dijo el kernel.
    p.rect(16, 14, 16, 16, ACENTO);

    bmo::consola("escritorio pintado\n");

    loop {
        bmo::ceder();
    }
}

/// Un pánico aquí no puede tumbar nada más que a este proceso: lo dice y sale
/// por la puerta normal. El kernel revoca sus capabilities —incluida la
/// pantalla— y sigue vivo. Que un proceso se muera es un martes cualquiera.
#[panic_handler]
fn panico(_info: &core::panic::PanicInfo) -> ! {
    bmo::consola("panico en el compositor\n");
    bmo::salir()
}
