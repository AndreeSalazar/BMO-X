//! `syscall::presupuesto` -- **lo que una puerta TIENE PERMITIDO costar.**
//!
//! ```text
//!    [eje]     NINGUNO -- una tabla de constantes; no corre en la puerta
//!    [camino]  P1 la puerta, pero se LEE desde Ring 3, no en el camino
//!    [gen]     PADRE -- nombra y compone (techo, meta, porque). No sabe que
//!              hay otras filas ni que significa la diferencia: eso es
//!              `bmo-juicio`, el nieto, que vive fuera y se prueba
//!    [exige]   R-CPU2 (nada entra en la puerta sin fila), R-TIME3
//! ```
//!
//! ** Este fichero declara y NO juzga, y esa frontera es la que hizo que el
//! fallo del 16-08 se pudiera arreglar en un `cargo test` de tres segundos en
//! vez de en un flasheo.
//!
//! # Por que existe
//!
//! El 2026-08-16 una puerta paso de 2663 a ~1050 ciclos en tres piezas, y cada
//! una se justifico con una medida. Pero **nada en el arbol impide que la
//! cuarta la devuelva a 2000**: el metro sabe decir cuanto cuesta hoy y no sabe
//! decir cuanto DEBERIA costar. Un numero sin contrato es una anecdota.
//!
//! Este fichero convierte *"optimizar"* en *"no incumplir"*, que es la misma
//! forma que ya tiene el censo de extensiones: se **declara** lo que se espera,
//! y lo que grita es la diferencia entre lo declarado y lo real.
//!
//! ** Y no es una idea nueva: en motores de tiempo real y de juego se llama
//! presupuesto de ciclos y es practica normal. Lo raro era no tenerlo.
//!
//! # DOS numeros por fila, y son dos contratos distintos
//!
//! ```text
//!    techo   lo que NO puede empeorar.  Es un trinquete contra regresiones.
//!    meta    a donde tiene que llegar.  Es la deuda, escrita.
//! ```
//!
//! El `techo` sale de la **ultima medida confirmada en metal**, no de un deseo:
//! si algo lo cruza, alguien acaba de meter trabajo en el camino y hay que
//! saberlo el mismo dia, no tres meses despues. La `meta` sale del analisis del
//! suelo fisico de esta maquina, y hasta que se alcance la fila dice, sin
//! adornos, que el trabajo no esta terminado.
//!
//! ** UNA FILA QUE CUMPLE EL TECHO Y NO LA META NO ESTA BIEN: esta en plazo.
//!
//! # De donde sale la meta, que es la parte que hay que poder discutir
//!
//! ```text
//!    cruce (`syscall` + `sysretq`)   ~150   IRREDUCIBLE
//!    prologo + epilogo                ~60
//!    dispatch (el Rust)              ~190
//!    ------------------------------------
//!    puerta pelada                    400
//! ```
//!
//! Los ~150 del cruce no son un objetivo: son el suelo. Salen de la calibracion
//! que dio `c/coste.bex` --un `rdtsc` mide **69 ciclos**, y `syscall`/`sysret`
//! son de la misma familia microcodificada pero hacen mas-- y coinciden con lo
//! que Liedtke consiguio con L4 en los 90 (~250 ciclos en un 486). El coste de
//! cruzar un anillo de privilegio es lo unico de esta cuenta que no ha bajado
//! en treinta anos.
//!
//! # Como se lee, y por que NO se comprueba en el arranque
//!
//! El censo de extensiones grita en CABINA al arrancar porque su verdad ya
//! existe entonces. La de este fichero **no**: al arrancar no se ha servido ni
//! una puerta y el metro esta vacio. Un presupuesto solo se puede juzgar contra
//! trafico real.
//!
//! Por eso lo lee `c/coste.bex` desde Ring 3, que es donde ya vive la medida --
//! y ademas es el unico sitio al que Eddi puede llegar desde el escritorio.
//!
//! [!] Estos numeros viajan a Ring 3 EMPAQUETADOS, `meta << 32 | techo`, por la
//! misma razon que `INFO_CPU_EXT_AVERIAS` empaqueta cuatro: son datos de la
//! misma fila y separarlos en dos campos permitiria leer uno y no el otro, que
//! es justo el error que hace decir *"cumple"* a algo que no llego a la meta.

