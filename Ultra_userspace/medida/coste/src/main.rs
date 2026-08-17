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
unsafe fn puertas(cap: u64, op: u64, n: u64) {
    core::arch::asm!(
        "2:",
        "mov eax, {nr}",
        "mov rdi, {cap}",
        "mov rsi, {op}",
        "xor edx, edx",
        "xor r10d, r10d",
        "xor r8d, r8d",
        "syscall",
        "dec {cnt}",
        "jnz 2b",
        nr = const NR_INVOKE,
        cap = in(reg) cap,
        op = in(reg) op,
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

/// Imprime el veredicto que el juez ya decidio. **Aqui no se juzga nada**: si
/// la decision estuviera repartida entre el juez y su impresora, la mitad de
/// las reglas no se podria probar en el anfitrion.
fn decir(l: &mut Linea, etiqueta: &str, v: juez::Veredicto) {
    use juez::Veredicto::*;
    match v {
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
    di!(l, "TSC {} Hz, lote {LOTE}, vueltas {VUELTAS}\n", bmo::info(bmo::INFO_TSC_HZ));

    // -- 1. el bucle, para poder restarlo -----------------------------
    let (vacio_min, vacio_media) = medir(|n| unsafe { vacio(n) });
    di!(l, "1. bucle vacio   min {vacio_min} ciclos/op, media {vacio_media}\n");

    // -- 2. la puerta pelada ------------------------------------------
    //
    // ** DESDE AQUI Y HASTA QUE LOS CUATRO CONTADORES ESTEN LEIDOS NO SE
    // IMPRIME NADA. Esa es la firma de `cerrada_sin_imprimir`, y es la regla que
    // el 16-08 no estaba escrita: `consola()` cruza la puerta, y una puerta de
    // consola dibuja glifos y hace scroll -- ~2,2 M ciclos que caerian dentro de
    // la ventana y la llenarian de algo que no se estaba midiendo.
    let puertas0 = bmo::info(bmo::INFO_SYSCALL_CUENTA);
    let ciclos0 = bmo::info(bmo::INFO_SYSCALL_CICLOS);
    let (pelada_min, pelada_media) = medir(|n| unsafe { puertas(TAREA_ACTUAL, OP_PID, n) });
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

    di!(l, "2. puerta pelada min {pelada_min} ciclos/op, media {pelada_media}\n");
    match medida.dispatch_medio() {
        Some(d) => match juez::stub_desde(pelada_min, d) {
            Some(stub) => di!(l, "   reparto: dentro de dispatch {d}, en el stub {stub}\n"),
            // [!] No se imprime una resta que dio la vuelta. `dispatch` es una
            // MEDIA y el total un MINIMO, asi que esto PUEDE pasar sin que nada
            // este roto -- y el 16-08 se creyo lo contrario.
            None => di!(l, "   reparto: dispatch {d} > el minimo total; media contra minimo\n"),
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

    // -- 3. la factura del instrumento --------------------------------
    let (tsc_min, _) = medir(|n| unsafe { rdtsc_suelto(n) });
    di!(l, "3. rdtsc suelto  min {tsc_min} ciclos/op (menos la fila 1 = un sello)\n");

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
