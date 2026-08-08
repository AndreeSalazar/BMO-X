//! BMO ABI core syscall surface (the frozen 3-call surface).
//!
//! Services such as files, network, audio, graphics and input are capability
//! operations transported through BMO Channel. They are not kernel syscalls.

use super::{syscall2, syscall3, syscall6, SyscallResult};

/// Synchronous, capability-scoped control operation.
pub const NR_INVOKE: u32 = 0x00;
/// Notify a channel consumer after publishing submissions.
pub const NR_CHANNEL_KICK: u32 = 0x01;
/// Block until a sequence changes or an absolute deadline expires.
pub const NR_WAIT: u32 = 0x02;
pub const CORE_SYSCALL_COUNT: usize = 3;

/// Process-local pseudo-handle that always resolves to the calling task.
/// It grants no authority over another task and must never be transferred.
pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
pub const TASK_OP_GET_PID: u64 = 0x01;
pub const TASK_OP_GET_TID: u64 = 0x02;
pub const TASK_OP_YIELD: u64 = 0x03;
pub const TASK_OP_EXIT: u64 = 0x04;
/// `INVOKE(CURRENT_TASK, CHANNEL_OPEN, index)` -> the caller's estuary
/// capability handle for BMO Channel `index`. Fails with NEEDS_CAP when
/// the process was not granted that estuary.
pub const TASK_OP_CHANNEL_OPEN: u64 = 0x05;
/// `INVOKE(CURRENT_TASK, CONSOLE_WRITE, packed)` -> emit up to 8 bytes of
/// text (packed little-endian in `packed`, NUL-terminated within the word)
/// to the kernel bootstrap console. This is the debug door that lets the
/// very first Ring 3 program prove the CPL3->CPL0 path visually before a
/// real console capability/estuary service exists; it will migrate to a
/// console handle once the display server lands.
pub const TASK_OP_CONSOLE_WRITE: u64 = 0x06;

/// Crea un endpoint atendido por este proceso: `arg0` es el estuario por el
/// que se le entregaran las llamadas, y devuelve el handle del endpoint.
///
/// Es lo unico que Endpoint RPC anade a la superficie. Llamar, atender y
/// responder NO son operaciones nuevas: son lo que `INVOKE` y `WAIT` ya
/// significan cuando el handle resuelve a un endpoint o a un reply. La
/// superficie sigue siendo de tres puertas.
pub const TASK_OP_ENDPOINT_CREATE: u64 = 0x07;

/// LEE de la consola asignada al proceso. La PAREJA de `CONSOLE_WRITE`.
///
/// Sin esto un programa lanzado desde un terminal no puede recibir nada: la
/// capability del teclado la tiene el compositor, y darsela a cada hijo seria
/// romper la exclusividad que hace que la entrada tenga un solo dueno. El
/// terminal que lo lanzo le pasa lo que se teclea, por el mismo objeto que ya
/// usa para hablar.
pub const TASK_OP_CONSOLE_READ: u64 = 0x0F;

/// Acumula hasta 8 bytes de una RUTA en el renglon del proceso.
///
/// La superficie congelada no acepta punteros, asi que una ruta viaja de 8 en
/// 8 y la consume la siguiente operacion que necesite una. **Un solo renglon**
/// para `EJECUTAR`, `DIR_ABRIR` y los dos de archivo: inventar un mecanismo
/// por cada consumidor seria tener cuatro sitios donde se pierde un byte.
pub const TASK_OP_RUTA: u64 = 0x0B;
/// Lanza lo acumulado con [`TASK_OP_RUTA`] y vacia el renglon. Devuelve el tid.
///
/// * Estas tres (`0x0C`-`0x0E`) vivian **solo dentro del kernel**: se anadieron
/// a `ring0/syscall.rs` y nunca subieron aqui, asi que el guardian de deriva de
/// `build.ps1` no las miraba -- no puede comparar lo que en un lado no existe.
/// La superficie es el contrato; el kernel es una implementacion suya.
pub const TASK_OP_EJECUTAR: u64 = 0x0C;
/// Crea una consola y devuelve su handle de LECTURA. Quien la crea es el
/// terminal: la consola es suya y la drena a su ritmo.
pub const TASK_OP_CONSOLA_CREAR: u64 = 0x0D;
/// Abre un directorio del volumen de datos. La ruta se acumula antes con
/// [`TASK_OP_RUTA`] -- el mismo renglon que usa `EJECUTAR`.
pub const TASK_OP_DIR_ABRIR: u64 = 0x0E;

