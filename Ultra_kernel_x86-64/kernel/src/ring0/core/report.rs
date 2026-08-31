//! El informe del sistema, servido a Ring 3.
//!
//! [carril]  AMARILLO  lo que el kernel le cuenta a Ring 3; crece cada semana
//!
//! ## Por que esto baja de anillo
//!
//! El shell de Ring 0 tenia `info`, `cpu`, `mem` y `tasks` desde siempre, y no
//! porque hiciera falta el privilegio: **porque los datos estaban a su
//! alcance**. Contar cuanta RAM hay no ejerce ningun poder -- leer un contador
//! no es tocar un puerto, ni mapear una pagina, ni reiniciar la maquina.
//!
//! Con este modulo el reparto queda donde debe: en Ring 0 se queda lo que **de
//! verdad** necesita el privilegio (E/S de puertos, tablas de paginas,
//! reinicio, la admision de un `.bex`), y la informacion baja a Ring 3, que es
//! quien tiene la pantalla y sabe pintarla.
//!
//! ## Una tabla, no una operacion por dato
//!
//! Dos operaciones (`TASK_OP_INFO` y `TASK_OP_INFO_TEXTO`) y una tabla de
//! campos. Anadir "cuantos programas se han lanzado" es **una fila**, no un
//! numero de syscall nuevo -- que es la misma forma que tienen las tablas de
//! `sem-asm` y la razon de que la superficie no crezca.
//!
//! ## Lo que este modulo NO hace
//!
//! No formatea. Devuelve enteros y bytes crudos: los KiB, los porcentajes, las
//! barras y el color son de Ring 3. Un kernel que decide como se ve un numero
//! es un kernel que tiene opiniones sobre la interfaz.

// Los codigos de campo, copiados de `bmo_abi::syscalls::surface`. Se
// redeclaran aqui por la misma razon que `syscall.rs` redeclara los suyos: Ring
// 0 no enlaza el ABI completo, que usa `alloc`. **La fuente de verdad es
// `surface.rs`** -- si estos numeros se separan, el que esta mal es este
// archivo.
const INFO_RAM_TOTAL: u64 = 0x01;
const INFO_RAM_LIBRE: u64 = 0x02;
const INFO_RAM_MARCOS: u64 = 0x03;
const INFO_RAM_MARCOS_LIBRES: u64 = 0x04;
const INFO_TSC_HZ: u64 = 0x05;
/// ** LA FRECUENCIA EFECTIVA, en Hz. `0` = no se puede medir.
///
/// No es `INFO_TSC_HZ`: ese dice a que va el RELOJ de referencia, que no cambia
/// nunca. Este dice a que va el NUCLEO ahora mismo, que en un Zen 3 va de 3,7 a
/// 4,6 GHz segun cuantos esten trabajando.
///
/// Es una MEDIDA y no un dato: sale de restar dos lecturas de MPERF/APERF, asi
/// que **preguntarlo dos veces seguidas da la velocidad de ese intervalo**. Un
/// panel que se repinta obtiene el valor del ultimo refresco, que es justo lo
/// que quiere. Ver `ring0/cpu/frecuencia.rs`.
const INFO_CPU_HZ_REAL: u64 = 0x20;
/// **Milivatios del PAQUETE desde la ultima consulta.** `0` = no se puede medir.
///
/// Como [`INFO_CPU_HZ_REAL`], es una MEDIDA por diferencia: preguntarlo dos
/// veces seguidas da el consumo de ese intervalo. Y como aquel, el cero
/// significa *"no se sabe"* y no *"no gasta"* -- que es una frase que no puede
/// ser verdad con la maquina encendida.
const INFO_CPU_MW_PAQUETE: u64 = 0x21;
/// **Milivatios del NUCLEO EN EL QUE SE LEE.** No de todos.
///
/// [!] Decia "de los NUCLEOS" y era poner un dato que no existe. El metal del
/// 12-08: con once nucleos GIRANDO al 100%, este numero **bajo** de 11,9 a
/// 9,2 W. No consumian menos -- es que `CORE_ENERGY_STAT` es un contador por
/// nucleo y solo se lee el del BSP. Los otros once no aparecen.
const INFO_CPU_MW_NUCLEO_ACTUAL: u64 = 0x22;
/// **Que sabe medir el PERFIL de este silicio**, como banderas.
///
/// bit 0 = frecuencia efectiva (MPERF/APERF) / bit 1 = consumo (RAPL)
///
/// Existe para que la terminal pueda decir **que esta aplicando** en vez de
/// pintar ceros y dejar al que mira adivinando si el sensor no existe o si el
/// numero es de verdad cero. Un panel que no distingue "no se" de "cero" hace
/// que sus dos casos se lean igual, y uno de los dos es una mentira.
const INFO_CPU_SENSORES: u64 = 0x23;

