//! **El runtime de Ring 3 de BMO.** La cara de userspace de los dos syscalls.
//!
//! Antes esta crate re-exportaba `BootContext` "para que userland tuviera la
//! misma struct que el kernel para el handoff". Eso era de otro sistema y era
//! el modelo equivocado entero: **un proceso Ring 3 no recibe la estructura de
//! arranque del kernel.** No sabe cuanta RAM hay, ni donde esta el
//! framebuffer, ni que discos existen. Recibe *capabilities*, y cada una es un
//! permiso concreto sobre un objeto concreto. Lo que no le hayan dado, no
//! existe para el. Ese es el trato.
//!
//! ## La superficie, entera
//!
//! **DOS syscalls** (2026-08-10; eran tres):
//!
//! ```text
//!   INVOKE(cap, operacion, a0, a1, a2)   haz esto AHORA
//!   WAIT(esperable, visto, timeout_ns)   despiertame CUANDO
//! ```
//!
//! Todo lo demas --abrir un endpoint, escribir en consola, reclamar la
//! pantalla-- es una *operacion* sobre una capability. La API crece por dentro,
//! en la pareja `(tipo de objeto, operation)`, y el ABI no se toca. Anadir
//! "abrir ventana" no es cambiar la frontera: es un numero mas en una tabla.
//!
//! ## Por que se fue el tercero, y por que no se van los dos a uno
//!
//! `CHANNEL_KICK(cap, secuencia)` resolvia un handle, comprobaba que era un
//! canal, y avisaba a su consumidor: **una operacion sobre un handle**, que es
//! la definicion de `INVOKE`. Tenia numero propio por como nacio, no por lo que
//! hace. Hoy es `CHANNEL_OP_KICK` y no se perdio nada.
//!
//! ** `WAIT` si es otra cosa, y por eso quedan dos. Lo unico que hace es **no
//! devolver el turno**, y eso una llamada sincrona no lo puede decir: `INVOKE`
//! tendria que contestar *"todavia no"* y dejar que el programa vuelva a
//! preguntar -- o sea, quemar su turno preguntando, que es exactamente lo que
//! `WAIT` existe para no hacer.
//!
//! El `1` queda **reservado**: un binario viejo que lo llame falla diciendolo.
//! Reciclarlo le haria hacer algo que nadie pidio, sin fallar en ningun sitio.
//!
//! ## Convencion de registros
//!
//! `rax` = numero de syscall; argumentos en `rdi, rsi, rdx, r10, r8`.
//!
//! * **`r10` y no `rcx`.** En `SYSCALL` el CPU mete el RIP de retorno en `rcx`
//! y las RFLAGS en `r11`: un argumento en `rcx` no seria el dato de nadie,
//! seria la direccion a la que volver. Linux salta a `r10` por lo mismo. Este
//! detalle ya se cobro una tarde en el lado del kernel.
//!
//! De vuelta: `rax` = `codigo | (flags << 32)`, `rdx` = valor.

#![no_std]

use core::arch::asm;

// -- La superficie congelada ---------------------------------------------

pub const NR_INVOKE: u32 = 0;
/// ** RETIRADO el 2026-08-10. Reservado, y no se reutiliza: un binario viejo
/// que llame al `1` tiene que fallar diciendolo. Ahora es `CHANNEL_OP_KICK`
/// sobre el canal -- ver `bmo_abi::...::NR_CHANNEL_KICK`.
pub const NR_CHANNEL_KICK: u32 = 1;
pub const NR_WAIT: u32 = 2;
/// **Avisar al consumidor de un canal.** Pide WRITE, al reves que las dos
/// preguntas de abajo: avisar es escribir.
pub const CHANNEL_OP_KICK: u32 = 0x03;

/// Pseudo-capability que se refiere al proceso que llama. No es un handle
/// concedido: es la forma de pedir lo que uno ya tiene por ser quien es.
pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;

