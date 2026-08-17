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

/// -- ** EL REPARTO DENTRO DEL STUB ---------------------------------------
///
/// El primer reparto dejo el 88% de una puerta en el ensamblador y no supo
/// decir en QUE parte. Cambiar `xsave64` por `xsaveopt64` --el sospechoso
/// nombrado-- compro 45 ciclos de 2345: **el 2%**. Estos dos campos parten esa
/// mitad ciega en tres trozos que se leen igual, como delta:
///
/// ```text
///    GUARDA     la cabecera a cero + el `xsaveopt64`
///    CICLOS     dentro de `dispatch`  (ya existia)
///    RESTAURA   las comprobaciones del sello + el `xrstor64`
///    resto      total - los tres = `syscall` + pushes + pops + `iretq`
/// ```
///
/// **`resto` es la casilla que decide.** Grande significa que el coste esta en
/// las dos transiciones de privilegio y que afinar el stub no lo va a mover --
/// lo que se mueve entonces es `sysretq` en vez de `iretq`, o agrupar llamadas.
///
/// [!] Se dividen entre [`INFO_SYSCALL_CUENTA`], la MISMA cuenta de puertas que
/// los ciclos de `dispatch`: las tres etapas ocurren una vez por puerta, asi
/// que las cuatro cifras se reparten el mismo denominador y **tienen que sumar
/// menos que el total**. Si suman mas, el instrumento miente y se dice.
pub const INFO_SYSCALL_CICLOS_GUARDA: u64 = 0x35;

/// Ciclos devolviendo el contexto. Ver [`INFO_SYSCALL_CICLOS_GUARDA`].
pub const INFO_SYSCALL_CICLOS_RESTAURA: u64 = 0x36;

/// -- ** EL HISTOGRAMA POR CLASE: donde se USA la puerta -------------------
///
/// `INFO_SYSCALL_CUENTA` dice **cuantas** puertas. Esto dice **de que tipo**, y
/// es la mitad que faltaba: se sabia lo que cuesta cada clase --875 / 1125 /
/// 2,2 M-- y **no cuantas veces se pide cada una**, asi que *"donde se usa mas"*
/// era una suposicion. Un coste por vez sin veces por segundo no es un
/// porcentaje y no puede ordenar el trabajo (`docs/CENSO_DE_EJES.md`, R-CENSO3).
///
/// ```text
///    0  TAREA     pseudo-capability: `INVOKE(CURRENT_TASK, ...)`   ~875
///    1  HANDLE    resolvio una capability REAL                    ~1125
///    2  CONSOLA   escritura de consola                            ~2,2 M
///    3  ESPERA    `WAIT`                                          cede el turno
/// ```
///
/// ** LAS CUATRO SE DECIDEN POR CONSTRUCCION, NO POR UNA LISTA. La primera
/// version iba a separar "operacion barata" de "operacion que camina una tabla",
/// y eso pedia una lista de operaciones escrita a mano -- que es lo que ya se
/// quedo congelada dos veces en este arbol. Estas cuatro salen de datos que el
/// despachador **ya tiene en registros**: que puerta es, si el handle era
/// `CURRENT_TASK`, y si la operacion es la de consola (que tiene su propia rama
/// desde siempre). Ninguna casilla necesita saber nada nuevo.
///
/// [!] **Por que CONSOLA se saca aparte aunque sea una operacion de tarea**:
/// porque cuesta **2.500 veces** una puerta pelada. Metida en `TAREA` se lleva
/// la media entera y tapa justo lo que se quiere ver. Es la unica operacion del
/// sistema con esa diferencia de orden de magnitud, y por eso es una casilla y
/// no el principio de una lista.
///
/// El indice va EMPAQUETADO en el campo, igual que `INFO_MEM_QUIEN_*`:
/// `INFO_SYSCALL_CLASS | (clase << 8)`. Un campo nuevo por casilla habria sido
/// cuatro numeros en el contrato congelado para responder una sola pregunta.
///
/// [!] **Y suman MENOS que [`INFO_SYSCALL_CUENTA`], a proposito**: lo que no cae
/// en ninguna casilla es una puerta que no era ninguna de las cuatro (hoy, el
/// numero de syscall retirado). Esa resta es la comprobacion del instrumento --
/// si algun dia sale grande, hay trafico que este histograma no esta viendo.
pub const INFO_SYSCALL_CLASS: u64 = 0x3A;

