//! **THE OPERATION TABLE** -- the numbers, and nothing that runs.
//!
//! ```text
//!    [eje]     NINGUNO -- nothing in this file executes. Constants cost no
//!              cycles, no cache lines and no bytes at run time
//!    [camino]  P1 la puerta, at COMPILE time only
//!    [gen]     ABUELO -- raw facts. A number here means nothing by itself
//!    [exige]   R-FW1 (a shared constant, never a repeated literal),
//!              L4 (the guards that read this file are proven to say NO)
//! ```
//!
//! ** Declaring `[eje] NINGUNO` is not a formality: it is what stops somebody
//! from ever "optimising" this file. What is expensive here is being WRONG, not
//! being slow -- two operations with the same number compile fine and answer
//! something nobody asked for.
//!
//! === Why the numbers live apart from the code that serves them ===
//!
//! Because they are not an implementation detail: they are **the contract**.
//! Every one of these constants has a twin in `bmo-abi`, another in
//! `sem-asm/tables/bmo/bmo.h`, and a third in the userland runtime. The build
//! guard reads all of them and refuses to link if they disagree -- **105
//! operations kernel<->ABI and 75 userland<->ABI**, none by hand.
//!
//! ** Eran 49 hasta el 2026-08-17, y el salto no es porque hayan aparecido
//! cincuenta operaciones: es que el guardian solo miraba TRES familias
//! (`TASK_OP_*`, `ARCH_OP_*`, `SYSCALL_CLASS_*`) y un solo fichero de `obj\`.
//! Las demas --las de cada handle-- no las cruzaba nadie, y ahi vivia
//! `MEM_OP_OFRECER` con **dos valores distintos dentro del mismo kernel**.
//!
//! Mixed in with the dispatcher, a reader could not tell the frozen part from
//! the part that is free to change. Here the rule is visible: **a number in
//! this file is a promise; the code in `mod.rs` is how the promise is kept
//! today.**
//!
//! === And the two doors, since this is where somebody comes to count them ===
//!
//! There are TWO: `INVOKE` (0) and `WAIT` (2). The `1` is a reserved tombstone
//! -- `CHANNEL_KICK` was withdrawn on 2026-08-10 because it was not a door but
//! an OPERATION with a syscall number of its own. Waking a channel's consumer
//! is `CHANNEL_OP_KICK` and goes in through `INVOKE`, like everything else.
//!
//! The number is not recycled. An old binary that calls it fails saying so,
//! which is the only acceptable outcome: reusing it would make that binary do
//! something nobody asked for.


