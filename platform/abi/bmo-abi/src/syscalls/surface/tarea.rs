//! **Lo que se le pide a `CURRENT_TASK`.** Los `TASK_OP_*`.
//!
//! Cuarenta y una operaciones sobre la propia tarea: quien soy, ceder, salir,
//! abrir un canal, reclamar la pantalla, lanzar un programa, preguntar por el
//! sistema. Es la familia mas grande del contrato **y la que mas crece**, y por
//! eso tiene fichero propio.
//!
//! [!] Dos operaciones no pueden llevar el mismo numero, y casi paso: la
//! autopsia se escribio en `0x1D` y `0x1E`, que ya eran PANTALLA_SOLTAR y
//! ENTRADA_SOLTAR -- o sea que **leer el informe de un fallo habria soltado la
//! pantalla**. Hoy lo vigila `build.ps1` barriendo el kernel entero.

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

/// Conectar con un endpoint de RPC ya creado.
pub const TASK_OP_ENDPOINT_CONNECT: u64 = 0x08;

/// **Soltar la pantalla sin morirse.** Pareja de reclamarla.
///
/// Existe porque prestar la pantalla y quedarse la ENTRADA no es prestar: es
/// dejar a un programa pintando en una habitacion cerrada. Las dos capabilities
/// van juntas o no van.
pub const TASK_OP_PANTALLA_SOLTAR: u64 = 0x1D;

/// Soltar la entrada. La otra mitad de [`TASK_OP_PANTALLA_SOLTAR`].
pub const TASK_OP_ENTRADA_SOLTAR: u64 = 0x1E;

/// **El censo de audio**: que el aparato diga como quiere las muestras.
///
/// Devuelve 1 si encontro uno de reproduccion; los ocho numeros van a CABINA,
/// porque por la puerta cabe uno. Paso 0 de `docs/maestro/AUDIO_MAESTRO.md`.
pub const TASK_OP_AUDIO_CENSO: u64 = 0x28;

pub const TASK_OP_INFO: u64 = 0x13;

/// Un dato de TEXTO. `arg0` = campo (`INFO_TXT_*`), `arg1` = que trozo.
///
/// Devuelve 8 bytes empaquetados en little-endian, el cero corta -- el mismo
/// formato que `TASK_OP_RUTA` y `TASK_OP_CONSOLE_WRITE`, y por la misma razon:
/// aqui no hay `copy_to_user`, asi que el texto viaja por valor.
pub const TASK_OP_INFO_TEXTO: u64 = 0x14;

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

/// ** LA AUTOPSIA de un fallo de Ring 3.
///
/// El klog cuenta el relato de la maquina; esto guarda el INFORME de cada
/// muerte: vector, codigo de error, la direccion que se toco, el `rip`, la
/// pila, **que programa era** y lo ultimo que llego a escribir.
///
/// Existe porque la linea que dejaba CABINA --`fault en CPL3: tarea eliminada,
/// BMO sigue vivo` con el `rip` detras-- alcanza para saber QUE paso y no para
/// saber DONDE. Un fallo que no se puede mandar a nadie se cuenta de memoria, y
/// contar un fallo de memoria es como se pierden los fallos.
///
/// * El kernel captura en RAM y **no toca el disco**: se corre dentro de un
/// fault, y el fallo puede ser justo del disco. Quien lo persiste es Ring 3,
/// que esta vivo y tiene la capability. El kernel CONTESTA, no actua.
///
/// `arg0` = campo (ver `AUTOPSIA_*`), y para `AUTOPSIA_RENGLONES`, `arg1` = que
/// informe (**0 es el mas reciente**).
pub const TASK_OP_AUTOPSIA_INFO: u64 = 0x1F;

/// **Ocho bytes de un renglon del informe.** `arg0` empaqueta
/// `(informe << 32) | fila` y `arg1` es el trozo de 8 en 8. Cero = se acabo.
///
/// Van los dos indices en un solo argumento porque la puerta tiene tres y dos
/// ya estan ocupados por la operacion y el trozo. Es la misma aritmetica que
/// usa `INPUT_OP_*` para el raton.
pub const TASK_OP_AUTOPSIA_TEXTO: u64 = 0x20;

