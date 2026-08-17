//! **EL PRESUPUESTO DE ESTE SILICIO** -- lo que una puerta tiene permitido
//! costar EN UN RYZEN 5 5600X, y en ningun otro sitio.
//!
//! ```text
//!    [eje]     NINGUNO -- constantes; no corren en la puerta
//!    [camino]  P1 la puerta, pero se LEE desde Ring 3
//!    [gen]     el PERFIL -- declara una expectativa de ESTE CPU
//!    [exige]   R-CPU2, R-CPU6
//! ```
//!
//! === Por que estas cifras viven en el perfil y no en el kernel ===
//!
//! Estaban en `syscall/presupuesto.rs` como `const` del kernel, y el kernel
//! arranca en cualquier x86-64. Un `techo: 960` es **ticks del TSC de esta
//! placa**: en otro CPU el mismo numero no es estricto ni laxo, es **de otra
//! maquina**, y juzgar con el da una falsa regresion o un falso aprobado. Ese es
//! el mismo fallo que esta casa lleva toda la semana cerrando: opinar donde no
//! hay derecho.
//!
//! `cpu_vendor/profile.rs` ya lo dice en su primera linea -- *"swapping the CPU
//! is a profile swap, never a kernel edit"*. Un presupuesto medido es
//! exactamente eso. Y encaja con la doctrina del perfil sin torcerla: **es una
//! EXPECTATIVA, y el hecho se le pregunta al silicio** (aqui, corriendo
//! `sys/precio.bex`), igual que el area de XSAVE.
//!
//! === Estrenar otro CPU, entero ===
//!
//! ```text
//!   1. copiar `cpu_vendor/<nuevo>/` con su `presupuesto.rs`
//!   2. arrancar y correr `sys/precio.bex`: dira `sin trinquete` -- correcto,
//!      todavia no hay medida de esa maquina
//!   3. pegar las tres cifras que salgan, +5% de margen de ruido
//! ```
//!
//! Ni una linea del kernel. Eso es lo que este fichero compra.

use crate::ring0::syscall::presupuesto::{Fila, Presupuestos, Suelo};