pub(crate) const NR_INVOKE: u32 = 0x00;
/// ** RETIRADO el 2026-08-10. El numero queda RESERVADO y no se reutiliza.
///
/// === Por que se fue ===
///
/// `CHANNEL_KICK(cap, secuencia)` hacia exactamente esto: resolver un handle,
/// comprobar que es un canal, y llamar a `channel::service`. O sea **una
/// operacion sobre un handle** -- que es la definicion de `INVOKE`. Tenia un
/// numero de syscall propio por como nacio, no por lo que hace.
///
/// Ahora es `CHANNEL_OP_KICK` sobre el canal, y la superficie baja de tres
/// puertas a dos con una frontera que se puede decir en una linea:
///
/// ```text
///   INVOKE   haz esto AHORA
///   WAIT     despiertame CUANDO
/// ```
///
/// Y esa frontera no es estetica: `WAIT` no se puede expresar con `INVOKE`
/// porque lo unico que hace es **no devolver el turno**, y una llamada sincrona
/// no puede decir eso sin mentir. Por eso quedan dos y no una.
///
/// === Por que el numero no se reutiliza ===
///
/// Un binario viejo que llame al 1 tiene que fallar **diciendolo**. Si el 1
/// pasara a significar otra cosa, ese mismo binario haria algo que nadie pidio y
/// no fallaria en ningun sitio -- la peor clase de rotura de ABI.
pub(crate) const NR_CHANNEL_KICK: u32 = 0x01;
pub(crate) const NR_WAIT: u32 = 0x02;
pub(crate) const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
pub(crate) const TASK_OP_GET_PID: u64 = 0x01;
pub(crate) const TASK_OP_GET_TID: u64 = 0x02;
pub(crate) const TASK_OP_YIELD: u64 = 0x03;
pub(crate) const TASK_OP_EXIT: u64 = 0x04;
pub(crate) const TASK_OP_CHANNEL_OPEN: u64 = 0x05;
pub(crate) const TASK_OP_CONSOLE_WRITE: u64 = 0x06;
/// Crea un endpoint atendido por este proceso. arg0 = estuario por el que se
/// le entregaran las llamadas. Devuelve el handle del endpoint.
pub(crate) const TASK_OP_ENDPOINT_CREATE: u64 = 0x07;
/// Pide el derecho a LLAMAR al endpoint `arg0`. Devuelve el handle de cliente.
///
/// Puerta de descubrimiento provisional, con el mismo aviso que lleva
/// `TASK_OP_CONSOLE_WRITE`: hoy cualquier proceso puede pedir cualquier
/// endpoint por su indice, y eso NO es disciplina de capabilities. Existe para
/// arrancar, y muere cuando haya un servicio de nombres que entregue el handle
/// a quien deba tenerlo. Se dice aqui para que nadie lo confunda con el
/// diseno final.
pub(crate) const TASK_OP_ENDPOINT_CONNECT: u64 = 0x08;
/// Reclamar la pantalla. Devuelve un handle `KIND_FRAMEBUFFER` y, con el, el
/// framebuffer mapeado en el espacio del proceso. Ver `ring0/fb.rs`: a partir
/// de aqui el kernel deja de dibujar y el proceso escribe pixeles con `mov`.
pub(crate) const TASK_OP_FRAMEBUFFER_CLAIM: u64 = 0x09;
/// Soltar la pantalla siendo su dueno y **seguir vivo**. Pareja de
/// `FRAMEBUFFER_CLAIM`.
///
/// `0x1D` elegido tras listar los opcodes ORDENADOS, que es la regla desde que
/// `MEMORIA_PEDIR` se puso en `0x12` --ya ocupado por `REINICIAR`-- y pedir
/// memoria habria reiniciado la maquina.
pub(crate) const TASK_OP_PANTALLA_SOLTAR: u64 = 0x1D;
/// Soltar la ENTRADA siendo su dueno y seguir vivo. Pareja de `INPUT_CLAIM`.
///
/// Va con `PANTALLA_SOLTAR` porque el caso de uso es el mismo y **separarlas fue
/// el bug**: prestar la pantalla sin la entrada dejo a `ray.bex` pintando sin
/// poder leer su propio ESC, y a la maquina sin teclado.
pub(crate) const TASK_OP_ENTRADA_SOLTAR: u64 = 0x1E;
/// Reclamar el raton. Devuelve un handle `KIND_INPUT`: el kernel lee el HID,
/// Ring 3 decide que hace con las coordenadas. Ver `ring0/input.rs`.
pub(crate) const TASK_OP_INPUT_CLAIM: u64 = 0x0A;
/// Acumula 8 bytes de ruta (LE, el cero corta) en el renglon del proceso.
///
/// Mismo formato que `TASK_OP_CONSOLE_WRITE`, y por la misma razon: los
/// argumentos van en registros y aqui no hay `copy_from_user`. Pasar un puntero
/// de Ring 3 obligaria al kernel a traducirlo contra el espacio del llamante y
/// a validar que el rango entero es suyo -- infraestructura que no existe todavia
/// y que no se va a improvisar en el camino de lanzar un programa. Ocho bytes
/// por llamada es feo y es seguro; lo segundo importa mas.
pub(crate) const TASK_OP_RUTA: u64 = 0x0B;
/// Lanza lo que se haya acumulado con `TASK_OP_RUTA` y vacia el renglon.
/// Devuelve el tid admitido. Ver `ring0/lanzar.rs` -- el gate de firma es el
/// mismo que el del shell, no una copia.
pub(crate) const TASK_OP_EJECUTAR: u64 = 0x0C;
/// Crea una consola y devuelve su handle de LECTURA. Quien la crea es el
/// terminal: la consola es suya y la drena a su ritmo. Ver `ring0/consola.rs`.
pub(crate) const TASK_OP_CONSOLA_CREAR: u64 = 0x0D;
/// Abre un directorio del volumen de datos y devuelve su handle. La ruta se
/// acumula antes con `TASK_OP_RUTA` -- el MISMO renglon que usa `EJECUTAR`, que
/// es lo que hace que no haga falta un segundo mecanismo para lo mismo.
pub(crate) const TASK_OP_DIR_ABRIR: u64 = 0x0E;
/// LEE de la consola asignada a este proceso. Devuelve `(n << 56) | bytes`.
///
/// La pareja de `TASK_OP_CONSOLE_WRITE`: el hijo escribe por una y escucha por
/// la otra, sobre el MISMO objeto. Es lo que permite un `ACCEPT` en un proceso
/// que no tiene --ni debe tener-- la capability del teclado: el terminal que lo
/// lanzo le pasa lo que se teclea.
pub(crate) const TASK_OP_CONSOLE_READ: u64 = 0x0F;
/// Abre un archivo del volumen de datos para LEER. La ruta se acumula antes
/// con `TASK_OP_RUTA` -- el MISMO renglon que `EJECUTAR` y que `DIR_ABRIR`.
/// Ver `ring0/archivo.rs`.
pub(crate) const TASK_OP_ARCHIVO_ABRIR: u64 = 0x10;
/// Pedir un bloque de memoria. Espejo de `bmo_abi::...::TASK_OP_MEMORIA_PEDIR`
/// -- el drift guard del build comprueba que los dos digan lo mismo.
pub(crate) const TASK_OP_MEMORIA_PEDIR: u64 = 0x15;
/// Igual, pero para ESCRIBIR. Son dos operaciones y no un argumento de modo
/// porque abrir para escribir puede fallar por motivos que abrir para leer no
/// tiene --volumen de solo lectura, nombre que no es 8.3-- y mezclarlas
/// obligaria a devolver errores que no aplican a la mitad de las llamadas.
pub(crate) const TASK_OP_ARCHIVO_CREAR: u64 = 0x11;
/// Abrir MI PROPIA imagen. Espejo de `bmo_abi::...::TASK_OP_MI_PAQUETE` -- el
/// drift guard del build comprueba que los dos digan lo mismo.
///
/// ** No lleva ruta, y esa es toda la diferencia con `ARCHIVO_ABRIR`: el
/// programa no dice CUAL, dice "el mio". Pedir el propio fichero por su ruta
/// seria pedir por nombre lo que se tiene por derecho -- y quien puede escribir
/// una ruta puede escribir otra.
pub(crate) const TASK_OP_MI_PAQUETE: u64 = 0x25;
/// **Quien me lanzo**, como TID. Espejo de `bmo_abi::...::TASK_OP_MI_PADRE`.
///
/// Una app dibuja en su memoria y se la OFRECE al que la puso en pantalla (ver
/// `<bmo/superficie.h>`). Ofrecer exige nombrar al destinatario, y el hijo no
/// tiene forma de nombrarlo: `MEM_OP_OFRECER` habla en tids y el tid del
/// compositor no aparece en ningun sitio de su espacio.
///
/// ** Y por eso NO es un registro de nombres. La pregunta no es *"quien manda"*
/// --eso seria autoridad ambiental, y el que la leyera podria pedirle cosas a
/// quien nunca se las ofrecio-- sino **"quien me lanzo a MI"**: una respuesta
/// local, concreta, y que no concede nada. Ver `ring0/task/familia.rs`.
///
/// Devuelve `0` si no hay padre --lanzado desde el shell de Ring 0-- y eso no es
/// un error: es la respuesta correcta a *"quien compone para mi"* cuando nadie
/// compone. El programa que la reciba se cae al camino de la pantalla
/// exclusiva, que es el degradado correcto.
pub(crate) const TASK_OP_MI_PADRE: u64 = 0x26;
/// **Abrir un archivo SIN esperar a que llegue entero.** Espejo de
/// `bmo_abi::...::TASK_OP_ARCHIVO_ASINC`.
///
/// Misma ruta y mismo handle que `ARCHIVO_ABRIR`; la diferencia es cuando
/// vuelve. `ABRIR` no vuelve hasta que el fichero esta en RAM --y con un `.bex`
/// de 813 KB eso es el que lo pidio sin existir durante toda la lectura--;
/// este vuelve en cuanto sabe que el archivo esta ahi.
///
/// El handle sale ademas con `RIGHT_WAIT`: se puede DORMIR sobre el hasta que
/// llegue el trozo siguiente. Ver `obj/archivo.rs::abrir_asinc`.
pub(crate) const TASK_OP_ARCHIVO_ASINC: u64 = 0x27;
/// Reinicia la maquina. No vuelve.
///
/// El reinicio de tres pasos (`0xCF9` -> 8042 -> triple fault) ya existia y solo
/// lo tenia el shell del kernel: la caja de Ring 3 contestaba "no lo conozco" a
/// `reboot`, y la unica salida era el boton. Reiniciar es tocar puertos de E/S,
/// que Ring 3 no puede --ni debe-- hacer; por eso es una operacion y no un
/// permiso ambiental.
///
/// **Limitacion declarada**: hoy no esta atada a una capability, igual que
/// `EJECUTAR`. Cualquier tarea de Ring 3 puede llamarla. Se apunta en CABINA
/// antes de reiniciar para que nunca sea silenciosa, y las dos operaciones
/// quieren la misma capability el dia que exista.
pub(crate) const TASK_OP_REINICIAR: u64 = 0x12;
/// Un dato numerico del sistema (`arg0` = campo) y uno de texto (`arg0` =
/// campo, `arg1` = trozo de 8 bytes). Ver `ring0/core/informe.rs`: leer cuanta
/// RAM hay no es un privilegio, es una pregunta.
pub(crate) const TASK_OP_INFO: u64 = 0x13;
pub(crate) const TASK_OP_INFO_TEXTO: u64 = 0x14;
/// El log del kernel, LEIDO desde Ring 3. `KLOG_INFO` cuantas hay
/// (`arg0` = 0 disponibles, 1 total), `KLOG_TEXTO` ocho bytes de una linea
/// (`arg0` = linea, **0 es la mas reciente**; `arg1` = trozo).
///
/// Mismo criterio que `INFO`: contesta texto y no concede nada. Ver
/// `ring0/core/klog.rs` -- existe porque desde que el escritorio es el arranque,
/// el panel del kernel no se pinta y el log no lo podia leer nadie.
pub(crate) const TASK_OP_KLOG_INFO: u64 = 0x16;
pub(crate) const TASK_OP_KLOG_TEXTO: u64 = 0x17;
/// La AUTOPSIA de un fallo de Ring 3. Ver `core::autopsy` y el `surface.rs`.
pub(crate) const TASK_OP_AUTOPSIA_INFO: u64 = 0x1F;
pub(crate) const TASK_OP_AUTOPSIA_TEXTO: u64 = 0x20;
/// **La primera operacion de la superficie que WRITES EN EL DISCO.** Cierra
/// una transaccion vacia en ESTRATOS y devuelve la generacion nueva, o 0.
/// Ver `ring0/fsys/estratos.rs::seal`.
pub(crate) const TASK_OP_ESTRATOS_SELLAR: u64 = 0x18;
/// El CURSOR de ESTRATOS: `arg0` la pregunta, `arg1` su argumento. Y los
/// nombres, de ocho en ocho.
///
/// Dos operaciones y no diez. `INFO_ES_*` ya contestaba *como esta* el almacen;
/// esto contesta **que hay dentro**, que es lo que la ventana de Datos no podia
/// ensenar porque `raiz`, `nodo`, `entries` y `entrada` eran funciones de
/// Ring 0 sin puerta. Mismo criterio que `INFO` y que el klog: contesta y no
/// concede -- aqui no hay una sola operacion que escriba.
pub(crate) const TASK_OP_ES_NODO: u64 = 0x19;
pub(crate) const TASK_OP_ES_TEXTO: u64 = 0x1A;
/// **ADMINISTRAR EL DISCO desde donde vive el dueno.** `arg0` = `DISCO_OP_*`.
///
/// La segunda operacion de la tabla que cambia el estado del almacen, y la
/// primera que se lo dice al APARATO en vez de al volumen. Ninguna de sus
/// ordenes acepta un LBA: el rango lo calcula el kernel y lo comprueba contra la
/// ventana de escritura. Ver `ring0/dev/disk/trim.rs`.
pub(crate) const TASK_OP_DISCO: u64 = 0x29;
/// **CREAR UN FICHERO EN ESTRATOS.** `arg0` = suborden (`ES_CREAR_*`).
///
/// La tercera operacion de la tabla que cambia el almacen, y la primera que
/// escribe CONTENIDO -- `sellar` commiteaba sin datos y el recorte le habla al
/// aparato. Espejo de `bmo_abi::...::TASK_OP_ES_GESTO`.
pub(crate) const TASK_OP_ES_GESTO: u64 = 0x2A;

