//! BMO ABI core syscall surface (the frozen 3-call surface).
//!
//! Services such as files, network, audio, graphics and input are capability
//! operations transported through BMO Channel. They are not kernel syscalls.

// == ** EL CONTRATO, REPARTIDO POR LO QUE ES ================================
//
// Este fichero llego a 1.166 lineas con 186 constantes en una sola lista plana:
// las dos puertas, las operaciones de tarea, las de cada handle, el teclado y
// los campos de informe, todo seguido. Anadir un campo era buscar sitio en mil
// lineas, y el resultado se notaba -- `INFO_CPU_HZ_REAL` se escribio en `0x1E`,
// que ya era `INFO_FUGAS`, y dos campos con el mismo numero **no dan error de
// compilacion: dan un panel que ensena el dato de otro**.
//
// El reparto sigue la misma regla que el shell de Ring 0: por lo que es cada
// familia, no por tamano.
//
//    puertas    las DOS, y la lapida de la tercera
//    tarea      TASK_OP_*      lo que se le pide a CURRENT_TASK
//    objetos    ARCH/CHANNEL/MEM/AUDIO/LIENZO/PRESTADO/ES  -- otros handles
//    entrada    INPUT_OP_*, TECLA_*, MOD_*  -- hechos fisicos, no ordenes
//    informe    INFO_*, CABINA_*, AUTOPSIA_*, KLOG_*  -- lo que NO ejerce poder
//
// [!] Y `objetos` es la unica agrupada por COMO se llaman --`INVOKE(handle, op)`--
// y no por que hacen. Se dice para que se note: el dia que una de esas familias
// crezca como crecio `TASK_OP_*`, se saca a su fichero.
//
// == LO QUE HUBO QUE ACTUALIZAR A LA VEZ, Y ES EL RIESGO DE ESTE CORTE ==
//
// **Tres guardianes leen este contrato con `Get-Content ... -Raw`.** Partirlo en
// ficheros habria hecho que dejaran de verlo todo -- y un guardian que lee menos
// **pasa creyendo que comprueba**, que es peor que no tenerlo. `build.ps1` lee
// ahora la carpeta entera y falla si no encuentra ni un fichero.
//
// Los otros dos guardianes --el de `bmo.h` y el de los dos lectores del BEF--
// usan `use`, asi que el compilador los sigue solo.

mod entrada;
mod informe;
mod objetos;
mod puertas;
mod tarea;

// Se reexporta TODO: quien usa este contrato lo escribe exactamente igual que
// ayer. El corte es de organizacion, no de superficie -- y si cambiara la
// superficie, seria otra clase de commit.
pub use entrada::*;
pub use informe::*;
pub use objetos::*;
pub use puertas::*;
pub use tarea::*;


use super::{syscall2, syscall3, syscall6, SyscallResult};








// -- Operaciones sobre un handle de archivo (`KIND_ARCHIVO`) --------------
//
// Viven aqui y no en el kernel porque las emite `bmo-lower` y las ejecuta el
// emulador: tres sitios que tienen que decir el mismo numero. Ver
// `Ultra_kernel_x86-64/kernel/src/ring0/archivo.rs`.


// -- INFORME DEL SISTEMA -------------------------------------------------
//
// Leer cuanta RAM hay no es un privilegio: es una PREGUNTA. El shell de Ring 0
// tenia `info`, `cpu` y `mem` solo porque los datos estaban a su alcance, no
// porque hiciera falta estar en Ring 0 para contarlos. Con estas dos
// operaciones el privilegio se queda con lo que de verdad lo necesita --tocar
// puertos, reiniciar, mapear paginas-- y la informacion baja a Ring 3, que es
// donde se pinta.
//
// Dos operaciones y una TABLA de campos, en vez de una operacion por dato: asi
// anadir "cuantos programas se han lanzado" es una fila, no un numero de
// syscall nuevo. Es la misma forma que tienen las tablas de `sem-asm`.

/// Un dato numerico del sistema. `arg0` = campo (`INFO_*`). Devuelve el valor.
// -- ** CUATRO OPERACIONES QUE EL KERNEL TENIA Y EL ABI NO (2026-08-12) ------
//
// Las tapaba el guardian: comparaba kernel contra ABI **con una lista escrita a
// mano**, y lo que no estuviera en la lista no se comparaba. Su propio
// comentario avisaba --*"una lista a mano es lo que ya se quedo congelada una
// vez"*-- y le habia vuelto a pasar.
//
// Tres son viejas y una es de hoy. Las dos de SOLTAR son ademas las que el mismo
// comentario del guardian cita como las que casi chocan con la autopsia: estaban
// en la historia del fichero y no en el contrato.
//
// El guardian ya no lleva lista: barre TODOS los `TASK_OP_*` del kernel.