/// `INVOKE` sobre `CURRENT_TASK`: no resuelve ningun handle.
pub const SYSCALL_CLASS_TASK: u64 = 0x00;
/// `INVOKE` que resolvio una capability real -- paga el handle.
pub const SYSCALL_CLASS_HANDLE: u64 = 0x01;
/// Escritura de consola: dibuja glifos y hace scroll.
pub const SYSCALL_CLASS_CONSOLE: u64 = 0x02;
/// `WAIT`: la unica puerta que puede no devolver el turno.
pub const SYSCALL_CLASS_WAIT: u64 = 0x03;
/// Cuantas casillas tiene el histograma. Ver [`INFO_SYSCALL_CLASS`].
///
/// [!] Va en HEXADECIMAL como sus cuatro hermanas, y no por gusto: el guardian
/// del build las barre con el mismo patron que las operaciones
/// (`= 0x...`), asi que un `= 4` decimal las dejaria **fuera de la
/// comprobacion sin que nada avise** -- un guardian que lee menos no avisa de
/// menos: avisa de nada.
pub const SYSCALL_CLASS_COUNT: u64 = 0x04;

/// -- ** EL PRESUPUESTO DE CICLOS ------------------------------------------
///
/// Lo que una puerta **tiene permitido** costar. El metro dice lo que cuesta
/// hoy; sin esto, nada en el arbol impide que la proxima pieza lo devuelva a
/// 2000. Un numero sin contrato es una anecdota.
///
/// Cada campo trae DOS numeros empaquetados, `meta << 32 | techo`:
///
/// ```text
///    techo   la ultima medida CONFIRMADA en metal. Cruzarlo es una regresion.
///    meta    a donde tiene que llegar. No alcanzarla es DEUDA, no fallo.
/// ```
///
/// Van juntos en un campo --como [`INFO_CPU_EXT_AVERIAS`] empaqueta cuatro--
/// porque separarlos permitiria leer uno y no el otro, que es justo el error
/// que hace decir *"cumple"* a algo que no llego a la meta.
///
/// [!] **No se comprueba en el arranque, y no es un olvido**: al arrancar no se
/// ha servido ni una puerta y el metro esta vacio. Un presupuesto solo se juzga
/// contra trafico real, asi que quien lo lee es `c/coste.bex` desde Ring 3 --
/// que ademas es el unico sitio alcanzable desde el escritorio.
///
/// La tabla y el porque de cada cifra viven en
/// `ring0/syscall/presupuesto.rs`; aqui solo esta la ventana.
pub const INFO_PRESUPUESTO_PUERTA: u64 = 0x37;

/// Presupuesto de la mitad Rust. Ver [`INFO_PRESUPUESTO_PUERTA`].
pub const INFO_PRESUPUESTO_DISPATCH: u64 = 0x38;

/// Presupuesto de resolver una capability. Ver [`INFO_PRESUPUESTO_PUERTA`].
pub const INFO_PRESUPUESTO_HANDLE: u64 = 0x39;

/// **1 si las tres filas de arriba se midieron en LA MAQUINA QUE ESTA
/// CORRIENDO**; 0 si no.
///
/// # Por que un presupuesto tiene dueno
///
/// Un techo son **ticks del TSC de una placa concreta**. El mismo kernel
/// arranca en cualquier x86-64, y alli esos numeros no son ni estrictos ni
/// laxos: son **de otra maquina**. Juzgar con ellos da una falsa regresion en un
/// CPU mas lento o un falso aprobado en uno mas rapido -- las dos son el mismo
/// fallo, opinar sin derecho.
///
/// La identidad es familia y modelo de CPUID **mas la frecuencia del TSC**, con
/// un 1% de tolerancia: dos CPU del mismo modelo con TSC distinto no pueden
/// compartir una tabla escrita en ticks.
///
/// ** Y CUANDO ESTO ES 0, LOS TRES CAMPOS DE ARRIBA CONTESTAN CERO -- o sea
/// `sin declarar`, que todo cliente ya sabe leer. La proteccion vive en el
/// valor, no en que alguien se acuerde de consultar este campo: quien no lo
/// conozca pierde el MOTIVO, jamas el freno. Al reves --contestar el techo bueno
/// y confiar en que el cliente compruebe-- bastaria con un olvido para producir
/// un veredicto falso.
/// ```text
///    bit 0        coincide TODO -- el unico que decide
///    bit 1        familia y modelo coinciden
///    bit 2        el TSC coincide (dentro del 1%)
///    bits  8..15  familia ESPERADA      16..23  modelo ESPERADO
///    bits 24..31  familia LEIDA         32..39  modelo LEIDO del silicio
/// ```
///
/// ** Lleva los DOS LADOS a proposito. Un `bool` frena el trinquete y no lo
/// arregla: el dia que diga que no, hay que saber si fallo el modelo o el reloj
/// y con que numeros. Un "no" sin motivo manda a leer codigo; este manda a
/// cambiar una cifra.
pub const INFO_PRESUPUESTO_MAQUINA: u64 = 0x3D;

