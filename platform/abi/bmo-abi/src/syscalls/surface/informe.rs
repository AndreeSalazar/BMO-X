//! **Lo que se CONSULTA y no cambia nada**: `INFO_*`, `CABINA_*`, `AUTOPSIA_*`
//! y `KLOG_*`.
//!
//! Cincuenta y ocho constantes con una propiedad en comun que decide el
//! reparto: **ninguna ejerce poder**. Leer un contador, una linea del log o el
//! informe de una muerte no concede nada, y por eso ninguna pide capability --
//! el mismo trato que tiene `INFO` desde el principio.
//!
//! # Por que es la familia que mas hay que vigilar
//!
//! Porque existe TRES veces: la implementa el kernel (`core/informe.rs`), la
//! declara este fichero y la consume el userland. Una fila escrita en dos de los
//! tres sitios es **un campo que contesta otra cosa de la que se pidio, sin que
//! nada falle al compilar**. Lo comprueba `build.ps1` sacando la lista de los
//! tres ficheros, nunca a mano.

/// Bytes de RAM que el asignador de marcos gobierna.
pub const INFO_RAM_TOTAL: u64 = 0x01;

/// Bytes libres AHORA.
pub const INFO_RAM_LIBRE: u64 = 0x02;

/// Marcos totales de 4 KiB.
pub const INFO_RAM_MARCOS: u64 = 0x03;

/// Marcos libres.
pub const INFO_RAM_MARCOS_LIBRES: u64 = 0x04;

/// Frecuencia del TSC en Hz. Es la que mide el tiempo de verdad en esta
/// maquina, no un numero nominal de la etiqueta.
pub const INFO_TSC_HZ: u64 = 0x05;

/// **La frecuencia efectiva del nucleo AHORA, en Hz.** `0` = no se puede medir.
///
/// No es [`INFO_TSC_HZ`]: ese es el reloj de referencia, que no cambia nunca.
/// Este es a que va el nucleo de verdad, que en un Zen 3 se mueve entre 3,7 y
/// 4,6 GHz segun cuantos trabajen.
///
/// ** Es una MEDIDA, no un dato: sale de restar dos lecturas de MPERF/APERF, o
/// sea que **preguntarlo dos veces seguidas da la velocidad de ese intervalo**.
/// Un panel que se repinta obtiene la del ultimo refresco, que es lo que quiere.
pub const INFO_CPU_HZ_REAL: u64 = 0x20;

/// **Milivatios del PAQUETE desde la ultima consulta.** `0` = no se puede medir.
/// Medida por diferencia, como [`INFO_CPU_HZ_REAL`].
pub const INFO_CPU_MW_PAQUETE: u64 = 0x21;

/// **Milivatios del NUCLEO EN EL QUE SE LEE.** No de todos.
///
/// [!] La primera version de esta linea decia "de los nucleos", en plural, y el
/// metal del 12-08 enseno por que eso es poner un dato que no existe: con once
/// nucleos GIRANDO al 100%, este numero **bajo** de 11,9 a 9,2 W. No es que
/// consumieran menos: es que `CORE_ENERGY_STAT` es un contador **por nucleo** y
/// solo se lee el del BSP. Los otros once no aparecen aqui en absoluto.
///
/// Para verlos hace falta que **cada nucleo lea el suyo**, o sea trabajo
/// repartido -- la seccion 5 de `AXION_MAESTRO.md` antes que esto.
pub const INFO_CPU_MW_NUCLEO_ACTUAL: u64 = 0x22;

