//! **CUANTO VALE UNA PUERTA** -- medido en ensamblador, juzgado fuera del metal.
//!
//! # Por que existe este, teniendo ya `c/coste.bex`
//!
//! No para reemplazarlo. **Para no estar de acuerdo con el.**
//!
//! El 16-08 el fallo de `c/coste.bex` lo cazo una discrepancia entre dos
//! funciones del mismo programa que calculaban lo mismo y daban 309 y 1116. Eso
//! no fue suerte: dos calculos independientes que TIENEN que coincidir son mas
//! fuertes que un calculo bueno. Este programa y el de C son ese par, en dos
//! lenguajes, con dos compiladores distintos.
//!
//! Si dan lo mismo, el numero es solido. Si difieren, **uno miente y ya se sabe
//! que hay que mirar**, que es todo lo que se le puede pedir a un instrumento.
//!
//! # Las tres cosas que este trae y el de C no puede
//!
//! ## 1. El bucle esta en ENSAMBLADOR, y los bytes se pueden comprobar
//!
//! C no tiene optimizador, y eso era una garantia -- pero una garantia que hay
//! que creerse. Rust si lo tiene, asi que un bucle de medida escrito en Rust
//! normal se evaporaria o se sacaria de sitio.
//!
//! La respuesta no es apagar el optimizador: es **escribir las instrucciones**.
//! `medir_puerta` es un `asm!` con seis instrucciones y un salto, y lo que
//! emite se lee con `llvm-objdump` igual que se leyo `syscall_entry` todo el
//! 16-08. Un instrumento cuyos bytes estan verificados es mas duro que uno que
//! confia en una propiedad del compilador.
//!
//! ## 2. El juez se prueba en el anfitrion
//!
//! Los invariantes viven en `bmo-juicio`, en `platform/shared/`, con 16 pruebas
//! que corren en 2,85 segundos sin arrancar nada. El fallo que costo un flasheo
//! entero es una de ellas.
//!
//! ## 3. La ventana se declara
//!
//! [`bmo_juicio::Medida::cerrada_sin_imprimir`] es un testigo que este programa
//! FIRMA. Y aqui esta la parte incomoda: el juez **no puede comprobarlo** --los
//! numeros de una ventana sucia son coherentes, solo describen otra cosa-- asi
//! que lo unico honesto era obligar a declararlo. Este fichero lo cumple de una
//! forma que se ve leyendo: **entre `bmo::info(...)` y el primer `consola(...)`
//! no hay nada**.
//!
//! # [!] Lo que este programa mide que el de C no
//!
//! La puerta **tal y como la ve un programa de Rust optimizado**. El de C la
//! mide como la ve un `.bex` de C, que es lo que son casi todos. Las dos son
//! ciertas y no tienen por que dar el mismo total: la fila 1 (el bucle vacio)
//! es la que dice cuanto se ha restado de cada lado. Comparar las filas 3 entre
//! los dos programas sin mirar la fila 1 es comparar dos cosas distintas.

#![no_std]
#![no_main]

use bmo_juicio as juez;
use bmo_userland as bmo;

/// Llamadas dentro de un bloque cronometrado. Suficientes para que los dos
/// `rdtsc` sean ruido, pocas para que una expropiacion no toque a la mayoria.
const LOTE: u64 = 4096;

/// Bloques por medida. El **minimo** de estos es la respuesta.
const VUELTAS: u64 = 16;

