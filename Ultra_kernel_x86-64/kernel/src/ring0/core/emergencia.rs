//! **LA PATADA: el kernel recupera la maquina sin que nadie se lo pida.**
//!
//! generacion: nieto -- es POLITICA, no driver. No habla con ningun aparato:
//!
//! [carril]  ROJO      la patada. Si esto falla, no queda ningun rescate detras
//!
//! [cuesta]  APARATO -- decide QUITARLE la pantalla a quien la tenga. Un
//!           disparo de mas deja al dueno sin escritorio hasta que teclee
//!           `escritorio`; uno de menos lo deja mirando una foto de algo que
//!           ya no existe. Por eso la lista de disparadores es de UNA fila
//! decide **cuando el kernel deja de fiarse de Ring 3**.
//!
//! # De donde sale, con la fecha y las palabras
//!
//! Del Ryzen, el 2026-08-26. El dueno abrio la calculadora, tecleo `10 * 60 =`,
//! y el motor de COBOL se murio. La maquina siguio viva --el aislamiento
//! funciono-- y lo que el vio fue esto:
//!
//! > *"para sorpresa la pantalla no se limpio ni me manda al Kernel con
//! > terminal. Me gustaria que en caso de corrupcion, por si sola, el Kernel lo
//! > bote a patada. El Kernel TIENE que tomar su lugar, como emergencia."*
//!
//! # *** POR QUE `Ctrl+Alt+Esc` NO LE SIRVIO, Y NO ERA UN BUG
//!
//! El rescate por teclado existe desde el 12-08 y funciona. Lo que pasa es que
//! `fb::rescue` **se niega a echar al primer dueno** --el escritorio-- y lo dice
//! con todas las letras:
//!
//! > *"Si echara al escritorio, la tecla de emergencia seria la tecla de romper
//! > la maquina."*
//!
//! Ese razonamiento era correcto cuando se escribio y hoy le falta un dato: **el
//! shell de Ring 0 no se para nunca.** `run_shell` es un bucle que no retorna y
//! que sigue leyendo el teclado mientras el escritorio corre. O sea que **si hay
//! sitio donde aterrizar**, y quitarle la pantalla al escritorio ya no es
//! romper la maquina: es volver al sitio del que se salio.
//!
//! # [!] LO QUE ESTO **NO** HACE, Y ES LA MITAD DEL DISENO
//!
//! **No se dispara porque una app se muera.** Que un programa reviente y el
//! escritorio siga es el aislamiento haciendo exactamente lo que promete: es la
//! linea `CPL3: tarea eliminada, BMO sigue vivo`. Si cada `.bex` que casca se
//! llevara el escritorio por delante, el aislamiento no serviria de nada --
//! seria un sistema donde la app mas fragil manda.
//!
//! ```text
//!    una app falla        -> la tarea muere, el escritorio sigue.  NO ES ESTO
//!    el kernel se rompe   -> pantalla azul y reinicio.             TAMPOCO
//!    el kernel DESCONFIA  -> la patada.                            <- ESTO
//! ```
//!
//! *** La patada es el escalon que faltaba entre los dos: **el kernel sigue
//! vivo, pero ha visto algo que dice que su propia contabilidad esta danada.**
//! Seguir dejando la pantalla en manos de Ring 3 en ese estado es apostar.
//!
//! # Que cuenta como corrupcion, hoy
//!
//! Una sola cosa, y es la unica que se ha MEDIDO en metal: que
//! `mm::vmm::caminable` rechace una entrada de tabla de paginas al desmontar un
//! proceso muerto. Eso no es una app portandose mal -- son **las tablas del
//! kernel diciendo algo imposible**.
//!
//! [!] La lista es corta a proposito. Un disparador de mas aqui no da un fallo:
//! da un escritorio que desaparece sin motivo, y eso se aprende a ignorar.
//!
//! # ** Y POR QUE ESTO ES UNA BANDERA Y NO EL TRABAJO
//!
//! Porque quien la levanta corre **con el cerrojo del planificador en la mano y
//! las interrupciones apagadas**: `destroy_address_space` se llama desde `reap`,
//! y `reap` desde `schedule_locked`. Hacer ahi el rescate seria volver a tomar
//! `SCHED_LOCK` --`fb::rescue` pregunta el CR3 al planificador-- y un `SpinLock`
//! no es reentrante: abrazo mortal en la ruta que corre 250 veces por segundo.
//!
//! Es la misma regla que ya sigue el manejador del disco, escrita en `plat/irq.rs`:
//!
//! > *"LO MINIMO Y NADA MAS. Aqui se apunta que llego; QUIEN esperaba ese dato
//! > lo recoge en su propio turno."*
//!
//! Aqui el que lo recoge es el hilo del bus (`dev/usb/bus.rs`), que ya despierta
//! cada 4 ms y ya vigila el rescate por teclado. **Mismo sitio, misma razon.**

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Hay una patada pendiente de darse.
static PENDIENTE: AtomicBool = AtomicBool::new(false);
/// Cuantas corrupciones se han declarado desde el arranque. **No se reinicia**:
/// que la maquina se haya recuperado no borra que paso.
static DECLARADAS: AtomicU32 = AtomicU32::new(0);
/// El valor de la primera, para poder decirlo en la pantalla de vuelta.
static PRIMER_VALOR: AtomicU64 = AtomicU64::new(0);
/// El motivo de la primera. `&'static str` en un atomic no cabe, asi que se
/// guarda el puntero y se reconstruye. Todos los motivos son literales del
/// binario, o sea que viven para siempre.
static PRIMER_MOTIVO: AtomicU64 = AtomicU64::new(0);
static PRIMER_MOTIVO_LARGO: AtomicU64 = AtomicU64::new(0);

