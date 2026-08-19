//! **Lo que se le pide AL DISCO.** Los `DISCO_OP_*` y sus motivos.
//!
//! Es la sexta familia del contrato, y nace con fichero propio por lo que es y
//! no por lo que mide: **es la unica que ACTUA sobre el almacen**. `informe`
//! contesta preguntas, `entrada` cuenta hechos fisicos, `objetos` opera sobre
//! handles que alguien concedio... y esto **le da ordenes al aparato donde vive
//! el trabajo del dueno**.
//!
//! # Por que hay motivos y no un booleano
//!
//! Porque un `0` obligaria a adivinar cual de las cinco puertas dijo que no, y
//! son cinco conversaciones distintas: *"este disco no sabe"* es una propiedad
//! del aparato, *"no esta armado"* es un estado que se puede ganar, *"fuera de
//! la ventana"* es un bug del que llama, y *"el disco fallo"* es hardware.
//!
//! La respuesta viaja empaquetada porque por la puerta cabe **un** numero:
//!
//! ```text
//!   (motivo << 56) | sectores
//!
//!   motivo 0 = HECHO, y entonces `sectores` es lo que se recorto de verdad
//!   motivo > 0 = no se hizo (o se hizo a medias, ver DISCO_TRIM_FALLO)
//! ```

/// **Devolverle al disco la cola libre del volumen ESTRATOS.**
///
/// Sin argumentos: **el rango no lo elige quien llama**. Lo calcula el kernel a
/// partir de `log_head` --el puntero que solo avanza-- y lo comprueba contra la
/// ventana de escritura. Un TRIM con LBA a gusto del llamante seria una orden de
/// borrado apuntable a cualquier sector desde Ring 3, y eso no es una operacion:
/// es un agujero.
pub const DISCO_OP_TRIM_LIBRE: u64 = 0x01;

/// **`FLUSH CACHE` a mano.** Devuelve 1 si el disco confirmo.
///
/// Existe porque este disco declara `SOLO_BARRERA`: no tiene condensadores, asi
/// que la barrera es lo unico que separa "el disco se quedo los bytes" de "los
/// bytes sobrevivirian a un corte". Poder pedirla desde donde se trabaja es lo
/// que hace comprobable esa frase.
pub const DISCO_OP_BARRERA: u64 = 0x02;

// -- Los motivos, en el byte alto de la respuesta ---------------------------

/// Se hizo. `sectores` dice cuantos.
pub const DISCO_TRIM_HECHO: u64 = 0;
/// No hay disco listo.
pub const DISCO_TRIM_SIN_DISCO: u64 = 1;
/// El disco **no declara TRIM** (palabra 169). No se manda a ver si suena.
pub const DISCO_TRIM_NO_SOPORTADO: u64 = 2;
/// El gate de identidad o la ventana de escritura dijeron que no.
pub const DISCO_TRIM_SIN_PERMISO: u64 = 3;
/// No hay volumen ESTRATOS montado, o su cola libre esta vacia.
pub const DISCO_TRIM_SIN_VOLUMEN: u64 = 4;
/// El rango no es representable: cero sectores, o fuera de LBA48.
pub const DISCO_TRIM_RANGO: u64 = 5;
/// El disco rechazo la orden. **`sectores` lleva lo que SI se recorto** antes
/// de romperse: un recorte a medias no se deshace, y callarlo haria que el
/// sistema volviera a mandar lo que ya estaba hecho.
pub const DISCO_TRIM_FALLO: u64 = 6;

/// Desplazamiento del motivo dentro de la respuesta.
pub const DISCO_TRIM_MOTIVO_SHIFT: u64 = 56;
/// Mascara de los sectores.
pub const DISCO_TRIM_SECTORES_MASK: u64 = (1 << 56) - 1;

// -- ** POR QUE FALLO, cuando el motivo es `DISCO_TRIM_FALLO` ---------------
//
// `DISCO_TRIM_FALLO` dice *que el disco no acepto la orden*; estas clases dicen
// **cual de las cinco maneras**, y viajan en `INFO_DISCO_TRIM_FALLO` junto al
// `PxTFD` crudo: `(clase << 32) | tfd`.
//
// === Por que hizo falta, y se pago en metal ===
//
// El primer recorte en el Ryzen (2026-08-17) contesto *"el disco RECHAZO la
// orden"* y ahi se acabo la informacion. El driver distingue las cinco desde
// siempre, pero su `name()` las aplana en una frase y el `tfd` --el registro
// donde el aparato dice por que-- no salia del `enum`.
//
// ** Y las cinco mandan a mirar sitios distintos: `SIN_TIEMPO` acusa al
// presupuesto de espera del driver, `APARATO` acusa al disco, y `PETICION`
// acusa al que armo el payload. Llamarlas a las tres "rechazo" es perder la
// unica pista que hay.

