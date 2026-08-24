//! **LAS TRES OPERACIONES QUE MANDAN SOBRE LA MAQUINA**: los nucleos, el sello
//! de ESTRATOS y la administracion del disco.
//!
//! ## Por que salen del despachador (L6a, L6b)
//!
//! *** Y antes que nada, la MEDIDA que corrigio el diagnostico (2026-08-24).
//!
//! `invoke_current_task` son 795 lineas y el censo lo marcaba `CON MONSTRUO`,
//! o sea *"partirla es diseno, no tijeras"*. Se midio su estado compartido y
//! salio esto:
//!
//! ```text
//!    locales declarados a nivel del `match`   0
//!    estado compartido                        los TRES parametros, y nada mas
//!    el `match` empieza                       en la primera linea del cuerpo
//! ```
//!
//! ** No es un monstruo: es un DESPACHADOR con unos cuarenta y cinco brazos
//! independientes. Un monstruo de verdad --el de `task/admitir.rs`-- comparte
//! decenas de locales entre sus ramas y por eso pide un struct antes de
//! tocarlo. Este no comparte NADA, y cada brazo es una funcion esperando a que
//! le pongan nombre.
//!
//! [!] La media dijo `mixto`, el detector de monstruos dijo `CON MONSTRUO`, y
//! **los dos se equivocaron de la misma forma**: midieron el TAMANO de la
//! funcion y no lo que decide si se puede partir, que es su ESTADO.
//!
//! ## Y por que estas tres y no otras
//!
//! Porque son las tres mas grandes --155, 18 y 70 lineas-- y porque contestan
//! la misma pregunta: **son las unicas operaciones que le dicen a la maquina lo
//! que tiene que hacer**, en vez de pedirle algo. Las demas abren un fichero,
//! escriben en la consola o preguntan un numero.
//!
//! ** Que se apunte en CABINA ANTES y DESPUES no es telemetria: las tres pueden
//! cambiar el estado del hardware, y la primera operacion que cambia el almacen
//! no puede ser silenciosa ni cuando funciona.
//!
//! ## [!] Esto NO es un reparto puro de L6d, y se dice
//!
//! El CUERPO de cada brazo se movio tal cual --ni una linea cambia de
//! contenido-- pero el brazo del `match` paso de llevar el cuerpo dentro a ser
//! una llamada. Eso es una linea distinta por operacion, y se cuenta como lo
//! que es en vez de llamarlo "mover texto".

use super::*;