/// # EL MARGEN DEL TRINQUETE, y por que no es cero
///
/// La primera version puso cada `techo` clavado en la ultima medida. La tanda
/// siguiente, **con el kernel byte a byte identico**, dio 915 donde la anterior
/// dio 895 y el presupuesto grito `SE PASA -- REGRESION`.
///
/// No habia regresion: hay ruido. El minimo de 16 bloques sigue dependiendo del
/// estado de la maquina --cache, historia del arranque, que mas hubiera listo--
/// y entre tandas se mueve un ~2%.
///
/// ** UN TRINQUETE MAS APRETADO QUE EL RUIDO NO ES ESTRICTO: ES UNA ALARMA
/// ALEATORIA. Y una alarma que salta sola se acaba ignorando, que es peor que
/// no tenerla.
///
/// Asi que el techo se pone en **la peor medida observada mas un 5%**: por
/// encima del ruido medido (2,2%) y muy por debajo de lo que mueve una pieza de
/// verdad (las de hoy movieron del 30% al 60%). Una regresion real sigue
/// gritando; el ruido, no.
pub const MARGEN_DE_RUIDO_POR_CIENTO: u64 = 5;

/// Una fila del presupuesto: lo que no puede empeorar y a donde tiene que ir.
pub struct Fila {
    /// Ultima medida confirmada en metal. Cruzarlo es una REGRESION.
    pub techo: u32,
    /// El objetivo que sale del analisis. No alcanzarlo es DEUDA, no fallo.
    pub meta: u32,
    /// Por que la meta es esa. Vive aqui para que cambiarla obligue a
    /// reescribir el motivo, no solo la cifra.
    pub porque: &'static str,
}

impl Fila {
    /// `meta << 32 | techo`, que es como cruza a Ring 3.
    pub const fn empaquetado(&self) -> u64 {
        ((self.meta as u64) << 32) | self.techo as u64
    }
}

/// -- ** UN PRESUPUESTO ES DE UNA MAQUINA, NO DEL KERNEL -------------------
///
/// # El defecto que esto cierra, dicho entero
///
/// Estas tres filas eran `const` del kernel: `techo 960` medido en el Ryzen del
/// dueno. **Y el kernel arranca en cualquier x86-64.** En otro CPU los mismos
/// numeros seguirian juzgando, y darian una de estas dos:
///
/// ```text
///    un CPU mas lento    -> [SE PASA] REGRESION   por no ser el mismo silicio
///    un CPU mas rapido   -> [META]                cuando igual hay una regresion
/// ```
///
/// Las dos son el mismo fallo de siempre en esta casa: **un veredicto donde no
/// hay derecho a opinar**. Es el hermano del cero que felicitaba.
///
/// # Por que va en el PERFIL y no aqui
///
/// Porque `cpu_vendor/profile.rs` ya dice, en su primera linea, que cambiar de
/// CPU es un cambio de perfil y nunca una edicion del kernel -- y un techo en
/// ticks es exactamente un dato de ese CPU. Este fichero se queda con lo que SI
/// es del kernel: la forma de la fila, la doctrina de los dos numeros y el
/// margen de ruido. Estrenar un CPU pasa a ser copiar el directorio del perfil y
/// pegar tres cifras medidas, sin tocar nada de aqui.
///
/// # [!] Y el TSC forma parte de la identidad, aunque no lo parezca
///
/// El presupuesto esta en **ticks**, y un tick vale lo que valga el TSC de esa
/// maquina. Dos CPU del mismo modelo con TSC distinto **no son la misma maquina
/// para esta tabla**, aunque CPUID diga lo mismo. Por eso se compara tambien la
/// frecuencia, con un 1% de tolerancia: la calibracion del arranque no da el
/// mismo entero dos veces.
pub struct Presupuestos {
    /// Familia y modelo de CPUID en los que se midieron estas filas.
    pub familia: u8,
    pub modelo: u8,
    /// Frecuencia del TSC de la maquina donde se midieron, en Hz.
    pub tsc_hz: u64,
    /// Como se llama esa maquina, para poder decirlo cuando no coincida.
    pub maquina: &'static str,
    /// Lo que cuesta cruzar el anillo en ESTE silicio. Ver [`Suelo`].
    pub suelo: Suelo,
    pub puerta: Fila,
    pub dispatch: Fila,
    pub handle: Fila,
}