pub const DISCO_FALLO_NINGUNO: u64 = 0;
/// El puerto no estaba preparado.
pub const DISCO_FALLO_NO_LISTO: u64 = 1;
/// El disco no solto BSY/DRQ: no se le pudo ni dar la orden.
pub const DISCO_FALLO_OCUPADO: u64 = 2;
/// **No termino dentro del limite.** No es que dijera que no: es que no
/// contesto -- y el sospechoso es el presupuesto de espera, no el aparato.
pub const DISCO_FALLO_SIN_TIEMPO: u64 = 3;
/// **El disco contesto con error.** El `PxTFD` de los bits bajos dice cual:
/// `0x01` ERR, y en el byte alto el registro de error -- `0x04` ABRT (no
/// conozco esa orden), `0x10` IDNF (ese sector no), `0x40` UNC.
pub const DISCO_FALLO_APARATO: u64 = 4;
/// La peticion era imposible antes de salir: cero bloques, o mas de lo que cabe.
pub const DISCO_FALLO_PETICION: u64 = 5;

/// Desplazamiento de la clase dentro de `INFO_DISCO_TRIM_FALLO`.
pub const DISCO_FALLO_CLASE_SHIFT: u64 = 32;
/// Mascara del `PxTFD` crudo.
pub const DISCO_FALLO_TFD_MASK: u64 = 0xFFFF_FFFF;

// == ** CREAR UN FICHERO EN ESTRATOS: las subordenes de `TASK_OP_ES_GESTO` ===
//
// Viven aqui y no en `objetos` por lo que son: **ordenes que cambian el
// almacen**, la misma familia que `DISCO_OP_*`. Que una escriba en el aparato y
// la otra en el volumen no las separa -- las dos son lo que este fichero reune.

/// Vacia el renglon del contenido. Se manda ANTES de acumular nada.
///
/// ** Existe para que un intento a medias no envenene al siguiente: si un
/// programa muere despues de mandar tres trozos, el renglon se queda con ellos
/// dentro. Empezar limpiando es mas barato que un tiempo de expiracion.
pub const ES_GESTO_LIMPIAR: u64 = 0x00;

/// Acumula contenido. `arg1` son 8 bytes en little-endian, y **cuantos de esos
/// ocho valen** viaja empaquetado con la suborden: `ES_GESTO_DATOS | (n << 8)`.
///
/// ** Se parte `arg0` porque por la puerta caben dos argumentos y los dos estan
/// ocupados. Es el mismo idioma que `INFO_MEM_QUIEN_*` y `AUTOPSIA_TEXTO`:
/// cuando cabe un numero y hacen falta dos, se parte el numero.
///
/// El cero NO corta -- ver `TASK_OP_ES_GESTO`.
pub const ES_GESTO_DATOS: u64 = 0x01;

/// **Cierra la transaccion.** El nombre sale del renglon de `TASK_OP_RUTA` y el
/// contenido del de arriba. Devuelve la generacion nueva, o `0`.
pub const ES_GESTO_FICHERO: u64 = 0x02;

/// **Crea una carpeta vacia** donde diga la ruta.
///
/// Una carpeta recien nacida es un nodo de directorio SIN `:entradas`. No es un
/// nodo a medias: un directorio es un nodo con `:entradas`, y uno vacio es uno
/// que todavia no la tiene.
pub const ES_GESTO_CARPETA: u64 = 0x03;

/// **Quita la entrada** que diga la ruta.
///
/// ** No destruye nada. Se publica un arbol nuevo sin esa entrada; el bloque de
/// ayer, el nodo del fichero y el estrato anterior siguen donde estaban.
/// **Borrar en ESTRATOS es dejar de nombrar**, y lo que se suelta de verdad es
/// cosa del recolector.
pub const ES_GESTO_QUITAR: u64 = 0x04;

/// **Renombra la entrada** que diga la ruta. El nombre NUEVO viaja por el
/// renglon del contenido ([`ES_GESTO_DATOS`]).
///
/// * El nodo NO se toca: la entrada nueva apunta al mismo bloque, asi que el
/// contenido, los atributos y la `:firma` siguen siendo los de antes.
/// Renombrar un fichero firmado no le invalida la firma.
pub const ES_GESTO_RENOMBRAR: u64 = 0x05;

/// **Trae un fichero de FAT32 a ESTRATOS.**
///
/// La ruta lleva el DESTINO; el renglon del contenido ([`ES_GESTO_DATOS`])
/// lleva el ORIGEN, como texto.
///
/// ** El contenido NO cruza la puerta. Viajan dos NOMBRES, y el kernel lee la
/// fuente el mismo: meter un fichero por el renglon de ocho en ocho serian 512
/// llamadas por bloque, y ese renglon no esta hecho para eso.
///
/// Es lo que hace util el techo que `flujo` levanto: el formato ya sabia partir
/// un fichero en bloques, pero Ring 3 seguia sin poder entregarlo.
pub const ES_GESTO_COPIA: u64 = 0x06;