// -- ** QUIEN ESTA COMIENDO MEMORIA -------------------------------------
//
// La vista de administrador de tareas: no *"cuanto come el proceso 4"* --que ya
// se sabia-- sino **"quien esta comiendo"**, que es la que hace falta cuando la
// RAM baja y no se sabe por culpa de quien.
//
// El indice de ranura va EMPAQUETADO con el campo: `campo | (ranura << 8)`. Por
// la puerta de `OP_INFO` cabe UN numero, y la alternativa --un buffer con un
// array de structs-- seria inventar un formato con su version y su alineacion
// para contestar tres enteros.
//
// [!] El indice cuenta solo las ranuras OCUPADAS: se pide 0, 1, 2... hasta que
// el pid conteste 0. Los agujeros de la tabla del kernel son suyos y no salen
// por aqui.









// -- La entrada: raton y teclado -----------------------------------------
//
// * Estas constantes vivian en DOS sitios --`ring0/obj/input.rs` y el userland
// de Rust-- y en ninguno de los dos que fuera el contrato. Mientras el unico
// cliente fue un compositor escrito en Rust eso se notaba poco; en cuanto un
// segundo lenguaje quiso leer la rueda, se vio lo que era: un contrato que no
// estaba publicado no lo puede cumplir nadie mas. Aqui no hay logica nueva,
// hay un sitio del que copiar en vez de dos de los que adivinar.











// -- PRESTAR memoria: se OFRECE y se TOMA --------------------------------
//
// El kernel mueve paginas y **no sabe para que**: el lienzo, el audio y los
// bloques grandes entre procesos salen todos de estas cuatro operaciones. Ver
// `ring0/obj/loan.rs`.






// =======================================================================
//  LIENZO -- una app pinta donde se va a ver
// =======================================================================
//
// El contrato completo esta en `docs/identidad/LIENZO.md`. Aqui va lo que el ABI necesita
// saber, y **la aritmetica de paginas**, que es la parte que puede dar acceso a
// los pixeles del vecino si se hace mal.
//
// * Esta aqui y no en el kernel A PROPOSITO: `bmo-abi` se prueba en el
// anfitrion. Esta cuenta se verifica con tests **antes** de que exista una sola
// linea que mapee una pagina -- que es la unica forma sensata de estrenar algo
// que, si se equivoca, deja a una app escribiendo en la ventana de al lado.






/// **Adelanta un desplazamiento hasta el principio de la fila siguiente.**
///
/// Lo usa el kernel al prestar: las ultimas N paginas empiezan donde empiezan, y
/// eso puede caer a media fila. Adelantar pierde menos de una fila --8 KB en el
/// peor caso-- y le ahorra a la app tener que saberlo.
///
/// * Es una division, no un maximo comun divisor. Esa es toda la diferencia con
/// el diseno anterior: aqui no hay nada que dos lados tengan que calcular igual.
pub const fn lienzo_alinear_a_fila(offset_bytes: u64, stride_px: u32) -> u64 {
    let fila = stride_px as u64 * 4;
    if fila == 0 {
        return offset_bytes;
    }
    offset_bytes.div_ceil(fila) * fila
}

/// **Cuantas filas enteras caben en `bytes`.** La cuenta de la app.
pub const fn lienzo_filas(bytes: u64, stride_px: u32) -> u32 {
    let fila = stride_px as u64 * 4;
    if fila == 0 {
        return 0;
    }
    (bytes / fila) as u32
}










/// Operations accepted by `CURRENT_TASK`.
pub mod task_op {
    pub const GET_PID: u64 = super::TASK_OP_GET_PID;
    pub const ENDPOINT_CREATE: u64 = super::TASK_OP_ENDPOINT_CREATE;
    pub const GET_TID: u64 = super::TASK_OP_GET_TID;
    pub const YIELD: u64 = super::TASK_OP_YIELD;
    pub const EXIT: u64 = super::TASK_OP_EXIT;
    pub const CHANNEL_OPEN: u64 = super::TASK_OP_CHANNEL_OPEN;
    pub const CONSOLE_WRITE: u64 = super::TASK_OP_CONSOLE_WRITE;
    pub const CONSOLE_READ: u64 = super::TASK_OP_CONSOLE_READ;
}