/// Abre un archivo del volumen de datos para LEER. La ruta viene del renglon.
pub const TASK_OP_ARCHIVO_ABRIR: u64 = 0x10;

/// Abre un archivo del volumen de datos para ESCRIBIR (lo crea).
///
/// Son dos operaciones y no un argumento de modo porque crear puede fallar por
/// motivos que abrir no tiene --volumen de solo lectura, nombre que no es 8.3--
/// y mezclarlas obligaria a devolver errores que no aplican a la mitad de las
/// llamadas.
pub const TASK_OP_ARCHIVO_CREAR: u64 = 0x11;

// -- Operaciones sobre un handle de archivo (`KIND_ARCHIVO`) --------------
//
// Viven aqui y no en el kernel porque las emite `bmo-lower` y las ejecuta el
// emulador: tres sitios que tienen que decir el mismo numero. Ver
// `Ultra_kernel_x86-64/kernel/src/ring0/archivo.rs`.

/// Saca hasta 7 bytes: `(n << 56) | bytes_LE`. `n == 0` = se acabo.
///
/// La cuenta va en el byte alto y NO se corta en el primer cero, al reves que
/// la consola: un archivo no es texto y un `\0` en medio es un dato.
/// Reinicia la maquina. No vuelve.
///
/// Reiniciar es tocar puertos de E/S (`0xCF9`, el 8042), que Ring 3 no puede
/// --ni debe-- hacer; por eso es una operacion y no un permiso ambiental.
/// **Hoy no esta atada a una capability**, igual que `EJECUTAR`: las dos
/// quieren la misma el dia que exista.
pub const TASK_OP_REINICIAR: u64 = 0x12;

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
pub const TASK_OP_INFO: u64 = 0x13;
/// Un dato de TEXTO. `arg0` = campo (`INFO_TXT_*`), `arg1` = que trozo.
///
/// Devuelve 8 bytes empaquetados en little-endian, el cero corta -- el mismo
/// formato que `TASK_OP_RUTA` y `TASK_OP_CONSOLE_WRITE`, y por la misma razon:
/// aqui no hay `copy_to_user`, asi que el texto viaja por valor.
pub const TASK_OP_INFO_TEXTO: u64 = 0x14;

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