// Operaciones sobre `CURRENT_TASK`.
pub const OP_GET_PID: u32 = 0x01;
pub const OP_GET_TID: u32 = 0x02;
pub const OP_YIELD: u32 = 0x03;
pub const OP_EXIT: u32 = 0x04;
pub const OP_CHANNEL_OPEN: u32 = 0x05;
pub const OP_CONSOLE_WRITE: u32 = 0x06;
pub const OP_ENDPOINT_CREATE: u32 = 0x07;
pub const OP_ENDPOINT_CONNECT: u32 = 0x08;
pub const OP_FRAMEBUFFER_CLAIM: u32 = 0x09;
/// Soltar la pantalla siendo su dueno y **seguir vivo**. Pareja de
/// [`OP_FRAMEBUFFER_CLAIM`]: hasta el 2026-08-07 la unica forma de dejar de ser
/// dueno era terminar, asi que el escritorio no podia prestarla ni queriendo.
pub const OP_PANTALLA_SOLTAR: u32 = 0x1D;
/// Soltar la ENTRADA siendo su dueno y seguir vivo.
///
/// Va junto a [`OP_PANTALLA_SOLTAR`] porque **separarlas fue el bug**: prestar la
/// pantalla sin la entrada deja al programa pintando sin poder leer su propia
/// tecla de salida, y a la maquina sin teclado hasta reiniciar.
pub const OP_ENTRADA_SOLTAR: u32 = 0x1E;
pub const OP_INPUT_CLAIM: u32 = 0x0A;
pub const OP_RUTA: u32 = 0x0B;
pub const OP_EJECUTAR: u32 = 0x0C;
pub const OP_CONSOLA_CREAR: u32 = 0x0D;
pub const OP_DIR_ABRIR: u32 = 0x0E;
pub const OP_CONSOLE_READ: u32 = 0x0F;
pub const OP_ARCHIVO_ABRIR: u32 = 0x10;
/// Abrir MI PROPIA imagen, para leer los datos que lleva dentro. Sin
/// argumentos: el programa no dice CUAL, dice "el mio", y quien sabe cual es el
/// kernel. Pedir el propio fichero por su ruta seria pedir por nombre lo que se
/// tiene por derecho.
pub const OP_MI_PAQUETE: u32 = 0x25;
pub const OP_ARCHIVO_CREAR: u32 = 0x11;
/// Reiniciar la maquina. No vuelve. Ver [`reiniciar`].
pub const OP_REINICIAR: u32 = 0x12;
/// Un dato del sistema. Ver [`info`] y [`info_texto`].
pub const OP_INFO: u32 = 0x13;
pub const OP_INFO_TEXTO: u32 = 0x14;
/// Pedir un bloque de memoria. Ver [`Memoria`].
pub const OP_MEMORIA_PEDIR: u32 = 0x15;
/// El log del kernel, leido desde Ring 3. Ver `klog_lineas`/`klog_texto`.
pub const OP_KLOG_INFO: u32 = 0x16;
pub const OP_KLOG_TEXTO: u32 = 0x17;
/// **Escribe en el disco.** Ver [`estratos_sellar`].
pub const OP_ESTRATOS_SELLAR: u32 = 0x18;
/// El cursor de ESTRATOS: `arg0` la pregunta, `arg1` su argumento.
pub const OP_ES_NODO: u32 = 0x19;
/// Ocho bytes del nombre del hijo `arg0`; `arg1` numera el trozo.
pub const OP_ES_TEXTO: u32 = 0x1A;
/// **Despierta los otros nucleos.** Ver [`crate::sys::smp_despertar`].
pub const OP_SMP_DESPERTAR: u32 = 0x1B;
/// ** **ADMINISTRAR EL DISCO.** `arg0` es la orden (`DISCO_OP_*`).
///
/// La segunda operacion del userland que cambia el estado del almacen --la
/// primera fue sellar-- y la unica que se lo dice al APARATO. Ninguna de sus
/// ordenes lleva un LBA: ver el modulo [`crate::disco`].
pub const OP_DISCO: u32 = 0x29;
/// **Armar y sondear la red.** Ver `red.rs`: existe porque `net rx` vivia solo
/// en Ring 0, y al shell de Ring 0 no se vuelve.
pub const OP_RED: u32 = 0x2C;
/// **Que cuenta la placa de si misma.** Contesta y no concede.
pub const OP_PLACA: u32 = 0x2D;
/// ** **CREAR UN FICHERO EN ESTRATOS.** `arg0` es la suborden (`ES_CREAR_*`).
///
/// El nombre viaja por el renglon de [`OP_RUTA`] y el contenido por el suyo, de
/// 8 en 8 y **con cuenta explicita**: en un fichero un cero es un dato, y
/// cortarlo ahi seria entregar la mitad de un fichero.
pub const OP_ES_GESTO: u32 = 0x2A;
/// **El censo de audio**: que el aparato diga como quiere las muestras.
/// Devuelve 1 si encontro uno; los ocho numeros van a CABINA.
pub const OP_AUDIO_CENSO: u32 = 0x28;
// -- S1 del suelo de Ring 3: la ventana de un aparato ----------------------
//
// *** El argumento es QUE APARATO, y no una direccion. Ver
// `docs/plan/PLAN_SUELO_RING3.md`: un proceso que pudiera nombrar una fisica
// estaria pidiendo ser el kernel.

/// **Toma la ventana de registros de un aparato.** `arg0` = cual (0 = xHCI).
/// Devuelve un handle `KIND_MMIO` de **solo lectura**.
pub const OP_APARATO_TOMAR: u32 = 0x2E;
/// Devuelve la ventana sin morirse.
pub const OP_APARATO_SOLTAR: u32 = 0x2F;
/// Donde quedo mapeada, en MI espacio.
pub const APARATO_OP_BASE: u32 = 0x01;
/// Cuantos bytes son. Se pregunta en vez de suponerse.
pub const APARATO_OP_BYTES: u32 = 0x02;
/// El controlador xHCI. La lista es cerrada: lo que no esta aqui no se nombra.
pub const APARATO_XHCI: u64 = 0;

/// **Toma lo que otro proceso me ofrecio.** Ver [`crate::sys::tomar_prestado`].
pub const OP_TOMAR: u32 = 0x1C;
/// Operacion sobre un bloque PROPIO: ofrecer un trozo a otra tarea.
pub const MEM_OP_OFRECER: u32 = 0x03;
/// **Quien me lanzo**, como TID. `0` si nadie -- ver [`crate::sys::mi_padre`].
pub const OP_MI_PADRE: u32 = 0x26;

/// **El handle sobre un hijo que YO lance**, por su tid. Solo BUSCA lo que
/// `EJECUTAR` ya concedio: quien no lo lanzo no encuentra nada.
pub const OP_HIJO: u32 = 0x2B;