pub mod channel_op {
    /// Completion-side sequence -- the value `WAIT` compares against.
    pub const GET_SEQ: u64 = super::CHANNEL_OP_GET_SEQ;
    /// Estuary index backing this capability.
    pub const GET_INDEX: u64 = super::CHANNEL_OP_GET_INDEX;
    /// Avisar al consumidor. Pide WRITE.
    pub const KICK: u64 = super::CHANNEL_OP_KICK;
}

/// Translate the temporary v1 task surface into its v2 capability operation.
///
/// This belongs at the ABI boundary so compilers and runtimes do not each
/// duplicate a legacy-number mapping. It can be removed with the v1 table.
pub const fn task_operation_for_legacy_syscall(number: u32) -> Option<u64> {
    match number {
        super::NR_PROC_GET_PID => Some(TASK_OP_GET_PID),
        super::NR_PROC_GET_TID | super::NR_THREAD_SELF => Some(TASK_OP_GET_TID),
        super::NR_PROC_YIELD => Some(TASK_OP_YIELD),
        super::NR_PROC_EXIT | super::NR_THREAD_EXIT => Some(TASK_OP_EXIT),
        _ => None,
    }
}

/// `INVOKE(capability, operation, a0, a1, a2, a3)`.
#[inline(always)]
pub unsafe fn invoke(
    capability: u64,
    operation: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
) -> SyscallResult {
    syscall6(NR_INVOKE, capability, operation, a0, a1, a2, a3)
}

/// **Avisar al consumidor de un canal.** Ya no es un syscall: es una operacion.
///
/// Se conserva la funcion --no el numero-- porque lo que hace sigue haciendo
/// falta; lo que cambio es por donde entra. Ver [`NR_CHANNEL_KICK`].
#[inline(always)]
pub unsafe fn channel_kick(channel: u64, _published_sequence: u64) -> SyscallResult {
    invoke(channel, CHANNEL_OP_KICK, 0, 0, 0, 0)
}

/// `WAIT(waitable, observed_sequence, timeout_ns)`.
///
/// Blocks until the waitable's sequence moves past `observed_sequence`
/// or `timeout_ns` elapses (0 = no timeout). `waitable = 0` is a pure
/// timed sleep. The kernel compares the sequence under its scheduler
/// lock, so a kick can never be lost between the caller's read and the
/// block. On resume, re-read the shared sequence -- the returned value
/// is advisory.
#[inline(always)]
pub unsafe fn wait(
    waitable: u64,
    observed_sequence: u64,
    timeout_ns: u64,
) -> SyscallResult {
    syscall3(NR_WAIT, waitable, observed_sequence, timeout_ns)
}

