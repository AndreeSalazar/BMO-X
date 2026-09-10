//! **CARRIL ROJO -- EL VUELO: quien esta escribiendo AHORA MISMO.**
//!
//! [carril]  ROJO      el nibble ALTO del byte: ponerlo, quitarlo y saber de
//!                     quien es
//!
//! [cuesta]  MAQUINA -- un bit mal puesto aqui deja un bufer reasignado
//!           mientras un aparato escribe dentro. No es una tarea perdida: es
//!           un dato ajeno apareciendo en la memoria de otro (L6e)
//!
//! [riesgo]  SILENCIO -- y es LA razon de que este sea el rojo. El marcado de
//!           al lado falla A GRITOS: `vmm::es_tabla` se niega a caminar y lo
//!           dice con nombre. Esto no avisa de nada, no da fault y no tiene
//!           sintoma; por eso las tres cuentas de `vuelos()` (L6f)
//!
//! # [!] ESTE CARRIL YA SE JUGO EN CONTRA UNA VEZ, Y DURO TRES HORAS
//!
//! `aterrizo` nacio pidiendo solo la direccion, asi que le quitaba el vuelo a
//! **quien fuera**. `vivos` habria llegado a cero con un vuelo todavia en el
//! aire: la mentira exacta que este bit existe para no contar. Lo encontro
//! una pregunta del dueno --*"si algo puede jugar en contra, aislar"*-- y no
//! una prueba. El arreglo esta escrito entero en el `///` de `aterrizo`.
//!
//! *** Que el fallo de este carril lo cazara una PREGUNTA en vez de un juez
//! es, el solo, el argumento de que sea el rojo.
//!
//! Las reglas que sostiene: `NEUTRO/DMA/REGLAS.txt`, R-DMA-2, R-DMA-3 y
//! R-DMA-4. La que le falta a la casa --el PLAZO, R-DMA-8-- se cablea aqui.

use super::{indice, tabla, EN_VUELO_CHOQUES, EN_VUELO_VIVOS};


// == LOS APARATOS, NUMERADOS -- y el orden NO es de gusto ==================
//
// ** Son las filas de `NEUTRO/CENSO.txt` en su orden, y esa es toda la regla.
// Un numero que no sale de la lista de quien alcanza la RAM por su cuenta
// seria un aparato que nadie censo escribiendo en la memoria de alguien.
//
// [!] El CERO esta reservado a proposito: significa **nada en vuelo**. Por eso
// se empieza en 1, y por eso `en_vuelo` rechaza el 0 en vez de aceptarlo como
// un aparato mas.

/// El HBA del disco. Fila 1 del censo.
pub const APARATO_AHCI: u8 = 1;
/// La tarjeta de red. Fila 2.
pub const APARATO_NIC: u8 = 2;
/// El controlador USB. Fila 3.
pub const APARATO_XHCI: u8 = 3;
/// La grafica, cuando llegue. Fila 4, hoy sin tarjeta.
pub const APARATO_GPU: u8 = 4;