/// **Los nucleos**: despertarlos, pararlos, o medir el reparto.
pub(super) fn smp_despertar(arg0: u64, arg1: u64) -> BmoStatus {
        use crate::ring0::plat::smp::{self, crew};
        let cuantos = if arg0 > u32::MAX as u64 { u32::MAX } else { arg0 as u32 };
        match arg1 {
            // Desactivar: los obreros vuelven a `hlt` y ahi se quedan.
            1 => {
                crew::parar();
                crate::ring0::core::dashboard::dashboard_log("[smp] obreros PARADOS");
                BmoStatus::ok_value(0)
            }
            // La prueba. Devuelve la aceleracion x100 --`842` son 8,42x--
            // porque por la puerta solo cabe un numero y una fraccion no
            // se puede mandar entera. El detalle en crudo va a CABINA.
            2 => {
                let (alive, _) = smp::alive();
                let (uno, todos, partes) = crew::prueba(alive);
                crate::ring0::cabina::info("smp", "ticks con UN nucleo", uno);
                crate::ring0::cabina::info("smp", "ticks con todos", todos);
                crate::ring0::cabina::info("smp", "partes que corrieron", partes as u64);
                // * LOS TRES TESTIGOS, siempre, salga bien o mal.
                //
                // En metal el 08-08 esto contesto `0.00x` y no habia nada
                // mas que mirar: "falto una parte" no dice cuantas
                // llegaron. Estos tres numeros parten el camino en los tres
                // sitios donde se puede romper -- entrar al bucle, ver la
                // ronda, terminar la faena-- y la diferencia entre dos
                // consecutivos senala el tramo culpable.
                let (entraron, vieron, hechos) = crew::testigos();
                crate::ring0::cabina::info("smp", "obreros que ENTRARON al bucle", entraron as u64);
                crate::ring0::cabina::info("smp", "obreros que VIERON la ronda", vieron as u64);
                crate::ring0::cabina::info("smp", "obreros que TERMINARON", hechos as u64);
                // ** Y LA MEDIDA, DENUNCIADA POR ELLA MISMA.
                //
                // El 08-11 esto dio `37` ticks para 400 millones de vueltas
                // con los once obreros entrando, viendo y terminando. Los
                // testigos decian que el reparto iba bien y el numero decia
                // que no, y **nadie sospecho del reloj**. Ahora lo dice el.
                crate::ring0::cabina::info("smp", "el hash que dejo la faena", crew::suma_testigo());
                if !crew::medida_creible(uno) {
                    crate::ring0::cabina::fault(
                        "smp",
                        "esa medida es IMPOSIBLE para las vueltas que son: el cronometro miente, no el reparto",
                        uno,
                    );
                }
                if hechos < alive {
                    crate::ring0::cabina::warn(
                        "smp",
                        "faltan obreros por terminar",
                        (alive - hechos) as u64,
                    );
                }
                // * Y la otra mitad del resultado, que no es la velocidad.
                // Doce nucleos calculando a la vez es justo el momento en
                // que un choque de cerrojo aparece si va a aparecer, y una
                // aceleracion contada sin mirar esto es media medida.
                // Ver `plat/spin.rs` y `docs/maestro/SMP_MAESTRO.md`.
                let (choques, pico) = crate::ring0::plat::spin::contention();
                if choques == 0 {
                    crate::ring0::cabina::info("smp", "cerrojos: ni un choque", 0);
                } else {
                    crate::ring0::cabina::warn(
                        "smp",
                        "CHOQUES de cerrojo: alguien entro en el kernel",
                        choques as u64,
                    );
                    crate::ring0::cabina::warn(
                        "smp",
                        crate::ring0::plat::spin::worst(),
                        pico as u64,
                    );
                }
                crate::ring0::core::dashboard::dashboard_log("[smp] prueba de reparto hecha");
                if todos > 0 && partes > 0 {
                    BmoStatus::ok_value(uno.saturating_mul(100) / todos)
                } else {
                    BmoStatus::ok_value(0)
                }
            }
            _ => {
                let (alive, esperados) = smp::despertar(cuantos, |_| {});
                // ** EN QUE ESTA CADA NUCLEO, A CABINA.
                //
                // Lo pidio el dueno con estas palabras: *"que el smp asi
                // natural ayude a verify los cores y hilos: que se estan
                // usando, y que la cabina con filtros pueda decir que esta
                // ejecutando"*.
                //
                // La tabla ya existia en el shell de Ring 0, y al shell de
                // Ring 0 se llega cuando el escritorio NO arranca. Desde la
                // caja del escritorio no habia forma de verla. Ahora va a
                // CABINA, que es el sitio que se mira desde los dos lados y
                // el unico que tiene filtros.
                //
                // El valor de cada evento es `nucleo * 16 + estado`, que
                // cabe en un numero y se lee de un vistazo: la decena es el
                // nucleo y la unidad el estado.
                let hilos = match (crate::ring0::cpu_vendor::profile::active().nucleos)() {
                    Some(t) => (t.hilos as u32).min(32),
                    None => alive + 1,
                };
                // ** CORE o THREAD en el propio mensaje, y en ingles.
                //
                // Lo pidio el dueno para la vista y para los FILTROS, y esa
                // segunda mitad es la que manda: CABINA filtra por texto de
                // modulo y por gravedad, asi que meter la palabra **dentro
                // del mensaje** es lo que permite leer de un vistazo cuantos
                // de los que estan en pie son nucleos de verdad.
                //
                // Y hace falta: `12 hilos` no dice si son doce nucleos o
                // seis con SMT, y de eso depende cuantos obreros pedir --
                // calculo denso quiere seis, no doce.
                for id in 0..hilos {
                    let e = smp::estado_de(id);
                    let t = smp::tipo_de(id);
                    // `CORE OBRERO` / `THREAD DORMIDO`: dos palabras, la
                    // primera dice QUE es y la segunda EN QUE esta.
                    let msg: &'static str = match (t, e) {
                        ("CORE", smp::Estado::Maestro) => "CORE   MASTER",
                        ("CORE", smp::Estado::Obrero) => "CORE   worker",
                        ("CORE", smp::Estado::Dormido) => "CORE   asleep",
                        ("CORE", smp::Estado::Ausente) => "CORE   ABSENT",
                        ("CORE", _) => "CORE   -",
                        ("THREAD", smp::Estado::Maestro) => "THREAD MASTER",
                        ("THREAD", smp::Estado::Obrero) => "THREAD worker",
                        ("THREAD", smp::Estado::Dormido) => "THREAD asleep",
                        ("THREAD", smp::Estado::Ausente) => "THREAD ABSENT",
                        ("THREAD", _) => "THREAD -",
                        _ => "?      -",
                    };
                    crate::ring0::cabina::info("smp", msg, id as u64);
                }
                // ** Y el coste, que es el numero del ahorro. Hoy es
                // incomodo a proposito: el que espera GIRA, no duerme.
                let girando = smp::girando();
                if girando > 0 {
                    crate::ring0::cabina::warn(
                        "smp",
                        "nucleos GIRANDO en vacio al 100% (con MWAIT serian 0)",
                        girando as u64,
                    );
                }
                // ** BIT 63 = LOS OBREROS ESTAN PARADOS.
                //
                // Cabe de sobra --`alive` no pasa de 32-- y hace falta
                // porque el numero solo mentia por omision: `smp stop`
                // seguido de `smp` contestaba `12 de 12`, que es cierto y se
                // lee como "el stop no hizo nada". Ring 3 pinta la mitad que
                // faltaba; el kernel no opina, solo dice el hecho.
                let parados = if crew::parados() { 1u64 << 63 } else { 0 };
                BmoStatus::ok_value(parados | ((alive as u64) << 32) | esperados as u64)
            }
        }
}