/// Los bits de [`INFO_PRESUPUESTO_MAQUINA`].
pub const MAQ_COINCIDE: u64 = 1 << 0;
pub const MAQ_CPU_OK: u64 = 1 << 1;
pub const MAQ_TSC_OK: u64 = 1 << 2;

/// **EL SUELO DEL HARDWARE**: `medido << 32 | ticks`.
///
/// Lo que cuesta cruzar el anillo en este silicio -- `syscall` + `sysretq` y
/// nada mas. **No es merito ni culpa de BMO**, y hoy va sumado dentro de los 792
/// ticks de una puerta sin que nada los separe.
///
/// # Para que sirve separarlo
///
/// Porque `suelo + sobrecoste` no dice si el kernel esta bien: mezcla el
/// silicio con el codigo. Restado, sale **la unica cifra de rendimiento que
/// sobrevive a un cambio de CPU**:
///
/// ```text
///    cuantas veces el suelo del hardware cuesta una puerta de BMO
///    hoy 5,3x  ->  la meta declarada seria 2,0x
/// ```
///
/// Si BMO adelgaza, ese numero baja **en todas las maquinas a la vez**.
///
/// # ** La regla que impide que esto sea una trampa
///
/// > **El suelo se MIDE. El multiplicador se ESCRIBE.**
///
/// Un presupuesto que se recalibrara solo entero se ceniria a lo que hubiera,
/// **incluida una regresion**: se convertiria en la talla nueva y el juez
/// aprobaria siempre. Se ajusta lo que es del CPU; jamas el veredicto.
///
/// [!] **Bit 32 = medido.** En `0` el numero es una estimacion del analisis y
/// **no puede derivar ningun techo**: solo vale para mirar el ratio, y quien lo
/// imprima tiene que decir que lo es.
pub const INFO_SUELO_CRUCE: u64 = 0x3E;

/// -- ** EL CENSO DE EXTENSIONES, legible desde Ring 3 ---------------------
///
/// Cuantas extensiones cubre el censo, y dos mascaras de bits sobre ESA lista
/// en ESE orden: bit `i` = la fila `i`. El nombre de cada fila se pide por
/// texto con [`INFO_TXT_EXT_NOMBRE`].
///
/// # Por que existen
///
/// El censo se escribio como orden `ext` del shell de Ring 0, y a ese shell no
/// se vuelve una vez arranca el escritorio -- el rescate `Ctrl+Alt+Esc` se
/// niega a echar al compositor a proposito. O sea que era una tabla correcta
/// que su dueno no podia mirar. Estas filas son la respuesta, y son filas de
/// tabla y no un syscall nuevo, que es para lo que `OP_INFO` existe.
///
/// # Por que mascaras y no una linea de texto ya pintada
///
/// Porque el kernel contesta HECHOS y quien pinta decide como. Un renglon
/// pre-formateado ataria a todo cliente al ancho, al orden y al color del
/// kernel. Con las mascaras, el escritorio pinta el conflicto en rojo y el
/// shell en su columna, **sin que ninguno de los dos lleve una segunda lista
/// de nombres** que un dia diga otra cosa.
pub const INFO_CPU_EXT_N: u64 = 0x31;

/// Bit `i` = el silicio DECLARA la extension `i`.
pub const INFO_CPU_EXT_HAY: u64 = 0x32;

