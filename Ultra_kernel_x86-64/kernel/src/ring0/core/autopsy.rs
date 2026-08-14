//! **La AUTOPSIA de un fallo de Ring 3.** Lo que el kernel guarda cuando mata
//! una tarea, para que se pueda leer despues y mandar.
//!
//! # Por que existe, y por que aqui
//!
//! El aislamiento de faults ya funcionaba: una tarea de Ring 3 revienta, el
//! kernel le quita las capabilities, la marca muerta y **BMO sigue vivo**. Eso
//! se ve en CABINA como una linea roja:
//!
//! ```text
//!   FAULT ring3: fault en CPL3: tarea eliminada, BMO sigue vivo =4000105
//! ```
//!
//! Una linea. Con el `rip` y nada mas. Y eso alcanza para saber QUE paso y no
//! para saber DONDE: falta el vector, el codigo de error, la direccion que se
//! toco, la pila, y sobre todo **de que programa se trataba**.
//!
//! Sin esas cinco cosas, un fallo en la maquina del dueno no se puede mandar a
//! nadie: se cuenta de memoria, y contar un fallo de memoria es como se pierden
//! los fallos. Con ellas, la maquina redacta su propio informe.
//!
//! Esto es lo que el README llama el "meta" del metakernel, y esta es su forma
//! mas literal: **el sistema deja escrito lo que le paso a el mismo.**
//!
//! # La regla que decide el diseno: aqui NO se toca el disco
//!
//! La tentacion es escribir el informe a un fichero desde el propio manejador
//! de faults. No se hace, y el motivo no es prudencia general:
//!
//! * Se corre **dentro de un fault**, con la pila del kernel y sin saber que
//!   estado dejo el que fallo. Escribir a disco ahi es entrar en el driver de
//!   AHCI, que tiene esperas y estado propio.
//! * Y el fallo **puede ser del disco**. Un informe que necesita el subsistema
//!   que acaba de caerse no es un informe: es un segundo fallo encima del
//!   primero, y del que no queda nada escrito.
//!
//! Asi que el kernel **captura en RAM** --que es barato, acotado y no puede
//! fallar-- y quien lo persiste es Ring 3, que esta vivo, tiene la capability
//! de escribir y puede permitirse tardar. Es la misma division que el resto del
//! sistema: el kernel CONTESTA, no actua por cuenta de nadie.

use crate::ring0::plat::timer;

/// Cuantas autopsias se guardan. Cuatro porque un fallo que se repite lo hace
/// en rafaga --el mismo programa relanzado tres veces-- y lo que interesa es
/// tener la primera Y la ultima: si son iguales es determinista, y si no, algo
/// del entorno cambio entre medias.
const CUANTAS: usize = 4;
/// Renglones por informe.
// Diez desde el 2026-08-13: el decimo es la PILA, y llego por un fallo que el
// informe de nueve no podia explicar. Ver la nota sobre `pila` mas abajo.
// ONCE desde el 2026-08-14: el undecimo es el VEREDICTO, y llego por lo
// contrario -- un fallo que el informe de diez SI podia explicar y no explico.
const RENGLONES: usize = 11;

/// **Una palabra de la pila de un proceso muerto, o `None`.**
///
/// === Por que esto NO es un `read_volatile` a pelo ===
///
/// Porque corre DENTRO del manejador de fallos, y la memoria que va a leer es
/// la del proceso que acaba de romperse. Si su `rsp` era basura --que es
/// exactamente el caso interesante-- leerlo sin comprobar produce un segundo
/// fallo con el primero a medio informar, y entonces no hay informe.
///
/// Se comprueba lo unico que se puede comprobar sin caminar las tablas: que la
/// direccion sea CANONICA y que caiga en el rango de una pila de Ring 3. Un
/// hueco en el informe es una respuesta; un triple fault no.
fn leer_palabra_de_ring3(dir: u64) -> Option<u64> {
    // Canonica: los 17 bits altos iguales. Una direccion de Ring 3 ademas vive
    // por debajo de la mitad del espacio.
    if dir >> 47 != 0 {
        return None;
    }
    if dir & 7 != 0 {
        return None;
    }
    // La pila de un proceso de Ring 3 se reserva por debajo de `0x8000_0000`;
    // fuera de ahi no se lee, aunque fuera canonica.
    if dir < 0x1000 || dir >= 0x8000_0000 {
        return None;
    }
    Some(unsafe { core::ptr::read_volatile(dir as *const u64) })
}
/// Ancho de cada renglon. El de la ventana de datos, para que quepa sin cortar.
const ANCHO: usize = 72;