/// **Marca la version en curso con un nombre**, que viaja por el renglon de la
/// ruta.
///
/// ** Un nombre no describe una version: la hace PERMANENTE. `con_nombre()` es
/// lo que el recolector mira para no soltar un estrato jamas, asi que los gestos
/// automaticos van SIN nombre y esto es el acto aparte de una persona.
///
/// Y es tambien la referencia que hace posible una rama: el superbloque apunta a
/// una sola punta, asi que una version vieja a la que se quiera volver tiene que
/// estar nombrada o nadie la alcanza.
///
/// Cuesta UN bloque: el estrato nuevo apunta a la MISMA raiz. Marcar un volumen
/// de 400 GiB cuesta lo mismo que marcar uno vacio.
pub const ES_GESTO_MARCAR: u64 = 0x07;

/// **Vuelve a la version `arg1` pasos atras.** `0` es la de ahora.
///
/// ** No copia nada. Los bloques de aquella version siguen todos en el disco
/// --nada se sobreescribio nunca-- asi que volver es publicar UN estrato que
/// apunta a la misma raiz. Volver un volumen de 400 GiB cuesta lo mismo que
/// volver uno vacio.
///
/// ** Y lo de en medio NO se pierde: el estrato nuevo tiene por padre la punta
/// de ahora, no la version a la que se vuelve. Es un *revert*, no un *reset* --
/// se deshace el contenido y se conserva el registro de que se deshizo.
pub const ES_GESTO_VOLVER: u64 = 0x08;

/// **DE DONDE SALE EL CONTENIDO: un bloque de `KIND_MEMORIA` propio.**
///
/// `arg1` es el handle del bloque. Los bits altos de `arg0` llevan el
/// DESPLAZAMIENTO dentro de el: `ES_GESTO_ORIGEN | (offset << 8)`.
///
/// Anota nada mas: no lee un byte y no escribe en el disco. Lo ejecuta
/// [`ES_GESTO_FICHERO_DE`], que es quien trae la cuenta.
///
/// === ** POR QUE UN HANDLE Y NO UN PUNTERO ===
///
/// Porque un puntero de Ring 3 habria que validarlo, y esa infraestructura no
/// existe en esta superficie -- a proposito. Un bloque de `KIND_MEMORIA` **lo
/// entrego el kernel**, asi que comprobar que el rango cae dentro es una RESTA
/// contra lo que se entrego, no un recorrido de tablas de pagina.
///
/// Es exactamente la forma que ya tienen `ARCH_OP_LEER_EN` y
/// `ARCH_OP_ESCRIBIR_DE` para FAT32, y la que este renglon no tenia.
///
/// ** Se pide con `RIGHT_READ` y no con `RIGHT_WRITE`: el kernel LEE el bloque,
/// no escribe dentro. Exigir mas autoridad de la que la operacion usa es
/// justo lo que un sistema de capabilities no debe hacer.
pub const ES_GESTO_ORIGEN: u64 = 0x09;

/// **Crea un fichero con el contenido del bloque anotado en
/// [`ES_GESTO_ORIGEN`].** `arg1` son los BYTES a tomar.
///
/// La ruta lleva el destino entero, igual que [`ES_GESTO_FICHERO`].
///
/// === Lo que esto cambia, y por que no es "el renglon pero mas grande" ===
///
/// El renglon acumula de ocho en ocho y para en [`ES_GESTO_MAX`]. Un MiB por
/// ahi serian 131.072 cruces de anillo. Aqui son DOS llamadas --anotar y
/// ejecutar-- para cualquier tamano, porque **el contenido no viaja: viaja
/// donde esta**.
///
/// ** Y quita el rodeo que hoy es obligatorio. Sin esto, la unica forma de
/// meter mas de 96 bytes en ESTRATOS es dejarlos antes en FAT32 y copiarlos
/// con [`ES_GESTO_COPIA`] -- o sea que el documento de una aplicacion tiene que
/// pasar por un sistema de ficheros **que sobreescribe** para llegar al que no
/// sobreescribe. Todo el argumento de ESTRATOS tiene delante un tramo donde no
/// se cumple.
///
/// El techo que queda es el del volumen, y lo dice el nivel de ocupacion.
pub const ES_GESTO_FICHERO_DE: u64 = 0x0A;

/// Cuanto contenido admite EL RENGLON. Es [`RESIDENTE_MAX`] de ESTRATOS: lo que
/// cabe DENTRO del nodo, sin gastar un bloque de datos.
///
/// ** ESTO YA NO ES EL TECHO DE UN FICHERO, y llego a serlo por accidente. Dos
/// limites distintos coincidieron en 96 --lo que cabe en el nodo y lo que
/// acumula el renglon-- y el segundo se quedo mandando sobre el primero.
/// [`ES_GESTO_FICHERO_DE`] entrega el contenido por un bloque de memoria y no
/// pasa por aqui: este numero solo mide el renglon corto.
pub const ES_GESTO_MAX: u64 = 96;