const NR_INVOKE: u32 = 0;
const TAREA_ACTUAL: u64 = 0xFFFF_FFFF_FFFF_FFFE;
const OP_PID: u64 = 0x0F;
const OP_INFO: u64 = 0x13;
const OP_MI_PAQUETE: u64 = 0x25;
const ARCH_TAMANO: u64 = 0x03;
const ARCH_CERRAR: u64 = 0x04;
/// **`INFO_TICKS`, y el numero importa.**
///
/// La primera sonda paso `0` aqui creyendo que *"da igual el campo, lo que se
/// mide es la operacion"*. Falso: los campos de `OP_INFO` empiezan en `0x01`,
/// asi que el `0` cae en el brazo por defecto (`_ => 0` en `report.rs`) y lo
/// que se midio fue **un rechazo**, el camino mas corto que existe dentro de
/// `INFO`. Por eso salio 784 contra los 870 de `PID`: una operacion "mas
/// gorda" que resulto mas barata que la barata.
const CAMPO_TICKS: u64 = 0x0B;

// ===================== EL INSTRUMENTO, EN INSTRUCCIONES =====================

/// `n` puertas seguidas y nada mas.
///
/// ** ESTE BUCLE ES EL INSTRUMENTO, y por eso esta escrito instruccion a
/// instruccion en vez de en Rust: lo que se mide tiene que ser lo que se
/// escribio, no lo que el optimizador decidio dejar. Seis instrucciones y un
/// salto; lo emitido se comprueba con `llvm-objdump`, igual que se comprobo
/// `syscall_entry`.
///
/// La convencion la fija `userland/sys.rs` y no se inventa aqui:
/// `rax` = numero, `rdi` = capability, `rsi` = operacion, `rdx`/`r10`/`r8` =
/// argumentos. `rcx` y `r11` los machaca el CPU en cada `syscall`.
///
/// # Safety
/// Ejecuta `syscall` en bucle. `cap` y `op` tienen que ser una pareja valida:
/// una operacion desconocida vuelve con error, que para medir vale igual, pero
/// una capability ajena no es cosa de un instrumento.
#[inline(never)]
unsafe fn puertas(cap: u64, op: u64, a0: u64, n: u64) {
    core::arch::asm!(
        "2:",
        "mov eax, {nr}",
        "mov rdi, {cap}",
        "mov rsi, {op}",
        // ** `a0` EN REGISTRO Y NO `xor edx, edx`, desde la segunda sonda.
        //
        // Estaba a cero fijo, y eso significaba que **ninguna fila podia pasar
        // argumento**. La fila que tenia que medir `OP_INFO` sobre un campo de
        // verdad acabo midiendo `INFO` sobre el campo 0, que no existe: un
        // rechazo por el brazo por defecto. Un instrumento que no deja
        // expresar la pregunta contesta otra.
        "mov rdx, {a0}",
        "xor r10d, r10d",
        "xor r8d, r8d",
        "syscall",
        "dec {cnt}",
        "jnz 2b",
        nr = const NR_INVOKE,
        cap = in(reg) cap,
        op = in(reg) op,
        a0 = in(reg) a0,
        cnt = inout(reg) n => _,
        out("rax") _,
        out("rdi") _,
        out("rsi") _,
        out("rdx") _,
        out("r10") _,
        out("r8") _,
        out("rcx") _,
        out("r11") _,
        options(nostack),
    );
}

/// El mismo bucle **sin la puerta**: lo que cuesta contar hasta `n`.
///
/// Se resta de todo lo demas. Y es la fila que hace comparables este programa y
/// el de C: alli el bucle cuesta 43 ciclos porque nadie lo optimiza, aqui son
/// dos instrucciones. **Sin esta fila, comparar los dos totales seria comparar
/// dos cosas distintas.**
#[inline(never)]
unsafe fn vacio(n: u64) {
    core::arch::asm!(
        "2:",
        "dec {cnt}",
        "jnz 2b",
        cnt = inout(reg) n => _,
        options(nostack, nomem),
    );
}

/// Un `rdtsc` suelto por vuelta: **la factura del propio instrumento.**
///
/// En el Ryzen esto dio 69 ciclos, contra los ~25 que la estimacion daba por
/// buenos. Esa diferencia decidio sacar cuatro sellos del stub del kernel, asi
/// que esta fila no es adorno: es la que convierte una correccion en una resta.
#[inline(never)]
unsafe fn rdtsc_suelto(n: u64) {
    core::arch::asm!(
        "2:",
        "rdtsc",
        "dec {cnt}",
        "jnz 2b",
        cnt = inout(reg) n => _,
        out("rax") _,
        out("rdx") _,
        options(nostack, nomem),
    );
}