pub const fn name(number: u32) -> Option<&'static str> {
    match number {
        NR_INVOKE => Some("bmo_invoke"),
        NR_WAIT => Some("bmo_wait"),
        // El `1` no tiene nombre porque **ya no existe una llamada ahi**. Darle
        // uno haria que una traza de un binario viejo pareciera correcta.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ** LA SUPERFICIE SON DOS PUERTAS, Y EL `1` ESTA RESERVADO.
    ///
    /// Esta prueba decia TRES y salto sola al retirar `CHANNEL_KICK`, que es
    /// exactamente para lo que estaba. Se actualiza a mano y a conciencia --
    /// nunca "para que pase"--: si un dia vuelve a saltar, alguien esta tocando
    /// la frontera del sistema y tiene que enterarse antes de que compile.
    ///
    /// Lo que se comprueba, y por que cada linea:
    ///
    /// - **Dos**, no tres. `CHANNEL_KICK` era una operacion sobre un handle con
    ///   numero de syscall propio; ahora entra por `INVOKE`, que es su sitio.
    /// - **Y no una.** `WAIT` no se puede expresar con `INVOKE`: lo unico que
    ///   hace es no devolver el turno.
    /// - **El `1` no tiene nombre.** Es la parte que de verdad protege algo: si
    ///   alguien recicla ese numero, un binario viejo haria una cosa distinta
    ///   **sin fallar**, que es la peor rotura de ABI que hay.
    #[test]
    fn la_superficie_son_dos_puertas_y_el_uno_esta_reservado() {
        assert_eq!(CORE_SYSCALL_COUNT, 2);
        assert_eq!(name(0), Some("bmo_invoke"));
        assert_eq!(name(2), Some("bmo_wait"));
        assert_eq!(name(1), None, "el 1 esta RETIRADO: darle nombre lo resucita");
        assert_eq!(name(3), None);
        // El numero sigue apartado: reservar es ocupar el hueco para que nadie
        // lo use, no borrarlo y dejar que el siguiente se lo encuentre libre.
        assert_eq!(NR_CHANNEL_KICK, 0x01);
    }

    #[test]
    fn legacy_task_translation_has_one_canonical_mapping() {
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_EXIT), Some(TASK_OP_EXIT));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_GET_PID), Some(TASK_OP_GET_PID));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_GET_TID), Some(TASK_OP_GET_TID));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_YIELD), Some(TASK_OP_YIELD));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_FS_OPEN), None);
    }

    // ======= LIENZO: lo poco que queda de aritmetica =======
    //
    // El diseno anterior necesitaba un maximo comun divisor para traducir filas
    // a paginas, y que el kernel y la app lo calcularan IGUAL para siempre. Este
    // no traduce nada: el kernel presta paginas, la app cuenta filas, y lo unico
    // compartido son bytes. Lo que queda son dos divisiones.

    /// Adelantar a fila no se pasa nunca, y nunca se queda corto.
    #[test]
    fn alinear_a_fila_siempre_avanza_hasta_el_principio_de_una() {
        let stride = 1920u32;
        let fila = stride as u64 * 4; // 7680
        for off in [0u64, 1, 7679, 7680, 7681, 61440, 999_999] {
            let a = lienzo_alinear_a_fila(off, stride);
            assert!(a >= off, "no puede retroceder");
            assert_eq!(a % fila, 0, "{a} no es principio de fila");
            assert!(a - off < fila, "se paso mas de una fila entera");
        }
    }

    /// Lo que ya esta alineado no se mueve: prestar no puede costar una fila
    /// cuando no hacia falta.
    #[test]
    fn lo_que_ya_cuadra_no_se_toca() {
        assert_eq!(lienzo_alinear_a_fila(0, 1920), 0);
        assert_eq!(lienzo_alinear_a_fila(7680, 1920), 7680);
        assert_eq!(lienzo_alinear_a_fila(7680 * 13, 1920), 7680 * 13);
    }

    /// Las filas salen de una division, y lo que sobra se ignora -- nunca se
    /// devuelve una fila a medias, que es lo que haria pintar fuera.
    #[test]
    fn las_filas_salen_de_una_division_y_lo_que_sobra_no_cuenta() {
        assert_eq!(lienzo_filas(7680, 1920), 1);
        assert_eq!(lienzo_filas(7680 * 200, 1920), 200);
        assert_eq!(lienzo_filas(7680 * 200 + 7679, 1920), 200, "la fila a medias no cuenta");
        assert_eq!(lienzo_filas(0, 1920), 0);
        assert_eq!(lienzo_filas(100, 1920), 0, "menos de una fila son cero filas");
    }

    /// La propiedad de punta a punta: prestar N paginas, alinear, contar filas --
    /// y que lo contado quepa SIEMPRE en lo prestado. Si esto falla, la app
    /// pinta fuera de lo suyo.
    #[test]
    fn lo_que_se_cuenta_cabe_en_lo_que_se_presta() {
        for stride in [640u32, 800, 1024, 1280, 1366, 1920, 2560, 3840] {
            for paginas in [1u64, 2, 7, 64, 512, 2048] {
                let bytes = paginas * 4096;
                // El peor caso: la banda empieza justo despues de un principio
                // de fila, asi que se pierde casi una fila entera al alinear.
                let perdido = lienzo_alinear_a_fila(1, stride) - 1;
                if perdido >= bytes {
                    continue; // no cabe ni una fila: no hay nada que comprobar
                }
                let utiles = bytes - perdido;
                let filas = lienzo_filas(utiles, stride) as u64;
                assert!(
                    filas * (stride as u64 * 4) <= utiles,
                    "stride {stride}, {paginas} paginas: {filas} filas no caben"
                );
            }
        }
    }

    /// Un stride de cero no revienta: contesta cero filas.
    #[test]
    fn un_stride_de_cero_no_revienta() {
        assert_eq!(lienzo_filas(4096, 0), 0);
        assert_eq!(lienzo_alinear_a_fila(123, 0), 123);
    }
}