/// **Reclamar el SONIDO.** Devuelve un handle `HandleKind::AudioEngine`: el
/// derecho a hacer ruido, exclusivo como la pantalla.
///
/// Es el CONTRATO y no un driver. Lo unico que suena hoy es el altavoz del PC;
/// HD Audio --codec, DMA, anillo de buffers-- es otra pieza y no existe todavia.
/// Se escribe el contrato primero a proposito: un motor de audio sin la
/// pregunta de quien tiene derecho a usarlo acaba en un sistema donde cualquier
/// programa pita encima de cualquier otro.
///
/// Las operaciones sobre el handle son `AUDIO_OP_*`.
/// **CABINA leida desde Ring 3.** `arg0` = campo (`CABINA_*`), `arg1` = que
/// evento (0 = el mas reciente).
///
/// El klog ya se podia leer, pero es la transcripcion en texto plano: **no
/// lleva severidad**. CABINA si -- severidad, capa y modulo por evento-- y sin
/// eso una linea que dice que el SMP levanto doce nucleos llega igual que
/// cualquier otra.
///
/// Contesta y no concede: **ni una de estas dos operaciones escribe nada**. Ver
/// y poder son cosas separadas.
pub const TASK_OP_CABINA_INFO: u64 = 0x23;

/// Ocho bytes del modulo o del mensaje. `arg0` = `(evento << 32) | cual`,
/// `arg1` = el trozo. El cero corta.
pub const TASK_OP_CABINA_TEXTO: u64 = 0x24;

/// **Abrir MI PROPIA imagen, para leer los datos que lleva dentro.**
///
/// Devuelve un handle de `KIND_ARCHIVO` de LECTURA sobre el `.bex` desde el que
/// se lanzo este proceso. No lleva argumentos: el programa no dice **cual** --
/// dice *"el mio"*, y quien sabe cual es el kernel.
///
/// ** Por que no vale con `TASK_OP_ARCHIVO_ABRIR` y la ruta: porque abrir por
/// ruta es **pedir por nombre lo que se tiene por derecho**. Un programa que
/// escribe su propia ruta podria escribir otra, y en un sistema de capabilities
/// eso es exactamente lo que no se hace. Ademas, un binario movido de sitio
/// dejaria de encontrarse a si mismo.
///
/// Falla si el kernel no recuerda de donde salio -- pasa con los programas que
/// el propio kernel embebe, que no vienen de ninguna ruta.
pub const TASK_OP_MI_PAQUETE: u64 = 0x25;

/// **Quien me lanzo**, como TID. `0` si no hay nadie.
///
/// Una app dibuja en su memoria y se la OFRECE al que la puso en pantalla (ver
/// `<bmo/superficie.h>`). Ofrecer exige nombrar al destinatario, y el hijo no
/// tiene forma de nombrarlo: [`MEM_OP_OFRECER`] habla en tids, y el tid del
/// compositor no aparece en ningun sitio de su espacio.
///
/// ** Y por eso NO es un registro de nombres. La pregunta no es *"quien manda"*
/// --eso seria autoridad ambiental, y quien la leyera podria pedirle cosas a
/// alguien que nunca se las ofrecio-- sino **"quien me lanzo a MI"**: local,
/// concreta, y no concede nada.
///
/// El `0` no es un error: es la respuesta correcta cuando nadie compone --
/// lanzado desde el shell de Ring 0--, y el programa que lo reciba se cae al
/// camino de la pantalla exclusiva.
pub const TASK_OP_MI_PADRE: u64 = 0x26;

/// **Abrir un archivo SIN esperar a que llegue entero.**
///
/// Misma ruta y mismo handle que [`TASK_OP_ARCHIVO_ABRIR`]; la diferencia es
/// **cuando vuelve**. `ABRIR` no vuelve hasta que el fichero esta en RAM, y con
/// un `.bex` de 813 KB eso deja al que lo pidio sin existir durante toda la
/// lectura -- si el que pide es el escritorio, el escritorio no pinta.
///
/// Este vuelve en cuanto sabe que el archivo esta ahi. Los bytes llegan a
/// trozos, y **preguntar por el archivo es lo que lo trae**: cada
/// [`ARCH_OP_LISTO`] avanza un trozo y vuelve a Ring 3, asi que entre trozo y
/// trozo el planificador puede dar el turno a otro.
pub const TASK_OP_ARCHIVO_ASINC: u64 = 0x27;

/// **Tomar lo que otro me haya ofrecido.** Devuelve un handle `KIND_PRESTADO`, o
/// `0` si no hay nada. El mapeo ocurre DENTRO de esta llamada, en el espacio de
/// quien la hace -- por eso se toma en vez de que el otro te lo coloque.
pub const TASK_OP_TOMAR: u64 = 0x1C;

pub const TASK_OP_AUDIO_CLAIM: u64 = 0x21;

/// Soltar el sonido siendo su dueno y **seguir vivo**.
///
/// Va desde el primer dia por lo que costo que faltara en la pantalla: alli la
/// unica forma de dejar de ser dueno era morir, asi que el escritorio no podia
/// prestarla ni queriendo. El mismo hueco aqui seria que el primer programa que
/// pite se queda el aparato para siempre.
pub const TASK_OP_AUDIO_RELEASE: u64 = 0x22;

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

/// **Despertar los otros nucleos.** Devuelve `alive<<32 | esperados`, ambos sin
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
