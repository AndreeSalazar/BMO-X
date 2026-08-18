/* bmo.h -- la superficie congelada de BMO, en C.
 *
 * == Por que esto es UNA cabecera y no un runtime ==
 *
 * En Linux o en Windows, `#include <unistd.h>` promete un `libc.so` que el
 * cargador resolvera mas tarde. Aqui no hay cargador que resuelva nada: no hay
 * enlazado dinamico, no hay libc, no hay un simbolo que alguien vaya a
 * rellenar. Asi que la cabecera **trae el cuerpo**, y lo que hay dentro del
 * cuerpo baja a la instruccion en la misma linea.
 *
 * Esa es la diferencia entera de BMO C/Control con el C de siempre: el ASM no
 * esta encapsulado en una caja negra `asm("...")` que el compilador copia sin
 * leer. `__syscall(...)` es una fila de la tabla
 * `forge/sem-asm/tables/arch/x86_64/intrinsics.toml`, con sus bytes exactos y
 * el registro de cada argumento escrito ahi. El compilador emite ESOS bytes.
 * Se lee como C, se comporta como ASM, y ninguna de las dos mitades esconde
 * nada de la otra.
 *
 * == La superficie ==
 *
 * DOS llamadas (2026-08-10; eran tres):
 *
 *     INVOKE(cap, operacion, a0, a1, a2)   haz esto AHORA
 *     WAIT(esperable, visto, timeout_ns)   despiertame CUANDO
 *
 * Todo lo demas --abrir un archivo, leer el raton, reclamar la pantalla-- es una
 * OPERACION sobre una capability. La API crece por dentro, en la pareja
 * (tipo de objeto, operacion), y el ABI no se toca.
 *
 * El tercero se fue porque no era una puerta: CHANNEL_KICK resolvia un handle y
 * avisaba a su consumidor, o sea una OPERACION con numero de syscall propio.
 * WAIT si se queda, y por algo que INVOKE no puede decir: lo unico que hace es
 * NO DEVOLVER EL TURNO. Una llamada sincrona tendria que contestar "todavia no"
 * y dejar que el programa vuelva a preguntar -- quemando el turno en preguntar,
 * que es justo lo que WAIT existe para no hacer.
 *
 * == Lo que un programa NO recibe ==
 *
 * No hay `argv`, no hay `environ`, no hay descriptores heredados. Un proceso
 * Ring 3 recibe *capabilities*, y lo que no le hayan dado no existe para el.
 * Por eso aqui no hay `open("/dev/input")`: hay `bmo_valor(BMO_TAREA_ACTUAL,
 * BMO_OP_ENTRADA_RECLAMAR, ...)`, que puede contestar que no.
 */
#ifndef BMO_BMO_H
#define BMO_BMO_H

/* -- Los DOS numeros de llamada, y el que quedo reservado --------------- */
#define BMO_INVOKE 0
/* ** RETIRADO el 2026-08-10 y RESERVADO: ya no hay una llamada en el 1.
 * Avisar al consumidor de un canal es ahora una operacion sobre el canal
 * (`CHANNEL_OP_KICK`, 0x03) y entra por `INVOKE`, como todo lo demas.
 * El numero no se recicla: un binario viejo que lo llame falla diciendolo. */
#define BMO_CHANNEL_KICK 1
#define BMO_WAIT 2

/* Pseudo-capability que se refiere al proceso que llama.
 *
 * No es un handle concedido: es la forma de pedir lo que uno ya tiene por ser
 * quien es. No otorga autoridad sobre nadie mas y nunca debe transferirse.
 *
 * * Este literal es la razon por la que el lexer de BMO C tuvo que aprender a
 *   leer hexadecimales de 64 bits: no cabe en un `long long` con signo, y
 *   antes se convertia en CERO en silencio -- o sea, en la capability 0. */
#define BMO_TAREA_ACTUAL 0xFFFFFFFFFFFFFFFE