// ============================ LA MEDIDA ============================

// ===================== LAS CUATRO GENERACIONES =====================
//
// ** ESTO NO ES ESTILO, ES EL ARREGLO DE UN EXPERIMENTO MAL HECHO.
//
// La pregunta abierta desde el 16-08 es: **por que resolver un handle anade
// ~246 ciclos FUERA de `dispatch`**, si el stub no sabe que operacion se pidio.
// Cuatro tandas sin contestarla, y no por falta de precision: por como estaba
// planteada la comparacion.
//
//     fila A:  pseudo-capability + operacion barata   (TAREA_ACTUAL, PID)
//     fila B:  capability REAL   + operacion gorda    (handle, TAMANO)
//
// **Cambia DOS variables a la vez.** Los +246 pueden ser el handle o pueden ser
// la operacion, y esa resta no puede separarlos. Una tanda mas de lo mismo iba
// a dar el mismo numero y la misma duda.
//
// El reparto de abajo es lo que lo desbloquea, y cada nivel ignora al de
// encima -- que es lo unico que hace que un reparto sea un reparto:
//
//     abuelo   `puertas`     N cruces y nada mas. No sabe que mide.
//     padre    `Fila`        un nombre, una capability, una operacion.
//                            No sabe que hay otras filas.
//     hijo     `contra`      la diferencia entre dos filas.
//                            No sabe que significa.
//     nieto    `bmo-juicio`  el veredicto. Vive FUERA de este binario y se
//                            prueba en el anfitrion.
//
// Con eso, las filas se eligen para que **entre dos consecutivas cambie UNA
// SOLA COSA**, y la resta pasa a significar algo:
//
//     1  TAREA_ACTUAL + PID      el suelo: no se camina ninguna tabla
//     2  TAREA_ACTUAL + INFO     MISMA capability, operacion mas gorda
//     3  handle real  + TAMANO   capability REAL
//
//     2 - 1  =  lo que cuesta una operacion mas complicada
//     3 - 2  =  lo que cuesta tener un handle de verdad
//
// Si el salto esta en `2 - 1`, los 246 nunca fueron del handle: son de que
// `OP_INFO` hace mas trabajo. Si esta en `3 - 2`, es el camino de la
// capability y hay un fallo donde se dijo.

/// **El PADRE**: una fila medible. Un nombre y la pareja que la define.
///
/// No sabe que hay otras filas ni que alguien va a restarla: si lo supiera,
/// anadir una cuarta obligaria a tocar esto.
struct Fila {
    nombre: &'static str,
    cap: u64,
    op: u64,
    /// El primer argumento. Existe porque sin el la fila de `OP_INFO` no puede
    /// nombrar su campo, y una fila que no puede expresar su pregunta mide
    /// otra cosa sin avisar.
    a0: u64,
}

/// **El HIJO**: la diferencia entre dos filas, que es lo que de verdad se
/// pregunta. `None` si sale al reves -- no se imprime una resta que dio la
/// vuelta.
fn contra(mayor: u64, menor: u64) -> Option<u64> {
    mayor.checked_sub(menor)
}