/// Operaciones sobre un handle de hijo. Ver `obj/tarea.rs` en el kernel.
pub const TAREA_OP_VIVE: u32 = 0x01;
pub const TAREA_OP_TID: u32 = 0x02;
pub const TAREA_OP_CERRAR: u32 = 0x03;
pub const TAREA_OP_DELANTE: u32 = 0x04;
/// **Abrir un archivo SIN esperar a que llegue entero.** Mismo handle que
/// `OP_ARCHIVO_ABRIR`; lo que cambia es cuando vuelve. Ver
/// [`crate::Archivo::leer_de_asinc`].
pub const OP_ARCHIVO_ASINC: u32 = 0x27;
/// `(entero << 63) | bytes que ya llegaron`. **Y avanza la carga**: preguntar
/// por el archivo es lo que lo trae.
pub const ARCH_OP_LISTO: u32 = 0x09;
/// Operaciones sobre un handle de memoria PRESTADA (`KIND_PRESTADO`).
pub const PRESTADO_OP_BASE: u32 = 0x01;
pub const PRESTADO_OP_BYTES: u32 = 0x02;
/// El TID de quien lo presto, o `0` si ya no vive. El detector de vida de una
/// ventana: ver [`crate::sys::prestado_dueno`].
pub const PRESTADO_OP_DUENO: u32 = 0x03;
/// Devolverlo. Ver [`crate::sys::soltar_prestado`].
pub const PRESTADO_OP_SOLTAR: u32 = 0x04;

/// Donde empieza el bloque, y cuanto se ha entregado a este proceso.
pub const MEM_OP_BASE: u32 = 0x01;
pub const MEM_OP_BYTES: u32 = 0x02;

// Campos de `OP_INFO`. Son una TABLA: anadir un dato es una fila, no una
// operacion nueva.
pub const INFO_RAM_TOTAL: u64 = 0x01;
pub const INFO_RAM_LIBRE: u64 = 0x02;
pub const INFO_RAM_MARCOS: u64 = 0x03;
pub const INFO_RAM_MARCOS_LIBRES: u64 = 0x04;
pub const INFO_TSC_HZ: u64 = 0x05;
/// La frecuencia efectiva del nucleo AHORA, en Hz. `0` = no se puede medir.
/// Es una MEDIDA: dos lecturas seguidas dan la velocidad de ese intervalo.
pub const INFO_CPU_HZ_REAL: u64 = 0x20;
/// Milivatios del paquete desde la ultima consulta. `0` = no se puede medir.
pub const INFO_CPU_MW_PAQUETE: u64 = 0x21;
/// **Milivatios del NUCLEO EN EL QUE SE LEE.** No de todos, y el metal del
/// 12-08 enseno por que importa: con once nucleos girando al 100%, este numero
/// BAJO. `CORE_ENERGY_STAT` es por nucleo y solo se lee el del BSP.
pub const INFO_CPU_MW_NUCLEO_ACTUAL: u64 = 0x22;
/// **Que sabe medir el perfil de este silicio**, como banderas.
/// bit 0 = frecuencia efectiva / bit 1 = consumo.
///
/// Es lo que permite a la terminal decir QUE esta aplicando, en vez de pintar
/// ceros y dejar al que mira sin saber si el sensor no existe o el valor es 0.
pub const INFO_CPU_SENSORES: u64 = 0x23;

// -- ** QUIEN ESTA COMIENDO MEMORIA -------------------------------------
//
// El indice de ranura va EMPAQUETADO con el campo: `campo | (ranura << 8)`.
// Por la puerta de `info` cabe un numero, y la alternativa --un buffer con un
// array de structs-- seria inventar un formato con su version y su alineacion
// para contestar tres enteros.
//
// [!] El indice cuenta solo las ranuras OCUPADAS: se pide 0, 1, 2... hasta que
// el pid conteste 0. Los agujeros de la tabla del kernel son suyos.
/// El pid de la ranura `n`. **`0` = no hay mas.**
pub const INFO_MEM_QUIEN_PID: u64 = 0x24;
/// Bytes que ese proceso tiene pedidos ahora mismo.
pub const INFO_MEM_QUIEN_BYTES: u64 = 0x25;
/// Cuantas peticiones lleva. Distingue "pidio un bloque grande" de "esta
/// pidiendo sin parar", que es la diferencia entre un juego y una fuga.
pub const INFO_MEM_QUIEN_PETICIONES: u64 = 0x26;
// == LA RED ======================================================
//
// Siete campos que hasta hoy no existian: el kernel conocia la NIC y Ring 3 no
// tenia forma de preguntar. Un panel de red no era cuestion de dibujar.
pub const INFO_NET_PRESENTE: u64 = 0x27;
pub const INFO_NET_VENDOR_DEVICE: u64 = 0x28;
/// Los seis bytes en los 48 bits bajos, byte 0 el mas significativo.
pub const INFO_NET_MAC: u64 = 0x29;
/// El `PHYstatus` crudo, sin interpretar. El byte es la prueba.
pub const INFO_NET_PHY_CRUDO: u64 = 0x2A;
/// 10, 100, 1000 -- o `0`, que es *"no hay cable"* y es una respuesta.
pub const INFO_NET_MEGABITS: u64 = 0x2B;
/// Distingue *"no llega nada"* de *"no estamos escuchando"*.
pub const INFO_NET_RX_ARMADO: u64 = 0x2C;
pub const INFO_NET_RX_TRAMAS: u64 = 0x2D;
pub const INFO_NET_RX_BYTES: u64 = 0x4A;
pub const INFO_NET_RX_PERDIDAS: u64 = 0x4B;
pub const INFO_NET_RX_TIPOS: u64 = 0x4C;
pub const INFO_NET_PCI: u64 = 0x2E;