// [!] AQUI VIVIA `INFO_CPU_MW_NUCLEOS`, y su borrado es la leccion.
//
// Se renombro a `INFO_CPU_MW_NUCLEO_ACTUAL` porque el plural mentia, y se dejo
// el nombre viejo como `#[deprecated]` "para no romper a quien lo use". **El
// guardian de contrato paro el build**, y tenia razon:
//
//   [X] OP_INFO field contract: INFO_CPU_MW_NUCLEOS falta en kernel, userland
//
// Una constante que vive en el ABI y no existe en los otros dos lados ES la
// deriva que ese guardian existe para cazar -- da igual que este marcada como
// obsoleta. Un alias amable en un CONTRATO no es amable: es un tercer nombre
// para un numero, y el contrato pasa a tener dos verdades.
//
// Y aqui no habia nada que no romper: el nombre nacio y murio el mismo dia.
/// **Que sabe medir el perfil de este silicio**, como banderas.
/// bit 0 = frecuencia efectiva / bit 1 = consumo.
///
/// Es lo que permite a la terminal decir QUE esta aplicando, en vez de pintar
/// ceros y dejar al que mira sin saber si el sensor no existe o el valor es 0.
pub const INFO_CPU_SENSORES: u64 = 0x23;

/// El pid de la ranura `n`. **`0` = no hay mas**, y es la condicion de parada.
pub const INFO_MEM_QUIEN_PID: u64 = 0x24;

/// Bytes que ese proceso tiene pedidos ahora mismo.
pub const INFO_MEM_QUIEN_BYTES: u64 = 0x25;

/// Cuantas peticiones lleva hechas. Distingue *"pidio un bloque grande"* de
/// *"esta pidiendo sin parar"*, que es la diferencia entre un juego y una fuga.
pub const INFO_MEM_QUIEN_PETICIONES: u64 = 0x26;


/* == LA RED, VISTA DESDE RING 3 ====================================
 *
 * ** Siete campos, y hasta hoy eran CERO: el kernel encontraba la NIC, le leia
 * la MAC y el estado del enlace, y **nada de eso cruzaba a Ring 3**. Un panel de
 * red en el compositor no era una cuestion de dibujar: era imposible, porque no
 * habia forma de preguntar.
 *
 * Son campos de INFORME y no operaciones sobre un handle a proposito: leer si
 * hay cable no es un privilegio, es una pregunta -- el mismo criterio que
 * `core::report` aplica a la RAM y a los nucleos. Transmitir SI necesitara una
 * capability; mirar, no.
 *
 * [!] Y son SIETE y no uno con banderas dentro. Un campo por hecho es lo que
 * permite que el panel diga *"hay NIC, no hay enlace"* en vez de *"red: 0"*, que
 * es la diferencia entre un diagnostico y un adorno.
 */

/// Hay una NIC reconocida: `1` o `0`. Lo primero que hay que saber, y lo unico
/// que distingue *"no hay tarjeta"* de *"hay tarjeta y no hay cable"*.
pub const INFO_NET_PRESENTE: u64 = 0x27;

/// `vendor << 16 | device` del PCI. En esta placa, `0x10EC8168` -- una Realtek
/// RTL8168. Se entrega crudo: el numero ES la identificacion, y traducirlo a un
/// nombre bonito en el kernel seria meter una tabla de fabricantes en Ring 0.
pub const INFO_NET_VENDOR_DEVICE: u64 = 0x28;

/// La MAC, los seis bytes en los 48 bits bajos, byte 0 el mas significativo.
///
/// ** Cabe en UN campo y por eso va en uno. Una MAC son 48 bits y un campo de
/// informe son 64: partirla en dos habria sido inventarse un problema de
/// ensamblado en el lado del cliente.
pub const INFO_NET_MAC: u64 = 0x29;

/// El byte `PHYstatus` **CRUDO**, sin interpretar.
///
/// ** Y va crudo a proposito, que es la misma decision que ya tomo el driver:
/// *"se guarda sin interpretar ademas de interpretado: el dia que un bit no
/// cuadre, el byte entero es la prueba y las funciones son la opinion"*. Un
/// panel que solo ensena la opinion no puede ayudar el dia que la opinion falle.
pub const INFO_NET_PHY_CRUDO: u64 = 0x2A;

