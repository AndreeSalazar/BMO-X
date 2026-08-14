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
/// **El censo de audio**: que el aparato diga como quiere las muestras.
/// Devuelve 1 si encontro uno; los ocho numeros van a CABINA.
pub const OP_AUDIO_CENSO: u32 = 0x28;
/// **Toma lo que otro proceso me ofrecio.** Ver [`crate::sys::tomar_prestado`].
pub const OP_TOMAR: u32 = 0x1C;
/// Operacion sobre un bloque PROPIO: ofrecer un trozo a otra tarea.
pub const MEM_OP_OFRECER: u32 = 0x03;
/// **Quien me lanzo**, como TID. `0` si nadie -- ver [`crate::sys::mi_padre`].
pub const OP_MI_PADRE: u32 = 0x26;
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
pub const INFO_NET_PCI: u64 = 0x2E;

pub const INFO_CPU_HILOS: u64 = 0x06;
pub const INFO_CPU_NUCLEOS: u64 = 0x07;
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
mod entrada;
mod memoria;
mod pantalla;
mod proceso;
/// [!!] **Lo que existe solo porque no hay driver de pantalla, y se borra entero
/// el dia que lo haya.** Ver su cabecera: es el escalon 8 de `docs/LA_RAM.md`.
mod sin_gpu;
mod sonido;
mod sys;

/// ESTRATOS desde Ring 3. Sigue siendo `bmo::estratos::...`.
pub mod estratos;

pub use archivo::*;
pub use dibujo::*;
pub use entrada::*;
pub use memoria::*;
pub use pantalla::*;
pub use proceso::*;
pub use sonido::*;
pub use sys::*;
