//! **Las operaciones sobre los OTROS handles.** Todo lo que no es la tarea.
//!
//! ```text
//!    ARCH_OP_*       un fichero abierto
//!    CHANNEL_OP_*    un canal
//!    MEM_OP_*        un bloque de KIND_MEMORIA
//!    AUDIO_OP_*      el aparato de sonido
//!    LIENZO_*        la superficie que se pinta
//!    PRESTADO_OP_*   un trozo de memoria de otro proceso
//!    ES_*            el cursor de ESTRATOS
//! ```
//!
//! Estan juntas porque comparten la forma --`INVOKE(handle, op, ...)`-- y no el
//! tema. Es la unica categoria de este contrato agrupada por COMO se llaman y
//! no por que hacen, y conviene decirlo: el dia que una de estas familias crezca
//! como crecio `TASK_OP_*`, se saca a su propio fichero.

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

// == ** EL ARBOL: preguntar por un nivel que NO es donde esta el cursor =====
//
// El cursor guarda la ruta desde la raiz, y desde 2026-08-18 cada nivel de esa
// ruta **se queda con su listado**. Estas tres son la puerta a esos listados.
//
// Sirven para lo que un panel de arbol necesita y una lista no: ensenar a la
// vez los hijos de la raiz, los del nivel siguiente y los del siguiente, con la
// rama abierta marcada. **No son un segundo cursor** -- son el mismo recorrido
// preguntado a otra profundidad, y por eso no piden handle, tabla ni
// revocacion.
//
// Ninguna toca el disco: contestan de lo que se leyo al pasar.

/// Cuantos hijos tiene el nivel `arg1`. `0` si ese nivel no existe todavia.
pub const ES_NODO_NIVEL_HIJOS: u64 = 0x0C;

/// El tipo del hijo de un nivel: `0` archivo, `1` directorio, `2` no hay.
///
/// `arg1` lleva los dos numeros: **`(nivel << 32) | indice`**.
pub const ES_NODO_NIVEL_HIJO_TIPO: u64 = 0x0D;

/// Por que hijo se bajo desde el nivel `arg1`, o `u64::MAX` si por ninguno.
///
/// * El valor imposible NO es un capricho: **cero es un hijo perfectamente
/// valido** --el primero--, asi que "por ninguno" no se puede decir con un
/// cero sin que el nivel donde acaba la rama se confunda con uno abierto por
/// su primer hijo.
pub const ES_NODO_NIVEL_ELEGIDO: u64 = 0x0E;

/// Que texto pide [`TASK_OP_ES_TEXTO`], en los bits altos de `arg0`.
///
/// Los bajos siguen siendo el indice. Se reparte el argumento en vez de anadir
/// una operacion porque **son el mismo mecanismo** --sacar un nombre de ocho en
/// ocho-- pidiendo dos cosas distintas, y una puerta por cada texto que devuelva
/// el sistema es como una superficie de dos syscalls acaba teniendo cuarenta.
pub const ES_TXT_HIJO: u64 = 0;

/// El nombre del nivel `indice` de la ruta. `0` es la raiz y contesta vacio.
pub const ES_TXT_RUTA: u64 = 1;

/// El nombre de un hijo de CUALQUIER nivel, para el panel de arbol.
///
/// Los bits bajos de `arg0` llevan aqui **dos** numeros: `(nivel << 16) |
/// indice`. Caben de sobra --como mucho dieciseis niveles y sesenta y cuatro
/// hijos-- y ahorran una tercera puerta para el mismo mecanismo de siempre:
/// sacar un nombre de ocho en ocho.
pub const ES_TXT_NIVEL_HIJO: u64 = 2;

// == ** LAS ONCE QUE FALTABAN EN EL CONTRATO (2026-08-17) ===================
//
// Tres del DIRECTORIO, cuatro de la CONSOLA y cuatro del FRAMEBUFFER. Las once
// llevaban desde que existen sirviendose en el kernel (`ring0/obj/*.rs`) y
// mandandose desde el userland con los mismos numeros -- pero **aqui no
// estaban**. O sea que el contrato no las conocia y ningun guardian podia
// compararlas: el mismo hueco que el 2026-08-12 dejo fuera a
// `ENDPOINT_CONNECT`, `PANTALLA_SOLTAR`, `ENTRADA_SOLTAR` y `AUDIO_CENSO`.
//
// Aparecieron al ensanchar el guardian de `build.ps1` para que barra TODAS las
// familias de operacion --y los cinco ficheros de `obj\`, no uno-- despues de
// que ese mismo ensanchamiento cazara el fallo de verdad: `MEM_OP_OFRECER`
// tenia dos copias en el kernel con numeros distintos.
//
// ** Los numeros son los que ya se usan en los dos lados: esto no cambia nada
// que corra. Cierra el hueco por el que no se miraban.
//
// > La superficie sigue siendo de DOS puertas. Estas no son syscalls: son
// > operaciones sobre handles que alguien concedio, y por eso viven aqui.

