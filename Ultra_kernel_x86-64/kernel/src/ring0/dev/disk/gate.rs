//! **THE IDENTITY GATE** -- the eighty lines that decide whether this machine
//!
//! [carril]  ROJO      las ochenta lineas que deciden si este disco es el suyo
//! may write to this disk at all.
//!
//! === Why this is a file of its own, and it is the clearest case in the tree ===
//!
//! Because of what is on the other drive. The development machine boots Windows
//! from an NVMe and BMO-X from a SATA Kingston, and **write is closed on
//! purpose**: the only thing standing between a bug in a filesystem and the
//! owner losing the rest of their computer is this function saying no.
//!
//! Eighty lines of that consequence were sitting in the middle of a 1.311-line
//! driver, between the DMA bounce buffer and the partition table. Anybody
//! auditing "can BMO overwrite my Windows?" had to read a device driver to find
//! out. Now the answer is a file, and the file is named after the question.
//!
//! === What it checks, in order, and every failure says why ===
//!
//! 1. There is a disk.
//! 2. The disk **declared who it is** -- an `IDENTIFY` that did not answer
//!    leaves the strings empty, and nothing is written on top of a stranger.
//! 3. Its declared size is not zero.
//! 4. The partition table exists and **agrees with the size the disk itself
//!    reports**. If the GPT says the disk is bigger than the disk says it is,
//!    one of the two is lying and there is no way to know which.
//!
//! Only then is `WRITE_ARMED` set. Every rejection records a reason readable
//! from Ring 3 with `gate_reason()` -- an armed/not-armed boolean with no reason
//! behind it is a guard nobody can debug, and a guard nobody can debug is one
//! somebody eventually widens.
//!
//! [!] **Widening a guard is removing it.** There are TWO named write windows --
//! the data partition and the ESTRATOS volume -- and the reason there are two
//! rather than one wide one is written on `VENTANA_ES` in the parent module.

use super::*;

/// Comprueba QUIEN es el disco y, si convence, abre la puerta de escritura.
///
/// Se llama una vez, despues de `scan_partitions()` y del montaje de arranque.
/// Devuelve `true` si la escritura queda armada. El motivo --diga que si o que
/// no-- queda en `gate_reason()` y en la bitacora de CABINA, porque un booleano
/// no se puede fotografiar.
pub fn verify_identity() -> bool {
    unsafe { WRITE_ARMED = false; }
    let deny = |reason: &'static str, val: u64| -> bool {
        unsafe { GATE_REASON = reason; }
        crate::ring0::cabina::warn("disk", reason, val);
        false
    };

    if !is_ready() {
        return deny("gate: no hay disco que identificar", 0);
    }
    // 1. El disco tiene que haber dicho quien es. Un IDENTIFY que no contesto
    //    deja las cadenas vacias, y sobre un desconocido no se escribe.
    if unsafe { MODEL_LEN } == 0 || unsafe { SERIAL_LEN } == 0 {
        return deny("gate: el disco no declaro modelo o serie", 0);
    }
    if unsafe { TOTAL_SECTORS } == 0 {
        return deny("gate: el disco no declaro su tamano", 0);
    }
    // 2. La tabla de particiones tiene que existir y CUADRAR con el tamano que
    //    el propio disco declara. Si la GPT dice que el disco es mas grande de
    //    lo que el disco dice ser, uno de los dos miente y no se sabe cual:
    //    puede ser una imagen clonada a un disco menor, y escribir ahi es
    //    escribir sobre datos que la tabla cree libres.
    let parts = partitions();
    if parts.is_empty() || last_lba() == 0 {
        return deny("gate: sin tabla GPT legible", 0);
    }
    let (total, last) = (unsafe { TOTAL_SECTORS }, last_lba());
    if last >= total {
        return deny("gate: la GPT declara mas sectores que el disco", last);
    }
    // La cola reservada de una GPT sana son ~34 sectores (copia de la tabla al
    // final). Un hueco grande significa que la tabla se hizo para otro disco.
    if total - last > 128 {
        return deny("gate: la GPT no cuadra con el tamano del disco", total - last);
    }
    // 3. Tiene que ser un disco de arranque EFI. No prueba que sea EL nuestro
    //    --eso llega con el `disco_id` grabado dentro del volumen-- pero descarta
    //    el caso que de verdad daba miedo: escribir en un disco de datos ajeno
    //    porque el barrido PCI lo puso primero.
    if !parts.iter().any(|p| p.is_esp()) {
        return deny("gate: el disco no tiene particion de arranque EFI", 0);
    }
    // 4. Y tiene que haber una particion de datos donde escribir que NO sea la
    //    de arranque.
    // [!] EXISTENCIA, no identidad -- y la diferencia importa por el ORDEN.
    //
    // Este gate corre ANTES de `fs::mount_data`, o sea antes de que nadie haya
    // demostrado cual es la particion de datos. Preguntar aqui por
    // `data_partition()` --que desde el 2026-08-11 solo contesta cuando alguien
    // lo ha probado-- denegaria siempre, el volumen no se montaria, y el sintoma
    // seria un disco entero desaparecido por una comprobacion que ni siquiera
    // pretendia saber cual es.
    //
    // Lo que este paso comprueba de verdad es mas modesto y es lo correcto:
    // **que este disco tenga donde escribir que no sea el arranque**. Cual de
    // ellas sea se decide despues, y lo decide quien pueda demostrarlo.
    if !parts.iter().any(|p| !p.is_esp()) {
        return deny("gate: no hay particion de datos fuera de la EFI", 0);
    }

    unsafe {
        WRITE_ARMED = true;
        GATE_REASON = "gate: disco identificado, escritura armada";
    }
    crate::ring0::cabina::info("disk", model(), total_sectors());
    // ** Se dice CUANTAS hay, no en cual se va a escribir. Esto es el gate, y a
    // estas alturas todavia no se sabe cual es la de datos -- lo dira `fs` en
    // cuanto consiga montar una. Antes esta linea decia el tamano de "la
    // primera que no era la EFI", que era justo la suposicion que hoy se retira.
    crate::ring0::cabina::info(
        "disk",
        "escritura ARMADA; particiones donde podria escribirse",
        parts.iter().filter(|p| !p.is_esp()).count() as u64,
    );
    true
}