/// Minimo y media por operacion sobre [`VUELTAS`] bloques de [`LOTE`].
///
/// ** EL MINIMO ES LA RESPUESTA Y LA MEDIA ES EL SEGUNDO DATO. El planificador
/// expropia en el borde de cada trap, o sea que toda puerta es una oportunidad
/// de cambio de tarea: el minimo es el unico valor que eso no puede inflar. Y
/// la media se devuelve igual porque **la diferencia entre las dos ES la
/// expropiacion**, que es una cifra util por su cuenta.
fn medir(cuerpo: impl Fn(u64)) -> (u64, u64) {
    let mut mejor = 0u64;
    let mut total = 0u64;
    for _ in 0..VUELTAS {
        let t0 = bmo::ciclos();
        cuerpo(LOTE);
        let dt = bmo::ciclos().wrapping_sub(t0);
        if mejor == 0 || dt < mejor {
            mejor = dt;
        }
        total = total.wrapping_add(dt);
    }
    (mejor / LOTE, (total / VUELTAS) / LOTE)
}

// ============================ LA SALIDA ============================

/// Una linea de consola sobre pila. Sin `alloc` no hay `format!`.
struct Linea {
    buf: [u8; 160],
    n: usize,
}

impl Linea {
    const fn nueva() -> Self {
        Self { buf: [0; 160], n: 0 }
    }
    /// Vuelca y vacia. **Cada llamada a esto cruza la puerta** -- por eso
    /// ninguna ocurre mientras una ventana de medida esta abierta.
    fn soltar(&mut self) {
        if let Ok(s) = core::str::from_utf8(&self.buf[..self.n]) {
            bmo::consola(s);
        }
        self.n = 0;
    }
}

impl core::fmt::Write for Linea {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.as_bytes() {
            if self.n < self.buf.len() {
                self.buf[self.n] = *b;
                self.n += 1;
            }
        }
        Ok(())
    }
}

macro_rules! di {
    ($l:expr, $($t:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($l, $($t)*);
        $l.soltar();
    }};
}

/// **Lo mismo, en la unidad que se siente.** Ticks es lo que se mide; ciclos es
/// lo que le cuesta al CPU; nanosegundos es lo que espera el que llama.
///
/// Las tres se dicen juntas o no se dice ninguna, y la conversion vive en
/// `bmo-juicio` --con pruebas de anfitrion-- por la misma razon que el
/// veredicto: es una regla sobre numeros, y una regla sobre numeros no se
/// comprueba flasheando.
fn decir_ciclos(l: &mut Linea, etiqueta: &str, ticks: u64, r: &juez::Reloj) {
    match (r.ciclos(ticks), r.nanos(ticks)) {
        (Some(c), Some(ns)) => di!(l, "{etiqueta} = {c} ciclos de nucleo = {ns} ns\n"),
        // El tiempo SI se puede dar sin el reloj del nucleo: para eso el TSC es
        // invariante. Se da lo que se sabe y se calla lo que no.
        (None, Some(ns)) => di!(l, "{etiqueta} = {ns} ns (sin MPERF: los ciclos no se saben)\n"),
        _ => di!(l, "{etiqueta}: sin reloj, ni ciclos ni tiempo\n"),
    }
}

