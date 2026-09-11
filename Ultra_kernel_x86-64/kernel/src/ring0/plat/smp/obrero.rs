//! **El bucle del obrero**: lo que hace un nucleo levantado mientras vive.
//!
//! [carril]  ROJO      corre en once nucleos a la vez; un obrero que no vuelve
//!                     deja a `repartir` esperando su barrera hasta el tope
//! [consumo] LATE      con los nucleos en pie, cada obrero despierta al menos
//!                     cada 27 ms (el plazo de MWAITX) aunque no haya faena
//!
//! # Por que es un fichero y no un trozo de `crew.rs` (L6h)
//!
//! `crew.rs` contestaba dos preguntas que gastan distinto: *como se reparte
//! una faena* --corre cuando alguien la pide-- y *que hace un nucleo mientras
//! espera* --corre SIEMPRE que los nucleos esten en pie--. La segunda es la
//! que gasta con la maquina quieta, y un fichero que late no esconde codigo
//! que se pide. La funcion se movio entera y sin tocar su codigo; los `static`
//! que comparte con el reparto siguen en `crew.rs`, ahora `pub(super)`.
//!
//! [!] Cuanto late: el plazo del `MWAITX` son 100 M ticks (~27 ms), o sea unos
//! 37 despertares por segundo y nucleo con la maquina quieta -- unos 400 con
//! los doce en pie. Las siestas que algo corta antes las cuenta `zzz=`.

use core::sync::atomic::Ordering;

use super::crew::{Faena, ENTRARON, HECHOS, PARAR, PARTES, RONDA, TAREA, VIERON};

/// **El bucle del obrero.** No vuelve.
///
/// `indice` es 0..n-1 entre los APs; su parte es `indice + 1` porque la parte
/// `0` se la queda el BSP, que tambien trabaja -- tener un nucleo mirando como
/// trabajan los otros es desperdiciar justo el mas caliente de cache.
pub fn obrero(indice: u32, apic: u32) -> ! {
    // Lo primero que hace un obrero es decir que existe. Antes el unico
    // testigo era `VIVOS`, y ese se incrementa en el trampolin -- o sea, dice
    // que el nucleo arranco, no que llegara hasta aqui.
    ENTRARON.fetch_add(1, Ordering::SeqCst);
    // Y de paso deja dicho QUIEN es: el indice es orden de llegada, el
    // APIC es domicilio. Ver `ficha`.
    super::ficha::alta(indice, apic);
    let mut vista = 0u32;
    loop {
        if PARAR.load(Ordering::SeqCst) {
            // Punto de no retorno: sin IPI no hay quien lo despierte, y volver
            // a llamarlo es un INIT+SIPI entero. Esta bien asi -- es la forma
            // honesta de "desactivar" con lo que hay.
            super::ficha::marcar(indice, super::ficha::PARADO);
            loop {
                unsafe { core::arch::asm!("cli; hlt", options(nomem, nostack)) };
            }
        }
        let r = RONDA.0.load(Ordering::SeqCst);
        if r != vista {
            vista = r;
            let f = TAREA.load(Ordering::SeqCst);
            let partes = PARTES.load(Ordering::SeqCst);
            let mia = indice + 1;
            if f != 0 && mia < partes {
                // Se apunta ANTES de la faena: si el obrero muere dentro, la
                // diferencia entre `VIERON` y `HECHOS` es exactamente cuantos
                // se quedaron por el camino.
                VIERON.fetch_add(1, Ordering::SeqCst);
                // ** El estado se pone ANTES y el reloj se lee ANTES: un
                // obrero que se cuelga dentro de la faena se queda en
                // TRABAJANDO, que en el panel se distingue de ESPERANDO.
                // Apuntar solo al terminar haria que colgarse y no tener
                // trabajo se vieran igual.
                super::ficha::marcar(indice, super::ficha::TRABAJANDO);
                let t0 = super::ficha::ciclos();
                let faena: Faena = unsafe { core::mem::transmute(f) };
                faena(mia, partes);
                super::ficha::apuntar(indice, super::ficha::ciclos().wrapping_sub(t0));
                HECHOS.fetch_add(1, Ordering::SeqCst);
            }
        }
        // == *** AQUI SE DEJA DE QUEMAR UN NUCLEO (2026-09-10) ============
        //
        // Esta linea era `spin_loop()`, o sea `pause`, o sea **el nucleo al
        // 100% sin hacer nada**. La cabecera de `crew.rs` lo dejo escrito
        // como precio antes de que existiera la salida.
        //
        // ** Y la salida no es `hlt`: lo que se espera aqui no es una
        // interrupcion, es UNA ESCRITURA en `RONDA`. `MWAITX` despierta por
        // escritura y no pide ni GS por-CPU ni TSS -- que era justo el trabajo
        // que este modulo evita. Ver `dormir.rs`.
        //
        // [!] Se le pasa `vista`, que es la ronda que este obrero YA atendio.
        // Si `RONDA` ya no vale eso, hay trabajo y no se duerme. Ese segundo
        // vistazo va DENTRO de `esperar`, despues de armar el `monitor`.
        super::dormir::esperar(&RONDA.0, vista);
    }
}