/// El handle sobre un hijo que YO lance. Espejo de
/// `bmo_abi::...::TASK_OP_HIJO`. Solo BUSCA lo ya concedido en `EJECUTAR`.
pub(crate) const TASK_OP_HIJO: u64 = 0x2B;
/// Las subordenes. Espejo de `bmo_abi::...::ES_CREAR_*`, y `pub(crate)` por lo
/// mismo que las de disco: una constante privada usada en un `match` de
/// `mod.rs` se convierte en un nombre de variable que se traga todos los casos.
pub(crate) const ES_GESTO_LIMPIAR: u64 = 0x00;
pub(crate) const ES_GESTO_DATOS: u64 = 0x01;
pub(crate) const ES_GESTO_FICHERO: u64 = 0x02;
/// Crea una carpeta vacia donde diga la ruta.
pub(crate) const ES_GESTO_CARPETA: u64 = 0x03;
/// Quita la entrada que diga la ruta. **No destruye**: deja de nombrar.
pub(crate) const ES_GESTO_QUITAR: u64 = 0x04;
/// Renombra la entrada que diga la ruta. El nombre NUEVO va por el renglon del
/// contenido -- es el unico verbo que necesita dos nombres.
pub(crate) const ES_GESTO_RENOMBRAR: u64 = 0x05;
/// Trae un fichero de FAT32. La ruta lleva el DESTINO y el renglon del
/// contenido lleva el ORIGEN -- el contenido de verdad no cruza la puerta.
pub(crate) const ES_GESTO_COPIA: u64 = 0x06;
/// Marca la version en curso con el nombre que traiga la ruta. Un nombre es lo
/// que hace PERMANENTE a una version: el recolector no la suelta jamas.
pub(crate) const ES_GESTO_MARCAR: u64 = 0x07;
/// Vuelve a la version `arg1` pasos atras. No copia nada: publica un estrato
/// que apunta a la MISMA raiz que aquella.
pub(crate) const ES_GESTO_VOLVER: u64 = 0x08;
/// Anota DE DONDE sale el contenido: `arg1` es el handle de un bloque de
/// `KIND_MEMORIA` propio y los bits altos de `arg0` el desplazamiento. No lee ni
/// escribe nada -- lo ejecuta `ES_GESTO_FICHERO_DE`.
pub(crate) const ES_GESTO_ORIGEN: u64 = 0x09;
/// Crea un fichero con los `arg1` bytes del bloque anotado. Es a este renglon lo
/// que `ARCH_OP_ESCRIBIR_DE` es al de FAT32: el contenido no viaja, viaja donde
/// esta. Sin esto, meter mas de 96 bytes obliga a pasar por FAT32.
pub(crate) const ES_GESTO_FICHERO_DE: u64 = 0x0A;
/// Guarda el contenido del bloque anotado: lo crea, o publica su version nueva.
/// El quinto verbo -- el unico que versiona un FICHERO y no solo el arbol.
pub(crate) const ES_GESTO_GUARDAR: u64 = 0x0B;
pub(crate) const ES_GESTO_MAX: u64 = 96;
/// Las ordenes del disco. Espejo de `bmo_abi::...::DISCO_OP_*`.
///
/// `pub(crate)` por correccion y no por estilo -- ver la nota de las `ES_NODO_*`
/// mas abajo: una constante privada usada en un `match` de `mod.rs` no falla, se
/// convierte en un nombre de variable que se traga todos los casos.
pub(crate) const DISCO_OP_TRIM_LIBRE: u64 = 0x01;
pub(crate) const DISCO_OP_BARRERA: u64 = 0x02;
/// Los motivos que viajan en el byte alto de la respuesta. Espejo de
/// `bmo_abi::...::DISCO_TRIM_*` -- un cero aqui y otro alli no son el mismo cero
/// si alguien los desincroniza, y el sintoma seria un terminal que dice
/// "recortado" cuando el disco dijo que no.
pub(crate) const DISCO_TRIM_HECHO: u64 = 0;
pub(crate) const DISCO_TRIM_SIN_DISCO: u64 = 1;
pub(crate) const DISCO_TRIM_NO_SOPORTADO: u64 = 2;
pub(crate) const DISCO_TRIM_SIN_PERMISO: u64 = 3;
pub(crate) const DISCO_TRIM_SIN_VOLUMEN: u64 = 4;
pub(crate) const DISCO_TRIM_RANGO: u64 = 5;
pub(crate) const DISCO_TRIM_FALLO: u64 = 6;
pub(crate) const DISCO_TRIM_MOTIVO_SHIFT: u64 = 56;
pub(crate) const DISCO_TRIM_SECTORES_MASK: u64 = (1 << 56) - 1;
/// Despertar los otros nucleos. Espejo de `bmo_abi::...::TASK_OP_SMP_DESPERTAR`.
pub(crate) const TASK_OP_SMP_DESPERTAR: u64 = 0x1B;
/// **El censo de audio, pedido desde Ring 3.**
///
/// El 2026-08-12 la orden `audio` se anadio SOLO al shell de Ring 0, y el dueno
/// la escribio en el compositor -- que tiene su propia lista. Contesto
/// *"no es un comando ni una ruta"* y la prueba del paso 0 se quedo sin hacer.
///
/// Dos shells con dos vocabularios distintos son dos productos, y el que se usa
/// todos los dias es el de Ring 3.
pub(crate) const TASK_OP_AUDIO_CENSO: u64 = 0x28;
/// Tomar lo que otro proceso me haya ofrecido. Espejo de `...::TASK_OP_TOMAR`.
pub(crate) const TASK_OP_TOMAR: u64 = 0x1C;
/// **Reclamar el SONIDO.** Devuelve un handle `KIND_AUDIO`: el derecho a hacer
/// ruido, exclusivo como la pantalla. Ver `ring0/obj/audio.rs` -- es el
/// CONTRATO, no un driver: lo unico que suena hoy es el altavoz del PC.
pub(crate) const TASK_OP_AUDIO_CLAIM: u64 = 0x21;
/// Soltar el sonido siendo su dueno y seguir vivo. Va desde el primer dia por
/// lo que costo que faltara en la pantalla: sin esto, el primero que pite se
/// queda el aparato hasta que muera.
pub(crate) const TASK_OP_AUDIO_RELEASE: u64 = 0x22;
/// **CABINA a Ring 3.** Lo que el kernel ve, con su SEVERIDAD y su capa -- que
/// es lo que el klog no lleva. Contesta y no concede: ni una operacion escribe.
pub(crate) const TASK_OP_CABINA_INFO: u64 = 0x23;
pub(crate) const TASK_OP_CABINA_TEXTO: u64 = 0x24;
/// Ofrecer un trozo del bloque propio. Es una operacion sobre `KIND_MEMORIA`.
///
/// ** Y ES LA UNICA COPIA, desde el 2026-08-17. Habia otra dentro de `mod.rs`
/// que decia `0x02` --o sea `MEM_OP_BYTES`-- y era la que usaba el despacho:
/// prestar memoria no llegaba a su brazo y preguntar el tamano de un bloque
/// entraba en el de prestar. Ninguna de las dos fallaba en voz alta.
pub(crate) const MEM_OP_OFRECER: u64 = 0x03;
// ** ESTAS CONSTANTES SON `pub(crate)` POR CORRECCION, NO POR ESTILO.
//
// === La trampa, que no da error de compilacion ===
//
// El despachador vive en `mod.rs`, o sea **fuera** de este modulo, y las mira en
// un `match`. Una constante privada aqui no es visible alli -- y Rust no dice
// "no existe": la trata como **una variable nueva que se ata a lo que venga**.
// O sea que el primer brazo del `match` se traga TODOS los valores y los demas
// quedan muertos, con un aviso de `unreachable_patterns` perdido entre otros.
//
// Se cazo el 2026-08-17 al escribir `TASK_OP_DISCO`, comprobandolo con un
// programa de tres lineas en vez de razonarlo. Las doce del cursor de ESTRATOS
// llevaban asi desde que se escribieron: `ES_NODO_RAIZ` era el brazo que
// contestaba a todo, y por eso **el arbol de la ventana F12 no podia tener
// hijos** -- `ES_NODO_HIJOS` no llegaba a ejecutarse nunca.
//
// > Un opcode que no compara es peor que uno duplicado: el duplicado lo caza el
// > guardian del build, y esto no lo caza nadie porque a los ojos del compilador
// > es un nombre de variable perfectamente legal.
//
/// Las preguntas del cursor. Espejo de `bmo_abi::...::ES_NODO_*`.
pub(crate) const ES_NODO_RAIZ: u64 = 0x00;
pub(crate) const ES_NODO_HIJOS: u64 = 0x01;
pub(crate) const ES_NODO_TRUNCADO: u64 = 0x02;
pub(crate) const ES_NODO_HONDO: u64 = 0x03;
pub(crate) const ES_NODO_TIPO: u64 = 0x04;
pub(crate) const ES_NODO_HIJO_TIPO: u64 = 0x05;
pub(crate) const ES_NODO_ENTRAR: u64 = 0x06;
pub(crate) const ES_NODO_SUBIR: u64 = 0x07;
pub(crate) const ES_NODO_HIJO_BYTES: u64 = 0x08;
pub(crate) const ES_NODO_HIJO_ATRIBUTOS: u64 = 0x09;
pub(crate) const ES_NODO_HIJO_FIRMADO: u64 = 0x0A;
pub(crate) const ES_NODO_VERIFICAR: u64 = 0x0B;
// -- ** Las tres del ARBOL: preguntar por un nivel que NO es donde estas ----
//
// Reciben el nivel en `arg1`, y las que ademas necesitan un hijo lo empaquetan:
// **`arg1 = (nivel << 32) | indice`**. Se reparte el argumento en vez de anadir
// una operacion por combinacion, que es la misma decision que ya tomo
// `ES_TEXTO` con sus bits altos.
pub(crate) const ES_NODO_NIVEL_HIJOS: u64 = 0x0C;
pub(crate) const ES_NODO_NIVEL_HIJO_TIPO: u64 = 0x0D;
pub(crate) const ES_NODO_NIVEL_ELEGIDO: u64 = 0x0E;
/// **Relee el arbol y deja el cursor donde estaba.** Se manda despues de
/// escribir: la pila guarda el estrato de antes y sin esto seguiria
/// ensenandolo.
pub(crate) const ES_NODO_RECARGAR: u64 = 0x0F;
/// -- LA HISTORIA: la cadena de versiones hacia atras --
///
/// `RELEER` es la unica que toca el disco --un bloque por version-- y por eso
/// se pide a mano. Las otras contestan de lo que aquella dejo guardado.
pub(crate) const ES_HIST_RELEER: u64 = 0x10;
pub(crate) const ES_HIST_CUANTAS: u64 = 0x11;
pub(crate) const ES_HIST_RECORTADA: u64 = 0x12;
pub(crate) const ES_HIST_CUANDO: u64 = 0x13;
pub(crate) const ES_HIST_QUIEN: u64 = 0x14;
pub(crate) const ES_HIST_CON_NOMBRE: u64 = 0x15;
/// Que texto pide `ES_TEXTO`, en los bits altos de `arg0`. Espejo de
/// `bmo_abi::...::ES_TXT_*`.
pub(crate) const ES_TXT_RUTA: u64 = 1;
/// El nombre de un hijo de CUALQUIER nivel. Los bits bajos de `arg0` llevan dos
/// numeros: `(nivel << 16) | indice`.
pub(crate) const ES_TXT_NIVEL_HIJO: u64 = 2;
/// El nombre de la version `indice` de la historia.
pub(crate) const ES_TXT_HIST_NOMBRE: u64 = 3;
pub(crate) const CHANNEL_OP_GET_SEQ: u64 = 0x01;
pub(crate) const CHANNEL_OP_GET_INDEX: u64 = 0x02;
/// **Avisar al consumidor.** Era el syscall numero 1; ahora es una operacion
/// sobre el handle del canal, como todo lo demas. Ver `NR_CHANNEL_KICK`.
pub(crate) const CHANNEL_OP_KICK: u64 = 0x03;
pub(crate) const ERROR_INVALID_ARGUMENT: u32 = 7;
pub(crate) const ERROR_UNSUPPORTED: u32 = 10;