/// -- ** EL SUELO DEL HARDWARE: lo que no es merito ni culpa de BMO --------
///
/// # Las dos cosas que hoy van pegadas en el mismo numero
///
/// ```text
///    SUELO       cruzar el anillo en ESE silicio. Cambia con el CPU y
///                BMO no puede hacer nada al respecto.
///    SOBRECOSTE  lo que BMO ANADE encima. Eso SI es este kernel, y NO
///                depende del CPU.
/// ```
///
/// Una puerta de 792 ticks no dice si el kernel esta bien o mal: dice
/// `suelo + sobrecoste` sin separarlos. Con el suelo aparte, sale **la cifra
/// que sobrevive a un cambio de CPU**: *cuantas veces el suelo del hardware
/// cuesta una puerta de BMO*. Hoy 5,3x; la meta declarada seria 2,0x.
///
/// # ** Y AQUI ESTA LA REGLA QUE HACE QUE ESTO NO SEA UNA TRAMPA
///
/// > **El suelo se MIDE. El multiplicador se ESCRIBE.**
///
/// Un presupuesto que se recalibrara solo entero se ceniria a lo que hubiera --
/// **incluida la grasa**: una regresion se convertiria en la talla nueva y el
/// juez aprobaria siempre. Un trinquete que se ajusta solo no es un trinquete.
///
/// Asi que se ajusta la parte que es del CPU (el suelo, medible) y **jamas la
/// que es el veredicto** (el multiplicador, que vive en este fichero porque es
/// una afirmacion sobre BMO, no sobre el silicio).
///
/// # [!] Hoy el suelo de este perfil es una ESTIMACION, y viaja diciendolo
///
/// Sale del analisis de `PUERTA_PELADA` --~150 ticks de cruce-- y **nadie lo ha
/// medido**: para medirlo hace falta una puerta que el stub conteste sin bajar a
/// Rust, y eso no puede vivir en el kernel que se despliega (rompe las DOS
/// puertas congeladas y la ignorancia del stub). Va en un build de medida, como
/// el metro. Hasta entonces `medido` es `false` y todo el que lo lee lo dice.
#[derive(Clone, Copy)]
pub struct Suelo {
    /// Ticks que cuesta el cruce de anillo en este silicio.
    pub ticks: u32,
    /// `false` = es una estimacion del analisis, no una medida. **Un suelo
    /// estimado no puede derivar un techo**: solo sirve para mirar el ratio.
    pub medido: bool,
}

/// **Cuantas veces el suelo del hardware puede costar una puerta de BMO**, en
/// centesimas. `640` = 6,40x.
///
/// ** VIVE AQUI Y NO EN EL PERFIL, y esa es la frontera entera: es una
/// afirmacion sobre **este kernel**, no sobre ningun CPU. Si BMO adelgaza, este
/// numero baja **para todas las maquinas a la vez** -- que es lo que significa
/// optimizar "a base de perfil" en vez de "a base de CPU".
///
/// Sale de lo medido el 17-08: techo 960 sobre un suelo de ~150.
pub const PUERTA_VECES_EL_SUELO: u64 = 640;

/// La meta, en la misma unidad: `200` = 2,00x el suelo. Sale de la meta de 300
/// ticks sobre el mismo suelo.
pub const PUERTA_META_VECES_EL_SUELO: u64 = 200;

/// El suelo de esta maquina, empaquetado para Ring 3: `medido << 32 | ticks`.
///
/// Cero = este perfil no declara suelo, y entonces el que lo lee **no puede
/// calcular el ratio** -- que es distinto de calcularlo mal.
pub fn suelo() -> u64 {
    let s = crate::ring0::cpu_vendor::profile::active().presupuesto.suelo;
    ((s.medido as u64) << 32) | s.ticks as u64
}

/// Cuanto se le permite variar al TSC antes de decir que es otra maquina.
///
/// La calibracion del arranque no repite el entero exacto entre reinicios. Un
/// 1% acepta ese ruido y sigue rechazando un CPU de otra frecuencia -- entre
/// 3,7 y 4,7 GHz hay un 27%, o sea que no hay zona gris.
const TOLERANCIA_TSC_POR_MIL: u64 = 10;