struct Autopsia {
    texto: [[u8; ANCHO]; RENGLONES],
    largo: [u8; RENGLONES],
    usados: u8,
}

static mut ANILLO: [Autopsia; CUANTAS] = [const {
    Autopsia {
        texto: [[0; ANCHO]; RENGLONES],
        largo: [0; RENGLONES],
        usados: 0,
    }
}; CUANTAS];
static mut WRITES: usize = 0;
/// Cuantas van desde el arranque. **No se reinicia**: es el numero que Ring 3
/// compara para saber si hay una nueva sin tener que leerla entera.
static mut TOTAL: u32 = 0;
/// Guarda contra reentrada: un fault dentro del manejador de faults no puede
/// volver a entrar aqui a medio escribir.
static mut DENTRO: bool = false;
/// **Recursos que un muerto dejo sin devolver, acumulados.** Tiene que ser
/// CERO, y por eso vale: es el kernel comprobandose a si mismo. Sube en `info`
/// al lado de los choques de cerrojo, que son la misma clase de numero.
static mut FUGAS_TOTAL: u32 = 0;

/// Un renglon en construccion. Sin `format!` ni asignaciones: esto corre en un
/// manejador de faults, donde el asignador puede ser justo lo que se rompio.
struct Renglon {
    b: [u8; ANCHO],
    n: usize,
}