/// Megabits que declara el enlace: 10, 100, 1000 -- o `0` si esta abajo.
///
/// [!] El cero no es un error: es *"no hay cable"*, y es una respuesta.
pub const INFO_NET_MEGABITS: u64 = 0x2B;

/// El receptor esta armado: `1` o `0`. Distingue *"no llega nada"* de *"no
/// estamos escuchando"*, que es la confusion mas cara de depurar en una red.
pub const INFO_NET_RX_ARMADO: u64 = 0x2C;

/// Tramas recibidas desde que se armo. **La cifra que dice si el cable vive.**
pub const INFO_NET_RX_TRAMAS: u64 = 0x2D;

/// Donde esta en el bus: `bus << 16 | dispositivo << 8 | funcion`.
///
/// Hace falta para el caso raro y real de dos NIC: sin esto, dos tarjetas dan
/// dos informes identicos y no hay forma de decir de cual habla cada uno.
pub const INFO_NET_PCI: u64 = 0x2E;

/// -- ** EL METRO DE LA PUERTA -------------------------------------------
///
/// Cuantas puertas ha servido el kernel, y cuantos ciclos ha pasado DENTRO de
/// `dispatch` sirviendolas. Se leen los dos y se dividen.
///
/// Existen porque el 2026-08-16 `c/coste.bex` midio una puerta desde Ring 3 en
/// **2615 ciclos** contra 20 de una llamada normal, y ese numero no podia
/// contestar la pregunta siguiente: **donde se van.** Restando lo de dentro de
/// `dispatch` al total queda **lo que tarda el stub de ensamblador** -- los
/// pushes, el `xsave64`, el `xrstor64` y el `iretq`. Sin esa resta, tocar el
/// stub seria operar sobre una sospecha, y es el codigo que produjo el `#GP` en
/// `xrstor`.
///
/// [!] **Se leen como DELTA, no como absoluto**: antes y despues del bucle que
/// se quiera medir. No hay operacion de puesta a cero a proposito -- un delta
/// mide justo la poblacion que interesa y no arrastra lo que hizo la maquina
/// arrancando. Y las dos lecturas son dos puertas, que tambien se cuentan.
///
/// ** LA RESPUESTA, medida en el Ryzen el mismo dia: `2663 = dispatch 318 +
/// stub 2345`, o sea el **88% en el ensamblador**. Y resolver una capability
/// son **83 ciclos**, 76 de ellos dentro de `dispatch`. Estos dos campos ya
/// hicieron su trabajo; siguen aqui porque el reparto vuelve a hacer falta
/// cada vez que se toque `entry.rs`.
pub const INFO_SYSCALL_CUENTA: u64 = 0x2F;

/// Ciclos de TSC acumulados dentro de `dispatch`. Ver [`INFO_SYSCALL_CUENTA`].
pub const INFO_SYSCALL_CICLOS: u64 = 0x30;

/// Hilos logicos y nucleos fisicos que el CPU declara.
pub const INFO_CPU_HILOS: u64 = 0x06;

pub const INFO_CPU_NUCLEOS: u64 = 0x07;

/// Tareas: ranuras ocupadas, listas para correr, y libres.
pub const INFO_TAREAS_TOTAL: u64 = 0x08;

pub const INFO_TAREAS_LISTAS: u64 = 0x09;

pub const INFO_TAREAS_LIBRES: u64 = 0x0A;

/// Ticks del temporizador desde el arranque.
pub const INFO_TICKS: u64 = 0x0B;

/// Bytes que ocupa el kernel en RAM, medidos (hasta el final de su `.bss`,
/// pila incluida). No es el tamano del archivo.
pub const INFO_KERNEL_BYTES: u64 = 0x0C;

/// Programas que se han intentado admitir, y los que ya no caben en la
/// bitacora. La suma es el total de verdad.
pub const INFO_PROGRAMAS: u64 = 0x0D;

pub const INFO_PROGRAMAS_OLVIDADOS: u64 = 0x0E;