// -- ** QUIEN ESTA COMIENDO: la vista de administrador de tareas -------------
//
// Tres campos y un indice, en vez de una estructura. `arg0` trae el numero de
// ranura empaquetado con el campo:
//
//    campo = INFO_MEM_QUIEN_* | (ranura << 8)
//
// Es feo y es a proposito: por la puerta de `INFO` cabe UN numero, y la
// alternativa --un buffer con un array de structs-- seria inventar un formato
// nuevo con su version y su alineacion para contestar tres enteros. Cuando haga
// falta un cuarto campo se anade otra constante, no un formato.
//
// [!] Y el indice cuenta SOLO LAS RANURAS OCUPADAS. Quien enumera pide 0, 1,
// 2... hasta que el pid conteste 0. No tiene que saber que la tabla del kernel
// tiene agujeros dentro, porque eso es un detalle del kernel y cambia solo.
/// El pid de la ranura `n`. **`0` = no hay mas**, y es la condicion de parada.
const INFO_MEM_QUIEN_PID: u64 = 0x24;
/// Bytes que ese proceso tiene pedidos ahora mismo.
const INFO_MEM_QUIEN_BYTES: u64 = 0x25;
/// Cuantas peticiones lleva hechas. Distingue "pidio un bloque grande" de
/// "esta pidiendo sin parar", que es la diferencia entre un juego y una fuga.
const INFO_MEM_QUIEN_PETICIONES: u64 = 0x26;

// == LA RED ==========================================================
//
// Los mismos siete numeros que declara `bmo-abi`, copiados a mano como todos
// los demas de este fichero -- y el guardian de `build.ps1` compara los tres
// lados en cada build, asi que una copia que derive no compila.
const INFO_NET_PRESENTE: u64 = 0x27;
const INFO_NET_VENDOR_DEVICE: u64 = 0x28;
const INFO_NET_MAC: u64 = 0x29;
const INFO_NET_PHY_CRUDO: u64 = 0x2A;
const INFO_NET_MEGABITS: u64 = 0x2B;
const INFO_NET_RX_ARMADO: u64 = 0x2C;
const INFO_NET_RX_TRAMAS: u64 = 0x2D;
const INFO_NET_RX_BYTES: u64 = 0x4A;
const INFO_NET_RX_PERDIDAS: u64 = 0x4B;
const INFO_NET_RX_TIPOS: u64 = 0x4C;
const INFO_NET_PCI: u64 = 0x2E;

// El metro de la puerta: cuantas y cuantos ciclos dentro de `dispatch`. Se
// leen como delta. Ver `ring0/syscall/meter.rs`.
const INFO_SYSCALL_CUENTA: u64 = 0x2F;
const INFO_SYSCALL_CICLOS: u64 = 0x30;
// Y el reparto DENTRO del stub: guardar el contexto y devolverlo. Lo que no
// cae en ninguna de las tres casillas son las dos transiciones de privilegio.
const INFO_SYSCALL_CICLOS_GUARDA: u64 = 0x35;
const INFO_SYSCALL_CICLOS_RESTAURA: u64 = 0x36;
// Y de QUE CLASE fue cada puerta, con el indice empaquetado como en
// `INFO_MEM_QUIEN_*`: `campo | (clase << 8)`. Las cuatro suman MENOS que
// `INFO_SYSCALL_CUENTA` y esa resta es la comprobacion del instrumento.
const INFO_SYSCALL_CLASS: u64 = 0x3A;
// Y lo que una puerta TIENE PERMITIDO costar: `meta << 32 | techo` en cada uno.
// La tabla vive en `ring0/syscall/presupuesto.rs`.
const INFO_PRESUPUESTO_PUERTA: u64 = 0x37;
const INFO_PRESUPUESTO_DISPATCH: u64 = 0x38;
const INFO_PRESUPUESTO_HANDLE: u64 = 0x39;
// 1 si las tres filas de arriba se midieron en ESTA maquina. Un 0 no es un
// fallo: es un kernel corriendo en un CPU que todavia no tiene presupuesto, y
// entonces las tres contestan cero para que nadie juzgue con numeros ajenos.
const INFO_PRESUPUESTO_MAQUINA: u64 = 0x3D;
// El suelo del hardware: `medido << 32 | ticks`. Lo que cuesta cruzar el anillo
// en este silicio, que no es merito ni culpa de BMO. Con el aparte sale la
// cifra que SI sobrevive a un cambio de CPU: cuantas veces el suelo cuesta una
// puerta. Ver `syscall/presupuesto.rs`.
const INFO_SUELO_CRUCE: u64 = 0x3E;