/// **PONER UN MARCO EN VUELO PARA UN APARATO.** Se llama al programar el
/// descriptor, ANTES de tocar la campana.
///
/// Devuelve `false` y no toca nada si:
///
/// ```text
///    el marco cae fuera del espejo      no hay donde apuntarlo
///    `aparato` es 0 o pasa de 15        no cabe en el nibble
///    ya esta en vuelo para OTRO         *** dos aparatos, un bufer
/// ```
///
/// ** El tercero se RECHAZA en vez de sobreescribir, y esa es la decision de
/// esta funcion. Sobreescribir dejaria al primer aparato escribiendo en un
/// bufer que el sistema cree del segundo, y el aterrizaje del segundo borraria
/// la marca del primero: **dos fallos silenciosos por el precio de uno**.
///
/// [!] Y volver a ponerlo en vuelo para EL MISMO aparato SI vale, y no suma:
/// un driver que reprograma el mismo bufer antes de que el anterior termine
/// esta haciendo algo suyo, y contarlo dos veces dejaria la cuenta sin poder
/// llegar a cero nunca.
pub fn en_vuelo(phys: u64, aparato: u8) -> bool {
    if aparato == 0 || aparato > 15 {
        return false;
    }
    let i = match indice(phys) {
        Some(i) => i,
        None => return false,
    };
    let antes = tabla()[i];
    let quien = (antes & 0xF0) >> 4;
    if quien != 0 && quien != aparato {
        unsafe { EN_VUELO_CHOQUES = EN_VUELO_CHOQUES.wrapping_add(1) };
        return false;
    }
    tabla()[i] = (antes & 0x0F) | (aparato << 4);
    if quien == 0 {
        unsafe { EN_VUELO_VIVOS += 1 };
    }
    true
}

/// **ATERRIZO: el aparato dijo que termino.** Se llama al consumir el fin.
///
/// # *** POR QUE PIDE EL APARATO, Y NO SOLO LA DIRECCION
///
/// La primera version era `aterrizo(phys)` y **le quitaba el vuelo a quien
/// fuera**. El dueno pidio buscar *"algo que pueda jugar en contra"* y era
/// esto, tres horas despues de escribirlo:
///
/// ```text
///    el AHCI aterriza un tramo de N paginas
///    una de ellas la tenia la NIC en vuelo
///    -> el AHCI le borra la bandera a la NIC, y nadie se entera
/// ```
///
/// ** El juez de arriba caza el caso al PROGRAMAR --`DeOtroAparato`-- pero
/// aterrizar no pasaba por ningun juez. Un contador que puede bajar por el
/// aparato equivocado deja de ser un contador: `vivos` llegaria a cero **con
/// un vuelo todavia en el aire**, que es la mentira exacta que este bit
/// existe para no contar.
///
/// > Poner el bit se comprueba. Quitarlo tambien tiene que comprobarse. Una
/// > barrera que solo mira en un sentido es una puerta.
///
/// # Que devuelve
///
/// ```text
///    true    era suyo y aterrizo
///    false   no habia vuelo, o **era de OTRO** -- y eso se CUENTA
/// ```
///
/// [!] `false` no es inocente en ninguno de los dos casos: o se consumio un
/// fin que nadie pidio, o alguien esta aterrizando lo ajeno. Quien llama
/// decide si le importa; aqui se contesta y se apunta.
pub fn aterrizo(phys: u64, aparato: u8) -> bool {
    let i = match indice(phys) {
        Some(i) => i,
        None => return false,
    };
    let antes = tabla()[i];
    let quien = (antes & 0xF0) >> 4;
    if quien == 0 {
        return false;
    }
    if quien != aparato {
        // *** ATERRIZAR LO AJENO. Se cuenta con los choques porque es el
        // mismo fallo por el otro lado: dos aparatos creyendose duenos del
        // mismo marco. Y NO se toca la tabla: el vuelo del otro sigue vivo,
        // que es lo unico correcto que se puede hacer aqui.
        unsafe { EN_VUELO_CHOQUES = EN_VUELO_CHOQUES.wrapping_add(1) };
        return false;
    }
    tabla()[i] = antes & 0x0F;
    unsafe { EN_VUELO_VIVOS = EN_VUELO_VIVOS.saturating_sub(1) };
    true
}

/// **QUIEN tiene un DMA en vuelo hacia este marco**, si es que alguno.
///
/// Es lo que `bmo-dma-juicio` pide como `en_vuelo_para`: el campo que hasta hoy
/// nadie podia rellenar con la verdad.
pub fn en_vuelo_de(phys: u64) -> Option<u8> {
    let i = indice(phys)?;
    match (tabla()[i] & 0xF0) >> 4 {
        0 => None,
        a => Some(a),
    }
}