/// **Declarar una corrupcion.** Seguro desde cualquier sitio, incluso con
/// cerrojos en la mano y las interrupciones apagadas.
///
/// No hace el trabajo: lo apunta. Ver la cabecera.
pub fn declarar(motivo: &'static str, valor: u64) {
    let primera = DECLARADAS.fetch_add(1, Ordering::SeqCst) == 0;
    if primera {
        PRIMER_VALOR.store(valor, Ordering::SeqCst);
        PRIMER_MOTIVO.store(motivo.as_ptr() as u64, Ordering::SeqCst);
        PRIMER_MOTIVO_LARGO.store(motivo.len() as u64, Ordering::SeqCst);
    }
    PENDIENTE.store(true, Ordering::SeqCst);
    // CABINA es segura aqui: tiene su propia guardia de reentrancia y no toma
    // cerrojos que puedan colgarse. Es lo unico que se hace en el sitio del
    // fallo, y se hace porque un volcado posterior tiene que poder decir CUAL
    // fue la primera.
    crate::ring0::cabina::fault("patada", motivo, valor);
}

/// Cuantas corrupciones van. Para el panel.
pub fn declaradas() -> u32 {
    DECLARADAS.load(Ordering::SeqCst)
}

fn primer_motivo() -> &'static str {
    let p = PRIMER_MOTIVO.load(Ordering::SeqCst);
    let n = PRIMER_MOTIVO_LARGO.load(Ordering::SeqCst) as usize;
    if p == 0 || n == 0 || n > 96 {
        return "corrupcion sin motivo apuntado";
    }
    // SAFETY: `p` y `n` salieron de un `&'static str` de este mismo binario en
    // `declarar`, y un literal no se mueve ni se libera. La cota de 96 es un
    // cinturon por si el atomic viniera de basura: preferible una frase corta
    // que un puntero suelto.
    unsafe {
        let bytes = core::slice::from_raw_parts(p as *const u8, n);
        core::str::from_utf8(bytes).unwrap_or("corrupcion con motivo ilegible")
    }
}

/// **Atender la patada.** Lo llama el hilo del bus, sin cerrojos en la mano.
///
/// Devuelve `true` si hubo patada que dar.
pub fn atender() -> bool {
    if !PENDIENTE.swap(false, Ordering::SeqCst) {
        return false;
    }
    // 1. La pantalla PRIMERO, y sin respetar al primer dueno. Es la diferencia
    //    entera con el rescate por teclado: aquel protege al escritorio porque
    //    el usuario puede haberse equivocado de tecla; esto se dispara porque el
    //    kernel ya vio que algo esta roto.
    let quien = crate::ring0::obj::fb::rescate_de_emergencia();
    if let Some(pid) = quien {
        // La entrada detras de la pantalla: si solo se pudiera una, la que
        // importa es la que devuelve la imagen.
        let _ = crate::ring0::obj::input::release(pid);
    }
    // 2. Y se DICE, en la pantalla que se acaba de recuperar. Un kernel que
    //    toma el control sin explicarse deja al dueno mirando un escritorio que
    //    desaparecio solo.
    anunciar(quien);
    crate::ring0::cabina::warn(
        "patada",
        "el kernel RECUPERO la maquina: Ring 3 dejo de ser de fiar",
        quien.unwrap_or(0) as u64,
    );
    true
}

/// Las cuatro lineas que el dueno tiene que ver al volver.
///
/// ** No es la pantalla azul y no se le parece a proposito: **el kernel esta
/// vivo**. Aquella dice *"esto se acabo, en 20 segundos reinicio"*; esta dice
/// *"te devuelvo la maquina y estas son las razones"*. Confundirlas seria
/// enterrar una maquina que funciona.
fn anunciar(quien: Option<u32>) {
    use crate::ring0::core::dashboard::dashboard_log;
    // *** SE LIMPIA LA PANTALLA, Y NO ES COSMETICA (2026-08-26).
    //
    // La primera version recuperaba la pantalla y escribia encima. El dueno lo
    // vio y lo dijo en una linea: **"eso no se limpio"**. Y tenia razon en algo
    // que no es de estetica:
    //
    // > Los pixeles del escritorio muerto siguen ahi. Lo que queda en pantalla
    // > es una foto de una cosa que ya no existe, con cuatro renglones del
    // > kernel encima. **Parece que el escritorio sigue vivo y no responde**,
    // > que es exactamente el sintoma del que se venia huyendo.
    //
    // `splash_dashboard_init` rellena la pantalla y repinta el marco del panel,
    // que es donde el shell de Ring 0 ya escribe. Asi lo que se ve DESPUES de la
    // patada es el sitio al que se ha vuelto, no el escombro del que se sale.
    crate::ring0::core::splash::splash_dashboard_init();
    dashboard_log("");
    dashboard_log("*** EL KERNEL RECUPERO LA MAQUINA (patada de emergencia) ***");
    let mut l = crate::ring0::cabina::format::Buf::new();
    l.txt("   motivo: ");
    l.txt(primer_motivo());
    l.txt("  =");
    l.hex_min(PRIMER_VALOR.load(Ordering::SeqCst));
    dashboard_log(l.as_str());
    let mut l2 = crate::ring0::cabina::format::Buf::new();
    l2.txt("   corrupciones declaradas: ");
    l2.dec(declaradas() as u64);
    if let Some(pid) = quien {
        l2.txt("   pantalla quitada al pid ");
        l2.dec(pid as u64);
    } else {
        l2.txt("   la pantalla ya era del kernel");
    }
    dashboard_log(l2.as_str());
    dashboard_log("   el shell de Ring 0 sigue vivo: escribe `cabina fallos`");
    dashboard_log("   y `escritorio` levanta el de Ring 3 otra vez");
}