/// -- El CURSOR de ESTRATOS ---------------------------------------------
///
/// `INFO_ES_*` contesta *como esta* el almacen: generacion, ocupacion, nivel.
/// **No contesta que hay dentro**, y por eso la ventana de Datos podia pintar
/// numeros y no un arbol: `raiz`, `nodo`, `entradas` y `entrada` eran funciones
/// de Ring 0 sin puerta.
///
/// Esto es esa puerta, y son **DOS operaciones y no diez**: un cursor que se
/// mueve (`TASK_OP_ES_NODO`, con la pregunta en `arg0` y su argumento en
/// `arg1`) y los nombres (`TASK_OP_ES_TEXTO`, de ocho en ocho). La superficie
/// crece por su tabla, no por sus puertas -- el mismo trato que el klog.
///
/// * **No concede nada.** Contesta, igual que `OP_INFO`: leer los nombres de un
/// directorio no ejerce ningun poder, y aqui no hay ni una operacion que
/// escriba.
pub const ES_NODO_RAIZ: u64 = 0x00;
/// Cuantos hijos tiene el nodo donde esta el cursor.
pub const ES_NODO_HIJOS: u64 = 0x01;
/// 1 si el listado NO cabia entero. Se pregunta y se dice: un directorio
/// truncado en silencio se ve igual que uno corto.
pub const ES_NODO_TRUNCADO: u64 = 0x02;
/// Cuantos niveles se ha bajado desde la raiz.
pub const ES_NODO_HONDO: u64 = 0x03;
/// Tipo del nodo actual: 0 archivo, 1 directorio, 2 no hay nada.
pub const ES_NODO_TIPO: u64 = 0x04;
/// Tipo del hijo `arg1`. Mismos codigos que [`ES_NODO_TIPO`].
pub const ES_NODO_HIJO_TIPO: u64 = 0x05;
/// Baja al hijo `arg1`. 1 si se pudo.
pub const ES_NODO_ENTRAR: u64 = 0x06;
/// Vuelve al padre. 1 si se pudo, 0 si ya estaba en la raiz.
pub const ES_NODO_SUBIR: u64 = 0x07;
/// -- El DETALLE del hijo `arg1` ----------------------------------------
///
/// Un grafo que solo ensena nombres contesta *que hay*; no contesta *que es
/// esto*. Esto es lo que el nodo ya lleva dentro y la ventana no podia pedir.
///
/// Bytes de su contenido. Un directorio contesta lo que ocupa su lista de
/// entradas -- que tambien es un dato, y distinto de lo que hay dentro.
pub const ES_NODO_HIJO_BYTES: u64 = 0x08;
/// Cuantos atributos lleva. Es el numero que dice que ESTRATOS no es un
/// sistema de carpetas: un nodo **es un conjunto de atributos**, y un archivo
/// y un directorio se diferencian en cual llevan, no en su estructura.
pub const ES_NODO_HIJO_ATRIBUTOS: u64 = 0x09;
/// 1 si lleva `:firma`. **Solo si la lleva, no si cuadra** -- comprobarlo exige
/// leer el contenido entero, y eso no puede pasar en cada repintado.
pub const ES_NODO_HIJO_FIRMADO: u64 = 0x0A;
/// * **Lee el hijo y compara su BLAKE3 con su `:firma`.** Se pide a mano.
///
/// `0` no lleva - `1` CUADRA - `2` NO CUADRA - `3` no se pudo leer.
///
/// [!] Demuestra que los bytes son los que se guardaron --caza corrupcion y
/// escrituras a medias--. **No demuestra autenticidad**: quien pueda escribir
/// en el volumen puede cambiar el archivo *y* recalcular su hash.
pub const ES_NODO_VERIFICAR: u64 = 0x0B;

/// Que texto pide [`TASK_OP_ES_TEXTO`], en los bits altos de `arg0`.
///
/// Los bajos siguen siendo el indice. Se reparte el argumento en vez de anadir
/// una operacion porque **son el mismo mecanismo** --sacar un nombre de ocho en
/// ocho-- pidiendo dos cosas distintas, y una puerta por cada texto que devuelva
/// el sistema es como una superficie de tres syscalls acaba teniendo treinta.
pub const ES_TXT_HIJO: u64 = 0;
/// El nombre del nivel `indice` de la ruta. `0` es la raiz y contesta vacio.
pub const ES_TXT_RUTA: u64 = 1;

/// **Bytes que Ring 3 ha PEDIDO** con `KIND_MEMORIA`, desde el arranque.
///
/// Es el unico dato de memoria que el kernel no puede deducir mirando lo que
/// cargo: la imagen y la pila de un proceso las puso el, pero un bloque pedido
/// solo existe porque alguien lo pidio. Y es la confirmacion **desde el otro
/// lado** de que la capability funciona -- el programa dice que le dieron
/// memoria; esto lo dice el kernel.
///
/// Contador que ya existia en `ring0::obj::memoria::total_entregado()` y que
/// **no leia nadie**. Un contador que nadie consulta no es telemetria: es una
/// variable.
pub const INFO_MEM_ENTREGADA: u64 = 0x19;