/// Hay disco listo? Esta montado el volumen de datos para escribir?
pub const INFO_DISCO_LISTO: u64 = 0x0F;

pub const INFO_DATOS_MONTADO: u64 = 0x10;

/// -- ESTRATOS ------------------------------------------------------
///
/// El volumen de datos grande. Ring 3 los necesita para poder ENSENAR el estado
/// del almacen sin cruzar a Ring 0 por cada dato: son una fila mas de la tabla
/// de `OP_INFO`, que es como crece esta superficie sin tocar el ABI.
pub const INFO_ES_MONTADO: u64 = 0x11;

/// Generacion del superbloque: cuantas transacciones lleva el volumen.
pub const INFO_ES_GENERACION: u64 = 0x12;

pub const INFO_ES_BLOQUES: u64 = 0x13;

pub const INFO_ES_USADOS: u64 = 0x14;

pub const INFO_ES_BLOQUE_TAM: u64 = 0x15;

/// 0 holgado, 1 ambar, 2 rojo, 3 solo lectura. Ver `bmo_estratos::espacio`.
pub const INFO_ES_NIVEL: u64 = 0x16;

/// El gate del section 5: 1 si el volumen nacio en ESTE disco.
pub const INFO_ES_IDENTIDAD: u64 = 0x17;

/// 1 si hoy se puede escribir. Hoy siempre 0: falta cablear la E/S.
pub const INFO_ES_ESCRIBIBLE: u64 = 0x18;

/// **Bytes que Ring 3 ha PEDIDO** con `KIND_MEMORIA`, desde el arranque.
///
/// Es el unico dato de memoria que el kernel no puede deducir mirando lo que
/// cargo: la imagen y la pila de un proceso las puso el, pero un bloque pedido
/// solo existe porque alguien lo pidio. Y es la confirmacion **desde el otro
/// lado** de que la capability funciona -- el programa dice que le dieron
/// memoria; esto lo dice el kernel.
///
/// Contador que ya existia en `ring0::obj::memoria::total_handed_over()` y que
/// **no leia nadie**. Un contador que nadie consulta no es telemetria: es una
/// variable.
pub const INFO_MEM_ENTREGADA: u64 = 0x19;

/// * Quien tiene la pantalla: su `pid`, o **`0` si no la tiene nadie**.
///
/// Se PREGUNTA en vez de intentar reclamarla, y la diferencia importa: probar a
/// reclamarla para saber si esta libre **te la deja puesta**, y entonces se la
/// robas al programa al que se la ibas a prestar.
///
/// Estaba en el kernel y en el userland y **faltaba aqui**, que es justo la
/// deriva que ahora vigila `build.ps1`.
pub const INFO_PANTALLA_DUENO: u64 = 0x1A;

/// -- SMP ------------------------------------------------------------
///
/// Nucleos de aplicacion en pie, **sin contar el BSP**. Es lo que contesto el
/// bring-up, no lo que declara el CPU: la diferencia entre los dos es
/// exactamente el fallo que un panel tiene que poder ensenar.
pub const INFO_SMP_VIVOS: u64 = 0x1B;

/// * Choques de cerrojo, y la espera mas larga en vueltas de giro.
///
/// **Los dos tienen que ser CERO**, y por eso valen. Con un solo nucleo nadie
/// puede encontrar un cerrojo tomado, y los obreros de SMP solo computan: no
/// entran en el kernel, asi que no hay quien pelee. Un numero distinto de cero
/// no mide rendimiento -- dice que una de esas dos frases dejo de ser cierta.
pub const INFO_SPIN_CHOQUES: u64 = 0x1C;

pub const INFO_SPIN_PICO: u64 = 0x1D;

