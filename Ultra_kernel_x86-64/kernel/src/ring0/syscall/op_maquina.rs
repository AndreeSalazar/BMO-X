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
            // *** 4 y 5: EL ANCHO DE BANDA DE LA MEMORIA, DESDE RING 3.
            //
            // ** Entraron el 2026-08-24 porque `banda` se escribio solo para el
            // shell de Ring 0 -- y al shell de Ring 0 el dueno NO VUELVE: vive
            // en el escritorio. Una medida a la que no se puede llegar desde
            // donde se trabaja es una medida que no existe.
            //
            // > Lo que solo es orden del kernel es codigo que el no puede usar.
            //
            // [!] Y son DOS y no una a proposito. Preparar reserva 256 MiB y los
            // llena; medir corre el barrido. Si fueran la misma llamada, el
            // coste de reservar caeria dentro del primer punto del barrido y ese
            // punto saldria lento -- justo el que sirve de referencia para todos
            // los demas. Separarlas es lo que mantiene la columna `x1` honesta.
            4 => {
                match crate::ring0::plat::smp::banda::preparar() {
                    Ok((bytes, _veces)) => BmoStatus::ok_value(bytes),
                    Err(_) => BmoStatus::ok_value(0),
                }
            }
            5 => {
                use crate::ring0::plat::smp::banda;
                let i = cuantos as usize;
                if i >= banda::PUNTOS.len() {
                    return BmoStatus::ok_value(0);
                }
                let extra = banda::PUNTOS[i];
                let (vivos, _) = smp::alive();
                if extra > vivos {
                    // No se mide un punto para el que no hay obreros: saldria el
                    // numero de una carrera incompleta, que es el mas bonito de
                    // todos y el unico que no significa nada.
                    return BmoStatus::ok_value(0);
                }
                let (ticks, leidos, todos) = banda::medir(extra);
                // El bus, otra vez: el barrido son decimas de segundo sin
                // bombear el USB, y perder un evento de endpoint PARA LA BOMBA.
                crate::ring0::dev::usb::rescatar_el_bus();
                if !todos {
                    return BmoStatus::ok_value(0);
                }
                match banda::mb_por_segundo(leidos, ticks) {
                    Some(mb) if banda::creible(mb) => BmoStatus::ok_value(mb),
                    // Un numero imposible sale como 0 y no como record. Ver
                    // `banda::creible`.
                    _ => BmoStatus::ok_value(0),
                }
            }
            // Desactivar: los obreros vuelven a `hlt` y ahi se quedan.
            1 => {
                crew::parar();
                crate::ring0::core::dashboard::dashboard_log("[smp] obreros PARADOS");
                BmoStatus::ok_value(0)
            }
            // La prueba. Devuelve la aceleracion x100 --`842` son 8,42x--
            // porque por la puerta solo cabe un numero y una fraccion no
            // se puede mandar entera. El detalle en crudo va a CABINA.
            // *** EL CENSO HILO A HILO, CON SU NOMBRE. (2026-08-24)
            //
            // Peticion del dueno, con estas palabras: *"en `smp all` me gustaria
            // que detalles TODO con nombres CORE y THREAD asi para no decir x12,
            // eso es mentir si pongo asi"*.
            //
            // *** Y tiene razon. "12 de 12" presenta doce cosas como si fueran
            // doce iguales, y no lo son: son **SEIS nucleos con dos hilos cada
            // uno**. Un hilo SMT no es medio nucleo ni es un nucleo: es un
            // sitio mas para meter trabajo en el MISMO nucleo, y cuanto rinde
            // depende de si la faena deja huecos.
            //
            // Es exactamente la misma queja que la de la aceleracion, en otro
            // sitio: **un numero sin el perfil al lado no se puede juzgar.**
            //
            // El kernel ya sabia el nombre --`smp::tipo_de` lleva desde antes
            // repartiendo CORE y THREAD por el APIC id-- y lo mandaba a CABINA
            // como eventos sueltos. Lo que faltaba era **poder pedirlo**, para
            // que el escritorio pueda pintar una tabla en vez de una x.
            //
            // Lo que devuelve, empaquetado porque por la puerta cabe UN numero:
            //
            //    bits  0..8    el estado: 0 maestro, 1 obrero, 2 dormido,
            //                  3 ausente, 4 desconocido
            //    bits  8..16   1 si es CORE, 2 si es THREAD, 0 si no se sabe
            //    bits 16..32   el nucleo FISICO al que pertenece
            //    bits 32..48   cuantos hilos por nucleo dice el PERFIL
            3 => {
                let id = if arg0 > 63 { 63 } else { arg0 as u32 };
                let e = match smp::estado_de(id) {
                    smp::Estado::Maestro => 0u64,
                    smp::Estado::Obrero => 1,
                    smp::Estado::Dormido => 2,
                    smp::Estado::Ausente => 3,
                    _ => 4,
                };
                let t = match smp::tipo_de(id) {
                    "CORE" => 1u64,
                    "THREAD" => 2,
                    _ => 0,
                };
                // ** El nucleo fisico sale del PERFIL, no de un desplazamiento
                // escrito a mano. Que los hermanos SMT sean IDs consecutivos es
                // un hecho de ESTA maquina, y el sitio de un hecho de maquina es
                // el perfil (ley 24). El dia que un CPU los reparta de otra
                // forma, cambia el perfil y esta cuenta no se entera.
                let (fisico, por_nucleo) =
                    match (crate::ring0::cpu_vendor::profile::active().nucleos)() {
                        Some(n) if n.nucleos > 0 && n.hilos >= n.nucleos => {
                            let hpc = (n.hilos / n.nucleos) as u64;
                            (if hpc > 0 { id as u64 / hpc } else { id as u64 }, hpc)
                        }
                        _ => (id as u64, 0),
                    };
                BmoStatus::ok_value(e | (t << 8) | (fisico << 16) | (por_nucleo << 32))
            }
            2 => {
                let (alive, _) = smp::alive();
                let (uno, todos, partes) = crew::prueba(alive);

                // *** Y EL BUS SE RESCATA AL SALIR. (2026-08-24)
                //
                // ** `crew::prueba` corre 400.000.000 vueltas DOS veces, o sea
                // que el BSP gira ~medio segundo sin bombear el USB. Y el
                // evento de un endpoint de interrupcion ES EL PERMISO para
                // volver a encolar: perder uno no pierde una pulsacion, PARA LA
                // BOMBA.
                //
                // *** Le paso al dueno en el Ryzen el 24-08: despues de `smp
                // prueba` el teclado se quedo mudo y `reboot` no llego nunca al
                // kernel. **Los tres sintomas que reporto eran UNO.**
                //
                // [!] Se bombea AL SALIR y no durante, a proposito: esto es un
                // CRONOMETRO, y meterle trabajo entre las dos lecturas
                // contaminaria el numero que existe para medir. La medida se
                // queda limpia y el rescate es explicito.
                crate::ring0::dev::usb::rescatar_el_bus();
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
                // ===========================================================
                //  *** LA SEGUNDA MEDIDA, Y ES LA QUE HACE HONESTO EL NUMERO
                // ===========================================================
                //
                // Peticion del dueno el 2026-08-24, con estas palabras: *"el SMP
                // si no es honesto a base de Perfil no me sirve"*. Y tenia razon.
                //
                // ** La faena de arriba es una CADENA DE DEPENDENCIAS --lo dice
                // su propio comentario en `crew.rs`-- y eso la hace perfecta
                // para comprobar que el reparto funciona y **la peor posible
                // para predecir un trabajo de verdad**:
                //
                //    LATENCIA   el nucleo esta casi parado esperando, asi que
                //               el segundo hilo SMT llena esos huecos
                //               -> hasta ~2x por nucleo. EL MEJOR CASO
                //    ANCHO      las unidades saturadas. El segundo hilo no
                //               encuentra hueco porque no lo hay
                //               -> el caso REAL de un calculo denso
                //
                // *** El 24-08 el Ryzen dio 11,59x sobre 12 hilos --el 96,6%--
                // contra una prediccion escrita que decia "~6x es el techo
                // honesto". **La prediccion no estaba equivocada: la faena no
                // era la que suponia.** Y un numero que solo vale para la faena
                // que lo produjo, presentado como "lo que acelera esta maquina",
                // es un numero deshonesto por bueno que sea.
                let (uno_a, todos_a, partes_a) = crew::prueba_ancho(alive);
                // El bus otra vez: son otros ~medio segundo sin bombear.
                crate::ring0::dev::usb::rescatar_el_bus();
                crate::ring0::cabina::info("smp", "ANCHO: ticks con UN nucleo", uno_a);
                crate::ring0::cabina::info("smp", "ANCHO: ticks con todos", todos_a);
                crate::ring0::cabina::info("smp", "ANCHO: partes que corrieron", partes_a as u64);

                // === Y AHORA CONTRA EL PERFIL, que es lo que se pidio =======
                //
                // ** Una aceleracion sin el perfil al lado no se puede juzgar:
                // "11,59x" es magnifico sobre 12 hilos y seria un desastre sobre
                // 64. El numero que importa no es la x -- es **la x DIVIDIDA
                // por lo que esta maquina tiene**, y eso lo dice el perfil del
                // CPU, no una constante escrita aqui (ley 24).
                if let Some(t) = (crate::ring0::cpu_vendor::profile::active().nucleos)() {
                    crate::ring0::cabina::count("smp", "perfil: nucleos FISICOS", t.nucleos as u64);
                    crate::ring0::cabina::count("smp", "perfil: hilos LOGICOS", t.hilos as u64);

                    let lat = if todos > 0 { uno.saturating_mul(100) / todos } else { 0 };
                    let anc = if todos_a > 0 { uno_a.saturating_mul(100) / todos_a } else { 0 };
                    crate::ring0::cabina::count("smp", "x100 LATENCIA -- el mejor caso", lat);
                    crate::ring0::cabina::count("smp", "x100 ANCHO -- el calculo denso", anc);

                    // *** LA FILA QUE CONTESTA DE VERDAD: cuanto rinde cada
                    // nucleo FISICO en un trabajo que satura las unidades. Si
                    // sale cerca de 100, esta maquina esta dando todo lo que
                    // tiene y el SMT no anade nada -- que es exactamente lo que
                    // le va a pasar a un motor de inferencia.
                    if t.nucleos > 0 {
                        crate::ring0::cabina::count(
                            "smp",
                            "  ...ANCHO por nucleo FISICO, x100",
                            anc / t.nucleos as u64,
                        );
                    }
                    // Y la distancia entre las dos, que es la medida del SMT.
                    if anc > 0 && lat > anc {
                        crate::ring0::cabina::count(
                            "smp",
                            "  ...lo que el SMT anade, x100 (latencia/ancho)",
                            lat.saturating_mul(100) / anc,
                        );
                    }
                }
                if partes_a == 0 {
                    crate::ring0::cabina::warn(
                        "smp",
                        "[!] la medida de ANCHO no completo: no se puede juzgar",
                        0,
                    );
                }
                crate::ring0::core::dashboard::dashboard_log("[smp] prueba de reparto hecha (latencia + ancho)");
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

/// **ARMAR Y SONDEAR LA RED.** Ver `TASK_OP_RED` en `ops.rs` para el por que.
pub(super) fn red(arg0: u64, _arg1: u64) -> BmoStatus {
    use crate::ring0::dev::net;
    match arg0 {
        RED_OP_ARMAR => {
            // ** ANTES de obedecer, con quien lo pide. Misma regla que el
            // disco: la primera operacion que cambia el estado de un aparato no
            // puede ser silenciosa ni cuando funciona.
            crate::ring0::cabina::info(
                "red",
                "armar el receptor, pedido por un proceso de Ring 3",
                scheduler::current_pid() as u64,
            );
            // ** AL APARATO, AHORA -- `releer()` y no `identidad()`.
            //
            // *** Esto se armaba mirando LA FOTO DEL ARRANQUE, y esa foto
            // contesta la pregunta equivocada de las dos maneras:
            //
            // ```text
            //    cable quitado despues de arrancar  ->  armaba igual, y el
            //                                           motivo "sin enlace"
            //                                           no salia nunca
            //    cable puesto despues de arrancar   ->  se NEGABA a armar con
            //                                           el cable enchufado
            // ```
            //
            // ** Y aqui no vale el argumento del panel: esto no se repinta a
            // 60 Hz, se TECLEA una vez. Una lectura de MMIO por orden del
            // dueno es exactamente lo que hay que gastar.
            let Some(id) = net::releer() else {
                crate::ring0::cabina::warn("red", "no hay tarjeta que este kernel sepa leer", 0);
                return BmoStatus::ok_value(RED_SIN_TARJETA);
            };
            // *** SIN CABLE NO SE ARMA, y es un motivo propio.
            //
            // ** No es que el anillo falle: es que no van a llegar tramas por
            // correcto que sea todo lo demas. Separarlo de un fallo del anillo
            // es lo que impide pasar una tarde buscando un bug en un driver que
            // funciona -- la leccion del `cero es lo esperado` del paso 1.
            if !id.enlace_arriba() {
                crate::ring0::cabina::warn("red", "el enlace esta ABAJO: enchufa el cable", 0);
                return BmoStatus::ok_value(RED_SIN_ENLACE);
            }
            if !net::rx_start() {
                crate::ring0::cabina::warn("red", "el receptor no se pudo armar", 0);
                return BmoStatus::ok_value(RED_NO_ARMA);
            }
            // Y DESPUES, con lo que trajo la primera vuelta.
            let n = net::rx_poll();
            crate::ring0::cabina::count("red", "receptor ARMADO. tramas en la 1a vuelta", n as u64);
            crate::ring0::cabina::count("red", "  ...y en total desde el arranque", net::rx_tramas());
            BmoStatus::ok_value(RED_ARMADO_OK)
        }
        // ** Vaciar lo que llego. No cambia nada del aparato: devuelve los
        // descriptores que la tarjeta ya uso, que es lo que hace que el anillo
        // no se llene. Devuelve cuantas tramas se leyeron ESTA vez.
        RED_OP_SONDEAR => {
            if !net::rx_activo() {
                return BmoStatus::ok_value(0);
            }
            BmoStatus::ok_value(net::rx_poll() as u64)
        }
        _ => unsupported(),
    }
}