/// Imprime el veredicto que el juez ya decidio. **Aqui no se juzga nada**: si
/// la decision estuviera repartida entre el juez y su impresora, la mitad de
/// las reglas no se podria probar en el anfitrion.
fn decir(l: &mut Linea, etiqueta: &str, v: juez::Veredicto) {
    use juez::Veredicto::*;
    match v {
        // ** DOS MOTIVOS DISTINTOS PARA EL MISMO CERO, y se distinguen.
        //
        // El kernel contesta `sin declarar` en dos casos: no tiene la fila, o
        // **la tiene medida en otra maquina**. Lo segundo no es una carencia:
        // es el trinquete negandose a condenar con ticks de otro CPU, que es lo
        // correcto. Decir "no declara presupuesto" en ese caso mandaria a
        // buscar una tabla que existe y esta bien.
        SinDeclarar if bmo::info(bmo::INFO_PRESUPUESTO_MAQUINA) & bmo::MAQ_COINCIDE == 0 => {
            let v = bmo::info(bmo::INFO_PRESUPUESTO_MAQUINA);
            di!(l, "   {etiqueta} [-] SIN TRINQUETE: el presupuesto es de OTRA maquina\n");
            // ** Y LOS DOS LADOS, que es lo que convierte un "no" en un arreglo
            // de una cifra. Se imprimen solo aqui: en la maquina buena esta
            // linea no sale y el informe no se llena de identidades.
            di!(
                l,
                "     esperaba CPU {:02x}h/{:02x}h y hay {:02x}h/{:02x}h; cpu {} tsc {}\n",
                (v >> 8) & 0xFF,
                (v >> 16) & 0xFF,
                (v >> 24) & 0xFF,
                (v >> 32) & 0xFF,
                if v & bmo::MAQ_CPU_OK != 0 { "ok" } else { "NO" },
                if v & bmo::MAQ_TSC_OK != 0 { "ok" } else { "NO" },
            );
        }
        SinDeclarar => di!(l, "   {etiqueta} [-] este kernel no declara presupuesto\n"),
        SePasa { medido, techo } => {
            di!(l, "   {etiqueta} [SE PASA] {medido} > techo {techo} -- REGRESION\n")
        }
        EnPlazo { medido, techo, meta, faltan } => di!(
            l,
            "   {etiqueta} [EN PLAZO] {medido}, techo {techo}, meta {meta} -- faltan {faltan}\n"
        ),
        Meta { medido, meta } => di!(l, "   {etiqueta} [META] {medido}, por debajo de {meta}\n"),
        Roto(r) => di!(l, "   {etiqueta} [ROTO] {r:?} -- NO HAY VEREDICTO\n"),
    }
}