/// El metro de la puerta: puertas servidas y ciclos dentro de `dispatch`.
/// **Se leen como DELTA** -- antes y despues del bucle que se quiera medir.
pub const INFO_SYSCALL_CUENTA: u64 = 0x2F;
pub const INFO_SYSCALL_CICLOS: u64 = 0x30;
/// Y el reparto DENTRO del stub: guardar el contexto (`xsaveopt64`) y
/// devolverlo (`xrstor64`). Lo que no cae en ninguna de las tres casillas son
/// las dos transiciones de privilegio -- el `syscall` y el `iretq`.
pub const INFO_SYSCALL_CICLOS_GUARDA: u64 = 0x35;
pub const INFO_SYSCALL_CICLOS_RESTAURA: u64 = 0x36;
/// El presupuesto de ciclos de la puerta: `meta << 32 | techo`. El `techo` es
/// lo que no puede empeorar y la `meta` a donde tiene que llegar -- cumplir el
/// primero y no la segunda es estar **en plazo**, no estar bien.
pub const INFO_PRESUPUESTO_PUERTA: u64 = 0x37;
pub const INFO_PRESUPUESTO_DISPATCH: u64 = 0x38;
pub const INFO_PRESUPUESTO_HANDLE: u64 = 0x39;

/// **1 si ese presupuesto se midio en la maquina que esta corriendo.**
///
/// Un techo son ticks del TSC de una placa concreta; en otro CPU no son
/// estrictos ni laxos, son **de otra maquina**. Cuando esto vale 0, los tres
/// campos de arriba contestan **cero** --`sin declarar`-- para que nadie juzgue
/// con numeros ajenos: el freno esta en el valor, y este campo solo da el
/// MOTIVO para poder decirlo con palabras.
pub const INFO_PRESUPUESTO_MAQUINA: u64 = 0x3D;

/// Los bits de [`INFO_PRESUPUESTO_MAQUINA`]. Lleva **los dos lados** --lo
/// esperado y lo leido del silicio-- porque un "no coincide" sin numeros manda
/// a leer codigo, y con numeros se arregla cambiando una cifra:
///
/// ```text
///    bit 0        coincide TODO -- el unico que decide
///    bit 1        familia y modelo coinciden
///    bit 2        el TSC coincide (dentro del 1%)
///    bits  8..15  familia ESPERADA      16..23  modelo ESPERADO
///    bits 24..31  familia LEIDA         32..39  modelo LEIDO
/// ```
pub const MAQ_COINCIDE: u64 = 1 << 0;
pub const MAQ_CPU_OK: u64 = 1 << 1;
pub const MAQ_TSC_OK: u64 = 1 << 2;

/// **El suelo del hardware**: `medido << 32 | ticks`. Lo que cuesta cruzar el
/// anillo en este silicio, que no es merito ni culpa de BMO.
///
/// Restandolo de una puerta sale **la unica cifra que sobrevive a un cambio de
/// CPU**: cuantas veces el suelo cuesta una puerta de BMO (hoy 5,3x, meta 2,0x).
///
/// [!] Bit 32 = medido. En 0 es una ESTIMACION y no puede derivar ningun techo:
/// el suelo se mide, el multiplicador se escribe.
pub const INFO_SUELO_CRUCE: u64 = 0x3E;

/// **De que CLASE fue cada puerta**, con el indice empaquetado:
/// `INFO_SYSCALL_CLASS | (clase << 8)`. Se lee como delta, igual que el resto
/// del metro.
///
/// El coste de cada clase ya estaba medido; lo que faltaba es **cuantas veces
/// se pide cada una**, sin lo cual no hay porcentaje y no se puede ordenar el
/// trabajo. Las cuatro suman MENOS que [`INFO_SYSCALL_CUENTA`] y la diferencia
/// son las puertas que no son ninguna de las cuatro.
pub const INFO_SYSCALL_CLASS: u64 = 0x3A;
/// Pseudo-capability: `INVOKE(CURRENT_TASK, ...)`. No resuelve handle. ~875.
pub const SYSCALL_CLASS_TASK: u64 = 0x00;
/// Resolvio una capability real -- paga el handle. ~1125.
pub const SYSCALL_CLASS_HANDLE: u64 = 0x01;
/// Escritura de consola: dibuja glifos y hace scroll. ~2,2 M.
pub const SYSCALL_CLASS_CONSOLE: u64 = 0x02;
/// `WAIT`: la unica puerta que puede no devolver el turno.
pub const SYSCALL_CLASS_WAIT: u64 = 0x03;
/// Cuantas casillas tiene el histograma.
pub const SYSCALL_CLASS_COUNT: u64 = 0x04;

/// El censo de extensiones del CPU, en tres numeros: cuantas filas, que
/// declara el silicio y que coge BMO. Bit `i` = fila `i`, y el nombre de esa
/// fila se pide con [`INFO_TXT_EXT_NOMBRE`].
pub const INFO_CPU_EXT_N: u64 = 0x31;
pub const INFO_CPU_EXT_HAY: u64 = 0x32;
pub const INFO_CPU_EXT_USA: u64 = 0x33;
/// conflictos | mudas<<16 | repetidas<<32 | sin_sitio<<48. Todos cero o hay
/// algo que arreglar.
pub const INFO_CPU_EXT_AVERIAS: u64 = 0x34;

pub const INFO_CPU_HILOS: u64 = 0x06;
pub const INFO_CPU_NUCLEOS: u64 = 0x07;