/// **El presupuesto de este arranque se midio en ESTA maquina?**
///
/// Si contesta que no, las tres filas cruzan a Ring 3 como `sin declarar` y el
/// juez **se calla**. Esa es toda la diferencia entre un trinquete y una alarma
/// aleatoria en un banco de pruebas ajeno.
pub fn es_esta_maquina() -> bool {
    veredicto_maquina() & MAQ_COINCIDE != 0
}

/// -- ** EL VEREDICTO DE IDENTIDAD, EMPAQUETADO Y CON LOS DOS LADOS ---------
///
/// Un `bool` habria bastado para frenar el trinquete, y **no basta para
/// arreglarlo**: el dia que conteste `false` en la maquina del dueno, hay que
/// saber si fallo el modelo o el reloj, y con que numeros. Un no sin motivo
/// manda a leer codigo; este campo manda a cambiar una cifra.
///
/// ```text
///    bit 0        coincide TODO -- es el unico que decide
///    bit 1        familia y modelo coinciden
///    bit 2        el TSC coincide (dentro del 1%)
///    bits  8..15  familia ESPERADA      16..23  modelo ESPERADO
///    bits 24..31  familia LEIDA         32..39  modelo LEIDO del silicio
/// ```
///
/// [!] **Y hace falta hoy, no en teoria: el arbol se contradice a si mismo.**
/// `cpu/mod.rs` dice que un 5600X es `19h/01h` y llama `19h/21h` a un Ryzen
/// 7000; el perfil de este directorio declara `family_model: "19h/21h"`. Los dos
/// no pueden tener razon, **y nadie ha leido nunca el byte de este chip** --
/// el unico sintoma era el nombre en `info`, que nadie mira. Este campo lo lee y
/// lo ensena, y con eso la discusion se cierra con un dato en vez de con una
/// opinion.
pub const MAQ_COINCIDE: u64 = 1 << 0;
pub const MAQ_CPU_OK: u64 = 1 << 1;
pub const MAQ_TSC_OK: u64 = 1 << 2;

pub fn veredicto_maquina() -> u64 {
    let perfil = crate::ring0::cpu_vendor::profile::active();
    let p = perfil.presupuesto;
    let mut v = ((p.familia as u64) << 8) | ((p.modelo as u64) << 16);

    // La identidad se le pregunta al SILICIO a traves del perfil, nunca al
    // nombre del modulo: es la cadena que `profile.rs` documenta y que ya se
    // rompio una vez en tres sitios.
    let leida = (perfil.identidad)();
    if let Some((familia, modelo)) = leida {
        v |= ((familia as u64) << 24) | ((modelo as u64) << 32);
        if familia == p.familia && modelo == p.modelo {
            v |= MAQ_CPU_OK;
        }
    }

    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz != 0 && p.tsc_hz != 0 {
        let (mayor, menor) = if hz > p.tsc_hz { (hz, p.tsc_hz) } else { (p.tsc_hz, hz) };
        if (mayor - menor) * 1000 <= p.tsc_hz * TOLERANCIA_TSC_POR_MIL {
            v |= MAQ_TSC_OK;
        }
    }

    if v & MAQ_CPU_OK != 0 && v & MAQ_TSC_OK != 0 {
        v |= MAQ_COINCIDE;
    }
    v
}

/// Las tres filas, ya empaquetadas para Ring 3 -- **y en cero si el presupuesto
/// no es de esta maquina**.
///
/// ** LA SEGURIDAD ESTA EN EL VALOR, NO EN QUE ALGUIEN SE ACUERDE DE MIRAR.
/// Devolver cero significa `sin declarar`, que todo cliente ya sabe leer desde
/// el primer dia: el que no se entere de que existe `INFO_PRESUPUESTO_MAQUINA`
/// pierde el MOTIVO, nunca la proteccion. Al reves --contestar el techo bueno y
/// confiar en que el cliente compruebe un segundo campo-- bastaria con que uno
/// se olvidara para que saliera un veredicto falso.
pub fn puerta() -> u64 {
    de(|p| &p.puerta)
}

pub fn dispatch() -> u64 {
    de(|p| &p.dispatch)
}

pub fn handle() -> u64 {
    de(|p| &p.handle)
}

fn de(cual: fn(&'static Presupuestos) -> &'static Fila) -> u64 {
    if !es_esta_maquina() {
        return 0;
    }
    cual(crate::ring0::cpu_vendor::profile::active().presupuesto).empaquetado()
}