/* -- Operaciones sobre BMO_TAREA_ACTUAL -------------------------------- */
#define BMO_OP_PID 0x01
#define BMO_OP_TID 0x02
#define BMO_OP_CEDER 0x03
#define BMO_OP_SALIR 0x04
#define BMO_OP_CONSOLA_ESCRIBIR 0x06
#define BMO_OP_PANTALLA_RECLAMAR 0x09
#define BMO_OP_ENTRADA_RECLAMAR 0x0A
#define BMO_OP_RUTA 0x0B
#define BMO_OP_EJECUTAR 0x0C
#define BMO_OP_CONSOLA_LEER 0x0F
#define BMO_OP_ARCHIVO_ABRIR 0x10
#define BMO_OP_ARCHIVO_CREAR 0x11
#define BMO_OP_INFO 0x13
/* El SONIDO. Exclusivo como la pantalla; ver <bmo/sonido.h>. */
#define BMO_OP_SONIDO_RECLAMAR 0x21
#define BMO_OP_SONIDO_SOLTAR 0x22

/* Campos de BMO_OP_INFO. Son una TABLA: anadir un dato es una fila, no una
 * operacion nueva. */
#define BMO_INFO_RAM_TOTAL 0x01
#define BMO_INFO_RAM_LIBRE 0x02
#define BMO_INFO_TSC_HZ 0x05
/* -- LA RED, para un programa de C ---------------------------------
 *
 * ** Mirar la red no necesita capability: son campos de INFORME, igual que la
 * RAM o los nucleos. Preguntar si hay cable no es un privilegio.
 * TRANSMITIR si la necesitara, y ese dia sera una operacion sobre un handle. */
#define BMO_INFO_NET_PRESENTE       0x27
#define BMO_INFO_NET_VENDOR_DEVICE  0x28
/* Los seis bytes en los 48 bits bajos, byte 0 el mas significativo. */
#define BMO_INFO_NET_MAC            0x29
/* El `PHYstatus` CRUDO. El byte entero es la prueba; los otros campos son la
 * opinion. */
#define BMO_INFO_NET_PHY_CRUDO      0x2A
/* 10, 100, 1000 -- o 0, que significa "no hay cable" y es una respuesta. */
#define BMO_INFO_NET_MEGABITS       0x2B
/* Distingue "no llega nada" de "no estamos escuchando". */
#define BMO_INFO_NET_RX_ARMADO      0x2C
#define BMO_INFO_NET_RX_TRAMAS      0x2D
#define BMO_INFO_NET_PCI            0x2E

/* El METRO de la puerta: cuantas puertas ha servido el kernel y cuantos ciclos
 * ha pasado DENTRO de `dispatch`. Se leen los DOS y se dividen.
 *
 * ** Se leen como DELTA, antes y despues del bucle que se quiera medir: no hay
 * puesta a cero, y un absoluto arrastraria lo que hizo la maquina arrancando.
 * Las dos lecturas son dos puertas y tambien se cuentan.
 *
 * Restando lo de dentro de `dispatch` al total que mide `c/coste.bex` queda lo
 * que tarda el stub de ensamblador: pushes, xsave64, xrstor64 e iretq. */
#define BMO_INFO_SYSCALL_CUENTA     0x2F
#define BMO_INFO_SYSCALL_CICLOS     0x30

/* Y el reparto DENTRO del stub, la mitad que el metro no sabia partir.
 *
 *    GUARDA     la cabecera a cero + el `xsaveopt64`
 *    CICLOS     dentro de `dispatch`
 *    RESTAURA   las comprobaciones del sello + el `xrstor64`
 *    resto      total menos los tres = `syscall` + pushes + pops + `iretq`
 *
 * Se dividen entre la MISMA cuenta de puertas (`BMO_INFO_SYSCALL_CUENTA`): las
 * tres etapas ocurren una vez por puerta. ** Y las tres tienen que sumar MENOS
 * que el total medido desde aqui; si suman mas, el instrumento miente. */
#define BMO_INFO_SYSCALL_CICLOS_GUARDA   0x35
#define BMO_INFO_SYSCALL_CICLOS_RESTAURA 0x36