const INFO_CPU_HILOS: u64 = 0x06;
const INFO_CPU_NUCLEOS: u64 = 0x07;
// ** Los dos que dicen CUANTO fiarse de los dos de arriba. Ver el ABI.
const INFO_CPU_TOPOLOGIA_DUDA: u64 = 0x4D;
const INFO_CPU_HILOS_POR_NUCLEO: u64 = 0x4E;
/// * Quien tiene la pantalla: su `pid`, o `0` si no la tiene nadie.
///
/// Existe para que el escritorio pueda PRESTARLA y esperar. Hacia falta
/// preguntar, no tomar: intentar reclamarla para saber si esta libre te la deja
/// puesta, y entonces se la robas al programa que ibas a prestarsela.
///
/// `0` como "de nadie" y no `u32::MAX`: el pid 0 no se concede a un proceso de
/// Ring 3, asi que no hay ambiguedad, y desde Ring 3 un `0` se lee sin tener
/// que conocer el centinela del kernel.
const INFO_PANTALLA_DUENO: u64 = 0x1A;
const INFO_TAREAS_TOTAL: u64 = 0x08;
const INFO_TAREAS_LISTAS: u64 = 0x09;
const INFO_TAREAS_LIBRES: u64 = 0x0A;
const INFO_TICKS: u64 = 0x0B;
const INFO_KERNEL_BYTES: u64 = 0x0C;
const INFO_PROGRAMAS: u64 = 0x0D;
const INFO_PROGRAMAS_OLVIDADOS: u64 = 0x0E;
const INFO_DISCO_LISTO: u64 = 0x0F;
const INFO_DATOS_MONTADO: u64 = 0x10;
// -- ESTRATOS --
//
// El volumen grande. Ring 3 los necesita para poder ENSENAR el estado del
// almacen; anadirlos es una fila cada uno, no una operacion nueva.
const INFO_ES_MONTADO: u64 = 0x11;
const INFO_ES_GENERACION: u64 = 0x12;
const INFO_ES_BLOQUES: u64 = 0x13;
const INFO_ES_USADOS: u64 = 0x14;
const INFO_ES_BLOQUE_TAM: u64 = 0x15;
const INFO_ES_NIVEL: u64 = 0x16;
const INFO_ES_IDENTIDAD: u64 = 0x17;
const INFO_ES_ESCRIBIBLE: u64 = 0x18;
/// Cuando se hizo la version en curso. `0` = sin fechar.
const INFO_ES_FECHA: u64 = 0x49;
/// Lo que Ring 3 ha pedido con `KIND_MEMORIA`. Ver `surface.rs`.
const INFO_MEM_ENTREGADA: u64 = 0x19;
// -- SMP --
//
// Los nucleos en pie y **lo que cuesta tenerlos**. Van juntos a proposito: un
// numero de nucleos sin un numero de choques es la mitad que suena bien.
const INFO_SMP_VIVOS: u64 = 0x1B;
const INFO_SPIN_CHOQUES: u64 = 0x1C;
const INFO_SPIN_PICO: u64 = 0x1D;
// Recursos que un muerto dejo sin devolver. Misma clase que los choques: tiene
// que ser CERO, y por eso vale. Ver `core/autopsia.rs`.
const INFO_FUGAS: u64 = 0x1E;
// -- ** LA SALUD DEL BUS USB, PARA QUIEN VIVE EN EL ESCRITORIO --
//
// E6 de `docs/componente/EL_TECLADO_EXIGE.md`. Las cinco exigencias anteriores estan
// cumplidas y cada una tiene su contador -- pero todos se leian desde el shell
// de Ring 0, y al escritorio no se vuelve. O sea: cinco instrumentos correctos
// que su dueno no podia mirar cuando el teclado se moria, que es el unico
// momento en que hacen falta.
//
// Dos filas, y la tabla entera del capitulo cabe en dos `OP_INFO`. Los bits y
// el empaquetado se documentan en `dev/usb/salud.rs`.
const INFO_USB_SALUD: u64 = 0x3B;
const INFO_USB_AVERIAS: u64 = 0x3C;

// -- ** LO QUE EL DISCO CONTESTA, y el veredicto sobre el (2026-08-17) -------
//
// Tres filas de HECHOS y una de VEREDICTO, y separadas a proposito: quien pinte
// puede ensenar lo que dijo el aparato aunque no este de acuerdo con lo que se
// concluyo. Un veredicto sin su evidencia al lado no se puede discutir.
//
// El reparto vive fuera de este fichero y en cuatro generaciones (L7): la
// palabra cruda y los campos en `bmo-identify`, el veredicto en
// `bmo-disco-juicio` --que esta en `platform/shared/` porque **alli se puede
// probar**, y en este componente equivocarse se lleva el trabajo de alguien.
// Aqui solo se empaqueta. Capitulo: `docs/componente/EL_DISCO_EXIGE.md`.
const INFO_DISCO_MEDIO: u64 = 0x3F;
const INFO_DISCO_ENLACE: u64 = 0x40;
const INFO_DISCO_GEOMETRIA: u64 = 0x41;
const INFO_DISCO_JUICIO: u64 = 0x42;
/// Lo que se le ha DEVUELTO al disco, y en cuantas ordenes cupo. Espejo de
/// `bmo_abi::...::INFO_DISCO_TRIM_*`. Van los dos: los mismos sectores en una
/// orden o en trescientas dicen cosas distintas de la palabra 105.
const INFO_DISCO_TRIM_SECTORES: u64 = 0x43;
const INFO_DISCO_TRIM_ORDENES: u64 = 0x44;
/// ** LA COLA LIBRE, TAL COMO LA VA A USAR EL RECORTE -- no una cuenta parecida.
///
/// Ring 3 podia sacar estos dos numeros de `INFO_ES_BLOQUES`, `INFO_ES_USADOS` y
/// `INFO_ES_BLOQUE_TAM`, y **esa era la curita**: la propuesta que se pinta y el
/// rango que se manda salian de dos cuentas distintas que hoy dan lo mismo. Dos
/// fuentes de una sola verdad se separan, y aqui separarse significa **ensenar
/// un rango y recortar otro**. Ahora los dos leen `estratos::cola_libre()`.
const INFO_DISCO_COLA_LBA: u64 = 0x45;
const INFO_DISCO_COLA_SECTORES: u64 = 0x46;
/// Bloques de payload que admite el disco en UNA orden (palabra 105). Con esto
/// el numero de ordenes de la propuesta es el REAL y no un techo de Ring 3.
const INFO_DISCO_TRIM_BLOQUES: u64 = 0x47;
/// Por que fallo el ultimo recorte: `(clase << 32) | PxTFD`. 0 = ninguno.
const INFO_DISCO_TRIM_FALLO: u64 = 0x48;
/// La fecha de la placa, empaquetada. Espejo de `bmo_abi::...::INFO_FECHA`.
const INFO_FECHA: u64 = 0x1F;