#[repr(C)]
pub(crate) struct BmoStatus {
    pub code: u32,
    pub flags: u32,
    pub value: u64,
}

impl BmoStatus {
    pub(crate) const fn ok_value(value: u64) -> Self { Self { code: 0, flags: 0, value } }
    pub(crate) const fn err(code: u32) -> Self { Self { code, flags: 0, value: 0 } }
    pub(crate) const fn err_with_flags(code: u32, flags: u32) -> Self { Self { code, flags, value: 0 } }
}

const _: () = assert!(core::mem::size_of::<BmoStatus>() == 16);

pub(crate) const MSR_STAR: u32 = 0xC000_0081;
pub(crate) const MSR_LSTAR: u32 = 0xC000_0082;
pub(crate) const MSR_SFMASK: u32 = 0xC000_0084;
pub(crate) const RFLAGS_TF: u64 = 1 << 8;
pub(crate) const RFLAGS_IF: u64 = 1 << 9;
pub(crate) const RFLAGS_DF: u64 = 1 << 10;
pub(crate) const RFLAGS_NT: u64 = 1 << 14;
pub(crate) const RFLAGS_AC: u64 = 1 << 18;
pub(crate) const KERNEL_CS: u64 = 0x08;

// El ultimo selector que estaba escrito dos veces. Este alimenta `MSR_STAR`
// --lo que `syscall` carga al ENTRAR-- y el de `plat::trap` alimenta la GDT y
// las comprobaciones del epilogo. Eran el mismo numero en dos ficheros que no
// se hablaban, que es exactamente la forma del `#GP(0x18)` del 16-08.
const _: () = assert!(
    KERNEL_CS == crate::ring0::plat::trap::KERNEL_CS,
    "el CS que carga `syscall` no es el CS de la GDT"
);