/* El PRESUPUESTO de ciclos: lo que una puerta TIENE PERMITIDO costar.
 *
 * El metro dice lo que cuesta hoy; sin esto nada impide que la proxima pieza
 * lo devuelva a 2000. Cada campo trae DOS numeros empaquetados:
 *
 *     techo = valor & 0xFFFFFFFF     lo que NO puede empeorar (trinquete)
 *     meta  = valor >> 32            a donde tiene que llegar (la deuda)
 *
 * ** Cumplir el techo y no la meta no es estar bien: es estar EN PLAZO.
 *
 * Van juntos en un campo a proposito -- separarlos permitiria leer uno y no el
 * otro, que es justo el error que hace decir "cumple" a lo que no llego. La
 * tabla y el porque de cada cifra viven en `ring0/syscall/presupuesto.rs`. */
#define BMO_INFO_PRESUPUESTO_PUERTA      0x37
#define BMO_INFO_PRESUPUESTO_DISPATCH    0x38
#define BMO_INFO_PRESUPUESTO_HANDLE      0x39
/* 1 si esas tres filas se midieron en LA MAQUINA QUE ESTA CORRIENDO. Un techo
 * son ticks del TSC de una placa concreta; en otro CPU no son estrictos ni
 * laxos, son de otra maquina. Con esto en 0 las tres contestan CERO --o sea
 * "sin declarar"-- y el juez se calla en vez de inventarse un veredicto. */
#define BMO_INFO_PRESUPUESTO_MAQUINA     0x3D
/* EL SUELO DEL HARDWARE: `medido << 32 | ticks`. Lo que cuesta cruzar el anillo
 * en este silicio, que no es merito ni culpa de BMO. Restado de una puerta sale
 * la unica cifra de rendimiento que sobrevive a un cambio de CPU: cuantas veces
 * el suelo cuesta una puerta (hoy 5,3x, meta 2,0x).
 *
 * [!] Bit 32 = medido. En 0 es una ESTIMACION y no puede derivar ningun techo:
 * el suelo se mide, el multiplicador se escribe. */
#define BMO_INFO_SUELO_CRUCE             0x3E

/* -- ** LO QUE EL DISCO CONTESTA (2026-08-17) ------------------------
 *
 * Tres filas de HECHOS y una de VEREDICTO. Hasta hoy BMO-X le preguntaba al
 * disco modelo, serie y capacidad, y **no sabia si giraba** -- mientras el
 * diseno de ESTRATOS razonaba sobre TRIM y la ley sobre colas. Ninguna de esas
 * frases era falsa; ninguna estaba comprobada.
 *
 * Capitulo con los numeros: docs/componente/EL_DISCO_EXIGE.md
 *
 * MEDIO      0..15 palabra 217 cruda | 16..17 clase | 32..47 rpm
 *            clase: 0 no contesta, 1 NO ROTA, 2 ROTA, 3 reservado
 * ENLACE     0..2 gen soportadas | 4..6 gen negociada | 8 NCQ
 *            16..23 cola (sesgo -1 ya deshecho) | 24..31 usadas | 32..39 ociosas
 * GEOMETRIA  0..3 EXPONENTE 2^n logicos por fisico | 4 la 106 valia
 *            8..21 desplazamiento LBA 0 | 22 la 209 valia | 23 TRIM
 * JUICIO     0 hay perfil | 1 solido confirmado | 2 la barrera es lo unico
 *            3 TRIM | 4 rendimiento medido | 5 solido sin trim | 6 desalineado
 *            7 enlace por debajo | 8..15 ociosas | 16..47 frontera en KiB
 *
 * [!] Bit 2 de JUICIO vale 1 tambien SIN perfil: no saber si el disco tiene
 * condensadores no autoriza a suponer que los tiene. Y la frontera contesta 0
 * en vez de un valor por defecto -- sin perfil no se alinea a nada. */
#define BMO_INFO_DISCO_MEDIO             0x3F
#define BMO_INFO_DISCO_ENLACE            0x40
#define BMO_INFO_DISCO_GEOMETRIA         0x41
#define BMO_INFO_DISCO_JUICIO            0x42