/// Bit `i` = BMO la USA. `USA & !HAY` es un conflicto: una instruccion que
/// dara `#UD` en esta maquina.
pub const INFO_CPU_EXT_USA: u64 = 0x33;

/// Los cuatro contadores que tienen que ser cero, de 16 en 16 bits:
/// conflictos, mudas, repetidas, sin_sitio.
///
/// [!] Solo el primero se puede deducir de las mascaras. Los otros tres son
/// sobre la TABLA y no sobre el silicio -- una fila sin motivo escrito, una
/// repetida, una que no cupo -- y sin ellos un panel diria que todo va bien
/// mirando la mitad.
pub const INFO_CPU_EXT_AVERIAS: u64 = 0x34;

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

/// -- ** LA SALUD DEL BUS USB, COMO ESTADO ---------------------------------
///
/// La sexta exigencia de `docs/componente/EL_TECLADO_EXIGE.md`, y la regla que la ordena:
///
/// > **Una averia viva es un ESTADO, no un evento.** Un aviso se dice una vez e
/// > informa a quien ya estaba mirando; una averia que sigue ocurriendo
/// > necesita una luz encendida mientras dure, y donde vive el dueno.
///
/// Las cinco exigencias anteriores del teclado estan cumplidas y **cada una
/// tiene su contador** -- pero todos vivian en funciones de kernel que solo se
/// leen desde el shell de Ring 0, al que no se vuelve. Estas dos filas son lo
/// que convierte ese cuadro de mandos en algo que el escritorio puede pintar.
///
/// # `INFO_USB_SALUD`: los bits, mas la EDAD DEL LATIDO
///
/// ```text
///    bit 0   hay controlador xHCI            sin esto, lo demas es cero y no
///                                            significa "roto"
///    bit 1   teclado adoptado
///    bit 2   teclado con transferencia ENCOLADA   <- "enumero" != "escucha"
///    bit 3   su endpoint en Running SEGUN EL HARDWARE
///    bit 4   raton adoptado
///    bit 5   raton bombeando
///    bit 6   raton en Running
///    bit 7   USBSTS dice HSE o HCE: el controlador esta muerto
///   16..31   milisegundos desde el ultimo latido del hilo del bus
/// ```
///
/// ** La edad viaja PEGADA a los bits y no en otro campo, porque es lo que
/// permite no fiarse de ellos. Los bits son una foto que saca el bombeo; si el
/// hilo del bus muere, la foto se congela y seguiria contestando *"todo bien"*
/// para siempre. La edad **envejece sola**, asi que delata al que la escribe.
/// `0xFFFF` = hace mucho, o no hay reloj con el que saberlo: las dos piden la
/// misma reaccion, que es dejar de creerse el resto de la palabra.
///
/// El raton va al lado del teclado a proposito: **la asimetria entre los dos es
/// medio diagnostico**. Lo que le pasa a uno y no al otro no puede ser del hilo
/// del bus, ni del CR3 del MMIO, ni de la enumeracion -- solo puede ser algo
/// por endpoint.
pub const INFO_USB_SALUD: u64 = 0x3B;

pub const USB_SALUD_XHCI: u64 = 1 << 0;
pub const USB_SALUD_KBD: u64 = 1 << 1;
pub const USB_SALUD_KBD_BOMBA: u64 = 1 << 2;
pub const USB_SALUD_KBD_CORRE: u64 = 1 << 3;
pub const USB_SALUD_RATON: u64 = 1 << 4;
pub const USB_SALUD_RATON_BOMBA: u64 = 1 << 5;
pub const USB_SALUD_RATON_CORRE: u64 = 1 << 6;
pub const USB_SALUD_XHC_AVERIADO: u64 = 1 << 7;
/// Donde empieza la edad del latido, en milisegundos, y su mascara.
pub const USB_SALUD_EDAD_SHIFT: u64 = 16;
pub const USB_SALUD_EDAD_MASK: u64 = 0xFFFF;
/// *"Hace mucho, o no se puede saber"*. Ver arriba por que comparten valor.
pub const USB_SALUD_EDAD_VIEJA: u64 = 0xFFFF;