/// **CUANTO FIARSE del par `INFO_CPU_NUCLEOS` / `INFO_CPU_HILOS`.** Un mapa de
/// bits, y `0` significa *"los tres testigos dicen lo mismo"*.
///
/// ```text
///    bit 0   las dos hojas de CPUID se contradicen
///    bit 1   los hilos por nucleo NO se pudieron medir
///    bit 2   *** el PERFIL desmiente al silicio: no es este chip
///    bit 3   la MADT declara otros hilos que CPUID
/// ```
///
/// # Por que un panel necesita esto (2026-08-25)
///
/// Ese dia el escritorio pinto `27 fisicos / 54 logicos` en un 6/12 y **no tenia
/// forma de saber que dudar**: los dos campos de arriba son `u64` pelados y un
/// numero malo se pinta igual de nitido que uno bueno.
///
/// *** El aviso existia --en el log del arranque, y solo si alguien tecleaba
/// `smp`-- y ahi no lo ve nadie: **el dueno vive en el escritorio y al shell de
/// Ring 0 no se vuelve.** Un diagnostico al que no se llega desde donde se ve el
/// sintoma no es un diagnostico.
pub const INFO_CPU_TOPOLOGIA_DUDA: u64 = 0x4D;

/// Hilos por nucleo, **medidos** (`CPUID.0B.0:EBX`). `0` = no se pudo medir.
///
/// Existe porque `nucleos` se calculaba dividiendo entre un 2 escrito a mano, y
/// con eso la comprobacion `hilos == nucleos * 2` no podia fallar nunca.
pub const INFO_CPU_HILOS_POR_NUCLEO: u64 = 0x4E;
pub const INFO_TAREAS_TOTAL: u64 = 0x08;
pub const INFO_TAREAS_LISTAS: u64 = 0x09;
/// * Quien tiene la pantalla: su `pid`, o **`0` si no la tiene nadie**.
///
/// Se PREGUNTA en vez de intentar reclamarla, y la diferencia importa: probar a
/// reclamarla para saber si esta libre **te la deja puesta**, y entonces se la
/// robas al programa al que se la ibas a prestar.
pub const INFO_PANTALLA_DUENO: u64 = 0x1A;
pub const INFO_TAREAS_LIBRES: u64 = 0x0A;
pub const INFO_TICKS: u64 = 0x0B;
pub const INFO_KERNEL_BYTES: u64 = 0x0C;
pub const INFO_PROGRAMAS: u64 = 0x0D;
pub const INFO_PROGRAMAS_OLVIDADOS: u64 = 0x0E;
pub const INFO_DISCO_LISTO: u64 = 0x0F;
pub const INFO_DATOS_MONTADO: u64 = 0x10;
/// -- ESTRATOS ------------------------------------------------------
///
/// El volumen de datos grande. Ring 3 los necesita para poder ENSENAR el estado
/// del almacen sin cruzar a Ring 0 por cada dato: son una fila mas de la tabla
/// de `OP_INFO`, que es como crece esta superficie sin tocar el ABI.
pub const INFO_ES_MONTADO: u64 = 0x11;
/// Generacion del superbloque: cuantas transacciones lleva el volumen.
/// Cuando se hizo la version en curso. `0` = sin fechar.
pub const INFO_ES_FECHA: u64 = 0x49;
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
/// Bytes que Ring 3 ha PEDIDO con `KIND_MEMORIA` desde el arranque. Es lo
/// unico del informe que solo se mueve si alguien ejercio la capability.
pub const INFO_MEM_ENTREGADA: u64 = 0x19;
/// Nucleos de aplicacion en pie (sin el BSP), choques de cerrojo y la espera
/// mas larga. Los dos ultimos tienen que dar CERO. Ver `plat/spin.rs`.
pub const INFO_SMP_VIVOS: u64 = 0x1B;
pub const INFO_SPIN_CHOQUES: u64 = 0x1C;
pub const INFO_SPIN_PICO: u64 = 0x1D;
/// Recursos que un muerto dejo sin devolver. Tiene que ser CERO: acusa al
/// kernel, no al programa. Ver `core/autopsia.rs`.
pub const INFO_FUGAS: u64 = 0x1E;
/// La fecha y hora de la placa, empaquetada. `0` = no hay reloj.
/// Ver `INFO_FECHA` en `bmo_abi::syscalls::surface`.
pub const INFO_FECHA: u64 = 0x1F;

/// ** LA SALUD DEL BUS USB: bits de estado + la EDAD DEL LATIDO en 16..31.
///
/// La sexta exigencia de `docs/componente/EL_TECLADO_EXIGE.md`. Con esto un programa de
/// Ring 3 --el escritorio-- puede encender una luz mientras el teclado este
/// caido, en vez de que la averia se cuente una vez en un panel que hay que
/// abrir **con el aparato que esta roto**.
///
/// La edad viaja pegada a los bits porque es lo que permite no fiarse de ellos:
/// los bits son una foto del ultimo bombeo y se congelarian si el hilo del bus
/// muriera. `USB_SALUD_EDAD_VIEJA` = hace mucho, o no hay reloj.
pub const INFO_USB_SALUD: u64 = 0x3B;

// -- ** LO QUE EL DISCO CONTESTA (2026-08-17) -------------------------------
//
// Cuatro filas: tres de HECHOS y una de VEREDICTO. Hasta hoy BMO-X preguntaba
// modelo, serie y capacidad, y **no sabia si su disco giraba** -- mientras el
// arbol razonaba sobre TRIM y sobre colas. El empaquetado se documenta en
// `bmo-abi`; el porque, en `docs/componente/EL_DISCO_EXIGE.md`.

/// Gira o no gira (palabra 217). `0..15` la palabra cruda, `16..17` la clase,
/// `32..47` las RPM. ** `no contesta` es una clase propia y NO significa HDD.
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

