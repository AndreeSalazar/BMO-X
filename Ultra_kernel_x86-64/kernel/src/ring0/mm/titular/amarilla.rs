//! **CARRIL AMARILLO -- EL MARCADO: de quien es un marco.**
//!
//! [carril]  AMARILLO  el nibble BAJO: quien lo tiene, y quien puede soltarlo
//!
//! [cuesta]  TAREA -- una etiqueta equivocada rehusa una devolucion buena, y
//!           un marco que no vuelve es un marco perdido. Uno no se nota; el
//!           mismo error en un camino que corre en cada muerte de proceso se
//!           come la RAM hasta que no arranca nada (L6e)
//!
//! [riesgo]  ESPEJO -- el byte lo comparten DOS preguntas desde el 09-09.
//!           `marcar` conserva el nibble alto A PROPOSITO; el dia que alguien
//!           escriba el byte entero desde aqui borra vuelos sin enterarse, y
//!           el que los borra no es el que lo nota (L6f)
//!
//! # Por que AMARILLO, escribiendo en el mismo byte que el rojo
//!
//! Porque **falla a gritos**. `vmm::es_tabla` le pregunta a esta tabla antes
//! de caminar una direccion, y si la respuesta no cuadra se planta y lo dice
//! con nombre en CABINA. El de al lado no tiene quien le grite.
//!
//! > El color no es cuanto dano hace. Es cuanto tarda en saberse.
//!
//! ** Y este carril hace algo mas que etiquetar: es el UNICO sitio por el que
//! un marco cambia de dueno, asi que es donde se cazan **las dos reglas que
//! no se pueden vigilar desde el camino rojo de devolucion** -- N3 (un neutro
//! no se suelta) y R-DMA-3 (un marco en vuelo no cambia de titular). Se ven
//! desde aqui sin entrar ahi, que es lo que `NEUTRO/REQUISITOS.md` R4 exige.

use super::{indice, tabla, Titular, EN_VUELO_PISADOS, NEUTROS_SOLTADOS,
            NEUTROS_VIVOS};


/// Apuntar para que se pidio un marco. Lo llama `alloc_frame_de`.
pub fn marcar(phys: u64, q: Titular) {
    if let Some(i) = indice(phys) {
        let antes = tabla()[i];
        // *** SE CONSERVA EL NIBBLE ALTO. Marcar un marco no cambia si tiene
        // un DMA en vuelo: son dos preguntas distintas sobre el mismo byte, y
        // pisar una al contestar la otra es como se pierde un aterrizaje.
        tabla()[i] = (antes & 0xF0) | (q as u8);
        // La cuenta del neutro, en O(1). Ver la nota de arriba.
        let era = (antes & 0x0F) == Titular::Neutro as u8;
        let es = q == Titular::Neutro;
        unsafe {
            if es && !era {
                NEUTROS_VIVOS += 1;
            } else if era && !es {
                NEUTROS_VIVOS = NEUTROS_VIVOS.saturating_sub(1);
                NEUTROS_SOLTADOS = NEUTROS_SOLTADOS.wrapping_add(1);
            }
            // == *** UN MARCO QUE CAMBIA DE TITULAR CON UN DMA EN VUELO ====
            //
            // ** Se detecta AQUI, desde el lado del marcado, y NO en el camino
            // de devolucion. Es la misma decision que la cuenta del neutro de
            // arriba y por el mismo motivo: ese camino es ROJO, es donde vive
            // la azul del 07-09, y `NEUTRO/REQUISITOS.md` (R4) dice que no se
            // toca hasta haberla reproducido.
            //
            // *** Un marco que cambia de titular con el nibble alto puesto es
            // **un bufer reasignado mientras un aparato todavia escribia en
            // el**. No da fault y no tiene sintoma: da un dato ajeno
            // apareciendo en la memoria de otro, mas tarde.
            //
            // > Es el fallo que `EL_NEUTRO` lleva describiendo sin poder
            // > nombrar: la azul de la purga, el xHC muerto y el asignador
            // > colgado A LA VEZ.
            //
            // [!] Y NO SE IMPIDE. Se CUENTA y se dice. Negarse a marcar desde
            // aqui seria decidir en el camino rojo con un dato que todavia no
            // se ha ganado la confianza -- y un asignador que rechaza un marco
            // por una sospecha deja la maquina sin memoria, que es peor que el
            // fallo que evita. Primero el numero; la barrera, cuando el numero
            // lleve arranques diciendo cero.
            if (antes & 0xF0) != 0 && (antes & 0x0F) != (q as u8) {
                EN_VUELO_PISADOS = EN_VUELO_PISADOS.wrapping_add(1);
            }
        }
    }
}

/// **De quien es?** `Nadie` si esta libre o si cae fuera del espejo.
pub fn titular_de(phys: u64) -> Titular {
    match indice(phys) {
        Some(i) => Titular::de_byte(tabla()[i]),
        None => Titular::Nadie,
    }
}

/// El veredicto de `puede_soltar`, con las tres respuestas separadas.
///
/// ** `SinOpinion` NO es `Adelante`, y son dos variantes distintas a proposito.
/// Juntarlas haria que "no lo se" y "lo he comprobado" se contaran igual, que
/// es exactamente el `[riesgo] SILENCIO` de la cabecera.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Veredicto {
    /// Los dos declararon y coinciden.
    Adelante,
    /// Alguno de los dos no declaro. Se deja pasar.
    SinOpinion,
    /// Los dos declararon y DIFIEREN: `(quien lo tiene, quien lo suelta)`.
    NoEsTuyo(Titular, Titular),
}

/// **Puede `quien` soltar este marco?** Ver la regla en la cabecera.
pub fn puede_soltar(phys: u64, quien: Titular) -> Veredicto {
    let tiene = titular_de(phys);
    if tiene == Titular::Anonimo || tiene == Titular::Nadie || quien == Titular::Anonimo {
        return Veredicto::SinOpinion;
    }
    if tiene == quien {
        Veredicto::Adelante
    } else {
        Veredicto::NoEsTuyo(tiene, quien)
    }
}