/// ** Recursos que una tarea muerta dejo SIN DEVOLVER, acumulados.
///
/// **Tiene que ser CERO**, y un numero distinto no acusa al programa que murio:
/// acusa al KERNEL, que dijo haberlo recuperado todo y no lo hizo.
///
/// Es la misma clase de numero que `INFO_SPIN_CHOQUES` y va al lado a
/// proposito: los dos son el sistema comprobandose a si mismo.
pub const INFO_FUGAS: u64 = 0x1E;

/// **La fecha y hora de la placa**, empaquetada en un solo numero:
/// `anio<<40 | mes<<32 | dia<<24 | hora<<16 | minuto<<8 | segundo`.
/// `0` = la maquina no sabe que dia es.
///
/// ** UN campo y no seis. La puerta contesta un numero por llamada, y seis
/// llamadas se pueden leer **a caballo de un cambio de minuto**: daria `10:59`
/// con los segundos del `11:00`. Empaquetada, la fecha es atomica por
/// construccion y no hace falta ningun cerrojo. Desempaquetarla es
/// `bmo_rtc::desempaquetar`.
pub const INFO_FECHA: u64 = 0x1F;

/// Fabricante ("AMD"), nombre comercial, microarquitectura y familia/modelo.
pub const INFO_TXT_CPU_VENDOR: u64 = 0x01;

pub const INFO_TXT_CPU_NOMBRE: u64 = 0x02;

pub const INFO_TXT_UARCH: u64 = 0x03;

pub const INFO_TXT_FAMILIA: u64 = 0x04;

/// Campos de [`TASK_OP_KLOG_INFO`].
pub const KLOG_DISPONIBLES: u64 = 0x00;

pub const KLOG_TOTAL: u64 = 0x01;

/// Campos de [`TASK_OP_AUTOPSIA_INFO`].
///
/// `AUTOPSIA_TOTAL` es el que se mira en bucle: **si cambio, hay un fallo
/// nuevo**, y eso se sabe sin leer un solo renglon.
pub const AUTOPSIA_TOTAL: u64 = 0x00;

pub const AUTOPSIA_DISPONIBLES: u64 = 0x01;

pub const AUTOPSIA_RENGLONES: u64 = 0x02;

/// Campos de [`TASK_OP_CABINA_INFO`].
pub const CABINA_TOTAL: u64 = 0x00;

pub const CABINA_PERDIDOS: u64 = 0x01;

pub const CABINA_DISPONIBLES: u64 = 0x02;

pub const CABINA_SEVERIDAD: u64 = 0x03;

pub const CABINA_CAPA: u64 = 0x04;

pub const CABINA_VALOR: u64 = 0x05;

pub const CABINA_SEQ: u64 = 0x06;

pub const CABINA_TICK: u64 = 0x07;

/// **De que INTENTO salio el evento.** `0` = de ninguno.
///
/// === Por que este campo cambia lo que CABINA puede hacer ===
///
/// Los otros siete dicen **que paso**. Este dice **a que accion pertenece**, y
/// esa es otra pregunta: un lanzamiento emite eventos desde cuatro modulos
/// --`lanzar`, `proc`, `bex`, `disk`-- y hasta ahora, para saber cuales eran de
/// TU pulsacion, habia que juntarlos de memoria mirando el `#N` impreso.
///
/// El kernel ya los agrupa (`cabina::intento`) y ya pinta el numero en su
/// panel. Lo que faltaba era **entregarselo a Ring 3**, que es donde esta la
/// ventana con filtros. Sin este campo, el filtro de la caja solo podia ser por
/// gravedad: "ensename los FALLO" -- que trae los de esta accion y los de las
/// diez anteriores mezclados.
///
/// Con el, la pregunta pasa a ser la util: **"ensename TODO lo que hizo esto que
/// acabo de pulsar"**.
pub const CABINA_INTENTO: u64 = 0x08;

/// Que texto pide [`TASK_OP_CABINA_TEXTO`].
pub const CABINA_TXT_MODULO: u64 = 0x00;

pub const CABINA_TXT_MENSAJE: u64 = 0x01;