// ============================ EL PROGRAMA ============================

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut l = Linea::nueva();
    di!(l, "COSTE(rust): cuanto vale una puerta\n");

    // -- ** LOS DOS RELOJES, Y CUAL CUENTA CADA COSA -------------------
    //
    // `rdtsc` cuenta TICKS del TSC, que es invariante: va a la frecuencia BASE
    // pase lo que pase con el boost. El nucleo va a otra. Llamar "ciclos" a lo
    // que mide `rdtsc` es un error del 22% en esta maquina -- lo destapo el
    // propio panel el 16-08, poniendo los dos numeros uno al lado del otro.
    //
    // Se leen ANTES de abrir ninguna ventana de medida: son dos puertas, y una
    // puerta dentro de la ventana es lo que ya contamino una tanda entera.
    let reloj = juez::Reloj {
        tsc_hz: bmo::info(bmo::INFO_TSC_HZ),
        nucleo_hz: bmo::info(bmo::INFO_CPU_HZ_REAL),
    };
    di!(
        l,
        "TSC {} MHz (lo que cuenta rdtsc), nucleo {} MHz (a lo que va el CPU)\n",
        reloj.tsc_hz / 1_000_000,
        reloj.nucleo_hz / 1_000_000
    );
    match reloj.centesimas() {
        Some(c) => di!(l, "un tick = {},{:02} ciclos. LOS PRESUPUESTOS VAN EN TICKS\n", c / 100, c % 100),
        // Sin MPERF/APERF no hay conversion, y no se rellena con la frecuencia
        // base: eso daria `ciclos == ticks`, o sea la afirmacion de que el
        // nucleo no hace boost -- que es justo lo que no se sabe.
        None => di!(l, "sin MPERF/APERF: se dan TICKS y nada mas\n"),
    }
    di!(l, "lote {LOTE}, vueltas {VUELTAS}\n");

    // -- 1. el bucle, para poder restarlo -----------------------------
    let (vacio_min, vacio_media) = medir(|n| unsafe { vacio(n) });
    di!(l, "1. bucle vacio   min {vacio_min} ticks/op, media {vacio_media}\n");

    // -- 2. la puerta pelada ------------------------------------------
    //
    // ** DESDE AQUI Y HASTA QUE LOS CUATRO CONTADORES ESTEN LEIDOS NO SE
    // IMPRIME NADA. Esa es la firma de `cerrada_sin_imprimir`, y es la regla que
    // el 16-08 no estaba escrita: `consola()` cruza la puerta, y una puerta de
    // consola dibuja glifos y hace scroll -- ~2,2 M ciclos que caerian dentro de
    // la ventana y la llenarian de algo que no se estaba midiendo.
    let puertas0 = bmo::info(bmo::INFO_SYSCALL_CUENTA);
    let ciclos0 = bmo::info(bmo::INFO_SYSCALL_CICLOS);
    let (pelada_min, pelada_media) = medir(|n| unsafe { puertas(TAREA_ACTUAL, OP_PID, 0, n) });
    let puertas_d = bmo::info(bmo::INFO_SYSCALL_CUENTA) - puertas0;
    let ciclos_d = bmo::info(bmo::INFO_SYSCALL_CICLOS) - ciclos0;
    // -- la ventana esta cerrada; a partir de aqui se puede imprimir --

    let medida = juez::Medida {
        min: pelada_min,
        media: pelada_media,
        puertas: puertas_d,
        ciclos_dispatch: ciclos_d,
        cerrada_sin_imprimir: true,
    };

    di!(l, "2. puerta pelada min {pelada_min} ticks/op, media {pelada_media}\n");
    decir_ciclos(&mut l, "   una puerta", pelada_min, &reloj);
    match medida.dispatch_medio() {
        Some(d) => match juez::reparto(pelada_min, d) {
            juez::Reparto::Stub(stub) => {
                di!(l, "   reparto: dentro de dispatch {d}, en el stub {stub}\n")
            }
            // ** EL CASO DEL 17-08. Con el metro retirado `dispatch` vale 0 y
            // la resta daba el total entero, impreso como "en el stub 792":
            // una medida que nadie tomo, con la forma de la respuesta que se
            // estaba buscando. Ahora se dice que no se midio.
            juez::Reparto::NoMedido => di!(
                l,
                "   reparto: NO MEDIDO -- el metro esta retirado (`--features metro_puerta`)\n"
            ),
            // [!] No se imprime una resta que dio la vuelta. `dispatch` es una
            // MEDIA y el total un MINIMO, asi que esto PUEDE pasar sin que nada
            // este roto -- y el 16-08 se creyo lo contrario.
            juez::Reparto::MediaSobreMinimo => {
                di!(l, "   reparto: dispatch {d} > el minimo total; media contra minimo\n")
            }
        },
        None => di!(l, "   reparto: el kernel no conto ni una puerta\n"),
    }
    decir(&mut l, "puerta  ", juez::juzgar(
        &medida,
        pelada_min,
        juez::Presupuesto::desempaquetar(bmo::info(bmo::INFO_PRESUPUESTO_PUERTA)),
    ));
    if let Some(d) = medida.dispatch_medio() {
        decir(&mut l, "dispatch", juez::juzgar(
            &medida,
            d,
            juez::Presupuesto::desempaquetar(bmo::info(bmo::INFO_PRESUPUESTO_DISPATCH)),
        ));
    }

    // -- 3. LA SONDA: una variable por escalon ------------------------
    //
    // Las tres filas comparten el MISMO bucle de nueve instrucciones, con la
    // capability y la operacion en registros. O sea que el lado de Ring 3 es
    // **byte a byte identico** en las tres y no puede explicar ninguna
    // diferencia -- que es justo lo que el programa de C no podia garantizar,
    // porque alli la fila del handle lee una variable y la otra un literal.
    let paquete = bmo::invoke(TAREA_ACTUAL, OP_MI_PAQUETE as u32, 0, 0, 0).value;
    let filas = [
        Fila { nombre: "1 pid    (pseudo-cap, op barata)", cap: TAREA_ACTUAL, op: OP_PID, a0: 0 },
        Fila {
            nombre: "2 ticks  (misma cap, op gorda)  ",
            cap: TAREA_ACTUAL,
            op: OP_INFO,
            a0: CAMPO_TICKS,
        },
        Fila { nombre: "3 tamano (cap REAL, op gorda)   ", cap: paquete, op: ARCH_TAMANO, a0: 0 },
    ];

    let mut minimos = [0u64; 3];
    let mut medias = [0u64; 3];
    let mut dispatches = [0u64; 3];
    for (i, f) in filas.iter().enumerate() {
        // La fila 3 solo existe si el kernel recuerda nuestra imagen. Un `.bex`
        // lanzado con `run` siempre la tiene; embebido, no.
        if f.cap == 0 {
            continue;
        }
        // ** `dispatch` POR FILA, que es lo que faltaba.
        //
        // La sonda anterior contesto que el handle cuesta 217 y la operacion
        // solo 33 -- o sea, que es el handle. Pero no podia decir **donde**
        // estan esos 217, porque media `dispatch` una sola vez, para la puerta
        // pelada. Y esa diferencia lo decide todo:
        //
        //   dentro de `dispatch`  -> resolver una capability es caro, pero es
        //                            codigo Rust normal. NO hay anomalia.
        //   fuera de `dispatch`   -> hay un fallo, porque el stub no sabe que
        //                            operacion se pidio.
        let p0 = bmo::info(bmo::INFO_SYSCALL_CUENTA);
        let c0 = bmo::info(bmo::INFO_SYSCALL_CICLOS);
        let (min, media) = medir(|n| unsafe { puertas(f.cap, f.op, f.a0, n) });
        let pd = bmo::info(bmo::INFO_SYSCALL_CUENTA) - p0;
        let cd = bmo::info(bmo::INFO_SYSCALL_CICLOS) - c0;
        minimos[i] = min;
        medias[i] = media;
        dispatches[i] = if pd > 0 { cd / pd } else { 0 };
    }
    // ** SE IMPRIME DESPUES DE MEDIR LAS TRES, y no dentro del bucle.
    //
    // Si se imprimiera dentro, la ventana de la fila 2 llevaria dentro las
    // puertas de consola de la fila 1 -- que es EXACTAMENTE el fallo que hizo
    // que `dispatch` pareciera 309 durante cuatro tandas. La regla ya se pago
    // una vez; aqui se aplica antes de que cueste la segunda.
    for (i, f) in filas.iter().enumerate() {
        if minimos[i] == 0 {
            continue;
        }
        di!(
            l,
            "{}  min {} media {}, dispatch {}, fuera {}\n",
            f.nombre,
            minimos[i],
            medias[i],
            dispatches[i],
            contra(minimos[i], dispatches[i]).unwrap_or(0)
        );
    }

    // -- las dos restas, que es la sonda de verdad ---------------------
    match contra(minimos[1], minimos[0]) {
        Some(d) => {
            di!(l, "   fila2-fila1 = {d} ticks <- lo que cuesta una operacion mas gorda\n");
            decir_ciclos(&mut l, "     esa operacion", d, &reloj);
        }
        None => di!(l, "   fila2-fila1: al reves; la operacion gorda salio mas barata\n"),
    }
    if minimos[2] != 0 {
        match contra(minimos[2], minimos[1]) {
            Some(d) => {
                di!(l, "   fila3-fila2 = {d} ticks <- lo que cuesta un HANDLE de verdad\n");
                decir_ciclos(&mut l, "     ese handle", d, &reloj);
            }
            None => di!(l, "   fila3-fila2: al reves; el handle salio mas barato\n"),
        }
        // ** Y LA RESTA QUE CIERRA LA PREGUNTA: de lo que cuesta el handle,
        // cuanto cae dentro de `dispatch` y cuanto fuera. Dentro es codigo
        // caro; fuera es un fallo.
        let dentro = contra(dispatches[2], dispatches[1]).unwrap_or(0);
        let total = contra(minimos[2], minimos[1]).unwrap_or(0);
        di!(
            l,
            "   ...de esos, {} dentro de dispatch y {} FUERA\n",
            dentro,
            contra(total, dentro).unwrap_or(0)
        );
        bmo::invoke(paquete, ARCH_CERRAR as u32, 0, 0, 0);
    } else {
        di!(l, "   fila 3 NO SE MIDIO: el kernel no recuerda mi imagen\n");
    }

    // -- 4. la factura del instrumento --------------------------------
    let (tsc_min, _) = medir(|n| unsafe { rdtsc_suelto(n) });
    di!(l, "4. rdtsc suelto  min {tsc_min} ticks/op (menos la fila 1 = un sello)\n");
    decir_ciclos(&mut l, "   un sello", tsc_min.saturating_sub(vacio_min), &reloj);

    // -- ** 5. EL TRAFICO: cuantas veces se pide cada clase de puerta --
    //
    // ** ESTA ES LA MITAD QUE FALTABA PARA PODER PRIORIZAR, y estaba escrita en
    // el kernel desde el 16-08 sin que nadie la leyera.
    //
    //     coste real por segundo  =  coste por vez  x  VECES por segundo
    //
    // Las cuatro filas de arriba dan el primer factor. Sin el segundo, "por
    // donde empiezo" es una intuicion -- y la intuicion, en este arbol, ya se
    // equivoco dos veces con este mismo camino. El kernel clasifica cada puerta
    // en `dispatch` sin ninguna lista, con tres hechos que ya estan en
    // registros (ver `syscall/mod.rs`).
    //
    // [!] Es el trafico DESDE EL ARRANQUE, no el de ahora: dice en que se ha
    // gastado la sesion entera, incluido el arranque del escritorio. Para "que
    // esta haciendo AHORA" haria falta restar dos lecturas separadas por un
    // segundo, y eso es otro instrumento.
    let total = bmo::info(bmo::INFO_SYSCALL_CUENTA);
    let mut suma = 0u64;
    di!(l, "5. el trafico de puertas desde el arranque: {total}\n");
    for (clase, nombre) in [
        (bmo::SYSCALL_CLASS_TASK, "tarea  "),
        (bmo::SYSCALL_CLASS_HANDLE, "handle "),
        (bmo::SYSCALL_CLASS_CONSOLE, "consola"),
        (bmo::SYSCALL_CLASS_WAIT, "wait   "),
    ] {
        // El indice va EMPAQUETADO en el campo, como `INFO_MEM_QUIEN_*`.
        let n = bmo::info(bmo::INFO_SYSCALL_CLASS | (clase << 8));
        suma = suma.saturating_add(n);
        // Porcentaje en decimas: sin coma flotante, y una clase que es el 0,4%
        // no puede salir como "0%" cuando es la que hay que mirar.
        let d = if total > 0 { n.saturating_mul(1000) / total } else { 0 };
        di!(l, "   {nombre} {n}  ({},{}%)\n", d / 10, d % 10);
    }
    // ** LA RESTA ES LA COMPROBACION DEL INSTRUMENTO, y por eso se imprime
    // aunque sea cero. Lo que no cae en ninguna casilla es la puerta RETIRADA,
    // a la que no se le invento una: si esto sale grande, hay trafico que el
    // reparto no esta viendo, y entonces los porcentajes de arriba no valen.
    match total.checked_sub(suma) {
        Some(fuera) => di!(l, "   sin casilla {fuera}  <- tiene que ser pequeno\n"),
        None => di!(l, "   AVISO: las clases suman MAS que el total -- NO LEER\n"),
    }

    di!(l, "COSTE(rust): la fila 2 menos la 1 = la puerta desnuda\n");
    bmo::salir();
}

#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    bmo::consola("panico en coste\n");
    let mut l = Linea::nueva();
    {
        use core::fmt::Write;
        let _ = write!(l, "{}\n", info.message());
    }
    l.soltar();
    bmo::salir();
}