/* -- ** LO QUE SE LE HA DEVUELTO AL DISCO (TRIM) ---------------------
 *
 * Sectores de 512 B recortados desde el arranque, y en cuantas ordenes de
 * `DATA SET MANAGEMENT` cupieron. Van los DOS: los mismos sectores en una
 * orden o en trescientas dicen cosas distintas del techo que declara el disco
 * (palabra 105), y sin esa division "cuanto" no tiene con que compararse.
 *
 * [!] Un cero aqui significa **que nadie lo ha pedido**, no que no se pueda.
 * Recortar en BMO-X lo pide una persona: la seccion 9 de ESTRATOS dice
 * "politica, no automatismo", y no hay ningun demonio que lo haga solo. */
#define BMO_INFO_DISCO_TRIM_SECTORES     0x43
#define BMO_INFO_DISCO_TRIM_ORDENES      0x44

/* -- ** EL RANGO QUE SE VA A RECORTAR, y lo que cabe en una orden ----
 *
 * COLA_LBA / COLA_SECTORES son la cola libre del volumen ESTRATOS **tal como
 * la va a usar el recorte**: los sirve la misma funcion del kernel que manda
 * la orden. Se podian deducir de las filas `INFO_ES_*`, y deducirlos era tener
 * dos cuentas de la misma verdad -- la que se ensena y la que se ejecuta.
 * 0 = no hay volumen montado, o la cola esta vacia.
 *
 * TRIM_BLOQUES es la palabra 105: bloques de payload de 512 B por orden, y uno
 * son 64 descriptores (~2 GiB de disco). [!] NUNCA contesta 0 -- ACS-3
 * garantiza uno, y el cero de esa palabra es el disco callandose. */
#define BMO_INFO_DISCO_COLA_LBA          0x45
#define BMO_INFO_DISCO_COLA_SECTORES     0x46
#define BMO_INFO_DISCO_TRIM_BLOQUES      0x47

/* Por que fallo el ultimo recorte: (clase << 32) | PxTFD. 0 = ninguno.
 * Clases: 1 puerto no listo, 2 ocupado, 3 SIN TIEMPO, 4 el aparato contesto
 * con error, 5 peticion imposible. El PxTFD va crudo: 0x01 ERR, y en el byte
 * alto 0x04 ABRT, 0x10 IDNF, 0x40 UNC. */
#define BMO_INFO_DISCO_TRIM_FALLO        0x48

/* -- EL CENSO DE EXTENSIONES DEL CPU ---------------------------------
 *
 * Cuantas filas cubre el censo, y dos mascaras sobre ESA lista en ESE orden:
 * bit i = fila i. `USA & ~HAY` es un conflicto -- una instruccion que dara
 * #UD en esta maquina.
 *
 * El nombre de la fila i se pide por TEXTO con
 * `BMO_INFO_TXT_EXT_NOMBRE | (i << 8)`, y su motivo con `..._NOTA`. El indice
 * en los bits altos del campo es el mismo idioma que ya hablan las filas de
 * memoria por ranura.
 *
 * ** AVERIAS empaqueta los cuatro contadores que tienen que ser cero, de 16 en
 * 16 bits: conflictos | mudas<<16 | repetidas<<32 | sin_sitio<<48. */
#define BMO_INFO_CPU_EXT_N          0x31
#define BMO_INFO_CPU_EXT_HAY        0x32
#define BMO_INFO_CPU_EXT_USA        0x33
#define BMO_INFO_CPU_EXT_AVERIAS    0x34
#define BMO_INFO_TXT_EXT_NOMBRE     0x05
#define BMO_INFO_TXT_EXT_NOTA       0x06

#define BMO_INFO_CPU_HILOS 0x06
#define BMO_INFO_CPU_NUCLEOS 0x07
#define BMO_INFO_TICKS 0x0B