/// La base de selectores que `sysretq` usa al volver a Ring 3.
///
/// # ** POR QUE 0x13 Y NO 0x10, QUE ES LO QUE PARECE
///
/// `sysret` no lee estos selectores de la pila: los CALCULA sumando a esta
/// base, y **cada mitad se comporta distinto en AMD**:
///
/// ```text
///    CS = base + 16, y el CPU le fuerza RPL 3    (CPL pasa a ser 3)
///    SS = base +  8, y AMD NO le fuerza NADA     (APM: "SS.sel <- SYSRET_CS+8")
/// ```
///
/// Con `base = 0x10` --que es lo que llevaba armado desde siempre, con un
/// comentario que decia *"legacy, el camino de salida es iretq"*-- salia
/// `CS = 0x23` (bien, el CPU le puso el RPL) y **`SS = 0x18`: el indice
/// correcto con RPL 0**. Y eso no revienta donde se hace.
///
/// == LO QUE COSTO, porque la forma del fallo es la leccion ==
///
/// La tarea volvia a Ring 3 con `SS.RPL = 0` y seguia funcionando: mientras el
/// CPL manda, nadie revalida SS. Hasta que **el timer la interrumpia** y el CPU
/// empujaba ese `0x18` al marco del trap. El epilogo del timer hacia `iretq`, y
/// `iretq` a un privilegio menor EXIGE `RPL(SS) == RPL(CS)`:
///
/// ```text
///    #GP  err=0x18  rip=0x4001BA (el `iretq` de `irq::disco_entry`)
///    marco:  cs=0x0023 (RPL 3)   ss=0x0018 (RPL 0)
/// ```
///
/// O sea: **el fallo aparecia en un stub que nadie habia tocado, disparado por
/// una interrupcion, por culpa de un registro que se cargo mal en otro sitio y
/// varios milisegundos antes.** Se encontro por el `err=0x18` de la pantalla de
/// parada -- un `#GP` trae el SELECTOR en el codigo de error, y ese 0x18 era el
/// dedo apuntando.
///
/// `0x13` es el mismo indice con el RPL 3 **ya metido en la base**: como `+8` y
/// `+16` no tocan los bits 0-1, el RPL sobrevive a la suma. Y en un CPU que si
/// haga `OR 3` sigue saliendo bien, porque ya lo trae puesto.
pub(crate) const SYSRET_SELECTOR_BASE: u64 = 0x13;