/// El cable y la cola. Soportado (76) y negociado (77) son campos distintos
/// porque son dos preguntas, y las ranuras USADAS viajan al lado de las que el
/// disco admite para que la resta se vea sin leer codigo: hoy 1 de 32.
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

/// El sector fisico y donde cae el LBA 0. ** Los bits `0..3` son un EXPONENTE:
/// un 3 son OCHO sectores logicos por fisico, no tres.
pub const INFO_DISCO_GEOMETRIA: u64 = 0x41;
pub const DISCO_GEO_EXP_MASK: u64 = 0xF;
pub const DISCO_GEO_106_VALIDA: u64 = 1 << 4;
pub const DISCO_GEO_DESPL_SHIFT: u64 = 8;
pub const DISCO_GEO_DESPL_MASK: u64 = 0x3FFF;
pub const DISCO_GEO_209_VALIDA: u64 = 1 << 22;
pub const DISCO_GEO_TRIM: u64 = 1 << 23;

/// El veredicto, y es el unico campo que opina. ** `SOLO_BARRERA` vale 1
/// tambien sin perfil: no saber si el disco tiene condensadores no autoriza a
/// suponer que los tiene. Y la frontera contesta 0 en vez de un valor por
/// defecto -- sin perfil no se alinea a un numero inventado.
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

/// ** LO QUE SE LE HA DEVUELTO AL DISCO, en sectores de 512 B y en ordenes.
///
/// Cero no significa "no se puede": significa **que nadie lo ha pedido**. En
/// BMO-X recortar lo pide una persona --la seccion 9 de ESTRATOS dice *politica,
/// no automatismo*-- asi que estas dos filas son la prueba de que la orden se
/// dio, de cuanto cubrio y de en cuantas ordenes cupo (la palabra 105).
pub const INFO_DISCO_TRIM_SECTORES: u64 = 0x43;
pub const INFO_DISCO_TRIM_ORDENES: u64 = 0x44;

/// ** EL RANGO QUE SE VA A RECORTAR, tal como lo va a usar la orden.
///
/// Estos dos numeros se podian deducir de `INFO_ES_*`, y deducirlos era tener
/// **dos cuentas de la misma verdad**: la que se pinta en la propuesta y la que
/// el kernel manda de verdad. Se separan el dia que una cambie, y separarse aqui
/// es ensenar un rango y recortar otro. `0` = no hay volumen o la cola esta
/// vacia.
pub const INFO_DISCO_COLA_LBA: u64 = 0x45;
pub const INFO_DISCO_COLA_SECTORES: u64 = 0x46;
/// Bloques de payload por orden (palabra 105). **Nunca contesta 0**: uno siempre
/// se admite, y el cero de esa palabra es el disco callandose.
pub const INFO_DISCO_TRIM_BLOQUES: u64 = 0x47;
/// **Por que fallo el ultimo recorte**: `(clase << 32) | PxTFD`, con las clases
/// en `DISCO_FALLO_*`. `0` = ninguno, y un recorte que sale bien lo borra.
pub const INFO_DISCO_TRIM_FALLO: u64 = 0x48;

pub const USB_SALUD_XHCI: u64 = 1 << 0;
pub const USB_SALUD_KBD: u64 = 1 << 1;
/// El teclado tiene transferencia ENCOLADA. Sin esto esta enumerado, en
/// `Running`, y mudo para siempre.
pub const USB_SALUD_KBD_BOMBA: u64 = 1 << 2;
/// Su endpoint corre **segun el hardware**, no segun lo que creemos.
pub const USB_SALUD_KBD_CORRE: u64 = 1 << 3;
pub const USB_SALUD_RATON: u64 = 1 << 4;
pub const USB_SALUD_RATON_BOMBA: u64 = 1 << 5;
pub const USB_SALUD_RATON_CORRE: u64 = 1 << 6;
/// HSE o HCE en `USBSTS`: el controlador esta muerto y lo demas es ruido.
pub const USB_SALUD_XHC_AVERIADO: u64 = 1 << 7;
pub const USB_SALUD_EDAD_SHIFT: u64 = 16;
pub const USB_SALUD_EDAD_MASK: u64 = 0xFFFF;
pub const USB_SALUD_EDAD_VIEJA: u64 = 0xFFFF;

/// Los cuatro contadores que tienen que ser CERO, de 16 en 16 bits: perdidos
/// del aparcadero (E2), recuperaciones fallidas y recuperaciones (E3), y
/// barridos que repararon algo (E5). Saturan; no dan la vuelta.
pub const INFO_USB_AVERIAS: u64 = 0x3C;

// ** LA AUTOPSIA de un fallo de Ring 3.
//
// El kernel guarda el informe COMPLETO de cada tarea que mata --vector, codigo
// en palabras, direccion, `rip`, `rsp`, que programa era y lo ultimo que
// escribio-- y aqui se lee. Contesta texto: no concede nada.
pub const OP_AUTOPSIA_INFO: u32 = 0x1F;
pub const OP_AUTOPSIA_TEXTO: u32 = 0x20;
pub const AUTOPSIA_TOTAL: u64 = 0x00;
pub const AUTOPSIA_DISPONIBLES: u64 = 0x01;
pub const AUTOPSIA_RENGLONES: u64 = 0x02;

