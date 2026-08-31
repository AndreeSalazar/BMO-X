//! **CARRIL VERDE** -- se cambia solo: nadie depende de su forma.
//!
//! [carril]  VERDE     el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//!
//! [cuesta]  NADA -- son numeros que los define el manual de Intel y una
//!           disposicion de direcciones que solo mira este modulo. Cambiar un
//!           valor de aqui rompe la compilacion, no la maquina.
//!
//! [riesgo]  -- ninguno declarado. Es el carril que no tiene letrero, y por eso
//!           existe el fichero: **saber que algo es verde tambien es saber**.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! # Que hay aqui, y por que se puede tocar
//!
//! ```text
//!    PTE_PRESENT..PTE_HUGE   los bits que define la ARQUITECTURA. No son una
//!                            decision nuestra: son el manual
//!    ADDR_MASK               los 52 bits de direccion de una entrada
//!    USER_IMAGE_BASE, ...    donde cae cada cosa en el espacio de un proceso
//!    translate               leer una traduccion. No escribe nada
//!    fisica_exacta           lo mismo, exacto
//! ```
//!
//! ** Los dos lectores estan aqui y no en rojo a proposito: **no escriben**. Un
//! fallo suyo devuelve una direccion equivocada a quien pregunto, y ese quien
//! decide. No paran la maquina por si mismos.

use super::roja::table;
use super::super::PAGE;

pub const PTE_PRESENT: u64 = 1 << 0;
pub const PTE_WRITABLE: u64 = 1 << 1;
pub const PTE_USER: u64 = 1 << 2;
pub const PTE_HUGE: u64 = 1 << 7;

pub(super) const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

pub const USER_IMAGE_BASE: u64 = 0x0000_0000_4000_0000;
pub const USER_STACK_TOP: u64 = 0x0000_0000_8000_0000;
/// **Lo que de verdad se mapea de pila a un proceso de Ring 3.**
///
/// [!] Esta constante decia `0x10_0000` (1 MiB) y **no la usaba nadie**:
/// `proc.rs` mapeaba 16 paginas por su cuenta. O sea que el kernel declaraba un
/// megabyte y entregaba sesenta y cuatro kilobytes, y nada en el arbol podia
/// contradecirlo porque los dos numeros no se cruzaban en ningun sitio.
///
/// Se cobro el 2026-08-14: el compositor paso a tener un marco de 94.208 bytes
/// y murio en la sonda de pila con `#PF` en `rip=0x4000001B`, sin escribir una
/// linea. Ahora hay **un solo numero**: `proc.rs` deriva sus paginas de aqui y
/// la autopsia compara contra el mismo limite. El valor no cambia -- lo que
/// cambia es que ya no puede mentir.
pub const USER_STACK_SIZE: u64 = 0x1_0000;
/// El byte mas bajo de la pila que EXISTE. Por debajo de esto no hay pagina, y
/// un `#PF` aqui abajo es un desbordamiento de pila y no otra cosa.
pub const USER_STACK_BOTTOM: u64 = USER_STACK_TOP - USER_STACK_SIZE;
pub const CHANNEL_VA_BASE: u64 = 0x0000_0000_C000_0000;
/// Donde se mapea el framebuffer en el espacio de quien reclame la pantalla.
/// Por encima de los estuarios y con sitio de sobra: 4K x 4K x 4 B son 64 MiB
/// y aqui hay un hueco entero de 1 GiB antes del limite del canonical bajo.
pub const FRAMEBUFFER_VA_BASE: u64 = 0x0000_0000_D000_0000;
/// Donde empiezan los bloques que un proceso PIDE (`KIND_MEMORIA`).
///
/// Detras del framebuffer y con 256 MiB de hueco antes: el tope por peticion
/// son 64 MiB y hay cuatro peticiones, asi que el peor caso cabe entero sin
/// acercarse a nada. Cada proceso avanza su propio cursor desde aqui -- dos
/// bloques del mismo proceso no se pisan y cada uno tiene su rango.
pub const MEMORIA_VA_BASE: u64 = 0x0000_0000_E000_0000;

/// Resolve a virtual address through the tables. Debugging aid; returns the
/// physical base of the mapped page.
pub fn translate(pml4: u64, va: u64) -> Option<u64> {
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;

    let p = table(pml4);
    let e = p[i4];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    let pdpt = table(e & ADDR_MASK);
    let e = pdpt[i3];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_C000_0000) + (va & 0x3FFF_FFFF));
    }
    let pd = table(e & ADDR_MASK);
    let e = pd[i2];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_FFE0_0000) + (va & 0x1F_FFFF));
    }
    let pt = table(e & ADDR_MASK);
    let e = pt[i1];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    Some(e & ADDR_MASK)
}

/// **La direccion FISICA EXACTA de `va`**, sea cual sea el tamano de pagina.
///
/// == [!] Por que existe, y por que no vale [`translate`] ==
///
/// `translate` **no contesta lo mismo segun el tamano de pagina**, y eso no se
/// ve leyendo su firma:
///
/// | mapeo | lo que devuelve |
/// |---|---|
/// | pagina de 4 KiB | la **base** de la pagina, sin el desplazamiento |
/// | pagina de 2 MiB o 1 GiB | la direccion **exacta**, desplazamiento incluido |
///
/// Su documentacion dice "the physical base of the mapped page", que es cierto
/// para el primer caso y no para los otros dos. Mientras sus dos usuarios eran
/// mapear paginas --donde lo que hace falta es la base-- y el autodiagnostico, la
/// diferencia no se notaba.
///
/// ** Con DMA si se nota, y de la peor manera. El HBA escribe donde se le diga:
/// sumarle el desplazamiento a una respuesta que ya lo llevaba dentro apunta
/// unos bytes mas alla, y como el physmap del kernel esta montado con paginas de
/// 2 MiB, ese es justo el caso de cualquier buffer que viva ahi --las pilas de
/// tarea, por ejemplo--. El resultado no seria una lectura mala: seria el disco
/// escribiendo encima de memoria de otro.
///
/// Asi que la pregunta se hace con su propio nombre. `translate` se queda como
/// esta porque sus usuarios quieren la base; quien quiera la direccion, pide la
/// direccion.
pub fn fisica_exacta(pml4: u64, va: u64) -> Option<u64> {
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;

    let p = table(pml4);
    let e = p[i4];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    let pdpt = table(e & ADDR_MASK);
    let e = pdpt[i3];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_C000_0000) + (va & 0x3FFF_FFFF));
    }
    let pd = table(e & ADDR_MASK);
    let e = pd[i2];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_FFE0_0000) + (va & 0x1F_FFFF));
    }
    let pt = table(e & ADDR_MASK);
    let e = pt[i1];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    // Y aqui esta la diferencia entera: la base MAS el desplazamiento.
    Some((e & ADDR_MASK) + (va & (PAGE - 1)))
}