impl Renglon {
    fn nuevo() -> Self {
        Self { b: [0; ANCHO], n: 0 }
    }
    fn s(&mut self, t: &str) {
        for &c in t.as_bytes() {
            if self.n < ANCHO {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    fn bytes(&mut self, t: &[u8]) {
        for &c in t {
            if c == 0 {
                break;
            }
            if self.n < ANCHO {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    /// Hexadecimal con `0x` y sin ceros de mas. Un `rip` con doce ceros
    /// delante es doce caracteres que no dicen nada y una linea que no cabe.
    fn hex(&mut self, v: u64) {
        self.s("0x");
        let mut visto = false;
        for i in (0..16).rev() {
            let d = ((v >> (i * 4)) & 0xF) as u8;
            if d != 0 {
                visto = true;
            }
            if visto || i == 0 {
                self.b[self.n.min(ANCHO - 1)] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
                if self.n < ANCHO {
                    self.n += 1;
                }
            }
        }
    }
    fn dec(&mut self, mut v: u64) {
        let mut cifras = [0u8; 20];
        let mut c = 0;
        if v == 0 {
            cifras[0] = b'0';
            c = 1;
        }
        while v > 0 && c < 20 {
            cifras[c] = b'0' + (v % 10) as u8;
            v /= 10;
            c += 1;
        }
        for i in (0..c).rev() {
            if self.n < ANCHO {
                self.b[self.n] = cifras[i];
                self.n += 1;
            }
        }
    }
}

/// El nombre de la excepcion. Un numero de vector no se lo sabe nadie de
/// memoria, y la diferencia entre un `#PF` y un `#GP` es la primera pregunta
/// que se hace quien lee el informe.
fn nombre_vector(v: u64) -> &'static str {
    match v {
        0 => "#DE division por cero",
        6 => "#UD instruccion invalida",
        8 => "#DF doble falta",
        11 => "#NP segmento ausente",
        12 => "#SS fallo de pila",
        13 => "#GP proteccion general",
        14 => "#PF fallo de pagina",
        16 => "#MF error x87",
        19 => "#XM error SSE",
        _ => "excepcion",
    }
}

/// **Una direccion como `+desplazamiento` dentro de la imagen del programa.**
///
/// Devuelve `false` --y no escribe nada-- si la direccion no cae en la imagen.
///
/// ** Este numero es el que `--map` del compilador convierte en un nombre de
/// funcion (`071c891e`). El kernel siempre tuvo el `rip` absoluto y la base de
/// la imagen es una constante suya, asi que la resta se podia hacer desde el
/// principio; se hacia a mano, en cada informe, con una calculadora. Hacerla
/// aqui es lo que cierra el circuito entre la autopsia y el mapa.
fn en_la_imagen(dir: u64, r: &mut Renglon) -> bool {
    use crate::ring0::mm::vmm::{USER_IMAGE_BASE, USER_STACK_BOTTOM};
    if dir >= USER_IMAGE_BASE && dir < USER_STACK_BOTTOM {
        r.s("+");
        r.hex(dir - USER_IMAGE_BASE);
        return true;
    }
    false
}

/// **EL VEREDICTO: por que paso, en una frase.**
///
/// # Por que hacia falta, y no era mas informacion
///
/// El informe de diez renglones ya llevaba TODAS las pruebas --vector, codigo
/// de error, `cr2`, `rsp`-- y aun asi el 2026-08-14 costo una tarde. El
/// compositor murio con `#PF` en `rip=0x4000001B` y el informe decia
/// exactamente eso: un vector, una direccion y un numero. Todo cierto y nada
/// concluido.
///
/// Y la conclusion estaba **enteramente dentro de los datos que ya tenia**:
/// `cr2` caia por debajo de `USER_STACK_BOTTOM`, el codigo de error decia
/// "escribiendo" y "pagina no presente". Eso es un desbordamiento de pila y no
/// puede ser otra cosa. El kernel tenia las tres piezas y no hacia la resta.
///
/// ** **Esa resta es la diferencia entre un jeroglifico y una frase**, y es lo
/// unico que separa "hay que bisecar seis commits" de "la pila se salio por
/// abajo". La regla que deja escrita: **cuando el informe tenga los datos para
/// deducir la causa, que la deduzca el kernel** -- quien lee un informe a las
/// tres de la manana no esta en condiciones de cruzar rangos de memoria.
///
/// # Lo que NO hace
///
/// No adivina. Cada rama de aqui abajo es una implicacion que se sostiene sola,
/// y cuando ninguna encaja **dice que no lo sabe** en vez de inventar la mas
/// probable. Un veredicto equivocado es peor que ninguno: manda a mirar al
/// sitio que no es, y con autoridad.
/// # Una sola clasificacion, dos salidas
///
/// El veredicto sale por dos sitios --la linea roja de la pantalla y el
/// renglon del informe-- y **la regla se escribe una vez**. Dos listas de
/// `if` que dijeran lo mismo se separarian el dia que se anada un caso, y
/// entonces la pantalla y el fichero acusarian a cosas distintas del mismo
/// fallo. Aqui `clasificar` decide, `nombre` pone las palabras, y solo el
/// informe largo anade los numeros.
#[derive(Clone, Copy, PartialEq)]
enum Causa {
    PilaDesbordada,
    PunteroNulo,
    EscrituraEnImagen,
    SaltoSinCodigo,
    SinMapear,
    NoEsInstruccion,
    Proteccion,
    Desconocida,
}

/// Cuanto por debajo de la pila cuenta todavia como "se salio de la pila".
/// 2 MiB: de sobra para el peor `sub rsp` de un marco grande, y muy lejos de la
/// imagen (`0x4000_0000`) y de los bloques pedidos, asi que un puntero basura
/// que caiga por ahi no puede confundirse con esto.
const VENTANA_PILA: u64 = 2 * 1024 * 1024;

fn clasificar(vector: u64, error: u64, cr2: u64) -> Causa {
    use crate::ring0::mm::vmm::{USER_IMAGE_BASE, USER_STACK_BOTTOM};
    if vector == 14 {
        if cr2 < USER_STACK_BOTTOM && USER_STACK_BOTTOM - cr2 <= VENTANA_PILA {
            return Causa::PilaDesbordada;
        }
        if cr2 < 0x1000 {
            return Causa::PunteroNulo;
        }
        // Escribir donde viven el codigo y las constantes: la imagen se mapea
        // de solo lectura, asi que una violacion de permisos escribiendo ahi
        // dentro es un puntero apuntando a la propia imagen.
        if error & 1 != 0 && error & 2 != 0 && cr2 >= USER_IMAGE_BASE && cr2 < USER_STACK_BOTTOM {
            return Causa::EscrituraEnImagen;
        }
        // El bit 4 dice que el CPU iba a BUSCAR una instruccion. Si ahi no hay
        // pagina, alguien salto a algo que no es codigo: puntero a funcion sin
        // inicializar, vtabla mal, o una direccion de retorno pisada.
        if error & 16 != 0 {
            return Causa::SaltoSinCodigo;
        }
        return Causa::SinMapear;
    }
    match vector {
        6 => Causa::NoEsInstruccion,
        13 => Causa::Proteccion,
        _ => Causa::Desconocida,
    }
}

fn nombre(c: Causa) -> &'static str {
    match c {
        Causa::PilaDesbordada => "*** PILA DESBORDADA",
        Causa::PunteroNulo => "*** PUNTERO NULO",
        Causa::EscrituraEnImagen => "*** ESCRITURA SOBRE CODIGO O CONSTANTES (solo lectura)",
        Causa::SaltoSinCodigo => "*** SALTO A MEMORIA QUE NO ES CODIGO: puntero de funcion",
        Causa::SinMapear => "*** SIN MAPEAR: puntero basura o indice fuera de rango",
        Causa::NoEsInstruccion => "*** SE EJECUTARON BYTES QUE NO SON UNA INSTRUCCION",
        Causa::Proteccion => "*** #GP: direccion no canonica o instruccion no permitida",
        // Y cuando no encaja ninguna, se dice. Ver la cabecera.
        Causa::Desconocida => "(sin veredicto: los datos de arriba no bastan para concluir)",
    }
}

/// **El veredicto para la linea roja de la pantalla**, sin numeros.
///
/// Se ve sin pedir nada y sin abrir un fichero; los numeros los tiene el
/// informe, que esta a un `fallo` de distancia.
pub fn veredicto_corto(vector: u64, error: u64, cr2: u64) -> &'static str {
    nombre(clasificar(vector, error, cr2))
}

fn veredicto(vector: u64, error: u64, cr2: u64, r: &mut Renglon) {
    use crate::ring0::mm::vmm::{USER_STACK_BOTTOM, USER_STACK_SIZE};
    let c = clasificar(vector, error, cr2);
    r.s(nombre(c));
    // Los numeros solo donde dicen algo que la frase no dice. En el
    // desbordamiento son LA respuesta: cuanto se paso y sobre cuanto.
    match c {
        Causa::PilaDesbordada => {
            // [!] Se dice DONDE cayo el toque, no cuanto pedia el marco. El
            // kernel no puede saber el tamano del marco: solo ve la primera
            // direccion que no estaba mapeada, que con la sonda de pila de LLVM
            // es la primera pagina que falta y no el fondo del marco. Decir
            // "pidio N" seria inventar un numero que nadie midio.
            r.s(": ");
            r.dec(USER_STACK_BOTTOM - cr2);
            r.s(" B bajo el fondo, pila ");
            r.dec(USER_STACK_SIZE);
        }
        Causa::PunteroNulo => {
            r.s(" en 0+");
            r.hex(cr2);
        }
        _ => {}
    }
}

/// Lo que el codigo de error de un `#PF` significa, en palabras. Son cuatro
/// bits y cada uno cambia el sitio donde hay que mirar.
fn causa_pf(err: u64, r: &mut Renglon) {
    r.s(if err & 1 == 0 { "pagina NO PRESENTE" } else { "violacion de permisos" });
    r.s(if err & 2 == 0 { ", leyendo" } else { ", escribiendo" });
    if err & 4 != 0 {
        r.s(", desde Ring 3");
    }
    if err & 16 != 0 {
        r.s(", buscando INSTRUCCIONES");
    }
}

/// **Guarda la autopsia.** Se llama desde el manejador de faults.
///
/// No devuelve nada y no puede fallar: si no cabe, se corta. Un informe a
/// medias sigue diciendo el vector y el `rip`, que es lo primero que se mira.
#[allow(clippy::too_many_arguments)]
pub fn registrar(
    vector: u64,
    error: u64,
    rip: u64,
    cr2: u64,
    rsp: u64,
    pid: u32,
    tid: u32,
) {
    unsafe {
        if DENTRO {
            return;
        }
        DENTRO = true;
    }

    let mut renglones: [Renglon; RENGLONES] = [
        Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(),
        Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(),
        Renglon::nuevo(), Renglon::nuevo(), Renglon::nuevo(),
    ];

    renglones[0].s("== FALLO EN RING 3 #");
    renglones[0].dec(unsafe { TOTAL } as u64 + 1);
    renglones[0].s("  t=");
    renglones[0].dec(timer::ticks());
    renglones[0].s("ms ==");

    renglones[1].s("programa  ");
    // El nombre del `.bex` que se lanzo. Es el dato que convierte "fallo algo"
    // en "fallo ESTO", y es justo el que la linea de CABINA no llevaba.
    let mut visto = false;
    for r in crate::ring0::task::proc::programs() {
        if r.pid == pid {
            renglones[1].s(r.name);
            visto = true;
            break;
        }
    }
    if !visto {
        renglones[1].s("(desconocido)");
    }
    renglones[1].s("   pid ");
    renglones[1].dec(pid as u64);
    renglones[1].s(" tid ");
    renglones[1].dec(tid as u64);

    renglones[2].s("causa     ");
    renglones[2].s(nombre_vector(vector));
    renglones[2].s("  (vector ");
    renglones[2].dec(vector);
    renglones[2].s(")");

    // ** EL VEREDICTO va JUNTO A LA CAUSA y antes que las pruebas.
    //
    // El orden de los renglones es el orden en que se leen, y quien abre una
    // autopsia quiere primero QUE fue y luego COMO se demuestra. Poner la
    // conclusion al final la deja debajo de siete lineas de hexadecimal, que es
    // justo donde no se lee.
    renglones[3].s("veredicto ");
    veredicto(vector, error, cr2, &mut renglones[3]);

    renglones[4].s("codigo    ");
    renglones[4].hex(error);
    if vector == 14 {
        renglones[4].s("  ");
        let (a, b) = renglones.split_at_mut(5);
        let _ = b;
        causa_pf(error, &mut a[4]);
    }

    // El `rip` Y SU DESPLAZAMIENTO EN LA IMAGEN, que es el numero que `--map`
    // del compilador convierte en un nombre de funcion. Estaba a una resta de
    // distancia y esa resta se hacia a mano en cada informe.
    renglones[5].s("rip       ");
    renglones[5].hex(rip);
    renglones[5].s("  ");
    if !en_la_imagen(rip, &mut renglones[5]) {
        renglones[5].s("(FUERA de la imagen)");
    }

    renglones[6].s("direccion ");
    renglones[6].hex(cr2);
    if vector == 14 {
        renglones[6].s("   lo que se intento tocar");
    }

    renglones[7].s("rsp       ");
    renglones[7].hex(rsp);

    // ** LA CIMA DE LA PILA, y esta linea nacio de un fallo concreto.
    //
    // El 2026-08-13 DOOM murio con `#GP` en `rip 0x400815f2`. Con `--map` del
    // compilador ese numero dijo la funcion --`SHA1_Update`+0x18-- y ahi se
    // acabo la pista: **nada llama a SHA1 en `I_Init`**, asi que lo que hubo no
    // fue SHA1 ejecutandose, sino un salto que aterrizo a mitad de su cuerpo.
    //
    // Y para saber QUIEN salto hace falta la pila, porque BMO pasa los
    // argumentos por ella y un `call` deja su direccion de retorno arriba. El
    // informe tenia el `rsp` y no lo que hay EN el `rsp`, que es como tener las
    // coordenadas del accidente y no la matricula.
    //
    // [!] Se leen cuatro palabras y con guarda: la pila de un proceso muerto es
    // memoria en la que ya no se confia, asi que una direccion no canonica o
    // sin mapear tiene que dar un hueco en el informe y no un segundo fallo
    // DENTRO del manejador de fallos.
    //
    // ** Y CADA PALABRA QUE APUNTA A LA IMAGEN SALE COMO `+desplazamiento`.
    //
    // Eso convierte esta linea en un RASTRO DE LLAMADAS: una palabra de la pila
    // que cae dentro de la imagen es, casi siempre, la direccion de retorno que
    // dejo un `call`. Con el `+0x...` delante, `--map` le pone nombre a cada
    // una y el informe pasa de decir donde se rompio a decir **quien llamo**.
    // Las que no apuntan a la imagen se dejan crudas: son datos, no matriculas.
    renglones[10].s("pila      ");
    let mut k = 0usize;
    while k < 4 {
        let dir = rsp.wrapping_add((k as u64) * 8);
        match leer_palabra_de_ring3(dir) {
            Some(v) => {
                if !en_la_imagen(v, &mut renglones[10]) {
                    renglones[10].hex(v);
                }
                renglones[10].s(" ");
            }
            None => {
                renglones[10].s("(ilegible) ");
                k = 4;
            }
        }
        k += 1;
    }

    // * Y LO QUE EL PROCESO DIJO ANTES DE MORIR.
    //
    // `uconsole` guarda las ultimas lineas que escribio cada proceso, y esa es
    // la unica pista sobre QUE ESTABA HACIENDO. El resto del informe dice donde
    // se rompio la maquina; esta linea dice por donde iba el programa.
    renglones[8].s("ultimo    ");
    if crate::ring0::uconsole::hubo_palabras(pid) {
        // `ultimas_palabras` entrega las que haya, de la mas vieja a la mas
        // nueva. Se queda la ULTIMA: es la que dice hasta donde llego.
        let mut ultima: [u8; ANCHO] = [0; ANCHO];
        let mut largo = 0usize;
        crate::ring0::uconsole::ultimas_palabras(pid, |l| {
            let b = l.as_bytes();
            let n = b.len().min(ANCHO);
            ultima[..n].copy_from_slice(&b[..n]);
            largo = n;
        });
        renglones[8].bytes(&ultima[..largo]);
    } else {
        renglones[8].s("(no escribio nada)");
    }

    // ** Y LA COMPROBACION DE QUE EL KERNEL RECUPERO LO SUYO.
    //
    // `revoke_all` corre ANTES de esto y hace su trabajo. Pero eso es lo que el
    // codigo DICE que hace, y hasta hoy **nadie miraba si funciono**.
    //
    // Una fuga de ranuras no da error: da un sistema que un dia no puede abrir
    // un directorio mas, sin nada que lo relacione con el proceso que murio
    // hace una hora. `AVANCES.md` la lleva abierta desde el 02-08 -- ranuras de
    // directorio que solo se liberan al morir, con un cliente (el escritorio)
    // que no muere nunca.
    //
    // Esta linea la convierte en un numero. Es el escalon 1 de
    // `docs/PLAN_AUTOCURACION.md`, y su regla es la de siempre: **tiene que
    // decir CERO**, y si no lo dice, dice QUE falto.
    let caps = crate::ring0::obj::cap::live_count_of(pid);
    let dirs = crate::ring0::obj::directory::pending_of(pid);
    let archs = crate::ring0::obj::file::pending_of(pid);
    let pantalla = crate::ring0::obj::fb::owner() == Some(pid);
    // El sonido entra en la cuenta desde el dia que existe la capability, y no
    // hubo que tocar nada mas: un aparato exclusivo que se recupera al morir es
    // exactamente la forma que este recuento ya sabia comprobar.
    let sonido = crate::ring0::obj::audio::owner() == Some(pid);
    let fugas = caps + dirs + archs + pantalla as u32 + sonido as u32;

    renglones[9].s("recursos  ");
    if fugas == 0 {
        renglones[9].s("todo devuelto");
    } else {
        renglones[9].s("*** SIN DEVOLVER:");
        if caps > 0 {
            renglones[9].s(" caps=");
            renglones[9].dec(caps as u64);
        }
        if dirs > 0 {
            renglones[9].s(" directorios=");
            renglones[9].dec(dirs as u64);
        }
        if archs > 0 {
            renglones[9].s(" archivos=");
            renglones[9].dec(archs as u64);
        }
        if pantalla {
            renglones[9].s(" LA PANTALLA");
        }
        if sonido {
            renglones[9].s(" EL SONIDO");
        }
        // Tambien a CABINA: una fuga es un fallo del KERNEL, no del programa
        // que murio, y merece su linea roja aunque nadie abra la autopsia.
        crate::ring0::cabina::warn("autopsia", "el muerto dejo recursos sin devolver", fugas as u64);
    }
    unsafe {
        FUGAS_TOTAL = FUGAS_TOTAL.wrapping_add(fugas);
    }

    unsafe {
        let anillo = &mut *core::ptr::addr_of_mut!(ANILLO);
        let a = &mut anillo[WRITES];
        for i in 0..RENGLONES {
            let n = renglones[i].n.min(ANCHO);
            a.texto[i][..n].copy_from_slice(&renglones[i].b[..n]);
            a.largo[i] = n as u8;
        }
        a.usados = RENGLONES as u8;
        WRITES = (WRITES + 1) % CUANTAS;
        TOTAL = TOTAL.wrapping_add(1);
        DENTRO = false;
    }
}

/// Cuantos fallos van desde el arranque. **Ring 3 mira este numero** para saber
/// si hay uno nuevo sin leer el informe entero: si cambio, hay autopsia nueva.
pub fn total() -> u64 {
    unsafe { TOTAL as u64 }
}

/// Recursos que los muertos dejaron sin devolver desde el arranque.
///
/// **Tiene que ser CERO.** Un numero distinto no acusa al programa que murio:
/// acusa al kernel, que dijo haberlo recuperado todo y no lo hizo.
pub fn fugas() -> u64 {
    unsafe { FUGAS_TOTAL as u64 }
}

/// Cuantos informes se pueden leer ahora.
pub fn disponibles() -> u64 {
    unsafe { (TOTAL as usize).min(CUANTAS) as u64 }
}

/// Cuantos renglones tiene el informe `n` (`0` = el mas reciente).
pub fn renglones(n: u64) -> u64 {
    if n >= disponibles() {
        return 0;
    }
    unsafe {
        let idx = (WRITES + CUANTAS - 1 - (n as usize % CUANTAS)) % CUANTAS;
        let anillo = &*core::ptr::addr_of!(ANILLO);
        anillo[idx].usados as u64
    }
}

/// **Un renglon entero, copiado tal cual.** Para quien lee desde DENTRO del
/// kernel, que no necesita empaquetar nada.
///
/// # Por que existe, y es un agujero que se cobro el 2026-08-14
///
/// Hasta hoy el UNICO lector de la autopsia era el escritorio: `save_autopsies`
/// corre dentro de su bucle de fotograma y la escribe a `datos/fallos.txt`.
///
/// ** Eso es circular, y se ve en cuanto el que muere es el escritorio: el
/// informe de por que no arranco el escritorio solo lo sabe leer el escritorio.
/// Queda escrito en RAM, correcto y completo, y no hay forma de sacarlo -- que
/// es exactamente el sitio donde mas falta hace.
///
/// La regla: **todo lo que el kernel guarda para diagnosticar tiene que ser
/// legible sin Ring 3.** Ring 3 puede estar muerto; el kernel, por diseno, no.
pub fn linea(n: u64, fila: u64, dst: &mut [u8]) -> usize {
    if n >= disponibles() || fila as usize >= RENGLONES {
        return 0;
    }
    unsafe {
        let idx = (WRITES + CUANTAS - 1 - (n as usize % CUANTAS)) % CUANTAS;
        let anillo = &*core::ptr::addr_of!(ANILLO);
        let a = &anillo[idx];
        let largo = (a.largo[fila as usize] as usize).min(dst.len());
        dst[..largo].copy_from_slice(&a.texto[fila as usize][..largo]);
        largo
    }
}

/// **Ocho bytes del renglon `fila` del informe `n`**, empaquetados.
///
/// Mismo contrato que `klog::texto` y por el mismo motivo: pasar un puntero de
/// Ring 3 obligaria al kernel a validar el rango contra el espacio del
/// llamante, y esa infraestructura no existe. El cero es el final.
pub fn texto(n: u64, fila: u64, trozo: u64) -> u64 {
    if n >= disponibles() || fila as usize >= RENGLONES {
        return 0;
    }
    unsafe {
        let idx = (WRITES + CUANTAS - 1 - (n as usize % CUANTAS)) % CUANTAS;
        let anillo = &*core::ptr::addr_of!(ANILLO);
        let a = &anillo[idx];
        let largo = a.largo[fila as usize] as usize;
        let base = (trozo as usize).saturating_mul(8);
        let mut w = [0u8; 8];
        for i in 0..8 {
            match a.texto[fila as usize].get(base + i) {
                Some(&c) if base + i < largo => w[i] = c,
                _ => break,
            }
        }
        u64::from_le_bytes(w)
    }
}