/// Avanza a la siguiente entrada del directorio:
/// `(hay << 63) | (es_dir << 62) | tamano`. `hay == 0` = se acabo.
///
/// El nombre NO viaja aqui --son 11 bytes y no caben con los demas campos-- y
/// se pide aparte con [`DIR_OP_NOMBRE`]. Es la misma decision que la consola:
/// un contador honesto vale mas que un byte apretado.
pub const DIR_OP_SIGUIENTE: u64 = 0x01;

/// Los 11 bytes del nombre 8.3 de la entrada ACTUAL, de 7 en 7. `arg0` es el
/// desplazamiento (0 o 7) y devuelve `(n << 56) | bytes_LE`.
///
/// Salen en 8.3 CRUDO --`COBOL   BEX`, con sus espacios-- porque convertirlo a
/// `COBOL.BEX` es presentacion, y la presentacion es de Ring 3.
pub const DIR_OP_NOMBRE: u64 = 0x02;

/// **Cerrar, y devolver la ranura.** Solo caben ocho directorios abiertos a la
/// vez, asi que quien no cierra se los come. Lo llama el `Drop` del userland.
pub const DIR_OP_CERRAR: u64 = 0x03;

// -- Las cuatro de la CONSOLA (`KIND_CONSOLA`) ------------------------------

/// Leer hasta **7** bytes de la salida del hijo: `(n << 56) | bytes_LE`.
/// `n == 0` = no hay nada.
///
/// ** Siete y no ocho, y el contador ARRIBA. Ocho bytes ocupan el `u64` entero
/// y no dejan sitio para decir cuantos valen: la primera version pisaba el
/// byte 4 y sacaba uno de cada ocho corrupto, en una ruta que solo se nota
/// leyendo texto raro. Se paga un byte de ancho de banda por un contador
/// honesto.
pub const CONSOLA_OP_LEER: u64 = 0x01;

/// Cuantos bytes se descartaron por anillo lleno. Un terminal que va lento
/// tiene derecho a saber que esta perdiendo salida en vez de creerse completo.
pub const CONSOLA_OP_PERDIDOS: u64 = 0x02;

/// El TERMINAL mete 8 bytes (LE, el cero corta) en el anillo de ENTRADA.
///
/// Es el segundo sentido del canal, y sin el no hay `ACCEPT`: un programa
/// lanzado desde la caja no puede reclamar `KIND_INPUT` --la tiene el
/// compositor-- asi que su unica via para recibir teclas es el mismo objeto por
/// el que ya habla. Un canal de un solo sentido deja al hijo mudo de oido.
pub const CONSOLA_OP_ESCRIBIR: u64 = 0x03;

/// Hay algun proceso escribiendo a esta consola ahora mismo?
///
/// Lo pregunta el terminal para saber a donde va lo que se teclea: si hay hijo
/// vivo, la linea es PARA EL; si no, es un comando. Sin esto habria que
/// inventar un prefijo o un modo, y las dos cosas se olvidan.
pub const CONSOLA_OP_HAY_HIJO: u64 = 0x04;

// -- Las cuatro del FRAMEBUFFER (`KIND_FRAMEBUFFER`) ------------------------
//
// Cada una devuelve UN `u64`, que es lo que cabe en `BmoStatus.value`. Los
// campos que van juntos viajan empaquetados en vez de gastar una llamada por
// numero: se leen una vez, al arrancar el compositor.

/// Direccion virtual, **en el espacio del proceso**, donde quedo mapeada.
pub const FB_OP_BASE: u64 = 0x01;

/// `(ancho << 32) | alto`, en pixeles.
pub const FB_OP_DIMS: u64 = 0x02;

/// `(stride << 32) | formato`. ** El stride va en PIXELES, no en bytes: es el
/// mismo numero que usa el kernel, y convertirlo en la frontera seria inventar
/// una unidad distinta a cada lado.
pub const FB_OP_STRIDE: u64 = 0x03;

/// Bytes mapeados en total. Es lo que hace falta para llenar la pantalla entera
/// con un `rep stosd` sin multiplicar nada.
pub const FB_OP_BYTES: u64 = 0x04;

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

/// **Escribe un bloque entero** desde una capability de memoria. El espejo
/// exacto de [`ARCH_OP_LEER_EN`]: `arg0` = handle del bloque, `arg1` = offset
/// dentro de el, `arg2` = cuantos bytes. Devuelve cuantos entraron.
///
/// Existe por lo mismo que su espejo. `ARCH_OP_ESCRIBIR` mete siete bytes por
/// llamada; guardar una partida de DOOM son cientos de KiB, o sea decenas de
/// miles de llamadas para mover algo que cabe en una copia. Y tampoco pide
/// validar punteros: el origen es un bloque que concedio el kernel.
pub const ARCH_OP_ESCRIBIR_DE: u64 = 0x08;