/// **Los cuatro contadores que tienen que ser CERO**, de 16 en 16 bits y
/// saturados -- el mismo empaquetado que [`INFO_CPU_EXT_AVERIAS`], y por el
/// mismo motivo: en campos separados se puede leer uno y no el otro, que es
/// como se dice *"todo bien"* habiendo mirado la mitad.
///
/// ```text
///    0..15   eventos PERDIDOS del aparcadero  E2  el endpoint se queda mudo
///   16..31   recuperaciones FALLIDAS          E3  se resucito y no salio
///   32..47   recuperaciones                   E3  hay errores de bus
///   48..63   barridos que REPARARON algo      E5  se pierden avisos de puerto
/// ```
///
/// Los dos primeros son averia; los dos ultimos son **desgaste**: el sistema se
/// repara solo y funciona, pero cada uno es medio segundo en que el teclado no
/// respondia. Quien pinta decide si eso es rojo o ambar; lo que no puede es
/// decir que no lo sabia.
///
/// Saturan a `0xFFFF` en vez de dar la vuelta: un contador que vuelve a cero
/// **apaga la luz**, que es justo el fallo que esta fila existe para no repetir.
pub const INFO_USB_AVERIAS: u64 = 0x3C;

// -- ** LO QUE EL DISCO CONTESTA (2026-08-17) -------------------------------
//
// Hasta hoy BMO-X le preguntaba al disco tres cosas --modelo, serie y
// capacidad-- y **no sabia si su disco giraba**. Mientras tanto el arbol si
// opinaba: el diseno de ESTRATOS razona sobre TRIM y la ley dice que un disco
// *"da caudal cuando tiene cola"*. Ninguna de las dos frases es falsa; ninguna
// estaba comprobada. Es L5 al reves -- hardcodea contratos, pregunta hechos.
//
// ** Los tres primeros campos son HECHOS y el cuarto es el VEREDICTO, y estan
// separados a proposito (L7): quien pinte puede ensenar lo que dijo el aparato
// aunque no este de acuerdo con lo que se concluyo de ello. Un veredicto sin su
// evidencia al lado no se puede discutir.
//
// Los numeros y su origen: `docs/componente/EL_DISCO_EXIGE.md`.

/// # `INFO_DISCO_MEDIO`: gira o no gira (palabra 217)
///
/// ```text
///    0..15   la palabra 217 CRUDA, tal como la dio el disco
///   16..17   0 no contesta - 1 NO ROTA - 2 ROTA - 3 valor reservado
///   32..47   revoluciones por minuto, 0 si no rota o no contesta
/// ```
///
/// ** **`no contesta` es un estado propio y no se colapsa a "es un HDD".** Los
/// SSD tempranos devolvian `0000h`, y por eso Windows 7 no se fio de esta
/// palabra sola: cruzaba su valor con una prueba real de lectura aleatoria. Es
/// R-FW2 --*lo que el firmware declara se comprueba contra lo que el aparato
/// hace*-- once anos antes de que esta casa la escribiera.
///
/// La palabra cruda viaja al lado del veredicto porque un rango reservado hay
/// que poder verlo, no deducirlo.
pub const INFO_DISCO_MEDIO: u64 = 0x3F;

pub const DISCO_MEDIO_CRUDO_MASK: u64 = 0xFFFF;
pub const DISCO_MEDIO_CLASE_SHIFT: u64 = 16;
pub const DISCO_MEDIO_CLASE_MASK: u64 = 0x3;
pub const DISCO_MEDIO_NO_CONTESTA: u64 = 0;
pub const DISCO_MEDIO_NO_ROTA: u64 = 1;
pub const DISCO_MEDIO_ROTA: u64 = 2;
pub const DISCO_MEDIO_RESERVADO: u64 = 3;
pub const DISCO_MEDIO_RPM_SHIFT: u64 = 32;
pub const DISCO_MEDIO_RPM_MASK: u64 = 0xFFFF;

/// # `INFO_DISCO_ENLACE`: el cable y la cola (palabras 75, 76 y 77)
///
/// ```text
///    0..2    generaciones SOPORTADAS: bit0 Gen1, bit1 Gen2, bit2 Gen3
///    4..6    generacion NEGOCIADA (1..3). 0 = el disco no lo dice
///    8       NCQ soportado
///   16..23   profundidad de cola, ** con el sesgo de -1 ya deshecho **
///   24..31   ranuras que BMO usa de verdad hoy
///   32..39   ranuras OCIOSAS: la resta de las dos de arriba
/// ```
///
/// ** **Soportado y negociado son dos campos porque son dos preguntas.** Un
/// disco Gen3 en un puerto Gen2 declara 3 y corre a 2; quedarse con la 76 da un
/// techo que no existe.
///
/// ** Y las ranuras usadas viajan aqui, junto a las que el disco admite, para
/// que **la resta se vea sin leer codigo**: hoy son 1 de 32.
pub const INFO_DISCO_ENLACE: u64 = 0x40;

