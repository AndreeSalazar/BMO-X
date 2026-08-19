//! **LA PESTANA `historial`**: la cadena de versiones, dibujada.
//!
//! === Que ensena, y por que no se podia antes ===
//!
//! Cada estrato guarda un puntero a su padre, asi que la historia estaba en el
//! disco desde el primer dia. Lo que faltaba para poder pintarla no era el
//! recorrido -- eran las **dos columnas**:
//!
//! ```text
//!   la FECHA    el campo `tiempo` llevaba un CERO desde siempre.  19-08
//!   el NOMBRE   se escribia en todas, asi que no distinguia nada. 19-08
//! ```
//!
//! Sin esas dos, esta pestana habria sido una columna de filas identicas: un
//! grafo bien dibujado que no dice nada. Por eso se hizo despues y no antes.
//!
//! === Por que un grafo y no una lista ===
//!
//! Porque una version **no es un renglon de un registro: es un nodo con un
//! padre**. La cadena es la forma del dato, y dibujarla como lista obliga a
//! imaginarse las flechas. Es el mismo argumento que ya tiene el panel de nodos
//! del explorador, aplicado al otro eje: alli se baja por el arbol, aqui se va
//! hacia atras en el tiempo.
//!
//! === ** LO QUE SE DISTINGUE DE UN VISTAZO ===
//!
//! ```text
//!   la de ahora        en azul, como todo lo seleccionable del sistema
//!   con NOMBRE         lleva su nombre y un filo: no se suelta JAMAS
//!   automatica         sin nombre y apagada: el recolector puede llevarsela
//! ```
//!
//! Esa diferencia no es decorativa. **Un nombre es lo que hace permanente a una
//! version** (`Estrato::con_nombre`, section 9), asi que esta columna dice
//! literalmente cuales sobreviven a una limpieza y cuales no.
//!
//! === Y lo que NO hace todavia, dicho ===
//!
//! Mirar. No hay volver, ni revertir, ni suprimir. Volver a una version es
//! cambiar un puntero --en copy-on-write cuesta lo mismo con 4 KB que con 4 GB--
//! pero es una operacion que ESCRIBE, y esta pestana no escribe nada.
//!
//! Se dice aqui en vez de poner un boton que no hace nada: en un almacen, una
//! promesa que no ocurre es como se pierde el trabajo de alguien.

use bmo_userland as bmo;

use super::zonas::Zona;
use super::{INK, INK_DIM, INK_OK};

/// Alto de una caja de version, y el hueco hasta la siguiente.
const CAJA_H: u32 = 44;
const HUECO: u32 = 10;
const ANCHO_MAX: u32 = 420;

/// Pinta la cadena de versiones dentro de `z`.
///
/// `desde` es la primera que se ve: la historia puede ser mas larga que el
/// panel, y desplazarse es de quien llama.
pub(crate) fn paint(
    p: &bmo::Pantalla,
    z: &Zona,
    desde: usize,
    sel: usize,
    borde: u32,
    fondo: u32,
    acento: u32,
    sel_fondo: u32,
) {
    if !z.hay() {
        return;
    }
    let cuantas = bmo::estratos::hist_cuantas() as usize;
    if cuantas == 0 {
        p.texto(z.x, z.y, "no hay historia que ensenar.", INK_BAD_O_DIM);
        p.texto(z.x, z.y + bmo::GLIFO_ALTO + 4, "el volumen no monta, o no tiene ni un estrato.", INK_DIM);
        return;
    }

    let ancho = z.w.min(ANCHO_MAX);
    let paso = CAJA_H + HUECO;
    let caben = (z.h / paso).max(1) as usize;
    let hasta = (desde + caben).min(cuantas);

    let mut y = z.y;
    for i in desde..hasta {
        let es_ahora = i == 0;
        let tiene_nombre = bmo::estratos::hist_con_nombre(i as u64);
        let senalada = i == sel;

        // La caja. El filo dice DOS cosas distintas y por eso son dos colores:
        // el acento es "esta senalada" y el verde es "esta no se suelta".
        let filo = if senalada {
            acento
        } else if tiene_nombre {
            INK_OK
        } else {
            borde
        };
        let cuerpo = if senalada { sel_fondo } else { fondo };
        p.rect(z.x, y, ancho, CAJA_H, filo);
        p.rect(z.x + 1, y + 1, ancho - 2, CAJA_H - 2, cuerpo);

        // -- Primera linea: cuando --
        let ty = y + 5;
        let v = bmo::estratos::hist_cuando(i as u64);
        match bmo_rtc::desempaquetar(v) {
            Some(f) => {
                let mut b = [0u8; 24];
                let n = bmo_rtc::escribir(&f, &mut b);
                p.texto_bytes(z.x + 8, ty, &b[..n.min(19)], INK);
            }
            // ** No se inventa una fecha. Un volumen escrito por una maquina sin
            // reloj creible tiene versiones sin fechar, y ponerles 1970 mentiria
            // con mas conviccion que dejarlo en blanco.
            None => {
                p.texto(z.x + 8, ty, "sin fechar", INK_DIM);
            }
        }
        if es_ahora {
            p.texto(z.x + ancho - 9 * bmo::GLIFO_ANCHO, ty, "la de ahora", acento);
        }

        // -- Segunda linea: el nombre, o que es automatica --
        let ny = ty + bmo::GLIFO_ALTO + 3;
        if tiene_nombre {
            let mut nom = [0u8; 64];
            let n = bmo::estratos::hist_nombre(i as u64, &mut nom);
            let x = p.texto(z.x + 8, ny, "* ", INK_OK);
            p.texto_bytes(x, ny, &nom[..n], INK_OK);
        } else {
            // Y se dice lo que significa no tener nombre, que es lo que de
            // verdad importa de esta columna.
            p.texto(z.x + 8, ny, "automatica -- el recolector puede soltarla", INK_DIM);
        }

        // -- La arista hacia el padre: la cadena ES el dato --
        if i + 1 < hasta {
            let cx = z.x + 18;
            p.rect(cx, y + CAJA_H, 1, HUECO, borde);
        }
        y += paso;
    }

    // Si hay mas de las que caben, se DICE. Y si la CADENA se corto por el tope
    // del kernel, tambien: son dos recortes distintos y confundirlos haria creer
    // que el volumen es mas joven de lo que es.
    if hasta < cuantas {
        p.texto(z.x, y + 2, "y mas abajo...", INK_DIM);
    } else if bmo::estratos::hist_recortada() {
        p.texto(z.x, y + 2, "...y mas atras, fuera de lo que se guarda", INK_DIM);
    }
}

/// El rojo apagado de "aqui no hay nada que ensenar". No es un error del disco,
/// asi que no lleva el rojo de alarma.
const INK_BAD_O_DIM: u32 = 0x008A_9BB4;

/// Sobre que version cayo el puntero.
pub(crate) fn version_en(z: &Zona, desde: usize, px: u32, py: u32) -> Option<usize> {
    if !z.contiene(px, py) {
        return None;
    }
    let paso = CAJA_H + HUECO;
    let k = ((py - z.y) / paso) as usize;
    let i = desde + k;
    if i < bmo::estratos::hist_cuantas() as usize {
        Some(i)
    } else {
        None
    }
}