// == THE COST CLASSES ====================================================
//
// Not what a door costs -- **which door happened**. The costs are already
// measured (~875 task, ~1125 handle, ~2,2 M console); what was missing is how
// often each one is asked, and without that the costs cannot be turned into a
// share of the machine.
//
// ** ALL FOUR ARE DECIDED BY CONSTRUCTION. The first sketch split "cheap
// operation" from "operation that walks a table", and that needs a hand-written
// list of operations -- the exact thing that froze twice in this tree, thirty
// lines above. These four come from facts the dispatcher already holds in
// registers: which door, whether the handle was `CURRENT_TASK`, and whether the
// operation is the console one (which has had its own branch from day one).
//
// The meaning of each index is the ABI's; the meter only counts. See
// `META-KERNEL_HARD.md` L7 -- the counter is the grandfather and must not learn
// what an operation is.
pub(crate) const SYSCALL_CLASS_TASK: u64 = 0x00;
pub(crate) const SYSCALL_CLASS_HANDLE: u64 = 0x01;
pub(crate) const SYSCALL_CLASS_CONSOLE: u64 = 0x02;
pub(crate) const SYSCALL_CLASS_WAIT: u64 = 0x03;
pub(crate) const SYSCALL_CLASS_COUNT: u64 = 0x04;