//// * Escribe en el disco. Se apunta en CABINA ANTES y DESPUES, pase lo
//// que pase: la primera operacion que cambia el almacen no puede ser
//// silenciosa ni cuando funciona.
pub(super) fn estratos_sellar(arg0: u64, arg1: u64) -> BmoStatus {
        crate::ring0::cabina::info(
            "estratos",
            "sellado pedido por un proceso de Ring 3",
            scheduler::current_pid() as u64,
        );
        match crate::ring0::fsys::estratos::seal() {
            Ok(g) => BmoStatus::ok_value(g),
            Err(e) => {
                crate::ring0::cabina::warn("estratos", e.name(), 0);
                BmoStatus::ok_value(0)
            }
        }
}

//// ** ADMINISTRAR EL DISCO, y por eso se apunta ANTES de obedecer.
///
//// === Por que esto vive en la superficie y no es una orden de Ring 0 ===
///
//// Porque al shell de Ring 0 **no se vuelve**: en cuanto el compositor
//// reclama la entrada, ese shell deja de leer el teclado. Una orden que
//// solo existe alli es codigo que el dueno de la maquina no puede usar --
//// ya paso con `smp`, con `audio` y con `ext`, y las tres tuvieron que
//// subir. Recortar el disco nace directamente arriba.
///
//// === Y lo que NO cruza esta puerta ===
///
//// El LBA. Ninguna orden de esta familia lo acepta: el rango lo calcula el
//// kernel --la cola libre de ESTRATOS, que sale de `log_head`-- y lo
//// vuelve a comprobar contra la ventana de escritura. Dejar que Ring 3
//// dijera donde recortar seria un borrado apuntable a cualquier sector,
//// incluida la ESP donde vive el arranque del dueno.
pub(super) fn disco(arg0: u64, arg1: u64) -> BmoStatus {
        use crate::ring0::dev::disk::{self, Recorte};
        match arg0 {
            DISCO_OP_TRIM_LIBRE => {
                crate::ring0::cabina::info(
                    "disk",
                    "recorte de la cola libre pedido por un proceso de Ring 3",
                    scheduler::current_pid() as u64,
                );
                // El rango sale del volumen, no del llamante. Sin volumen
                // montado --o con la cola vacia-- no hay nada que devolver, y
                // eso es un motivo propio: no es que el disco no pueda.
                let Some((lba, sectores)) = crate::ring0::fsys::estratos::cola_libre() else {
                    return BmoStatus::ok_value(
                        DISCO_TRIM_SIN_VOLUMEN << DISCO_TRIM_MOTIVO_SHIFT,
                    );
                };
                let (motivo, hechos) = match disk::recortar(lba, sectores) {
                    Recorte::Hecho { sectores, ordenes } => {
                        crate::ring0::cabina::info("disk", "sectores devueltos al disco", sectores);
                        crate::ring0::cabina::info("disk", "ordenes DATA SET MANAGEMENT", ordenes);
                        (DISCO_TRIM_HECHO, sectores)
                    }
                    Recorte::SinDisco => (DISCO_TRIM_SIN_DISCO, 0),
                    Recorte::NoLoSoporta => (DISCO_TRIM_NO_SOPORTADO, 0),
                    // El motivo en palabras va a CABINA porque por la puerta
                    // cabe un numero; el numero dice CUAL de las puertas.
                    Recorte::SinPermiso(why) => {
                        crate::ring0::cabina::warn("disk", why, lba);
                        (DISCO_TRIM_SIN_PERMISO, 0)
                    }
                    Recorte::RangoImposible => (DISCO_TRIM_RANGO, 0),
                    // ** Lo que SI se recorto viaja con el fallo. Un recorte a
                    // medias no se deshace, y callarlo haria que el sistema
                    // volviera a mandar lo que ya estaba hecho.
                    Recorte::Fallo { sectores } => (DISCO_TRIM_FALLO, sectores),
                };
                BmoStatus::ok_value(
                    (motivo << DISCO_TRIM_MOTIVO_SHIFT)
                        | (hechos & DISCO_TRIM_SECTORES_MASK),
                )
            }
            // La barrera a mano. Este disco declara `SOLO_BARRERA` --no tiene
            // condensadores-- asi que esto es literalmente lo unico que tiene
            // para terminar lo que empezo.
            DISCO_OP_BARRERA => BmoStatus::ok_value(disk::flush() as u64),
            // Una orden que no existe se contesta con cero, igual que en el
            // cursor: quien pregunte de mas se entera, y sin obligar al
            // llamante a distinguir dos formas de "nada".
            _ => BmoStatus::ok_value(0),
        }
}