/// Fabricante ("AMD"), nombre comercial, microarquitectura y familia/modelo.
pub const INFO_TXT_CPU_VENDOR: u64 = 0x01;
pub const INFO_TXT_CPU_NOMBRE: u64 = 0x02;
pub const INFO_TXT_UARCH: u64 = 0x03;
pub const INFO_TXT_FAMILIA: u64 = 0x04;

pub const ARCH_OP_LEER: u64 = 0x01;
/// Saca hasta 7 bytes **sin pasar del salto de linea**:
/// `(fin << 63) | (n << 56) | bytes_LE`.
///
/// - `fin = 1` -- se llego al salto, que se CONSUME. El registro esta completo.
/// - `n = 0` y `fin = 0` -- se acabo el archivo.
///
/// Existe porque `ARCH_OP_LEER` no sirve para leer registros: devuelve siete
/// bytes y avanza el cursor siete, asi que si el salto cae en medio del
/// paquete, lo que venia detras **se pierde**. Un fichero de movimientos
/// leido asi da bien el primer registro y basura los demas.
///
/// El corte lo hace el kernel y no el llamante porque el cursor es del kernel:
/// nadie de fuera puede devolverle los bytes que ya le dio.
pub const ARCH_OP_LEER_LINEA: u64 = 0x05;
/// Mete hasta 7 bytes: `arg0 = (n << 56) | bytes_LE`. Devuelve los aceptados.
pub const ARCH_OP_ESCRIBIR: u64 = 0x02;
/// **Lee un BLOQUE entero de golpe, dentro de memoria que el kernel concedio.**
///
/// `arg0` = handle del bloque (`TASK_OP_MEMORIA_PEDIR`), `arg1` = desplazamiento
/// dentro del bloque, `arg2` = cuantos bytes. Devuelve los leidos de verdad.
///
/// === Por que existe, y por que asi ===
///
/// [`ARCH_OP_LEER`] devuelve **siete bytes** metidos en un registro. Para un WAD
/// de DOOM de 4 MB eso son **seiscientas mil llamadas al sistema** para cargarlo
/// una vez. No era pereza: `core/informe.rs` lo dejo escrito -- *"pasar un
/// puntero de Ring 3 obligaria al kernel a validar el rango entero contra el
/// espacio del llamante, y esa infraestructura no existe"*.
///
/// * Y la salida no es construir esa infraestructura: es **no necesitarla**. El
/// destino no es un puntero que el llamante inventa, es **un bloque que el
/// kernel concedio y cuyos limites tiene apuntados**. No hay nada que validar
/// contra el espacio de nadie: se comprueba `desplazamiento + n` contra lo que
/// se entrego, y ya. Es la misma razon por la que reclamar la pantalla es
/// seguro -- el kernel no comprueba el framebuffer, **lo dio el**.
///
/// Un contrato en vez de una comprobacion, que es como crece todo aqui.
pub const ARCH_OP_LEER_EN: u64 = 0x06;
/// **Mueve el cursor** a un desplazamiento absoluto. Devuelve donde quedo.
///
/// Es `fseek`, y cuesta lo que cuesta poner un numero porque **el archivo ya
/// esta entero en un bufer del kernel** desde que se abrio. Sin esto, DOOM no
/// puede leer su WAD ni empezando: el directorio de lumps esta al FINAL.
pub const ARCH_OP_SALTAR: u64 = 0x07;
/// Bytes que quedan por leer, o los acumulados si es de escritura.
pub const ARCH_OP_TAMANO: u64 = 0x03;
/// Cierra. En uno de escritura **es donde el contenido llega al disco**.
pub const ARCH_OP_CERRAR: u64 = 0x04;