// -- EL CENSO DE EXTENSIONES, para quien no vive en el shell de Ring 0 --
//
// ** Por que estas filas existen: `ext` se escribio SOLO como orden del shell
// de Ring 0, y al escritorio no se vuelve una vez arranca --`fb::rescue` se
// niega a echar al que sostiene la casa, y con razon--. O sea que el censo era
// codigo correcto que su dueno no podia mirar. Ver `shell/extensions.rs`.
//
// ** Y por que MASCARAS y no texto ya formateado: el kernel contesta HECHOS y
// Ring 3 decide la presentacion. Una linea pre-pintada aqui obligaria a todo
// cliente a la anchura, al orden y al color que eligiera el kernel -- que es
// exactamente el "cerebro" que esta casa no mete en el kernel. Con dos mascaras
// el escritorio puede pintar el conflicto en rojo y el shell en su columna, y
// ninguno de los dos tiene una segunda lista de nombres que se le desincronice.
const INFO_CPU_EXT_N: u64 = 0x31;
const INFO_CPU_EXT_HAY: u64 = 0x32;
const INFO_CPU_EXT_USA: u64 = 0x33;
/// Los cuatro contadores que tienen que ser CERO, empaquetados como la fecha:
/// conflictos, mudas, repetidas y sin_sitio, de 16 en 16 bits.
///
/// No se derivan de las mascaras --`conflictos` si, los otros tres no-- y por
/// eso viajan. Un panel que solo pudiera ensenar los conflictos diria que todo
/// esta bien cuando lo que falla es que una fila no tiene motivo escrito.
const INFO_CPU_EXT_AVERIAS: u64 = 0x34;

const INFO_TXT_CPU_VENDOR: u64 = 0x01;
const INFO_TXT_CPU_NOMBRE: u64 = 0x02;
const INFO_TXT_UARCH: u64 = 0x03;
const INFO_TXT_FAMILIA: u64 = 0x04;
/// El nombre de la extension numero `n >> 8`, y su motivo escrito a mano.
///
/// El indice viaja en los bits altos del CAMPO, que es el idioma que esta tabla
/// ya habla con `INFO_MEM_QUIEN_*` y `AUTOPSIA_TEXTO`. Asi los nombres viven en
/// UN sitio --`Feat::name()`, que el compilador obliga a completar-- y el
/// escritorio no lleva copia de treinta y seis cadenas que un dia dirian otra
/// cosa que el kernel.
const INFO_TXT_EXT_NOMBRE: u64 = 0x05;
const INFO_TXT_EXT_NOTA: u64 = 0x06;

const PAGE: u64 = 4096;

