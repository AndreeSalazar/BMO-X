//! **EL PRESUPUESTO** -- cuanto cuesta pintar un fotograma de la ciudad.
//!
//! ## Por que esto es lo primero que hay que saber
//!
//! La pregunta que se hace siempre al mirar una referencia bonita es *"se puede
//! hacer eso?"*, y se contesta con ganas cuando deberia contestarse con un
//! numero. El numero es este: **cuantos pixeles se escriben por fotograma**.
//!
//! La escena no la limita la imaginacion ni el numero de objetos. La limita el
//! ancho de banda al framebuffer, que en esta maquina esta MEDIDO: unos 300 MB/s
//! escribiendo a memoria write-combining (salio del trabajo de DOOM, donde el
//! deficit entero a 1600x1000 era el blit). Un 1080p en BGRA son 8,3 MB, o sea
//! que **un repintado completo de pantalla ya cuesta unos 28 ms**.
//!
//! ## Y la moneda es el SOBREDIBUJO
//!
//! No es lo mismo escribir 8,3 MB que escribir la pantalla. El algoritmo del
//! pintor --cielo, luego fondo, luego frente-- pinta cielo debajo de cada torre y
//! **lo tira**. Ese desperdicio es el sobredibujo, y es exactamente el
//! presupuesto que se puede recuperar sin renunciar a un solo elemento de la
//! escena.
//!
//! Por eso este programa cuenta dos cosas por separado: los pixeles que se
//! escriben y las veces que se pisa la pantalla entera. La segunda cifra es la
//! que dice si hay sitio para otro plano.
//!
//! ```text
//!   cargo run -p bmo-vista-ciudad --bin presupuesto
//! ```

use bmo_ciudad::{Camara, Ciudad};

/// Lo medido en el Ryzen escribiendo al framebuffer. Ver la cabecera.
const MB_POR_SEGUNDO: f64 = 300.0;

fn main() {
    println!("presupuesto de la ciudad -- {} MB/s al framebuffer\n", MB_POR_SEGUNDO);
    println!(
        "{:>11}  {:>7}  {:>11}  {:>7}  {:>9}  {:>7}",
        "resolucion", "rects", "px escritos", "sobredib", "MB/frame", "ms/frame"
    );
    for (w, h) in [(1920u32, 1080u32), (1600, 900), (1366, 768), (1280, 720)] {
        let mut c = Ciudad::nueva(w as i32, h as i32, 42);
        c.encender(100);
        let mut px: u64 = 0;
        let mut rects: u64 = 0;
        // Se cuenta solo lo que cae DENTRO de la pantalla: la ciudad se genera
        // mas ancha que el lienzo a proposito --para que la camara tenga por
        // donde avanzar-- y lo que queda fuera no se escribe.
        c.dibujar(Camara::nueva(120), |x, y, rw, rh, _| {
            let x0 = x.max(0);
            let y0 = y.max(0);
            let x1 = (x + rw).min(w as i32);
            let y1 = (y + rh).min(h as i32);
            if x1 > x0 && y1 > y0 {
                px += ((x1 - x0) as u64) * ((y1 - y0) as u64);
                rects += 1;
            }
        });
        let pantalla = (w as u64) * (h as u64);
        let mb = (px * 4) as f64 / 1_000_000.0;
        println!(
            "{:>5}x{:<5}  {:>7}  {:>11}  {:>6.2}x  {:>9.1}  {:>7.1}",
            w,
            h,
            rects,
            px,
            px as f64 / pantalla as f64,
            mb,
            mb / MB_POR_SEGUNDO * 1000.0
        );
    }
    println!(
        "\n[!] El sobredibujo es lo que se paga por el algoritmo del pintor: el cielo\n    \
         se pinta entero y las torres tapan mas de la mitad. Recuperarlo es lo que\n    \
         compra planos nuevos sin subir el coste."
    );
}