// -- La entrada: raton y teclado -----------------------------------------
//
// * Estas constantes vivian en DOS sitios --`ring0/obj/input.rs` y el userland
// de Rust-- y en ninguno de los dos que fuera el contrato. Mientras el unico
// cliente fue un compositor escrito en Rust eso se notaba poco; en cuanto un
// segundo lenguaje quiso leer la rueda, se vio lo que era: un contrato que no
// estaba publicado no lo puede cumplir nadie mas. Aqui no hay logica nueva,
// hay un sitio del que copiar en vez de dos de los que adivinar.

/// Reclama raton + teclado. **Exclusivo**: mientras un proceso lo tenga, el
/// shell de Ring 0 deja de leer el teclado fisico. No es un reparto -- dos
/// lectores de la misma cola se robarian las letras.
pub const TASK_OP_INPUT_CLAIM: u64 = 0x0A;
/// Reclama la pantalla. Tambien exclusivo.
pub const TASK_OP_FRAMEBUFFER_CLAIM: u64 = 0x09;

/// **Pide un bloque de memoria.** `arg0` = bytes. Devuelve el handle de una
/// capability `KIND_MEMORIA`; la direccion se pregunta con `MEM_OP_BASE`.
///
/// * NO es un `malloc`: entrega **un bloque grande, entero y contiguo**, y no
/// hay forma de devolverlo. El asignador no es trabajo del kernel -- se escribe
/// encima, en Ring 3, con la politica que quiera cada lenguaje. El caso que lo
/// decidio es DOOM: pide ~8 MiB una vez y se los administra el.
pub const TASK_OP_MEMORIA_PEDIR: u64 = 0x15;

/// **Cuantas lineas del log del kernel se pueden leer.** `arg0` elige el dato:
/// 0 = disponibles ahora, 1 = escritas desde el arranque (la resta son las que
/// se cayeron por el borde del anillo).
///
/// * Esto **no da privilegio, da vista**. Ring 3 no ejecuta nada en Ring 0:
/// pide texto por su numero y recibe bytes, igual que `TASK_OP_INFO`. En un
/// sistema de capabilities *ver* y *poder* son cosas separadas, y juntarlas es
/// como se acaba teniendo un "modo administrador".
///
/// Hace falta desde que **el escritorio es el arranque**: mientras el
/// compositor tiene la pantalla, el panel del kernel no se pinta, y con el
/// desaparecia el relato entero de como arranco la maquina.
pub const TASK_OP_KLOG_INFO: u64 = 0x16;
/// **Ocho bytes de una linea del log.** `arg0` = linea (**0 es la mas
/// reciente**), `arg1` = trozo de 8 en 8. Cero = se acabo, igual que
/// `TASK_OP_INFO_TEXTO`.
pub const TASK_OP_KLOG_TEXTO: u64 = 0x17;

/// Campos de [`TASK_OP_KLOG_INFO`].
pub const KLOG_DISPONIBLES: u64 = 0x00;
pub const KLOG_TOTAL: u64 = 0x01;

// =======================================================================
//  LIENZO -- una app pinta donde se va a ver
// =======================================================================
//
// El contrato completo esta en `docs/LIENZO.md`. Aqui va lo que el ABI necesita
// saber, y **la aritmetica de paginas**, que es la parte que puede dar acceso a
// los pixeles del vecino si se hace mal.
//
// * Esta aqui y no en el kernel A PROPOSITO: `bmo-abi` se prueba en el
// anfitrion. Esta cuenta se verifica con tests **antes** de que exista una sola
// linea que mapee una pagina -- que es la unica forma sensata de estrenar algo
// que, si se equivoca, deja a una app escribiendo en la ventana de al lado.