/* -- A QUE VELOCIDAD VA ESTO DE VERDAD ------------------------------
 *
 * ** `BMO_INFO_TSC_HZ` dice a que va el RELOJ DE REFERENCIA, y ese no cambia
 * nunca: 3,7 GHz en esta maquina, encendida o a medio gas. No es la velocidad
 * del nucleo. En un Zen 3 la diferencia es de gigahercios enteros -- un nucleo
 * solo bajo carga sube a 4,6 y doce a la vez se quedan cerca de la base.
 *
 * El kernel sabia medirlo desde hace tiempo (MPERF/APERF, y el consumo por
 * RAPL) y **ningun programa de C podia preguntarlo**, porque estas filas no
 * estaban aqui. El panel del compositor, que es Rust, si las leia. O sea que la
 * respuesta existia y no cruzaba la frontera del lenguaje.
 *
 * ** Son MEDIDAS POR DIFERENCIA, no datos: salen de restar dos lecturas, asi que
 * **preguntarlas dos veces seguidas da lo de ESE intervalo**. Quien las quiere
 * de verdad las pregunta una vez por vuelta de su bucle, no dos.
 *
 * Y `0` significa **"no se puede medir aqui"**, que no es lo mismo que cero: un
 * CPU sin los MSR contesta 0 y sigue funcionando. `BMO_INFO_CPU_SENSORES` dice
 * cuales hay antes de creerse un numero -- bit 0 la frecuencia, bit 1 el
 * consumo. */
#define BMO_INFO_CPU_HZ_REAL 0x20
#define BMO_INFO_CPU_MW_PAQUETE 0x21
/* [!] Del nucleo EN EL QUE SE LEE, no de todos: `CORE_ENERGY_STAT` es un
 * contador por nucleo. El metal del 12-08 lo enseno a base de mentira -- con
 * once nucleos GIRANDO al 100% este numero BAJO de 11,9 a 9,2 W, porque los
 * otros once no aparecen aqui en absoluto. */
#define BMO_INFO_CPU_MW_NUCLEO_ACTUAL 0x22
#define BMO_INFO_CPU_SENSORES 0x23
/* Cuantos nucleos estan EN PIE. Uno significa que los otros once duermen -- y
 * eso, en un Zen 3, es la condicion para que este suba a 4,6 GHz. */
#define BMO_INFO_SMP_VIVOS 0x1B

/* -- La puerta --------------------------------------------------------- */

/* El VALOR que devuelve una operacion.
 *
 * La puerta contesta dos cosas: `rax` lleva el codigo y las banderas, `rdx`
 * lleva el valor. En C un par no cabe en un registro de retorno, asi que hay
 * dos funciones y cada una recoge una mitad. Esto no es una limitacion que se
 * pueda tapar: es la forma real de la llamada, y taparla obligaria a inventar
 * una struct que el codegen tendria que devolver por memoria. */
unsigned long long bmo_valor(unsigned long long cap, unsigned long long op,
                             unsigned long long a0, unsigned long long a1,
                             unsigned long long a2) {
    return __syscall_valor(BMO_INVOKE, cap, op, a0, a1, a2);
}

/* El CODIGO de la misma operacion. `0` es lo unico que significa exito.
 *
 * Los 32 bits altos llevan las banderas del kernel -- por ejemplo la que
 * distingue "no tienes permiso" de "ese handle no existe". */
unsigned long long bmo_codigo(unsigned long long cap, unsigned long long op,
                              unsigned long long a0, unsigned long long a1,
                              unsigned long long a2) {
    return __syscall(BMO_INVOKE, cap, op, a0, a1, a2);
}

/* -- Lo que uno tiene por ser quien es --------------------------------- */

unsigned long long bmo_pid() {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PID, 0, 0, 0);
}

/* Ceder el turno.
 *
 * Un bucle de espera en Ring 3 que no cede se come el quantum entero sin
 * avanzar nada -- y como aqui casi todas las lecturas son NO BLOQUEANTES
 * (`bmo_entrada_tecla`, `bmo_entrada_rueda`), el bucle de espera es la forma
 * normal de esperar. Sin este `ceder` el sistema entero va a tirones. */
void bmo_ceder() {
    bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_CEDER, 0, 0, 0);
}

/* Un dato numerico del sistema. `0` si el kernel no sabe contestar ese campo. */
unsigned long long bmo_info(unsigned long long campo) {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_INFO, campo, 0, 0);
}

/* Terminar. No vuelve: el kernel revoca las capabilities del proceso y cambia
 * de contexto en el propio borde del syscall.
 *
 * `main` ya termina asi sola; esto es para salir desde dentro de un bucle. */
void bmo_salir() {
    bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_SALIR, 0, 0, 0);
}

#endif /* BMO_BMO_H */