/// El valor del campo, o 0 si no existe.
///
/// Cero y no un error: un campo que este kernel todavia no sabe contestar tiene
/// que poder pedirse sin que el programa se caiga. Ring 3 pinta "--" y sigue,
/// que es lo que hace un panel cuando un dato no esta.
pub fn campo(n: u64) -> u64 {
    use crate::ring0::mm::phys;
    match n {
        INFO_RAM_TOTAL => phys::stats().0 * PAGE,
        INFO_RAM_LIBRE => phys::stats().1 * PAGE,
        INFO_RAM_MARCOS => phys::stats().0,
        INFO_RAM_MARCOS_LIBRES => phys::stats().1,
        INFO_TSC_HZ => crate::ring0::task::scheduler::tsc_freq(),
        // La UNICA fila de esta tabla que MIDE en vez de consultar. Cuesta dos
        // `rdmsr`, y por eso puede vivir en un camino que se repinta.
        INFO_CPU_HZ_REAL => crate::ring0::cpu::frequency::medir(),
        INFO_CPU_MW_PAQUETE => crate::ring0::cpu::power::medir().0,
        INFO_CPU_MW_NUCLEO_ACTUAL => crate::ring0::cpu::power::medir().1,
        // El histograma por clase. Mismo desempaquetado que `INFO_MEM_QUIEN_*`
        // y por la misma razon: una pregunta, un numero de campo, y el indice
        // arriba. Una clase que no existe contesta 0, que es lo que significa
        // "no hay tal casilla" -- y no se puede confundir con "cero puertas de
        // esa clase" porque las clases que existen son un contrato.
        c if c & 0xFF == INFO_SYSCALL_CLASS => {
            crate::ring0::syscall::meter::doors_of_class((c >> 8) as usize)
        }
        // Los tres se atienden juntos porque comparten el desempaquetado del
        // indice. Separarlos seria repetir el `>> 8` tres veces.
        c if c & 0xFF == INFO_MEM_QUIEN_PID
            || c & 0xFF == INFO_MEM_QUIEN_BYTES
            || c & 0xFF == INFO_MEM_QUIEN_PETICIONES =>
        {
            let ranura = (c >> 8) as usize;
            match crate::ring0::obj::memory::ranura(ranura) {
                Some((pid, bytes, peticiones)) => match c & 0xFF {
                    INFO_MEM_QUIEN_PID => pid as u64,
                    INFO_MEM_QUIEN_BYTES => bytes,
                    _ => peticiones as u64,
                },
                // Cero en las tres: el pid a cero es la senal de parada y las
                // otras dos acompanan sin inventar nada.
                None => 0,
            }
        }
        INFO_CPU_SENSORES => {
            let mut b = 0u64;
            if crate::ring0::cpu::frequency::disponible() { b |= 1; }
            if crate::ring0::cpu::power::disponible() { b |= 2; }
            b
        }
        // La topologia esta cacheada desde `init_bmo_cpu`: aqui no se vuelve a
        // preguntar al CPUID. Un panel que se repinta no debe costar CPUID.
        // == LA RED ==================================================
        //
        // ** `identidad()` devuelve la foto CACHEADA del arranque; `releer()`
        // vuelve a preguntarle al aparato. Aqui se usan LAS DOS, y el reparto
        // no es un gusto: **lo decide quien lee cada campo**.
        //
        // ```text
        //    MAC, MEGABITS   los pinta `splash`, que se REPINTA. Cacheados: un
        //                    panel a 60 Hz no puede tocar el BAR de una NIC
        //                    60 veces por segundo
        //    PHY_CRUDO       lo lee SOLO la orden `red`, que se TECLEA. Vivo
        // ```
        //
        // *** Y por que el crudo tenia que ser el vivo, y no otro:
        //
        // > La fila se llama *"PHYstatus crudo: la prueba, no la opinion"*, y
        // > una prueba que devuelve la foto del arranque **no puede fallar**.
        // > Desenchufar el cable y escribir `red` tiene que mover ese byte; si
        // > no lo mueve, lo que se esta leyendo no es el silicio -- y eso hay
        // > que saberlo ANTES de montar un anillo de DMA encima.
        //
        // ** Hasta el 2026-08-28 el crudo tambien salia de la cache, y el unico
        // sitio donde la NIC se releia de verdad era la orden `net` del shell
        // de Ring 0 -- o sea **un sitio al que el dueno no vuelve**. Es la
        // misma forma del arreglo de `net rx` del 24-08, y ya van varias.
        //
        // [!] El precedente de que un INFO puede tocar MMIO ya estaba al lado:
        // `INFO_NET_RX_PERDIDAS` lee `MPC` del aparato, y lo lee esta misma
        // orden tecleada. Lo que no puede hacerlo es lo que pinta `splash`.
        INFO_NET_PRESENTE => crate::ring0::dev::net::hay() as u64,
        INFO_NET_VENDOR_DEVICE => {
            let (v, d, _, _, _, _) = crate::ring0::dev::net::donde();
            ((v as u64) << 16) | (d as u64)
        }
        INFO_NET_MAC => crate::ring0::dev::net::identidad()
            .map(|i| {
                let m = i.mac;
                let mut v = 0u64;
                let mut k = 0;
                while k < 6 {
                    v = (v << 8) | (m[k] as u64);
                    k += 1;
                }
                v
            })
            .unwrap_or(0),
        // ** AL APARATO, AHORA. Ver el reparto de arriba: este campo es la
        // prueba, y una prueba cacheada no prueba nada.
        INFO_NET_PHY_CRUDO => crate::ring0::dev::net::releer()
            .map(|i| i.phy as u64)
            .unwrap_or(0),
        // Cero es *"no hay enlace"*, y es una respuesta -- no un fallo.
        //
        // [!] Este SI se queda cacheado, y no por descuido: lo pinta `splash`.
        INFO_NET_MEGABITS => crate::ring0::dev::net::identidad()
            .map(|i| if i.enlace_arriba() { i.megabits() as u64 } else { 0 })
            .unwrap_or(0),
        INFO_NET_RX_ARMADO => crate::ring0::dev::net::rx_activo() as u64,
        INFO_NET_RX_TRAMAS => crate::ring0::dev::net::rx_tramas(),
        INFO_NET_RX_BYTES => crate::ring0::dev::net::rx_consumo().1,
        // ** El unico contador de red que NO lleva BMO-X. Un contador propio
        // solo puede contar lo que se cogio -- lo que se perdio por no haber
        // descriptor libre solo lo sabe el silicio. `None` sale como cero: no
        // hay tarjeta que preguntar, que es distinto de "no se perdio nada" y
        // por eso `INFO_NET_RX_ARMADO` va al lado.
        INFO_NET_RX_PERDIDAS => crate::ring0::dev::net::rx_perdidas().unwrap_or(0) as u64,
        INFO_NET_RX_TIPOS => {
            let (_, _, t, _) = crate::ring0::dev::net::rx_consumo();
            // Se recortan a 16 bits cada uno. Un contador que desborda su
            // casilla y se lleva por delante al vecino diria que llegaron
            // millones de tramas de otro protocolo, y eso es peor que saturar.
            let c = |v: u64| v.min(0xFFFF);
            c(t[0]) | (c(t[1]) << 16) | (c(t[2]) << 32) | (c(t[3]) << 48)
        }
        INFO_NET_PCI => {
            let (_, _, bus, dev, fun, _) = crate::ring0::dev::net::donde();
            ((bus as u64) << 16) | ((dev as u64) << 8) | (fun as u64)
        }
        INFO_CPU_HILOS => cpu_topo().map(|t| t.hilos as u64).unwrap_or(0),
        INFO_CPU_NUCLEOS => cpu_topo().map(|t| t.nucleos as u64).unwrap_or(0),
        // * Se lee de un atomico, no se recalcula: esto lo pide un panel que se
        // repinta, y enumerar la MADT por fotograma seria un impuesto. El careo
        // corre UNA vez, en `phase::main`.
        INFO_CPU_TOPOLOGIA_DUDA => crate::ring0::cpu_vendor::profile::duda() as u64,
        INFO_CPU_HILOS_POR_NUCLEO => cpu_topo().map(|t| t.hilos_por_nucleo as u64).unwrap_or(0),
        INFO_TAREAS_TOTAL => crate::ring0::task::scheduler::counts().0 as u64,
        INFO_TAREAS_LISTAS => crate::ring0::task::scheduler::counts().1 as u64,
        INFO_PANTALLA_DUENO => crate::ring0::obj::fb::owner().unwrap_or(0) as u64,
        INFO_TAREAS_LIBRES => crate::ring0::task::scheduler::huecos_libres() as u64,
        INFO_TICKS => crate::ring0::plat::timer::ticks(),
        INFO_SYSCALL_CUENTA => crate::ring0::syscall::meter::doors(),
        INFO_SYSCALL_CICLOS => crate::ring0::syscall::meter::cycles(),
        INFO_SYSCALL_CICLOS_GUARDA => crate::ring0::syscall::meter::ciclos_guarda(),
        INFO_SYSCALL_CICLOS_RESTAURA => crate::ring0::syscall::meter::ciclos_restaura(),
        // * Las tres contestan CERO --o sea "sin declarar"-- cuando el CPU de
        // delante no es aquel en el que se midieron. La proteccion esta en el
        // VALOR y no en que el cliente se acuerde de mirar el campo de al lado:
        // quien no conozca `INFO_PRESUPUESTO_MAQUINA` pierde el motivo, nunca
        // el freno. Ver `syscall/presupuesto.rs`.
        INFO_PRESUPUESTO_PUERTA => crate::ring0::syscall::presupuesto::puerta(),
        INFO_PRESUPUESTO_DISPATCH => crate::ring0::syscall::presupuesto::dispatch(),
        INFO_PRESUPUESTO_HANDLE => crate::ring0::syscall::presupuesto::handle(),
        INFO_PRESUPUESTO_MAQUINA => crate::ring0::syscall::presupuesto::veredicto_maquina(),
        INFO_SUELO_CRUCE => crate::ring0::syscall::presupuesto::suelo(),
        // ** El censo entero cabe en tres numeros porque son treinta y seis
        // filas: una mascara de 64 bits sobra. Si algun dia [`ALL`] pasa de 64,
        // `INFO_CPU_EXT_N` es lo que lo dice en voz alta -- por eso viaja el
        // tamano y no se da por sabido en el otro lado.
        INFO_CPU_EXT_N => {
            crate::ring0::cpu_vendor::features::ALL.len() as u64
        }
        INFO_CPU_EXT_HAY | INFO_CPU_EXT_USA => {
            let c = crate::ring0::cpu_vendor::features::censar();
            let mut m = 0u64;
            let mut i = 0;
            while i < c.filas.len() && i < 64 {
                let f = &c.filas[i];
                let bit = if n == INFO_CPU_EXT_HAY { f.hay } else { f.uso.is_yes() };
                if bit {
                    m |= 1u64 << i;
                }
                i += 1;
            }
            m
        }
        INFO_CPU_EXT_AVERIAS => {
            let c = crate::ring0::cpu_vendor::features::censar();
            (c.conflictos as u64)
                | ((c.mudas as u64) << 16)
                | ((c.repetidas as u64) << 32)
                | ((c.sin_sitio as u64) << 48)
        }
        INFO_FECHA => crate::ring0::dev::clock::ahora(),
        // Medido, no declarado: desde donde lo enlaza el guion hasta el final
        // de su `.bss`, que incluye la pila de 64 KiB.
        INFO_KERNEL_BYTES => {
            extern "C" {
                static __bss_end: u8;
            }
            let fin = unsafe { &__bss_end as *const u8 as u64 };
            fin.saturating_sub(0x400000)
        }
        INFO_PROGRAMAS => crate::ring0::task::proc::programs().len() as u64,
        INFO_PROGRAMAS_OLVIDADOS => crate::ring0::task::proc::programas_olvidados() as u64,
        INFO_DISCO_LISTO => crate::ring0::dev::disk::is_ready() as u64,
        INFO_DATOS_MONTADO => crate::ring0::fsys::fs::data_mounted() as u64,
        INFO_ES_MONTADO => crate::ring0::fsys::estratos::is_mounted() as u64,
        INFO_ES_IDENTIDAD => crate::ring0::fsys::estratos::identidad_ok() as u64,
        INFO_ES_FECHA => crate::ring0::fsys::estratos::fecha_estrato(),
        INFO_ES_GENERACION => {
            crate::ring0::fsys::estratos::superbloque().map_or(0, |sb| sb.generation)
        }
        INFO_ES_BLOQUES => {
            crate::ring0::fsys::estratos::ocupacion().map_or(0, |o| o.totales)
        }
        INFO_ES_USADOS => {
            crate::ring0::fsys::estratos::ocupacion().map_or(0, |o| o.usados)
        }
        INFO_ES_BLOQUE_TAM => {
            crate::ring0::fsys::estratos::superbloque().map_or(0, |sb| sb.block_size as u64)
        }
        INFO_ES_NIVEL => crate::ring0::fsys::estratos::ocupacion().map_or(0, |o| {
            match o.nivel() {
                bmo_estratos::Nivel::Holgado => 0,
                bmo_estratos::Nivel::Ambar => 1,
                bmo_estratos::Nivel::Rojo => 2,
                bmo_estratos::Nivel::SoloLectura => 3,
            }
        }),
        // ** ESTO CONTESTABA UN CERO CONSTANTE, Y ERA CIERTO HASTA QUE DEJO DE SERLO.
        //
        // El comentario que lo defendia decia que la maquina de estados existia
        // pero que **nadie la habia cableado al dispositivo**: ni `write` ni
        // `FLUSH CACHE`. Eso era verdad el dia que se escribio.
        //
        // Hoy `sellar` escribe el superbloque de verdad y hace las dos barreras
        // --el disco de Eddi va por la generacion 3, o sea que ha commiteado
        // tres veces-- y el recorte escribe tambien. O sea que el cero se
        // convirtio en **una fila que dice que no se puede hacer lo que se
        // acaba de hacer**, y en metal salio como `escribir NO` debajo de un
        // volumen montado y sano.
        //
        // > Un valor fijo puesto por prudencia envejece hacia la MENTIRA, y no
        // > avisa: lo unico que cambia a su alrededor es el mundo.
        //
        // Ahora son las tres condiciones que de verdad deciden, y las tres se
        // preguntan a quien las sabe.
        INFO_ES_ESCRIBIBLE => {
            let hay_volumen = crate::ring0::fsys::estratos::is_mounted()
                && crate::ring0::fsys::estratos::identidad_ok();
            let sitio = crate::ring0::fsys::estratos::ocupacion()
                .map(|o| o.nivel().admite_escritura())
                .unwrap_or(false);
            (hay_volumen && sitio && crate::ring0::dev::disk::write_armed()) as u64
        }
        // Lo que Ring 3 ha PEDIDO. Cero hasta que un programa llame a
        // `KIND_MEMORIA` -- y por eso vale: es la unica fila del informe que
        // solo se mueve si alguien ejercio la capability.
        INFO_MEM_ENTREGADA => crate::ring0::obj::memory::total_handed_over(),
        // Sin contar el BSP, que siempre esta. Es el numero que devolvio el
        // bring-up, no una suposicion sobre lo que declara el CPU.
        INFO_SMP_VIVOS => crate::ring0::plat::smp::alive().0 as u64,
        // * Y los dos que tienen que dar CERO. Un choque de cerrojo hoy
        // significa que alguien rompio la regla de oro --un obrero que solo
        // computa no entra en el kernel-- y se ve aqui antes de que corrompa
        // nada. Ver `plat/spin.rs`.
        INFO_SPIN_CHOQUES => crate::ring0::plat::spin::contention().0 as u64,
        INFO_SPIN_PICO => crate::ring0::plat::spin::contention().1 as u64,
        INFO_FUGAS => crate::ring0::core::autopsy::fugas(),
        // * Estas dos NO miran el hardware: leen la foto que dejo el ultimo
        // bombeo. Tocar el xHCI aqui seria hacerlo con el CR3 del programa que
        // pregunta, y su MMIO no esta ahi. Ver `dev/usb/salud.rs`.
        INFO_USB_SALUD => crate::ring0::dev::usb::salud::estado(),
        INFO_USB_AVERIAS => crate::ring0::dev::usb::salud::averias(),
        // * Las cuatro leen la foto que dejo `identify()` en el arranque, no el
        // aparato: mandar un IDENTIFY aqui seria hablarle al disco con el CR3
        // del programa que pregunta, y ademas robarle la unica ranura de
        // comando a quien la tuviera. Es la misma razon que las dos del USB.
        INFO_DISCO_MEDIO => crate::ring0::dev::disk::medio(),
        INFO_DISCO_ENLACE => crate::ring0::dev::disk::enlace(),
        INFO_DISCO_GEOMETRIA => crate::ring0::dev::disk::geometria(),
        INFO_DISCO_JUICIO => crate::ring0::dev::disk::juicio(),
        INFO_DISCO_TRIM_SECTORES => crate::ring0::dev::disk::cuentas_trim().0,
        INFO_DISCO_TRIM_ORDENES => crate::ring0::dev::disk::cuentas_trim().1,
        // Los dos salen de la MISMA funcion que usa el recorte. Cero = no hay
        // volumen montado o la cola esta vacia, que es la misma respuesta que
        // le dara la orden a quien la pida.
        INFO_DISCO_COLA_LBA => {
            crate::ring0::fsys::estratos::cola_libre().map(|(lba, _)| lba).unwrap_or(0)
        }
        INFO_DISCO_COLA_SECTORES => {
            crate::ring0::fsys::estratos::cola_libre().map(|(_, n)| n).unwrap_or(0)
        }
        INFO_DISCO_TRIM_BLOQUES => crate::ring0::dev::disk::trim_bloques_max() as u64,
        // ** POR QUE fallo el ultimo recorte: `(clase << 32) | PxTFD`. Sin este
        // campo, "el disco respondio con error" es una frase sin numero -- y en
        // este componente el numero es lo unico que separa un ABRT de un
        // timeout, o sea dos conversaciones distintas.
        INFO_DISCO_TRIM_FALLO => crate::ring0::dev::disk::ultimo_fallo(),
        _ => 0,
    }
}