/// **Pedir el REFLEJO: pintar directamente donde se ve.**
///
/// `arg0` = **paginas** de 4 KiB - `arg1` = formato. Devuelve un handle
/// (`KIND_LIENZO`), o `0` si no hay reflejo que prestar.
///
/// === Por que se pide en PAGINAS y no en filas ===
///
/// Porque es la unidad que el kernel sabe repartir y proteger. Pedirlo en filas
/// obligaba a traducir filas<->paginas, y esa traduccion es donde vivia todo lo
/// dificil: una fila mide `stridex4` bytes --7680 con stride 1920-- que no es
/// multiplo de 4096, asi que solo cada ocho filas cae en un limite de pagina.
/// Habia que calcular ese grano con un maximo comun divisor, en los dos lados, y
/// que los dos lados coincidieran para siempre.
///
/// * **Ese problema no se resuelve: se elimina.** Cada lado habla en su unidad:
///
/// | | Habla en | Porque |
/// |---|---|---|
/// | el kernel | **paginas** | es lo unico que sabe repartir |
/// | la app | **filas** | es lo unico que sabe pintar |
/// | el contrato | **bytes** | `base`, `bytes`, `stride` |
///
/// Las ultimas N paginas de un bloque alineado estan alineadas **siempre**, sin
/// una cuenta. El kernel adelanta la `base` hasta el principio de fila --una
/// division local, no una formula compartida-- y la app saca sus filas con
/// `bytes / (stridex4)`. **Ninguno necesita la aritmetica del otro.**
pub const TASK_OP_LIENZO_REFLEJO: u64 = 0x1C;

/// El formato **lo declara la app**, no lo fija el kernel.
///
/// Decision del dueno el 2026-08-07, y tiene destinatario: DOOM pinta en 8 bits
/// con paleta. Fijar 32 bits ahora obligaria a reabrir el contrato el dia que
/// llegue -- y un contrato que se reabre no era un contrato.
pub const LIENZO_FMT_XRGB32: u64 = 0x00;
/// 8 bits con paleta. **Todavia no se sirve**: el numero queda reservado para
/// que nadie lo use para otra cosa. Pedirlo hoy recibe un no, que es distinto de
/// que el campo no exista.
pub const LIENZO_FMT_PAL8: u64 = 0x01;

/// Operaciones sobre el handle del reflejo. Mismo trato que el framebuffer:
/// se pregunta donde esta y cuanto mide, y se pinta sin volver a llamar.
pub const LIENZO_OP_BASE: u64 = 0x01;
/// Bytes utilizables **desde `BASE`**, ya descontado lo que se perdio al
/// alinear a fila.
pub const LIENZO_OP_BYTES: u64 = 0x02;
/// Stride **en pixeles**, el del panel. La app indexa `y*stride + x`.
///
/// [!] No es el ancho de la banda: es el ancho del lienzo entero. Usar el ancho
/// en vez del stride es el bug mas viejo de los graficos -- la imagen sale
/// inclinada en diagonal y compila perfectamente.
pub const LIENZO_OP_STRIDE: u64 = 0x03;

/// **Solo hay UN reflejo, y es a pantalla completa.**
///
/// No hay tabla de bandas, ni apilado, ni cuentas de lo reservado: un
/// `Option<pid>` en el kernel y ya. Cubre exactamente el caso que importa --DOOM
/// y el raycaster van a pantalla completa-- y el dia que hagan falta dos
/// programas a la vez, ese es el **modo ventana con copia**, que es el que
/// compone de verdad.
///
/// Reflejo = uno, sin copias. Ventana = varios, con copia. Dos modos con un
/// trabajo cada uno, y ninguno intentando ser el otro.
pub const LIENZO_UNICO: bool = true;

/// Filas del lienzo que **nunca** se prestan: la barra del escritorio y su caja
/// de Ejecutar viven arriba.
///
/// Que sea un numero fijo y no "lo que el compositor diga" es a proposito: el
/// kernel no puede preguntarle a un proceso de Ring 3 cuanto sitio necesita para
/// decidir si le presta memoria a otro. Un minimo fijo se audita de un vistazo.
pub const LIENZO_FILAS_RESERVADAS_ARRIBA: u32 = 64;

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