// ** Y LA RELACION SE COMPRUEBA, igual que con los selectores de abajo: el
// histograma tiene tantas casillas como clases, y la ultima clase es la ultima
// casilla. Escrito como assert y no como comentario porque anadir una clase
// quinta sin ampliar el array daria un contador que se descarta en silencio --
// y un contador que se pierde no se nota, que es justo lo que este fichero
// existe para impedir.
const _: () = assert!(
    SYSCALL_CLASS_COUNT as usize == super::meter::CLASS_COUNT,
    "el histograma del metro tiene otro numero de casillas que clases hay"
);
const _: () = assert!(
    SYSCALL_CLASS_WAIT + 1 == SYSCALL_CLASS_COUNT,
    "la ultima clase tiene que ser la ultima casilla: hay un hueco o un sobrante"
);

// ** Y LA RELACION SE COMPRUEBA, que es el arreglo de verdad.
//
// El numero de arriba solo es correcto porque `base+8` y `base+16` caen
// exactamente en los selectores que el prologo empuja como constantes. Eso era
// una coincidencia que funcionaba, y una coincidencia que funciona es la unica
// clase de error que nadie revisa. Aqui deja de serlo: si alguien mueve la GDT
// o toca `USER_CS`/`USER_SS`, **esto no compila** en vez de arrancar y morir en
// el `iretq` de un stub que no tiene nada que ver.
const _: () = assert!(
    SYSRET_SELECTOR_BASE + 8 == crate::ring0::plat::trap::USER_SS,
    "sysret dejaria un SS distinto del que empuja el prologo"
);
const _: () = assert!(
    SYSRET_SELECTOR_BASE + 16 == crate::ring0::plat::trap::USER_CS,
    "sysret dejaria un CS distinto del que empuja el prologo"
);