pub const DISCO_ENLACE_GEN1: u64 = 1 << 0;
pub const DISCO_ENLACE_GEN2: u64 = 1 << 1;
pub const DISCO_ENLACE_GEN3: u64 = 1 << 2;
pub const DISCO_ENLACE_NEGOCIADA_SHIFT: u64 = 4;
pub const DISCO_ENLACE_NEGOCIADA_MASK: u64 = 0x7;
pub const DISCO_ENLACE_NCQ: u64 = 1 << 8;
pub const DISCO_ENLACE_COLA_SHIFT: u64 = 16;
pub const DISCO_ENLACE_COLA_MASK: u64 = 0xFF;
pub const DISCO_ENLACE_USADAS_SHIFT: u64 = 24;
pub const DISCO_ENLACE_USADAS_MASK: u64 = 0xFF;
pub const DISCO_ENLACE_OCIOSAS_SHIFT: u64 = 32;
pub const DISCO_ENLACE_OCIOSAS_MASK: u64 = 0xFF;

/// # `INFO_DISCO_GEOMETRIA`: el sector fisico y donde cae el LBA 0
///
/// ```text
///    0..3    ** EXPONENTE **: hay 2^n sectores logicos en uno fisico
///    4       la palabra 106 paso su guarda (bit15=0 y bit14=1)
///    8..21   desplazamiento del LBA 0 dentro del primer sector fisico
///   22       la palabra 209 paso su guarda
///   23       TRIM soportado (palabra 169 bit 0)
/// ```
///
/// ** **Los bits 0..3 son un exponente, no una cuenta**: un `3` son OCHO
/// sectores logicos por fisico. Es la misma familia de campo que el `bInterval`
/// del teclado, que se leyo como numero siendo exponente y dejo un teclado
/// sondeado cada 35 minutos (R-DISCO2).
///
/// ** El desplazamiento existe por la herencia de MS-DOS: la primera particion
/// empezaba en el **LBA 63**, que no es multiplo de 8, asi que sobre un disco de
/// 4096 B fisicos cada escritura de 4 KB caia a caballo de dos sectores. Le
/// importa a ESTRATOS porque su log crece en bloques de 4096: **desalineado,
/// cada avance paga dos sectores fisicos en vez de uno, en silencio.**
pub const INFO_DISCO_GEOMETRIA: u64 = 0x41;

pub const DISCO_GEO_EXP_MASK: u64 = 0xF;
pub const DISCO_GEO_106_VALIDA: u64 = 1 << 4;
pub const DISCO_GEO_DESPL_SHIFT: u64 = 8;
pub const DISCO_GEO_DESPL_MASK: u64 = 0x3FFF;
pub const DISCO_GEO_209_VALIDA: u64 = 1 << 22;
pub const DISCO_GEO_TRIM: u64 = 1 << 23;

/// # `INFO_DISCO_JUICIO`: el veredicto -- y es el unico campo que OPINA
///
/// ```text
///    0       hay PERFIL para este disco
///    1       medio solido CONFIRMADO (no basta con que el perfil lo diga)
///    2       ** la barrera FLUSH CACHE es lo unico que hay **
///    3       el recolector puede avisar al disco (TRIM)
///    4       el rendimiento del perfil esta MEDIDO, no es de catalogo
///    5       solido SIN TRIM  -- R-DISCO10
///    6       desalineado
///    7       el enlace negocio por debajo de lo que el disco sabe hacer
///    8..15   ranuras ociosas
///   16..47   frontera de escritura en KiB. ** 0 = no se puede alinear **
/// ```
///
/// Lo emite `bmo-disco-juicio`, que vive en `platform/shared/` y no en el
/// kernel **porque alli se puede probar** (L7b). En este componente equivocarse
/// no da un fault en pantalla: se lleva el trabajo de alguien.
///
/// ** **El bit 2 vale 1 tambien cuando NO hay perfil**, y ese es el diseno: no
/// saber si el disco tiene condensadores **no autoriza a suponer que los
/// tiene**. Un juez de rendimiento que se calla deja una cifra sin publicar; uno
/// de almacenamiento que se calla tiene que dejar el sistema en el camino que no
/// pierde datos.
///
/// ** Y la frontera contesta **0 sin perfil** en vez de un valor por defecto: el
/// bloque de borrado no lo expone ningun SSD de consumo (R-DISCO8), asi que sin
/// perfil no se alinea a nada -- y quien escriba tiene que saberlo en vez de
/// alinear a un numero inventado.
pub const INFO_DISCO_JUICIO: u64 = 0x42;