/// **Despertar los otros nucleos.** Devuelve `vivos<<32 | esperados`, ambos sin
/// contar el BSP.
///
/// * Existe porque el comando `smp` vivia **solo en el shell de Ring 0**, y ese
/// shell deja de leer el teclado en cuanto el compositor reclama `KIND_INPUT`.
/// O sea: habia codigo que no se podia ejecutar desde donde el dueno estaba
/// sentado. Un mando al que no se llega es un mando que no existe.
///
/// Y encaja sin tocar nada de lo congelado: la superficie sigue siendo tres
/// syscalls, y esto es **una fila mas** en la tabla de operaciones de la tarea
/// -- que es exactamente por donde la arquitectura dice que la API crece.
///
/// [!] Bloquea mientras dura el bring-up (hasta ~10 ms por nucleo). Quien la
/// llama deberia avisar en pantalla ANTES, porque es la unica operacion del
/// sistema que puede tardar un segundo entero.
pub const TASK_OP_SMP_DESPERTAR: u64 = 0x1B;

/// **SELLAR: cierra una transaccion vacia en ESTRATOS.** Devuelve la generacion
/// nueva, o 0 si no se pudo.
///
/// * **Es la primera operacion de la superficie que ESCRIBE EN EL DISCO**, y
/// por eso lleva su propia operacion en vez de esconderse detras de un campo de
/// otra: lo que cambia el estado del almacen se pide por su nombre.
///
/// Lo que hace es deliberadamente lo mas pequeno posible: ni un bloque de
/// datos, el mismo estrato, y el commit va a **la copia del superbloque que no
/// manda**. Recorre el camino entero --`FLUSH CACHE`, barrera, commit, vaciar
/// otra vez-- sin poder perder nada aunque salga mal. Ver
/// `ring0/fsys/estratos.rs::sellar`.
pub const TASK_OP_ESTRATOS_SELLAR: u64 = 0x18;

/// El cursor de ESTRATOS: `arg0` es la pregunta ([`ES_NODO_RAIZ`] y compania),
/// `arg1` su argumento cuando lo lleva.
pub const TASK_OP_ES_NODO: u64 = 0x19;
/// Ocho bytes del nombre del hijo `arg0`; `arg1` numera el trozo.
///
/// De ocho en ocho porque la superficie congelada no acepta punteros, y es el
/// mismo mecanismo que `KLOG_TEXTO` y `DIR_OP_NOMBRE` -- inventar uno nuevo por
/// cada cosa que devuelve texto seria tener tres sitios donde se pierde un byte.
pub const TASK_OP_ES_TEXTO: u64 = 0x1A;

/// Donde empieza el bloque, en el espacio del proceso que lo pidio.
pub const MEM_OP_BASE: u64 = 0x01;
/// Cuantos bytes se le han entregado en total a este proceso.
pub const MEM_OP_BYTES: u64 = 0x02;

/// Donde esta el puntero y que botones tiene: `(x << 32) | (y << 16) | botones`.
/// Ya viene recortado al panel: el kernel es quien sabe de que tamano es.
pub const INPUT_OP_PUNTERO: u64 = 0x01;
/// Cuantos informes HID se han visto desde el arranque. Distingue "el raton no
/// se mueve" de "el raton no llega": si esto no sube, el problema esta en el USB.
pub const INPUT_OP_EVENTOS: u64 = 0x02;
/// La siguiente tecla: `0x100 | byte`, o `0` si no hay ninguna esperando.
/// **No bloquea.** El byte es Latin-1 ya resuelto (la `n` es `0xF1`).
pub const INPUT_OP_TECLA: u64 = 0x03;
/// Mascara de modificadores pulsados AHORA. Es estado, no consume nada.
pub const INPUT_OP_MODIFICADORES: u64 = 0x04;
/// Las muescas de rueda **desde la ultima vez**, como `i32` en complemento a
/// dos dentro del `u64`. Positivo = hacia arriba.
///
/// * **Consume**: dos lecturas seguidas sin girar dan cero la segunda. Devolver
/// un acumulado desde el arranque obligaria a cada llamante a guardar el
/// anterior y restar, y el primero que lo olvide tiene un scroll que se mueve
/// solo.
pub const INPUT_OP_RUEDA: u64 = 0x05;