/// Las tres filas, con la maquina en la que se midieron pegada a ellas.
///
/// [!] `familia`/`modelo` son los de CPUID, y el TSC entra en la identidad
/// **porque el presupuesto esta en ticks**: el mismo modelo con otro TSC no
/// puede usar estos numeros. Ver `es_esta_maquina`.
pub static PRESUPUESTO: Presupuestos = Presupuestos {
    familia: 0x19,
    // ** CONFIRMADO POR EL SILICIO EL 2026-08-17, y de la mejor manera: sin
    // preguntarselo.
    //
    // El arbol se contradecia sobre el modelo de este chip -- el perfil decia
    // `19h/21h` y `ring0/cpu/mod.rs` decia `19h/01h`, llamando ademas "Ryzen
    // 7000 (Raphael, Zen 4)" al 21h. Se tomo el del perfil, y la tanda lo
    // desempato: el trinquete **compara este byte contra CPUID antes de juzgar**
    // y contesto `puerta [EN PLAZO] 839, techo 960`. Si el silicio hubiera dicho
    // otra cosa, la linea habria sido `SIN TRINQUETE` con los dos numeros.
    //
    // ** O sea que el guardian del presupuesto midio, de paso, algo que no era
    // su trabajo: **cual de las dos copias del kernel mentia**. Las dos estaban
    // en `0x01`; las dos corregidas.
    //
    // [!] Y el unico sintoma que tuvo esa mentira durante meses fue el NOMBRE
    // del CPU en `info`. Un dato que nadie mira no se comprueba solo.
    modelo: 0x21,
    // Medido por la calibracion del arranque en esta placa, y confirmado por los
    // dos testigos el 2026-08-17: `TSC 3700000000 Hz`.
    tsc_hz: 3_700_000_000,
    maquina: "Ryzen 5 5600X (19h/21h), TSC 3700 MHz",

    // ** EL SUELO DE ESTE SILICIO, y hoy es una ESTIMACION que lo dice.
    //
    // ~150 ticks de `syscall` + `sysretq`. No sale de una medida: sale del
    // analisis de la fila `puerta` --y coincide con lo que Liedtke consiguio en
    // L4 en los 90, ~250 ciclos en un 486, que es el unico numero de esta cuenta
    // que no ha bajado en treinta anos--.
    //
    // Medirlo de verdad pide una puerta que el stub conteste SIN bajar a Rust, y
    // eso no puede vivir aqui: rompe las dos puertas congeladas y la ignorancia
    // del stub (`entry.rs` lo prohibe por escrito). Va en un build de medida,
    // igual que el metro -- el instrumento se instala, contesta y se retira.
    //
    // Mientras `medido` sea `false`, este numero **no deriva ningun techo**:
    // solo sirve para mirar el ratio, y todo el que lo imprime dice que es una
    // estimacion.
    suelo: Suelo { ticks: 150, medido: false },

    // **La puerta pelada**: `INVOKE` de `BMO_OP_PID` sobre la tarea actual, medida
    // desde Ring 3. Es el suelo del sistema: no resuelve ningun handle, asi que
    // nada puede costar menos que esto.
    //
    // Un trinquete se aprieta con lo que YA se consiguio, nunca con lo que se
    // cree que se va a conseguir. Historia de este techo:
    //
    // ```text
    //    2618   antes de todo
    //    1625   pieza 1 (el XSAVE que no tenia por que existir)
    //     895   pieza 2 (`sysretq`) + los cuatro sellos fuera   <- HOY, 242 ns
    // ```
    //
    // Se aprieta DESPUES de cada tanda que el metal confirma, no antes: cuando
    // aqui ponia 1625, la pieza 2 todavia era una estimacion mia de ~1050 y salio
    // en 895. Si hubiera puesto 1050 y la pieza saliera en 1100, el trinquete
    // habria gritado por una mejora.
    puerta: Fila {
        // 915 fue la peor de las tres tandas, +5% de margen de ruido.
        techo: 960,
        // ** LA META BAJA DE 400 A 300, y no por optimismo: por medida. Se puso 400
        // contando 190 para `dispatch`, y `dispatch` resulto ser ~90. La cuenta
        // buena es 150 de cruce + 60 de prologo/epilogo + 90 de Rust.
        meta: 300,
        porque: "150 de cruce irreducible + 60 de prologo/epilogo + 90 de dispatch",
    },

    // **La mitad de Rust**: lo que tarda `dispatch` por dentro, que es lo unico
    // que el metro sabe medir solo.
    //
    // ** ESTA FILA SE REESCRIBIO ENTERA CUANDO LA VENTANA SE LIMPIO.
    //
    // Decia `techo 320, meta 190` porque las cuatro primeras tandas midieron
    // **309-319**. Ese numero era falso: la ventana llevaba dentro un `printf`, y
    // una puerta de consola cuesta ~2,1 M ciclos. Con la ventana cerrada antes de
    // imprimir, las dos implementaciones dan **84 (C) y 99 (Rust)**.
    //
    // O sea que **la mitad Rust de una puerta nunca fue el 12%: es el 10%, y son
    // ~90 ciclos.** El 309 era un `printf` disfrazado de dispatcher.
    //
    // ** Y LA TANDA DEL 16-08 EN METAL CERRO LA DISCUSION: 87 (C) y 104 (Rust),
    // contra un `rdtsc` suelto que cuesta **69 en un bucle de 43 y 107 en uno de
    // 4** -- no un numero, porque el CPU es fuera de orden y un bucle largo lo
    // solapa.
    //
    // O sea que el termometro es del tamano del enfermo: **el trabajo real de Rust
    // son ~20 ticks debajo de ~70-107 de instrumento**, y esta fila, tal como
    // estaba, NO SE PODIA LEER -- 104 contra techo 105 es un tick de gritar
    // REGRESION por algo que no es el codigo.
    //
    // ** POR ESO EL METRO SE RETIRO A `--features metro_puerta` (paso 1 de la
    // biseccion). Consecuencias, dichas las dos:
    //
    // ```text
    //    build normal    sin `rdtsc` aqui. La puerta cuesta 69-107 ticks menos.
    //                    `dispatch` vale 0 y el juez contesta ROTO, que es lo
    //                    correcto: no hay medida, no hay veredicto.
    //    --features ...  como hasta hoy, y es donde esta fila se juzga.
    // ```
    //
    // [!] La meta de 60 se queda **sin tocar** hasta que el build de medida diga
    // cuanto cuesta `dispatch` con el metro fuera... que es imposible por
    // definicion. Lo que la contestara es la resta entre los dos builds, medida
    // desde Ring 3, donde no hay instrumento en medio.
    dispatch: Fila {
        // ** ESTE TECHO SE AFLOJA, DE 105 A 110, Y SE DICE EN VOZ ALTA.
        //
        // Aflojar un trinquete es lo contrario de para lo que existe, asi que el
        // motivo va aqui y no en el mensaje de un commit: el 105 salia de 99 +5%, y
        // el metal dio **104** -- dentro del techo, pero al 99% de el. Un trinquete
        // sin margen sobre el ruido no es estricto, **es una alarma aleatoria**, y
        // una alarma que salta sola se acaba ignorando (ver
        // `MARGEN_DE_RUIDO_POR_CIENTO`). 104 es la ultima medida CONFIRMADA en
        // metal; +5% da 110, que es la regla de la casa aplicada tal cual.
        techo: 110,
        meta: 60,
        porque: "de los 87-104 medidos, 69-107 son los dos rdtsc del propio metro",
    },

    // **Lo que cuesta resolver una capability**: la fila 4 menos la fila 3.
    //
    // ** ESTA FILA EXISTE POR UNA ANOMALIA, y es el mejor argumento de todo el
    // fichero. Resolver un handle costaba 83 ciclos, de los que 76 caian dentro de
    // `dispatch` y 7 en el stub -- ruido, y correcto: **el stub no sabe que
    // operacion se pidio**. Con la pieza 1 puesta el mismo hueco salio en 342, con
    // **257 de ellos en el stub**, que es un sitio donde no pueden estar.
    //
    // Nadie lo habria visto si no se hubieran comparado las dos tandas a mano. Un
    // trinquete lo habria gritado solo, y por eso esta fila se declara aunque su
    // techo sea, hoy, un numero que no me gusta.
    //
    // ** Y LA ANOMALIA SOBREVIVIO A LAS DOS PIEZAS, o sea que es REAL y no era el
    // instrumento:
    //
    // ```text
    //                    total    dispatch   stub
    //    antes            +83       +76       +7     correcto
    //    pieza 1         +342       +85     +257     <- aparece
    //    pieza 2         +327       +84     +243     <- sigue
    //    ventana limpia  +338       +92     +246     <- y sigue
    // ```
    //
    // La cuarta fila es la que la confirma del todo: se midio con la ventana ya
    // cerrada antes de imprimir, o sea sin la contaminacion que hundio todo lo
    // demas de este fichero. **La mitad de `dispatch` es correcta en las cuatro**
    // --~85, la capability, donde tiene que estar-- y los ~246 del stub siguen
    // exactamente igual.
    //
    // La mitad de `dispatch` se comporta perfecto en las tres tandas: ~85, que es
    // la capability y esta donde tiene que estar. Lo que no puede existir son los
    // **243 en el stub**, porque el stub no sabe que operacion se pidio.
    //
    // [!] No hay explicacion, y despues de fallar dos veces razonando sobre este
    // camino no se va a poner una tercera hipotesis por escrito. Lo que la
    // resuelve es UNA sonda concreta: una fila mas en `c/coste.bex` que use un
    // handle REAL con la operacion mas barata que exista. Si esa fila tambien
    // carga los 243, es el camino del handle; si no, es `BMO_ARCH_TAMANO`.
    handle: Fila {
        // 338 fue la peor observada, +5% de margen de ruido.
        techo: 355,
        meta: 80,
        porque: "~90 en dispatch es correcto; los ~246 del stub son la anomalia viva",
    },
};
