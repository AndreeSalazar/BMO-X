//! **Keys that belong to a panel that is already open** -- sound, CABINA, data.
//!
//! Every one of them is guarded by the focus, and that guard is the whole
//! rule: with the focus on Run, a `z` is a letter the owner is typing, and
//! stealing it for a shortcut would be the worst possible trade.

use bmo_userland as bmo;

use super::Key;
use crate::desktop::{Desktop, W_DATA, W_SOUND};
use crate::scene::{self};

pub(crate) fn on_key(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8, _alt_alone: bool) -> Key {
// Las teclas de la ventana del sonido. **Solo con el foco
// AQUI**: con el foco en Ejecutar, una `z` es una letra que el
// dueno esta escribiendo, y robarsela para un atajo seria el
// peor intercambio posible. Es la misma regla que la `f` del
// klog.
if dsk.win.sound_open && dsk.win.focus.es_para(W_SOUND) {
    if let Some(s) = &dsk.snd.cap {
        // Flechas: el volumen, de diez en diez.
        //
        // * `KEY_LEFT` es 0x82 y `KEY_RIGHT` 0x83 -- ver
        // `ring0/dev/keyboard.rs`. Esto se escribio con 0x83 y
        // 0x84, y **0x84 es INICIO**: la flecha izquierda no
        // habria bajado el volumen y la tecla Inicio lo habria
        // subido. No da error, da un control que obedece a la
        // tecla equivocada.
        if c == 0x82 || c == 0x83 {
            dsk.snd.volume = if c == 0x83 {
                (dsk.snd.volume + 10).min(100)
            } else {
                dsk.snd.volume.saturating_sub(10)
            };
            s.volumen(dsk.snd.volume);
            scene::sound::paint(
                &p, &dsk.win.sound, true, dsk.snd.devices,
                dsk.snd.volume, dsk.snd.pressed,
            );
            return Key::Taken;
        }
        // Z..M: una octava. Se pinta la tecla ANTES de pitar
        // porque `pitar` bloquea el nucleo mientras suena: al
        // reves, la tecla se veria encendida cuando ya callo.
        let min = c.to_ascii_lowercase();
        if let Some(i) = scene::sound::NOTES.iter().position(|note| note.0 == min) {
            dsk.snd.pressed = Some(i);
            scene::sound::paint(
                &p, &dsk.win.sound, true, dsk.snd.devices,
                dsk.snd.volume, dsk.snd.pressed,
            );
            s.pitar(scene::sound::NOTES[i].1, 160);
            dsk.snd.pressed = None;
            scene::sound::paint(
                &p, &dsk.win.sound, true, dsk.snd.devices,
                dsk.snd.volume, dsk.snd.pressed,
            );
            return Key::Taken;
        }
        // P: la frase. La misma que toca `c/musica.bex`, para
        // que la ventana y el programa suenen igual -- si no,
        // no se sabria cual de los dos esta mal.
        if min == b'p' {
            for (hz, ms) in [
                (440u32, 170u32), (523, 170), (659, 240),
                (587, 170), (523, 170), (659, 300),
            ] {
                s.pitar(hz, ms);
                s.pitar(0, 30);
            }
            return Key::Taken;
        }
    }
}

// RePag/AvPag dentro de la consola del kernel: recorrer el log.
//
// ** MIENTRAS CABINA ESTA ABIERTA, ESTAS TRES TECLAS SON SUYAS
// -- RePag, AvPag y `G`-- y no se le piden al foco.
//
// Antes se exigia `focus.es_para(W_CABINA)`, y el 2026-08-09 eso
// dio una ventana que **prometia en su pie algo que no hacia**:
// el dueno abrio CABINA con F11, la vio ocupando la pantalla, y
// RePag no movio nada. No era un fallo del scroll: era la
// politica funcionando. **Abrir no es enfocar** --y no debe
// serlo, porque robar el teclado a quien esta escribiendo es
// mucho peor-- pero el compositor la PINTA encima igualmente,
// asi que lo que se ve y lo que manda dejaban de coincidir.
//
// La regla que queda: **las teclas de ESCRITURA son del foco;
// las de NAVEGACION, de la ventana que estas mirando.** Una
// letra sigue cayendo en Ejecutar; un RePag mueve lo que se ve.
// Se paga que no se pueda recorrer el historial de Ejecutar con
// CABINA delante -- y eso no se pierde, porque debajo de CABINA
// no se ve.
// -- F: cambiar el filtro de la ventana del kernel --
//
// Solo con el foco AQUI: con el foco en Ejecutar, una `f` es una
// letra que el dueno esta escribiendo, y robarsela para un atajo
// seria el peor intercambio posible.
//
// Se reinicia el desplazamiento al cambiar: lo que se estaba
// mirando en la lista vieja no senala nada en la nueva, y dejar
// el numero puesto haria que la ventana pareciera vacia.
// G: subir el listero de GRAVEDAD. Cinco escalones y vuelta.
//
// Es `G` y no `F` porque ya no filtra por FAMILIA de modulo
// --eso lo hacia el klog, adivinando por el prefijo de la
// linea-- sino por la severidad que CABINA lleva de verdad.
if dsk.win.cabina_open && (c == b'g' || c == b'G') {
    dsk.win.cabina.minima = (dsk.win.cabina.minima + 1) % 5;
    dsk.win.cabina.from = 0;
    scene::cabina::paint(&p, &dsk.win.cabina);
    return Key::Taken;
}
// ** A: SOLO LO QUE HIZO LA ULTIMA ACCION.
//
// Lo pidio el dueno asi: *"que lea en tiempo real que hace el
// puntero, y al escribir doom.bex y ejecutar, que lo filtre --
// para no quedarse en que falla sino poder verificar todo"*.
//
// `G` contesta *"que fue grave"* y mezcla lo de esta accion con
// lo de las diez anteriores. `A` contesta la pregunta que uno se
// hace de verdad delante de la pantalla: **todo lo que produjo
// esa pulsacion**, lo bueno y lo malo, en orden y sin nada de
// antes. El kernel ya lo agrupaba; faltaba leerlo.
if dsk.win.cabina_open && (c == b'a' || c == b'A') {
    dsk.win.cabina.last_only = !dsk.win.cabina.last_only;
    dsk.win.cabina.from = 0;
    scene::cabina::paint(&p, &dsk.win.cabina);
    return Key::Taken;
}
if dsk.win.cabina_open && (c == 0x87 || c == 0x88) {
    let any = bmo::cabina_disponibles();
    if c == 0x87 {
        // Hacia atras en el tiempo, sin pasarse del principio.
        dsk.win.cabina.from = (dsk.win.cabina.from + 6).min(any.saturating_sub(1));
    } else {
        dsk.win.cabina.from = dsk.win.cabina.from.saturating_sub(6);
    }
    scene::cabina::paint(&p, &dsk.win.cabina);
    return Key::Taken;
}

// -- * La consola de DATOS: cambiar de vista y recorrer el arbol --
//
// Va aqui, junto al bloque del klog y por el mismo motivo: son
// teclas DE ESTA VENTANA. Con Datos delante, las flechas no
// tienen nada que ver con el historial de comandos de Ejecutar,
// y hasta hoy iban alli -- se navegaba una ventana tapada.
if dsk.win.data_open && dsk.win.focus.es_para(W_DATA) {
    use scene::data::{Seal, View};
    let mut served = true;
    match c {
        // TAB: numeros <-> explorador. Es la misma tecla que cambia
        // de pestana en todas partes.
        b'\t' => {
            dsk.win.data.view = match dsk.win.data.view {
                View::Numbers => {
                    // ** AQUI Y SOLO AQUI SE VA A LA RAIZ.
                    //
                    // Al ENTRAR en el explorador se empieza por
                    // arriba: conservar el sitio de la ultima vez
                    // ensenaria un directorio que ya no se sabe
                    // cual es.
                    //
                    // [!] Y este es el UNICO sitio del compositor
                    // que mueve el cursor sin que nadie lo haya
                    // pedido. Lo era tambien `paint_nodes`, que
                    // llamaba a `a_la_raiz()` en cada repintado y
                    // por eso la vista de nodos no podia navegar.
                    // Pintar no navega.
                    bmo::estratos::a_la_raiz();
                    dsk.win.data.to_top();
                    dsk.win.data.arbol_from = 0;
                    View::Obra
                }
                // ** Y AL SALIR NO SE TOCA EL CURSOR.
                //
                // Estas en `/cobol/10`, miras los numeros, vuelves
                // con TAB y sigues en `/cobol/10`. Devolverlo a la
                // raiz al salir convertiria las dos pestanas en
                // dos programas.
                View::Obra => View::Numbers,
            };
            dsk.win.data.seal = Seal::Idle;
        }
        _ if dsk.win.data.view == View::Numbers => served = false,
        // ARRIBA / ABAJO por la lista de hijos.
        // Al cambiar de caja se borra la verificacion: es de
        // UN archivo, y un `CUADRA` viejo bajo el nombre de
        // otro es peor que no decir nada.
        0x80 => { dsk.win.data.move_sel(-1, bmo::estratos::hijos() as usize); dsk.win.data.verified = None; }
        0x81 => { dsk.win.data.move_sel(1, bmo::estratos::hijos() as usize); dsk.win.data.verified = None; }
        0x87 => dsk.win.data.move_sel(-5, bmo::estratos::hijos() as usize),
        0x88 => dsk.win.data.move_sel(5, bmo::estratos::hijos() as usize),
        // ENTRAR / DERECHA: bajar al hijo senalado. `entrar`
        // dice que no si es un archivo, y entonces no pasa nada
        // -- que es lo correcto: un archivo no tiene dentro.
        b'\r' | b'\n' | 0x83 => {
            if bmo::estratos::entrar(dsk.win.data.sel as u64) {
                dsk.win.data.to_top();
                dsk.win.data.verified = None;
            }
        }
        // RETROCESO / IZQUIERDA: subir al padre.
        0x08 | 0x82 => {
            if bmo::estratos::subir() {
                dsk.win.data.to_top();
                dsk.win.data.verified = None;
            }
        }
        // * V: COMPROBAR LA FIRMA del nodo senalado.
        //
        // Se pide a mano y no se calcula al pintar: lee el
        // archivo entero y le hace el BLAKE3, y hacer eso
        // sesenta veces por segundo convertiria este panel en
        // un martillo sobre el disco.
        b'v' | b'V' => {
            dsk.win.data.verified =
                Some(bmo::estratos::verificar(dsk.win.data.sel as u64));
            dsk.win.data.seal = Seal::Idle;
        }
        // * S: SELLAR, en dos tiempos. Ver `data::Seal`.
        //
        // Se mudo aqui desde el terminal principal porque el
        // verbo vive donde vive el objeto: sellar es de
        // ESTRATOS, y esta es la ventana de ESTRATOS. Y va en
        // dos tiempos porque una tecla suelta que escribe en el
        // disco, en una ventana donde se pulsan flechas, seria
        // peor que las dos palabras que se quitaron.
        b's' | b'S' => {
            dsk.win.data.seal = match dsk.win.data.seal {
                Seal::Asking => match bmo::estratos_sellar() {
                    0 => Seal::Failed,
                    g => Seal::Done(g),
                },
                _ => Seal::Asking,
            };
        }
        _ => {
            // Cualquier otra tecla CANCELA la pregunta. Es la
            // salida que hace que preguntar sea barato: si te
            // arrepientes, sigue navegando y ya esta.
            dsk.win.data.seal = Seal::Idle;
            served = false;
        }
    }
    if served {
        scene::data::paint(&p, &dsk.win.data);
        return Key::Taken;
    }
}
    Key::Pass
}