/// Bits de la mascara de [`INPUT_OP_MODIFICADORES`].
pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_ALTGR: u8 = 1 << 3;
pub const MOD_CAPS: u8 = 1 << 4;

/// Las teclas sin glifo, en el rango C1 (0x80..0x9F) que eligio el driver.
///
/// No son ASCII y no lo pretenden: son bytes que ninguna distribucion produce
/// como caracter, asi que un programa puede distinguirlas de lo que se escribe
/// sin un segundo canal.
/// Son los mismos bytes que `ring0::dev::keyboard::KEY_*`, y esa igualdad es
/// el contrato: si divergen, un programa lee flechas donde hay paginas.
pub const TECLA_ARRIBA: u8 = 0x80;
pub const TECLA_ABAJO: u8 = 0x81;
pub const TECLA_IZQUIERDA: u8 = 0x82;
pub const TECLA_DERECHA: u8 = 0x83;
pub const TECLA_INICIO: u8 = 0x84;
pub const TECLA_FIN: u8 = 0x85;
pub const TECLA_SUPR: u8 = 0x86;
pub const TECLA_REPAG: u8 = 0x87;
pub const TECLA_AVPAG: u8 = 0x88;

/// Las teclas de funcion, detras de la navegacion en el mismo rango C1.
///
/// * Son el sitio correcto para un atajo del sistema porque **no producen
/// caracter en ninguna distribucion**: no pueden chocar con escribir. Una
/// combinacion con `Ctrl+Alt` si puede -- en espanol `Ctrl+Alt` *es* AltGr.
pub const TECLA_F1: u8 = 0x89;
pub const TECLA_F2: u8 = 0x8A;
pub const TECLA_F3: u8 = 0x8B;
pub const TECLA_F4: u8 = 0x8C;
pub const TECLA_F5: u8 = 0x8D;
pub const TECLA_F6: u8 = 0x8E;
pub const TECLA_F7: u8 = 0x8F;
pub const TECLA_F8: u8 = 0x90;
pub const TECLA_F9: u8 = 0x91;
pub const TECLA_F10: u8 = 0x92;
pub const TECLA_F11: u8 = 0x93;
pub const TECLA_F12: u8 = 0x94;

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

/// `INVOKE` operations accepted by a channel (estuary) capability.
pub const CHANNEL_OP_GET_SEQ: u64 = 0x01;
pub const CHANNEL_OP_GET_INDEX: u64 = 0x02;

pub mod channel_op {
    /// Completion-side sequence -- the value `WAIT` compares against.
    pub const GET_SEQ: u64 = super::CHANNEL_OP_GET_SEQ;
    /// Estuary index backing this capability.
    pub const GET_INDEX: u64 = super::CHANNEL_OP_GET_INDEX;
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

/// `CHANNEL_KICK(channel, published_sequence)`.
#[inline(always)]
pub unsafe fn channel_kick(channel: u64, published_sequence: u64) -> SyscallResult {
    syscall2(NR_CHANNEL_KICK, channel, published_sequence)
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
        NR_CHANNEL_KICK => Some("bmo_channel_kick"),
        NR_WAIT => Some("bmo_wait"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_surface_is_frozen_to_three_calls() {
        assert_eq!(CORE_SYSCALL_COUNT, 3);
        assert_eq!(name(0), Some("bmo_invoke"));
        assert_eq!(name(1), Some("bmo_channel_kick"));
        assert_eq!(name(2), Some("bmo_wait"));
        assert_eq!(name(3), None);
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