/// Bytes que quedan por leer, o los acumulados si es de escritura.
pub const ARCH_OP_TAMANO: u64 = 0x03;

/// Cierra. En uno de escritura **es donde el contenido llega al disco**.
pub const ARCH_OP_CERRAR: u64 = 0x04;

/// `(entero << 63) | bytes que ya llegaron`. Avanza la carga y contesta.
///
/// Los dos datos van juntos a proposito: *"cuanto hay"* y *"queda mas"* son la
/// misma pregunta, y contestarlas por separado abre la puerta a leerlas de
/// vueltas distintas.
pub const ARCH_OP_LISTO: u64 = 0x09;

/// **Ofrecer un trozo del bloque propio.** Operacion sobre `KIND_MEMORIA`:
/// `arg0` = desde (contra la base del bloque), `arg1` = bytes, `arg2` = el TID
/// del destinatario. Solo el puede tomarlo.
pub const MEM_OP_OFRECER: u64 = 0x03;

/// Donde quedo lo prestado, en MI espacio.
pub const PRESTADO_OP_BASE: u64 = 0x01;

/// Cuantos bytes son.
pub const PRESTADO_OP_BYTES: u64 = 0x02;

/// **El TID de quien me lo presto, o `0` si ya no vive.** Es el detector de vida
/// de una ventana: componer la memoria de otro proceso sin poder preguntar si
/// sigue ahi seria no distinguir una app muerta de una app pensando.
pub const PRESTADO_OP_DUENO: u64 = 0x03;

/// **Devolverlo**: se desmapea de mi espacio y la ranura queda libre. Sin esto,
/// abrir y cerrar ventanas agota las ranuras de prestamo hasta reiniciar.
pub const PRESTADO_OP_SOLTAR: u64 = 0x04;

/// Operaciones sobre un handle de sonido.
///
/// `AUDIO_OP_DEVICES` existe para **preguntar en vez de suponer**: contesta una
/// mascara (`DEVICE_SPEAKER`, `DEVICE_HDA`) y el dia que exista HDA el mismo
/// binario se entera sin recompilarse.
///
/// [!] Un bit puesto dice que hay CAMINO, no que se oiga: el puerto del altavoz
/// existe en todo x86 y el zumbador fisico no.
pub const AUDIO_OP_DEVICES: u64 = 0x01;

/// Pitar. `arg0` = Hz, `arg1` = ms. Devuelve los ms que de verdad sonaron.
///
/// **Bloquea mientras dura**, y por eso hay tope: el altavoz del PC no tiene
/// interrupcion que avise de que el tono acabo, asi que el nucleo se queda en un
/// bucle de espera. Sin el tope, un programa de Ring 3 para el planificador el
/// tiempo que quiera.
pub const AUDIO_OP_BEEP: u64 = 0x02;

/// Volumen global, `arg0` de 0 a 100. En el altavoz del PC son **dos
/// escalones**, no cien: el volumen es el modo del temporizador.
pub const AUDIO_OP_VOLUME: u64 = 0x03;

/// Callar ahora mismo.
pub const AUDIO_OP_SILENCE: u64 = 0x04;

/// Hay altavoz de PC (el puerto que lo controla; ver la nota de
/// [`AUDIO_OP_DEVICES`]).
pub const DEVICE_SPEAKER: u64 = 1 << 0;

/// Hay HD Audio con su codec abierto. **Hoy siempre 0.**
pub const DEVICE_HDA: u64 = 1 << 1;

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

/// Donde empieza el bloque, en el espacio del proceso que lo pidio.
pub const MEM_OP_BASE: u64 = 0x01;

/// Cuantos bytes se le han entregado en total a este proceso.
pub const MEM_OP_BYTES: u64 = 0x02;

/// `INVOKE` operations accepted by a channel (estuary) capability.
pub const CHANNEL_OP_GET_SEQ: u64 = 0x01;

pub const CHANNEL_OP_GET_INDEX: u64 = 0x02;

/// **Avisar al consumidor.** Era el syscall numero 1 -- ver [`NR_CHANNEL_KICK`].
///
/// Pide `RIGHT_WRITE` y no `RIGHT_READ`, al reves que las dos de arriba, y esa
/// diferencia es la que habria que perder para meterlo con ellas: **avisar es
/// escribir**. Quien solo puede leer la secuencia no puede empujarla.
pub const CHANNEL_OP_KICK: u64 = 0x03;