pub const DISCO_JUICIO_HAY_PERFIL: u64 = 1 << 0;
pub const DISCO_JUICIO_SOLIDO: u64 = 1 << 1;
pub const DISCO_JUICIO_SOLO_BARRERA: u64 = 1 << 2;
pub const DISCO_JUICIO_TRIM: u64 = 1 << 3;
pub const DISCO_JUICIO_MEDIDO: u64 = 1 << 4;
pub const DISCO_JUICIO_SOLIDO_SIN_TRIM: u64 = 1 << 5;
pub const DISCO_JUICIO_DESALINEADO: u64 = 1 << 6;
pub const DISCO_JUICIO_ENLACE_BAJO: u64 = 1 << 7;
pub const DISCO_JUICIO_OCIOSAS_SHIFT: u64 = 8;
pub const DISCO_JUICIO_OCIOSAS_MASK: u64 = 0xFF;
pub const DISCO_JUICIO_FRONTERA_SHIFT: u64 = 16;
pub const DISCO_JUICIO_FRONTERA_MASK: u64 = 0xFFFF_FFFF;

/// # `INFO_DISCO_TRIM_SECTORES`: cuanto se le ha devuelto al disco
///
/// Sectores de 512 B recortados desde el arranque. Cero significa **que nadie lo
/// ha pedido**, no que no se pueda: recortar en BMO-X lo pide una persona (la
/// seccion 9 de ESTRATOS: *politica, no automatismo*), asi que este numero es la
/// prueba de que la orden se dio y de cuanto cubrio.
pub const INFO_DISCO_TRIM_SECTORES: u64 = 0x43;

/// # `INFO_DISCO_TRIM_ORDENES`: en cuantos `DATA SET MANAGEMENT` cupo
///
/// ** Va al lado del anterior y no sobra: los mismos sectores en una orden o en
/// trescientas dicen cosas distintas del techo que declara el disco (palabra
/// 105). Es la unica pista si un dia recortar se vuelve lento, y sin ella
/// "cuanto" no tiene con que compararse.
pub const INFO_DISCO_TRIM_ORDENES: u64 = 0x44;

/// Fabricante ("AMD"), nombre comercial, microarquitectura y familia/modelo.
pub const INFO_TXT_CPU_VENDOR: u64 = 0x01;

pub const INFO_TXT_CPU_NOMBRE: u64 = 0x02;

pub const INFO_TXT_UARCH: u64 = 0x03;

pub const INFO_TXT_FAMILIA: u64 = 0x04;

/// El nombre de la extension `i` del censo: se pide como
/// `INFO_TXT_EXT_NOMBRE | (i << 8)`.
///
/// ** El indice viaja en los bits altos del campo, que es el idioma que esta
/// superficie ya habla (`INFO_MEM_QUIEN_*`, `AUTOPSIA_TEXTO`). Con esto los
/// treinta y seis nombres viven **en un solo sitio del arbol** --el `match`
/// exhaustivo del kernel, que el compilador obliga a completar al anadir una
/// fila-- en vez de en una copia de Ring 3 que envejece en silencio.
pub const INFO_TXT_EXT_NOMBRE: u64 = 0x05;

/// El motivo escrito a mano de esa misma fila: por que se usa, o por que no.
/// Misma forma de indexar. Es la columna que convierte el censo en una
/// decision en vez de trivia -- y la que el kernel cuenta como `muda` si esta
/// vacia.
pub const INFO_TXT_EXT_NOTA: u64 = 0x06;

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