// ** EL SONIDO como capability. Ver `ring0/obj/audio.rs`.
//
// Es el CONTRATO, no un driver: quien puede sonar, quien no, y que pasa con el
// aparato cuando su dueno se muere. Lo unico que suena hoy es el altavoz del
// PC, y `AUDIO_OP_DEVICES` lo dice en vez de que haya que suponerlo.
// ** CABINA: lo que el kernel ve, CON severidad.
//
// El klog ya se leia, pero es texto plano sin severidad ni capa. Esto es el
// anillo de CABINA: cada evento con quien lo dijo y como de grave es.
pub const OP_CABINA_INFO: u32 = 0x23;
pub const OP_CABINA_TEXTO: u32 = 0x24;
pub const CABINA_TOTAL: u64 = 0x00;
pub const CABINA_PERDIDOS: u64 = 0x01;
pub const CABINA_DISPONIBLES: u64 = 0x02;
pub const CABINA_SEVERIDAD: u64 = 0x03;
pub const CABINA_CAPA: u64 = 0x04;
pub const CABINA_VALOR: u64 = 0x05;
pub const CABINA_SEQ: u64 = 0x06;
pub const CABINA_TICK: u64 = 0x07;
/// De que INTENTO salio el evento. `0` = de ninguno. Es lo que permite filtrar
/// por ACCION -- todo lo que produjo una sola pulsacion -- y no solo por
/// gravedad. Ver `bmo-abi`.
pub const CABINA_INTENTO: u64 = 0x08;

/// **EL BARRIDO: cuantos hubo de cada clase, y NUNCA se pierde ninguno.**
///
/// `n` empaqueta `(capa << 8) | severidad`. Ocho capas por cinco severidades.
///
/// # Por que esto existe, y por que un filtro no lo sustituye
///
/// El anillo son 48 eventos y gira. Un filtro --*"ensename los fallos"*-- solo
/// puede mirar lo que sobrevivio, asi que **un FAULT del arranque contesta
/// "ninguno" cuando ya se cayo**. Y esa respuesta es indistinguible de estar
/// bien, que es lo mas caro que puede decir un sistema de vigilancia.
///
/// *** El barrido se incrementa en `record` **antes del cerrojo del anillo**, asi
/// que cuenta tambien lo que el anillo va a perder -- por giro y por reentrancia.
///
/// > Lo que se pierde del anillo no se pierde de la cuenta.
pub const CABINA_BARRIDO_CUENTA: u64 = 0x10;

/// El `seq` del **ultimo** evento de esa clase. `0` = no hubo ninguno.
///
/// Con [`CABINA_VENTANA`] contesta la pregunta que importa cuando algo va mal:
/// **todavia se puede leer, o solo queda la cuenta?**
pub const CABINA_BARRIDO_ULTIMO: u64 = 0x11;

/// **El `seq` mas bajo que sigue dentro del anillo.** Todo lo anterior existio y
/// ya no se puede leer.
pub const CABINA_VENTANA: u64 = 0x12;

/// Cuantas clases tienen **todo** fuera del anillo. `0` = no se ha escapado nada.
///
/// Es el barrido resumido en un numero: el que se mira primero.
pub const CABINA_CLASES_FUERA: u64 = 0x13;

pub const CABINA_TXT_MODULO: u64 = 0x00;
pub const CABINA_TXT_MENSAJE: u64 = 0x01;
/// Severidades, en el orden de `cabina_core::Severity`.
pub const SEV_INFO: u64 = 0;
pub const SEV_TRACE: u64 = 1;
pub const SEV_WARNING: u64 = 2;
pub const SEV_FAULT: u64 = 3;
pub const SEV_PANIC: u64 = 4;

pub const OP_AUDIO_CLAIM: u32 = 0x21;
pub const OP_AUDIO_RELEASE: u32 = 0x22;
/// Operaciones sobre el handle `KIND_AUDIO`.
pub const AUDIO_OP_DEVICES: u32 = 0x01;
pub const AUDIO_OP_BEEP: u32 = 0x02;
pub const AUDIO_OP_VOLUME: u32 = 0x03;
pub const AUDIO_OP_SILENCE: u32 = 0x04;

/// **EL TUBO ISOCRONO**: abrirlo, armarlo y preguntarle. `arg0` dice que.
///
/// ```text
///    0  esta abierto?          1 / 0
///    1  ARMAR el silencio      1 si quedo armado
///    2  callar                 1
///    3  bytes por trama        el numero que cuadra con `wMaxPacketSize`
///    4  frecuencia elegida     en Hz
///    5  tramas encoladas       tiene que SUBIR SOLA mientras suene
///    6  *** tramas TARDE       la cifra que separa "suena bien" de "chasquea"
///    7  esta armado?           1 / 0
/// ```
///
/// [!] Armar es TRAFICO, no configuracion: 250 latidos por segundo empujando
/// tramas al bus. Por eso no se enciende solo al arrancar y hay que pedirlo.
pub const AUDIO_OP_TUBO: u32 = 0x05;
/// Bits que devuelve [`AUDIO_OP_DEVICES`].
pub const DEVICE_SPEAKER: u64 = 1 << 0;
pub const DEVICE_HDA: u64 = 1 << 1;
/// **Audifono USB Audio con control de volumen.** En esta maquina es el unico
/// aparato que suena de verdad: la placa no trae zumbador.
pub const DEVICE_USB: u64 = 1 << 2;
/// Tope de duracion de un pitido, en ms. Espejo de `obj::audio::MAX_MS`: el
/// kernel lo recorta igual, esto solo evita la sorpresa.
pub const AUDIO_MAX_MS: u64 = 250;

