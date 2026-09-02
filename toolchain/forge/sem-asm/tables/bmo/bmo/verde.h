/* bmo/verde.h -- la tabla de INFO -- crece por filas, y por eso es verde
 *
 * Un CARRIL de `<bmo/bmo.h>` (L6g). La cabecera entera --que
 * explica por que existe esta pieza-- esta en la fachada; aqui va lo
 * que cambia de color.
 *
 * [carril]  VERDE        anadir un dato es una FILA, y el fichero ya lo decia
 *                        con esas palabras. Nadie hereda de aqui y ningun
 *                        binario viejo se entera de una fila nueva
 * [cuesta]  NADA         un numero mal en un panel. Se ve y se arregla -- es
 *                        el `=1100` de la bitacora
 * [riesgo]  SILENCIO     `bmo_info` contesta 0 tanto para `no se` como para un
 *                        cero de verdad, y las filas de medida por DIFERENCIA
 *                        dan lo de ESE intervalo si se preguntan dos veces
 */
#ifndef BMO_BMO_VERDE_H
#define BMO_BMO_VERDE_H

#include <bmo/bmo/roja.h>

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
#define BMO_INFO_NET_RX_BYTES       0x4A
/* Lo que la TARJETA tiro por no haber descriptor libre. Es el unico
   contador de red que no lleva BMO-X, y sin el "40 tramas" no tiene
   denominador: suena igual si se perdieron cuatro que cuatro mil. */
#define BMO_INFO_NET_RX_PERDIDAS    0x4B
/* arp | ipv4<<16 | ipv6<<32 | otros<<48. Por la puerta cabe UN valor. */
#define BMO_INFO_NET_RX_TIPOS       0x4C
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
/* Cuanto fiarse del par de arriba. 0 = los tres testigos coinciden.
   bit 0 CPUID se contradice | bit 1 hilos/nucleo sin medir
   bit 2 el PERFIL desmiente | bit 3 la MADT discrepa            */
#define BMO_INFO_CPU_TOPOLOGIA_DUDA 0x4D
#define BMO_INFO_CPU_HILOS_POR_NUCLEO 0x4E
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

#endif /* BMO_BMO_VERDE_H */