/// La topologia, **por el perfil y no por el nombre del fabricante**.
///
/// Esta funcion nombraba `cpu_vendor::ryzen_5_5600x::bmo_cpu::topology()`
/// directamente, y la cabecera de `profile.rs` dice literalmente que el resto de
/// Ring 0 *"nunca nombra un modulo de fabricante directamente"*. La regla estaba
/// escrita en el propio fichero que define la abstraccion, y rota aqui.
fn cpu_topo() -> Option<crate::ring0::cpu_vendor::profile::Nucleos> {
    (crate::ring0::cpu_vendor::profile::active().nucleos)()
}

/// Ocho bytes del campo de texto, empaquetados en little-endian.
///
/// `trozo` cuenta de 8 en 8. Fuera del texto devuelve 0, y el cero es el final:
/// el llamante lee trozos hasta que llega uno con un cero dentro, igual que en
/// `console::write_const`. Es feo y es seguro, y lo segundo importa mas -- pasar
/// un puntero de Ring 3 obligaria al kernel a validar el rango entero contra el
/// espacio del llamante, y esa infraestructura no existe.
pub fn texto(n: u64, trozo: u64) -> u64 {
    let p = crate::ring0::cpu_vendor::profile::active();
    let s: &str = match n {
        INFO_TXT_CPU_VENDOR => p.vendor,
        INFO_TXT_CPU_NOMBRE => p.name,
        INFO_TXT_UARCH => p.microarch,
        INFO_TXT_FAMILIA => p.family_model,
        // El indice en los bits altos, igual que `INFO_MEM_QUIEN_*`. Fuera de
        // rango contesta la cadena vacia, que el llamante ya sabe leer como
        // final -- pedir la fila 200 no es un error, es el final de la tabla.
        c if c & 0xFF == INFO_TXT_EXT_NOMBRE || c & 0xFF == INFO_TXT_EXT_NOTA => {
            let i = (c >> 8) as usize;
            match crate::ring0::cpu_vendor::features::ALL.get(i) {
                Some(&f) => {
                    if c & 0xFF == INFO_TXT_EXT_NOMBRE {
                        f.name()
                    } else {
                        crate::ring0::cpu_vendor::features::usage::of(f).nota()
                    }
                }
                None => "",
            }
        }
        _ => "",
    };
    let b = s.as_bytes();
    let base = (trozo as usize).saturating_mul(8);
    let mut w = [0u8; 8];
    for i in 0..8 {
        match b.get(base + i) {
            Some(&c) => w[i] = c,
            None => break,
        }
    }
    u64::from_le_bytes(w)
}