// Campos de `OP_INFO_TEXTO`.
pub const INFO_TXT_CPU_VENDOR: u64 = 0x01;
pub const INFO_TXT_CPU_NOMBRE: u64 = 0x02;
pub const INFO_TXT_UARCH: u64 = 0x03;
pub const INFO_TXT_FAMILIA: u64 = 0x04;
/// El nombre y el motivo de la fila `i` del censo: `INFO_TXT_EXT_NOMBRE | (i << 8)`.
pub const INFO_TXT_EXT_NOMBRE: u64 = 0x05;
pub const INFO_TXT_EXT_NOTA: u64 = 0x06;

// Operaciones sobre un handle de directorio (`KIND_DIRECTORIO`).
pub const DIR_OP_SIGUIENTE: u32 = 0x01;
pub const DIR_OP_NOMBRE: u32 = 0x02;
/// Cierra el directorio y devuelve su ranura. Lo llama `Drop`, no tu.
pub const DIR_OP_CERRAR: u32 = 0x03;

// Operaciones sobre un handle de archivo (`KIND_ARCHIVO`).
pub const ARCH_OP_LEER: u32 = 0x01;
pub const ARCH_OP_ESCRIBIR: u32 = 0x02;
pub const ARCH_OP_TAMANO: u32 = 0x03;
pub const ARCH_OP_CERRAR: u32 = 0x04;
/// Mueve el cursor a una posicion absoluta. Ver [`archivo::Archivo::saltar`].
pub const ARCH_OP_SALTAR: u32 = 0x07;

// Operaciones sobre un handle de consola (`KIND_CONSOLE`).
pub const CONSOLA_OP_LEER: u32 = 0x01;
pub const CONSOLA_OP_PERDIDOS: u32 = 0x02;
pub const CONSOLA_OP_ESCRIBIR: u32 = 0x03;
pub const CONSOLA_OP_HAY_HIJO: u32 = 0x04;

// Operaciones sobre un handle de pantalla (`KIND_FRAMEBUFFER`).
pub const FB_OP_BASE: u32 = 0x01;
pub const FB_OP_DIMS: u32 = 0x02;
pub const FB_OP_STRIDE: u32 = 0x03;
pub const FB_OP_BYTES: u32 = 0x04;

// Operaciones sobre un handle de entrada (`KIND_INPUT`).
pub const INPUT_OP_PUNTERO: u32 = 0x01;
pub const INPUT_OP_EVENTOS: u32 = 0x02;
pub const INPUT_OP_TECLA: u32 = 0x03;
pub const INPUT_OP_MODIFICADORES: u32 = 0x04;
pub const INPUT_OP_RUEDA: u32 = 0x05;
/// El evento CRUDO de teclado: scancode y si fue pulsar o soltar.
///
/// Es la segunda cola de `KIND_ENTRADA`, y convive con [`INPUT_OP_TECLA`]: las
/// dos se llenan del MISMO sondeo, y leer una no le roba nada a la otra. El
/// gemelo en C es `BMO_ENTRADA_EVENTO_TECLA` de `<bmo/entrada.h>`.
pub const INPUT_OP_EVENTO_TECLA: u32 = 0x06;

/// Bits de la mascara de modificadores.
pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_ALTGR: u8 = 1 << 3;
pub const MOD_CAPS: u8 = 1 << 4;


// ===========================================================================
//  LOS MODULOS
// ===========================================================================
//
// * Este fichero llego a tener **1624 lineas con siete trabajos dentro**: la
// puerta de syscalls, la pantalla con su fuente, los archivos, la consola, la
// entrada, la memoria y ESTRATOS. Nada de eso tiene que ver con lo demas.
//
// Y no es el compositor: **es la libreria que enlaza TODA app de Ring 3**. Su
// forma se copia sola en lo que se escriba encima, asi que un cajon aqui acaba
// siendo un cajon en todos los programas que vengan.
//
// Lo que se queda en la raiz es lo unico que de verdad es del crate entero:
// **la superficie congelada**. Las tres puertas, los opcodes y los campos de
// `INFO` son el CONTRATO con el kernel, y un contrato vive donde se entra.
//
// Todo se reexporta, asi que `bmo::Pantalla` y `bmo::info` se siguen
// escribiendo igual. **Partir una libreria no puede costarle una linea a quien
// la usa**: si costara seria una version nueva, no una reordenacion.

mod archivo;
/// DIBUJO: recorte, linea y triangulo. Lo que el sistema no sabia hacer, y
/// el oraculo con el que se juzgara la GPU. Ver la cabecera del modulo.
mod dibujo;
/// ** ADMINISTRAR EL DISCO: las dos unicas llamadas del userland que le dan una
/// ORDEN al aparato en vez de preguntarle algo. Ver su cabecera.
mod disco;
mod entrada;
mod memoria;
/// ** LA RED desde donde vive el dueno: armar el receptor y sondearlo.
///
/// Existe porque `net rx` vivia SOLO en el shell de Ring 0, y a ese shell no se
/// vuelve. Ver su cabecera.
pub mod red;
mod pantalla;
mod proceso;
/// [!!] **Lo que existe solo porque no hay driver de pantalla, y se borra entero
/// el dia que lo haya.** Ver su cabecera: es el escalon 8 de `docs/identidad/LA_RAM.md`.
mod sin_gpu;
mod sonido;
mod sys;

/// ESTRATOS desde Ring 3. Sigue siendo `bmo::estratos::...`.
pub mod estratos;

pub use sys::smp_hilo;
pub use red::{placa_cuantas, placa_ecam, placa_iommu, placa_tabla};
pub use archivo::*;
pub use dibujo::*;
pub use disco::*;
pub use entrada::*;
pub use memoria::*;
pub use pantalla::*;
pub use proceso::*;
pub use sonido::*;
pub use sys::*;
