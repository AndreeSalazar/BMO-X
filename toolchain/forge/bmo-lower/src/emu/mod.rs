//! Emulador x86-64 minimo -- el banco de pruebas del toolchain.
//!
//! # Por que existe
//!
//! Un emisor de codigo maquina que solo se testea comparando bytes contra
//! bytes escritos a mano no prueba nada: si el autor entendio mal una
//! codificacion, el test la repite y pasa igual de mal. Peor aun: un
//! `IF` que emite un salto con desplazamiento cero **parece** codigo
//! correcto en un volcado de bytes, compila, valida el BEF, y ejecuta las
//! dos ramas en hardware. Esa clase de mentira solo la caza la ejecucion.
//!
//! Asi que este modulo no compara: **ejecuta**. Corre el codigo emitido y
//! modela la puerta del kernel (`uconsole::write_packed`: 8 bytes LE,
//! NUL-stop) para reconstruir el texto que apareceria en pantalla. El test
//! compara ese texto con lo que el programa deberia imprimir.
//!
//! Modela tambien dos cosas que el silicio hace y es facil olvidar:
//! - `syscall` destruye `rcx` y `r11` -> aqui se llenan de veneno, para que
//!   cualquier codigo que dependa de ellos falle en el test y no en el metal.
//! - Escribir un registro de 32 bits pone a cero la mitad alta del de 64.
//!
//! # Alcance
//!
//! Cubre el subconjunto que emiten los frontends de BMO: movimientos,
//! aritmetica entera con signo, `imul`/`idiv`, pila, direccionamiento
//! `[rbp+disp]` y `[rsp]`, comparaciones, saltos condicionales e
//! incondicionales, y `syscall`. Son 58 opcodes de un byte mas los grupos
//! ModRM (`80`/`81`/`83`/`C1`/`D3`/`F7`/`FF`) y unos pocos de dos bytes.
//! **No** es un emulador general: ante un opcode que ningun emisor de BMO
//! produce hace panic con el byte, que es la respuesta correcta -- significa
//! que alguien emitio algo sin pensar en como lo iba a verificar.
//!
//! Se activa con la feature `emulator` para que no viaje en las builds
//! normales del toolchain.
//!
//! # * FIDELIDAD: que prueba esto y que NO puede probar
//!
//! Esta seccion existe porque la pregunta *"cuanto se parece esto al Ryzen?"*
//! tiene una respuesta util y una enganosa. La enganosa es un porcentaje. La
//! util es que **la cobertura no esta repartida: esta concentrada en un eje y
//! es cero en los otros dos.**
//!
//! Y no es teoria de sobremesa: la lista de abajo salio de auditar este modulo
//! contra `BITACORA.md`, y explica por que los 24 episodios de aquella --todos--
//! se cazaron en hardware y ninguno aqui.
//!
//! ## Eje 1 -- "los bytes que emiti calculan lo que dice la fuente?" -> ALTO
//!
//! Es para lo que se construyo y donde vale su peso. Aritmetica, flujo de
//! control, marcos de pila, agregados, cadenas, `printf`, File I/O, consola,
//! entrada. Ejemplo del 2026-08-02: `malloc` emitia su salto de la rama de
//! fallo **seis bytes corto** y aqui salio como `opcode 0x05 no emitido por
//! BMO`, que es la firma de aterrizar a media instruccion. En el Ryzen habria
//! sido un proceso muerto sin explicacion, un flasheo y una foto.
//!
//! ## Eje 2 -- "el sistema de debajo hace lo que el modelo dice?" -> CERO
//!
//! **Este modulo no ejecuta el kernel: lo imita.** De ahi sale la trampa mas
//! fea que tiene, y conviene tenerla escrita: si el modelo y el kernel se
//! separan, **los dos parecen sanos** y nada avisa.
//!
//! Ocurrio el mismo dia: `TASK_OP_MEMORIA_PEDIR` no estaba modelado, caia en el
//! `_ => {}` del despacho y salia por el epilogo de EXITO con el valor a cero --
//! o sea "toma tu bloque" con el puntero nulo--, mientras el kernel de verdad
//! contesta con un codigo de error en `rax`. Dos comportamientos incompatibles,
//! cero tests en rojo.
//!
//! La regla que deja: **un contrato modelado necesita su prueba en metal
//! igual**, y el modelo no la sustituye. Lo que si hace es acotar donde mirar
//! cuando falle.
//!
//! ## Eje 3 -- lo FISICO -> CERO, y por construccion
//!
//! Paginacion y CR3, el cruce de anillos, XSAVE, la preempcion por temporizador,
//! DMA, el write-combining y las barreras, los tiempos reales, el USB, el
//! framebuffer, la memoria con huecos. La ley 1 de la bitacora dice que *"QEMU
//! miente por omision"*; esto esta bastante por debajo de QEMU.
//!
//! ## Los agujeros concretos, con nombre (auditado 2026-08-02)
//!
//! - ~~**No hay SSE**~~ -- **TAPADO el 2026-08-02.** Se modelan las quince
//!   instrucciones escalares que BMO C emite (`movsd`/`movss`, las cuatro
//!   aritmeticas, `comisd`, `xorpd`, `cvtsi2sd`, `cvttsd2si`, `cvtsd2ss`,
//!   `cvtss2sd`, `movq xmm,r64`), y con ellas la ruta de coma flotante
//!   **se ejecuta por primera vez**: 7 tests que corren donde antes habia 9
//!   que solo miraban bytes. Lo que sigue sin modelarse es SSE **empaquetado**
//!   -- y el `panic` por opcode desconocido lo dira el dia que alguien lo
//!   emita, que es la respuesta correcta.
//! - **La memoria es un mapa disperso**: toda direccion funciona. No hay fallo
//!   de pagina, ni aliasing, ni marcos no contiguos, asi que `KIND_MEMORIA`
//!   puede probar aqui sus limites y sus rangos, pero **no su fisica**. Por eso
//!   la prueba de las 16 paginas vive en `examples/memoria_C.c` y no solo en un
//!   test: ahi solo puede fallar en el Ryzen.
//! - **No hay tope de pila.** El proceso real recibe 64 KiB; una recursion
//!   profunda pasa aqui y muere alli.
//! - **No hay cargador.** El banco de pruebas rearma las secciones a mano
//!   (Code + RoData + Data) y salta a la entrada. El cargador del kernel, la
//!   alineacion a pagina y la admision de `bmo-verify` **no se ejercen**.
//! - **El presupuesto de instrucciones** (`run(m, N)`) convierte un bucle
//!   infinito en un test que falla; en el Ryzen es una maquina colgada. Aqui es
//!   una ventaja, pero significa que "termina" no quiere decir lo mismo.
//!
//! ## Como usar esto sin enganarse
//!
//! El valor de este modulo no es un porcentaje: es el **coste por bug**. Uno
//! cazado aqui cuesta segundos; el mismo en el Ryzen cuesta flashear, reiniciar,
//! fotografiar y una teoria que puede estar equivocada -- el Ep. 21 costo tres
//! arranques culpando al compositor de algo que hacia un programa de ejemplo.
//!
//! Asi que la regla de reparto es: **lo que se puede equivocar en la aritmetica
//! o en el flujo, aqui; lo que depende del silicio o del kernel, alli, y con su
//! numero escrito antes de arrancar.**

mod sistema;

// ** El reparto de este directorio (L6b), y el corte es por la PREGUNTA:
//
//    mod.rs      QUE HACE EL CPU      registros, memoria, banderas, despacho
//    sistema.rs  QUE CONTESTA EL SO   la puerta y como se siembra
//
// Eran 2.373 lineas en un fichero y el guardian L6a lo tumbo. Que los dos
// trozos crezcan por motivos distintos --uno con cada instruccion nueva, otro
// con cada operacion del sistema-- es lo que dice que el corte es el correcto.

use std::collections::{HashMap, HashSet};

/// Direccion base del area de datos que carga el test.
pub const DATA_BASE: u64 = 0x1_0000;
/// Tope de pila inicial. Alineado a 64 como pide el contrato de BMO.
pub const STACK_TOP: u64 = 0x7000_0000;

/// Donde cae el primer bloque de `KIND_MEMORIA`.
///
/// Espejo de `vmm::MEMORIA_VA_BASE`, **que es la fuente de verdad** -- el kernel
/// no enlaza este crate ni al reves. Se copia el numero por lo mismo que lo
/// copia `ring0/core/informe.rs`: si los dos se separan, el que esta mal es
/// este. Un test que compruebe la direccion exacta esta comprobando el
/// contrato, y por eso vale la pena que sea un numero y no "lo que salga".
pub const MEMORIA_VA_BASE: u64 = 0xE000_0000;

/// Cuanto puede pedir un proceso de una vez, y cuantas veces.
/// Espejo de `ring0::obj::memoria::{MAX_BYTES, MAX_PETICIONES}`.
pub const MEMORIA_MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const MEMORIA_MAX_PETICIONES: usize = 4;

/// Pagina de 4 KiB: el bloque se redondea hacia ARRIBA, igual que el kernel.
const MEMORIA_PAGE: u64 = 4096;

const POISON: u64 = 0xDEAD_BEEF_DEAD_BEEF;

/// El handle que devuelve `TASK_OP_INPUT_CLAIM` aqui dentro. Lejos del rango
/// de los archivos (1..n) a proposito: un programa que confunda los dos
/// handles tiene que fallar en la prueba, no acertar por casualidad.
const CAP_ENTRADA: u64 = 0x0001_0001;

/// Primer handle de `KIND_MEMORIA`. Por encima del de entrada y muy por encima
/// de los de archivo (1..n), por el mismo motivo: un programa que confunda dos
/// handles tiene que fallar aqui, no acertar por casualidad.
const CAP_MEMORIA: u64 = 0x0002_0001;

/// El handle de `KIND_AUDIO`. Otro rango propio, por el mismo motivo que los
/// dos de arriba: confundir handles tiene que fallar, no acertar de rebote.
const CAP_AUDIO: u64 = 0x0003_0001;

/// Tope de un pitido, en ms. Espejo de `ring0::obj::audio::MAX_MS`.
///
/// Se modela porque **es lo que obliga a la libreria a trocear**: una blanca a
/// 100 pulsos son 1200 ms, y sin este tope aqui dentro el troceo de
/// `bmo_sostener` no se ejercitaria nunca y la prueba pasaria igual con la
/// funcion vacia.
pub const AUDIO_MAX_MS: u64 = 250;

const RAX: usize = 0;
const RCX: usize = 1;
const RDX: usize = 2;
const RSP: usize = 4;
const RSI: usize = 6;
const RDI: usize = 7;
/// El cuarto argumento de la puerta. `syscall` machaca `rcx` con el RIP de
/// retorno, asi que el ABI usa `r10` donde SysV usaria `rcx` -- y por eso hace
/// falta nombrarlo: `AUDIO_OP_BEEP` lleva la duracion ahi.
const R10: usize = 10;
const R11: usize = 11;

/// Una llamada observada cruzando CPL3->CPL0.
#[derive(Debug, Clone, Copy)]
pub struct ObservedSyscall {
    pub nr: u64,
    pub capability: u64,
    pub operation: u64,
    pub arg0: u64,
}

/// Un archivo abierto dentro del emulador.
///
/// Modela lo mismo que `ring0/archivo.rs`, **incluido que lo escrito no llega
/// al disco hasta cerrar**. Si el emulador guardara sobre la marcha, un
/// programa que se olvida del `CLOSE` pasaria los tests y perderia el fichero
/// en la maquina real -- que es exactamente la clase de mentira que este modulo
/// existe para no contar.
struct Abierto {
    ruta: String,
    datos: Vec<u8>,
    cursor: usize,
    escribe: bool,
    vivo: bool,
}

pub struct Machine {
    pub regs: [u64; 16],
    /// **Los registros SSE, mitad baja.**
    ///
    /// === Por que solo la mitad baja ===
    ///
    /// Un `xmm` son 128 bits, y aqui se guardan 64. No es un atajo: **todo lo
    /// que BMO emite es SSE ESCALAR** -- `movsd`, `addsd`, `comisd`,
    /// `cvtsi2sd`. Ninguna de esas instrucciones toca la mitad alta salvo para
    /// dejarla como estaba, y nada en el toolchain emite una operacion
    /// empaquetada.
    ///
    /// Modelar 128 bits para que 64 esten siempre a cero seria un emulador mas
    /// grande que dice exactamente lo mismo. El dia que alguien emita
    /// `addpd`, el `panic` por opcode desconocido lo dira -- que es la
    /// respuesta correcta y la razon de que este emulador reviente en vez de
    /// adivinar.
    ///
    /// * La unica excepcion es `xorpd xmm,xmm`, que si borra los 128. Como
    /// solo se emite consigo mismo (para hacer 0.0), poner la mitad baja a
    /// cero es exactamente correcto.
    pub xmm: [u64; 16],
    pub code: Vec<u8>,
    pub rip: usize,
    /// Texto que el kernel habria pintado.
    pub console: String,
    /// Toda llamada observada, en orden.
    pub syscalls: Vec<ObservedSyscall>,
    /// True cuando el programa invoco `TASK_OP_EXIT`.
    pub exited: bool,
    /// El disco, modelado: ruta -> contenido.
    ///
    /// Sin esto el File I/O de COBOL no se podria probar de ninguna forma --
    /// `OPEN`/`READ`/`WRITE` solo se distinguen de un no-op **ejecutandolos**,
    /// que es la leccion entera de este modulo. Las pruebas siembran los
    /// archivos con [`Machine::poner_archivo`] y leen lo escrito con
    /// [`Machine::archivo`].
    pub archivos: HashMap<String, Vec<u8>>,
    /// Rutas cuyo `CERRAR` va a decir que **no se guardo**.
    ///
    /// * Existe porque guardar puede fallar y hasta hoy el emulador no sabia
    /// fingirlo: `ARCH_OP_CERRAR` devolvia `1` siempre, asi que el camino del
    /// fallo --el que pone `FILE STATUS` a `30`-- era codigo que ninguna prueba
    /// podia pisar. Y no es un caso raro: hoy `TASK_OP_ARCHIVO_CREAR` **no
    /// puede reemplazar un fichero que ya existe**, o sea que la segunda
    /// corrida de cualquier programa que escriba su salida cae por aqui.
    ///
    /// Modela el `0` del kernel y nada mas: no se escribe el archivo y se
    /// contesta que no. El motivo (sin sitio, no se pudo reemplazar, desbordo)
    /// **el kernel tampoco lo distingue** -- se queda en la CABINA.
    fallo_al_guardar: HashSet<String>,
    /// Lo que el terminal habria tecleado para este proceso. Lo siembra
    /// [`Machine::poner_entrada`] y lo drena `TASK_OP_CONSOLE_READ`.
    entrada: Vec<u8>,
    entrada_cursor: usize,
    /// El renglon donde se acumula una ruta byte a byte (`TASK_OP_RUTA`),
    /// igual que en el kernel: la superficie no acepta punteros.
    ruta: Vec<u8>,
    /// Archivos abiertos: `(ruta, contenido, cursor, escribe)`.
    abiertos: Vec<Abierto>,
    /// -- La entrada, modelada --------------------------------------------
    ///
    /// Esto no estaba, y el comentario que lo justificaba --"ningun codigo
    /// emitido toca el raton, lo usa el compositor, que es Rust normal"-- dejo
    /// de ser verdad en cuanto un frontend pudo emitir la puerta. Mientras no
    /// estuvo, **la rueda solo se podia probar en el Ryzen**: un `INPUT_OP_RUEDA`
    /// que devuelve siempre lo mismo se ve identico a uno que consume, y esa
    /// es justo la diferencia que decide si un scroll se mueve solo.
    ///
    /// El raton se declara AUSENTE por defecto (`entrada_cedida = false`), que
    /// es lo que ve un programa cuando otro proceso ya reclamo la entrada.
    entrada_cedida: bool,
    /// Teclas pendientes, en orden. Las siembra [`Machine::poner_teclas`] y
    /// las drena `INPUT_OP_TECLA`, una por llamada.
    teclas: Vec<u8>,
    teclas_cursor: usize,
    /// Teclas CRUDAS pendientes: `(scancode Set 1, pulsada)`. Cola aparte de la
    /// de caracteres porque son dos preguntas distintas -- "que se escribio" y
    /// "que esta pulsado" -- y en el kernel tambien son dos colas. Las siembra
    /// [`Machine::poner_eventos_tecla`].
    eventos_tecla: Vec<(u8, bool)>,
    eventos_tecla_cursor: usize,
    /// Teclas que aun no han LLEGADO: un lote por fotograma.
    ///
    /// Sin esto, todo lo sembrado esta disponible en la primera vuelta del
    /// bucle, y un programa que drena el teclado hasta vaciarlo --que es lo
    /// correcto-- ve la sesion entera de golpe. Un ESC al final de la lista
    /// mata el programa antes de que llegue a reaccionar a nada.
    ///
    /// El reloj es `YIELD`, y no es una convencion inventada: un bucle de
    /// fotograma que no cede se come el quantum, asi que ceder **es** el borde
    /// del fotograma. Ver [`Machine::poner_teclas_por_fotograma`].
    lotes: Vec<Vec<u8>>,
    /// Muescas de rueda acumuladas. **Leerlas las vacia**, igual que el kernel.
    rueda: i32,
    /// `(x, y, botones)` y el pulsometro de informes HID.
    puntero: (u32, u32, u8),
    eventos_hid: u64,
    modificadores: u8,
    /// -- `KIND_MEMORIA`, modelada ----------------------------------------
    ///
    /// Sin esto, `TASK_OP_MEMORIA_PEDIR` caia en el `_ => {}` del despacho y
    /// salia por `finalizar_syscall(0)`: **codigo 0 (exito) con handle 0**. O
    /// sea que el emulador contestaba "toma tu bloque" y entregaba el puntero
    /// nulo, y todo `malloc` devolvia 0 sin que nadie pudiera distinguir eso de
    /// un kernel que rechaza. Un modelo que dice que si y no da nada es peor
    /// que ninguno.
    ///
    /// Se modelan las DOS cosas que un programa puede notar: que cada bloque
    /// cae en un rango propio (el cursor avanza) y que **el tope existe**.
    /// Lo que no se modela es la fisica --marcos contiguos, aliasing de
    /// paginas--, y no se puede: aqui la memoria es un mapa disperso, asi que
    /// toda direccion funciona. Eso se prueba en el Ryzen y en ningun otro
    /// sitio.
    mem_cursor: u64,
    mem_peticiones: usize,
    mem_entregados: u64,
    /// Base de cada handle concedido, en orden de concesion.
    mem_bloques: Vec<u64>,
    /// La imagen desde la que se "lanzo" este programa, si el banco la puso.
    /// Es lo que contesta `TASK_OP_MI_PAQUETE`. `None` = el kernel no recuerda
    /// de donde salio, que es lo que le pasa a los binarios que el propio
    /// kernel embebe.
    mi_paquete: Option<String>,
    mem: HashMap<u64, u8>,
    /// -- `KIND_AUDIO`, modelada ------------------------------------------
    ///
    /// Se modelan las tres cosas que un programa puede NOTAR, que son las
    /// mismas que comprueba `sonido_C.c` en el Ryzen: que es exclusivo, que el
    /// tope de duracion se cumple, y que **el handle soltado deja de valer**.
    ///
    /// Lo que NO se modela es que suene: aqui no hay altavoz, y en la mitad de
    /// las placas reales tampoco. Por eso lo que se guarda es la PARTITURA --
    /// la lista de `(hercios, milisegundos)` que el programa mando-- y eso es
    /// justo lo que hace comprobable una libreria de musica: que `LA4` en negra
    /// a 120 pulsos son 440 Hz durante 425 ms, y no algo aproximado.
    audio_dueno: bool,
    audio_volumen: u64,
    /// **Todos** los volumenes que se pidieron, en orden. `audio_volumen` solo
    /// guarda el ultimo, y eso no distingue "se puso una vez" de "se puso
    /// cuatro" -- que es justo lo que una pieza con eco necesita comprobar.
    audio_volumenes: Vec<u64>,
    /// Todo lo que sono, en orden: `(hz, ms)`.
    audio_partitura: Vec<(u64, u64)>,
    data_len: u64,
    zf: bool,
    sf: bool,
    of: bool,
    cf: bool,
    /// **La bandera de paridad.** En la aritmetica de enteros no la mira casi
    /// nadie; en la de coma flotante es la que distingue *"no son iguales"* de
    /// *"no se pueden comparar"*.
    ///
    /// `comisd` la enciende cuando alguno de los dos es NaN, y ESA es la unica
    /// forma de preguntarlo: sin ella, `a = b` con un NaN dentro contesta que
    /// si, porque el no-ordenado tambien enciende la de igualdad. Se modela por
    /// lo mismo que `df`: es estado invisible que hace que algo funcione aqui y
    /// conteste al reves en el silicio.
    pf: bool,
    /// **La bandera de direccion.** Con `df` en falso las instrucciones de
    /// cadena avanzan y con `df` puesta retroceden.
    ///
    /// Se modela --en vez de darla por cero-- porque es exactamente la clase de
    /// estado invisible que hace que un `memcpy` funcione en el emulador y
    /// escriba hacia atras en el Ryzen. Si algun dia un emisor pone `std` y se
    /// olvida el `cld`, aqui se ve.
    df: bool,
}

impl Machine {
    pub fn new(code: Vec<u8>) -> Self {
        let mut m = Self {
            regs: [0; 16],
            xmm: [0; 16],
            code,
            rip: 0,
            console: String::new(),
            syscalls: Vec::new(),
            exited: false,
            archivos: HashMap::new(),
            fallo_al_guardar: HashSet::new(),
            entrada: Vec::new(),
            entrada_cursor: 0,
            ruta: Vec::new(),
            abiertos: Vec::new(),
            entrada_cedida: false,
            teclas: Vec::new(),
            teclas_cursor: 0,
            eventos_tecla: Vec::new(),
            eventos_tecla_cursor: 0,
            lotes: Vec::new(),
            rueda: 0,
            puntero: (0, 0, 0),
            eventos_hid: 0,
            modificadores: 0,
            mem_cursor: MEMORIA_VA_BASE,
            mem_peticiones: 0,
            mem_entregados: 0,
            mem_bloques: Vec::new(),
            mi_paquete: None,
            mem: HashMap::new(),
            audio_dueno: false,
            audio_volumen: 50,
            audio_volumenes: Vec::new(),
            audio_partitura: Vec::new(),
            data_len: 0,
            zf: false,
            sf: false,
            of: false,
            cf: false,
            pf: false,
            df: false,
        };
        m.regs[RSP] = STACK_TOP;
        m
    }

    /// Coloca bytes en memoria y devuelve su direccion.
    pub fn load_data(&mut self, bytes: &[u8]) -> u64 {
        let addr = DATA_BASE + self.data_len;
        for (i, b) in bytes.iter().enumerate() {
            self.mem.insert(addr + i as u64, *b);
        }
        self.data_len += bytes.len() as u64;
        addr
    }

    /// Un byte de memoria, para que los tests puedan mirar si el emisor
    /// escribio donde no debia. Sin esto, un desbordamiento de buffer solo se
    /// ve cuando ya ha corrompido otra cosa.
    pub fn read_u8_pub(&self, addr: u64) -> u8 {
        self.read_u8_mem(addr)
    }

    /// Lee 8 bytes de memoria.
    pub fn read_u64(&self, addr: u64) -> u64 {
        let mut v = 0u64;
        for i in 0..8 {
            v |= (self.read_u8_mem(addr + i) as u64) << (i * 8);
        }
        v
    }

    fn write_u64(&mut self, addr: u64, value: u64) {
        for i in 0..8 {
            self.mem.insert(addr + i, ((value >> (i * 8)) & 0xFF) as u8);
        }
    }

    /// Lee un byte de memoria.
    ///
    /// Si nadie escribio ahi, cae a la propia imagen: los frontends colocan
    /// las cadenas y los globales DENTRO de la seccion de codigo, justo
    /// detras de las instrucciones, y los alcanzan con `lea [rip+disp]`. Un
    /// `%s` leeria ceros si el emulador no modelara eso. Fuera de la imagen
    /// devuelve cero, que es lo que hace el kernel con una pagina nueva.
    fn read_u8_mem(&self, addr: u64) -> u8 {
        if let Some(b) = self.mem.get(&addr) {
            return *b;
        }
        self.code.get(addr as usize).copied().unwrap_or(0)
    }

    fn fetch_u8(&mut self) -> u8 {
        let b = self.code[self.rip];
        self.rip += 1;
        b
    }

    fn fetch_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&self.code[self.rip..self.rip + 4]);
        self.rip += 4;
        u32::from_le_bytes(buf)
    }

    fn fetch_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.code[self.rip..self.rip + 8]);
        self.rip += 8;
        u64::from_le_bytes(buf)
    }

    fn write_reg(&mut self, reg: usize, value: u64, wide: bool) {
        self.regs[reg] = if wide { value } else { value as u32 as u64 };
    }

    fn read_reg(&self, reg: usize, wide: bool) -> u64 {
        if wide {
            self.regs[reg]
        } else {
            self.regs[reg] as u32 as u64
        }
    }

    fn push(&mut self, value: u64) {
        self.regs[RSP] = self.regs[RSP].wrapping_sub(8);
        let sp = self.regs[RSP];
        self.write_u64(sp, value);
    }

    fn pop(&mut self) -> u64 {
        let sp = self.regs[RSP];
        let v = self.read_u64(sp);
        self.regs[RSP] = sp.wrapping_add(8);
        v
    }

    /// Flags de una resta `a - b`, que es lo que produce `cmp`.
    fn flags_sub(&mut self, a: u64, b: u64) {
        let r = a.wrapping_sub(b);
        self.zf = r == 0;
        self.sf = (r as i64) < 0;
        self.cf = a < b;
        // Overflow con signo: los operandos difieren en signo y el
        // resultado toma el del sustraendo.
        self.of = ((a ^ b) & (a ^ r)) >> 63 != 0;
        self.paridad(r);
    }

    /// Flags de una suma `a + b`.
    ///
    /// ** Existe desde el 2026-08-19, y el hueco duro meses sin verse: `add`
    /// usaba `flags_logic`, que pone `of` a FALSO siempre. Ningun lenguaje de
    /// BMO lo habia notado porque **ninguno emitia un `jo`** -- C no comprueba
    /// el desbordamiento, y lo que no se emite no se emula.
    ///
    /// INTI es el primero que lo necesita: su regla 1 dice que desbordar
    /// ATRAPA, y eso baja a `add` + `jo`. Sin esto, el emulador contestaba que
    /// nunca desborda -- que es la peor respuesta posible, porque hace pasar el
    /// test que deberia fallar.
    fn flags_add(&mut self, a: u64, b: u64) {
        let r = a.wrapping_add(b);
        self.zf = r == 0;
        self.sf = (r as i64) < 0;
        self.cf = r < a;
        // Con signo: si los dos operandos tienen el mismo signo y el resultado
        // sale con el contrario, se paso de la cuenta.
        self.of = ((!(a ^ b)) & (a ^ r)) >> 63 != 0;
        self.paridad(r);
    }

    /// Las banderas de un producto con signo: `cf` y `of` a la vez si el
    /// resultado no cabe en el ancho del destino.
    ///
    /// ** `imul` NO toca `zf` ni `sf` en el silicio, y aqui tampoco: dejarlas a
    /// algo razonable seria inventarse un estado que un `jz` detras leeria.
    fn banderas_producto(&mut self, a: i64, b: i64, wide: bool) {
        let cabe = if wide {
            a.checked_mul(b).is_some()
        } else {
            // De 32 bits: cabe si el producto de los dos truncados a 32 sigue
            // entrando en 32 con signo.
            let (a, b) = (a as i32 as i64, b as i32 as i64);
            let r = a.wrapping_mul(b);
            r == r as i32 as i64
        };
        self.cf = !cabe;
        self.of = !cabe;
    }

    fn flags_logic(&mut self, r: u64) {
        self.zf = r == 0;
        self.sf = (r as i64) < 0;
        self.cf = false;
        self.of = false;
        self.paridad(r);
    }

    /// La paridad del BYTE BAJO, que es lo que mide `pf` en x86 -- no la del
    /// resultado entero. Es una rareza heredada del 8080 y esta aqui escrita
    /// para que nadie la "arregle" a la del valor completo.
    fn paridad(&mut self, r: u64) {
        self.pf = (r as u8).count_ones() % 2 == 0;
    }

    /// Decodifica un ModRM y devuelve `(reg, destino)`.
    fn modrm(&mut self, rex_r: usize, rex_x: usize, rex_b: usize) -> (usize, Operand) {
        let modrm = self.fetch_u8();
        let md = modrm >> 6;
        let reg = (((modrm >> 3) & 7) as usize) | (rex_r << 3);
        let rm = (modrm & 7) as usize;

        if md == 3 {
            return (reg, Operand::Reg(rm | (rex_b << 3)));
        }

        // mod=00 con rm=101 NO es "[rbp]": en 64 bits es direccionamiento
        // RELATIVO A RIP con disp32. Es como los frontends alcanzan sus
        // cadenas y variables globales (`lea rax, [rip+disp]`), asi que sin
        // esto el emulador se comia los 4 bytes del desplazamiento como si
        // fueran instrucciones y descarrilaba.
        if md == 0 && rm == 0b101 {
            let disp = self.fetch_u32() as i32 as i64;
            let addr = (self.rip as i64 + disp) as u64;
            return (reg, Operand::Mem(addr));
        }

        // Base (+ indice si hay SIB).
        let (base, index, scale) = if rm == 0b100 {
            let sib = self.fetch_u8();
            let idx = (((sib >> 3) & 7) as usize) | (rex_x << 3);
            let base = ((sib & 7) as usize) | (rex_b << 3);
            // indice 4 sin REX.X significa "sin indice".
            let idx = if idx == 4 { None } else { Some(idx) };
            (base, idx, 1u64 << (sib >> 6))
        } else {
            (rm | (rex_b << 3), None, 1)
        };

        let disp = match md {
            0 => 0i64,
            1 => self.fetch_u8() as i8 as i64,
            2 => self.fetch_u32() as i32 as i64,
            _ => unreachable!(),
        };

        let mut addr = (self.regs[base] as i64 + disp) as u64;
        if let Some(i) = index {
            addr = addr.wrapping_add(self.regs[i].wrapping_mul(scale));
        }
        (reg, Operand::Mem(addr))
    }

    fn load(&self, op: Operand, wide: bool) -> u64 {
        match op {
            Operand::Reg(r) => self.read_reg(r, wide),
            Operand::Mem(a) => {
                let v = self.read_u64(a);
                if wide {
                    v
                } else {
                    v as u32 as u64
                }
            }
        }
    }

    /// Lee un solo byte del operando. En registro es el byte BAJO -- con
    /// REX presente `dl`/`sil` son eso y no los registros altos heredados.
    /// El operando de una instruccion SSE: registro `xmm` o 64 bits de memoria.
    ///
    /// Existe porque [`Self::load`] resuelve `Operand::Reg` contra los enteros,
    /// y aqui el mismo numero significa otro banco de registros. Confundirlos
    /// da un `addsd` que suma el valor de `rax` interpretado como double -- un
    /// numero enorme y sin sentido, del que costaria volver hasta aqui.
    fn leer_xmm(&self, op: Operand) -> u64 {
        match op {
            Operand::Reg(r) => self.xmm[r],
            Operand::Mem(a) => self.read_u64(a),
        }
    }

    fn load_u8(&self, op: Operand) -> u64 {
        match op {
            Operand::Reg(r) => self.regs[r] & 0xFF,
            Operand::Mem(a) => self.read_u8_mem(a) as u64,
        }
    }

    fn store_u8(&mut self, op: Operand, value: u64) {
        match op {
            Operand::Reg(r) => self.regs[r] = (self.regs[r] & !0xFF) | (value & 0xFF),
            Operand::Mem(a) => {
                self.mem.insert(a, (value & 0xFF) as u8);
            }
        }
    }

    /// * Un `mov [mem], eax` escribe **CUATRO** bytes, no ocho.
    ///
    /// Esto hacia `write_u64(a, value as u32 as u64)`: los cuatro bytes de
    /// arriba se ponian a CERO. En un registro eso es correcto --escribir un
    /// registro de 32 bits en modo largo si borra la mitad alta-- pero en
    /// **memoria** es destruir lo de al lado.
    ///
    /// Lo pago el primer struct con dos `int`: `{.x = 1, .y = 2, .x = 9}` daba
    /// `x=9, y=0`, porque la ultima escritura de `x` borraba la `y` que hay
    /// justo detras. Y llevaba ahi desde siempre -- solo que ningun test tenia
    /// dos campos de 4 bytes seguidos donde el segundo se escribiera ANTES que
    /// el primero.
    ///
    /// Es el peor tipo de mentira de un emulador: la que hace fallar codigo
    /// correcto, porque manda a buscar el bug al sitio equivocado.
    fn store(&mut self, op: Operand, value: u64, bytes: usize) {
        match op {
            Operand::Reg(r) => self.write_reg(r, value, bytes == 8),
            Operand::Mem(a) => {
                for i in 0..bytes as u64 {
                    self.mem.insert(a + i, ((value >> (i * 8)) & 0xFF) as u8);
                }
            }
        }
    }

    fn step(&mut self) {
        let mut byte = self.fetch_u8();
        // * `0x66` -- anular el tamano de operando: la instruccion trabaja a 16
        // bits. Va ANTES del REX, que es el orden que manda el manual.
        //
        // No estaba, asi que el emulador reventaba con "opcode 0x66 no emitido
        // por BMO" en cuanto alguien guardara un `short`. Era una mina: el
        // codegen SI emite `66 89` para los campos de 16 bits desde que existen
        // los structs, y no habia ni un test que guardara uno.
        let mut op16 = false;
        // `F0` (LOCK) y `F3` (REP/obligatorio) son prefijos de grupo, igual que
        // `66`, y pueden venir en cualquier orden delante del REX.
        //
        // * LOCK se acepta y **no cambia nada aqui**: con un solo nucleo emulado
        // toda instruccion es atomica por construccion. Lo que si se puede
        // probar --y es lo que se equivoca-- es la SEMANTICA: que `xchg` y
        // `cmpxchg` devuelvan **lo que habia** y no lo que se puso. Eso no se ve
        // en un volcado de bytes.
        //
        // `F3` era "solo puede ser PAUSE" y hacia `assert`. Desde que la tabla
        // tiene `popcnt`, `tzcnt` y `lzcnt` --que lo llevan como prefijo
        // obligatorio-- eso reventaba en cuanto alguien contara bits.
        let mut lock = false;
        let mut f3 = false;
        // * `F2` -- el prefijo del ESCALAR DOBLE, y no estaba.
        //
        // Sin el, el primer `movsd` reventaba el emulador con "opcode 0xF2 no
        // emitido por BMO" -- que era mentira: BMO lo emite desde que C tiene
        // `double`. Lo que pasaba es que **ningun test ejecutaba coma
        // flotante**, asi que el prefijo nunca llegaba hasta aqui.
        let mut f2 = false;
        loop {
            match byte {
                0x66 => op16 = true,
                0xF0 => lock = true,
                0xF3 => f3 = true,
                0xF2 => f2 = true,
                _ => break,
            }
            byte = self.fetch_u8();
        }
        let _ = lock;
        let mut rex = 0u8;
        if (0x40..=0x4F).contains(&byte) {
            rex = byte;
            byte = self.fetch_u8();
        }
        let wide = rex & 0x08 != 0;
        // Cuantos bytes toca la instruccion. REX.W manda sobre 0x66.
        let ancho: usize = if wide {
            8
        } else if op16 {
            2
        } else {
            4
        };
        let rex_r = ((rex >> 2) & 1) as usize;
        let rex_x = ((rex >> 1) & 1) as usize;
        let rex_b = (rex & 1) as usize;

        match byte {
            // push <reg> / pop <reg>
            0x50..=0x57 => {
                let r = ((byte & 7) as usize) | (rex_b << 3);
                let v = self.regs[r];
                self.push(v);
            }
            0x58..=0x5F => {
                let r = ((byte & 7) as usize) | (rex_b << 3);
                let v = self.pop();
                self.regs[r] = v;
            }
            // * `pop qword [mem]` (8F /0). Sin pareja en `0x58`: ese saca a un
            // REGISTRO y este a memoria. Lo emite el PERFORM de parrafo de
            // COBOL para devolver a su sitio la salida del PERFORM de fuera.
            //
            // El orden importa y es el del manual: se saca de la pila ANTES de
            // calcular la direccion, porque `pop [rsp+8]` es legal y usa el
            // `rsp` ya subido. Aqui las direcciones son `[rbp+disp]`, asi que no
            // se nota -- pero hacerlo al reves seria una trampa esperando.
            0x8F => {
                let v = self.pop();
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                assert_eq!(ext & 7, 0, "8F /{} no existe", ext & 7);
                self.store(dst, v, ancho);
            }
            // mov <reg>, imm
            0xB8..=0xBF => {
                let reg = ((byte & 7) as usize) | (rex_b << 3);
                let imm = if wide {
                    self.fetch_u64()
                } else {
                    self.fetch_u32() as u64
                };
                self.write_reg(reg, imm, wide);
            }
            // ** ALU de UN BYTE: `and r/m8, r8` y `or r/m8, r8`.
            //
            // Son las hermanas estrechas de `0x21` y `0x09`, y hacen falta
            // desde que una comparacion de coma flotante junta DOS `setcc`:
            // `setcc` escribe un byte, asi que combinarlos tiene que ser de un
            // byte tambien. Con la version ancha se leerian los siete bytes de
            // arriba, que no son de nadie.
            0x20 | 0x08 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let a = self.load(dst, false) & 0xFF;
                let b = self.read_reg(reg, false) & 0xFF;
                let r = if byte == 0x20 { a & b } else { a | b };
                self.flags_logic(r);
                self.store_u8(dst, r);
            }
            // ALU  r/m, reg
            0x89 | 0x09 | 0x01 | 0x29 | 0x85 | 0x31 | 0x39 | 0x21 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let a = self.load(dst, wide);
                let b = self.read_reg(reg, wide);
                match byte {
                    0x89 => self.store(dst, b, ancho),
                    0x09 => {
                        let r = a | b;
                        self.flags_logic(r);
                        self.store(dst, r, ancho);
                    }
                    0x21 => {
                        let r = a & b;
                        self.flags_logic(r);
                        self.store(dst, r, ancho);
                    }
                    0x31 => {
                        let r = a ^ b;
                        self.flags_logic(r);
                        self.store(dst, r, ancho);
                    }
                    0x01 => {
                        let r = a.wrapping_add(b);
                        self.flags_add(a, b);
                        self.store(dst, r, ancho);
                    }
                    0x29 => {
                        self.flags_sub(a, b);
                        let r = a.wrapping_sub(b);
                        self.store(dst, r, ancho);
                    }
                    0x39 => self.flags_sub(a, b), // cmp
                    0x85 => self.flags_logic(a & b), // test
                    _ => unreachable!(),
                }
            }
            // ALU  reg, r/m  (direccion contraria)
            0x8B | 0x0B | 0x03 | 0x2B | 0x3B => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let a = self.read_reg(reg, wide);
                let b = self.load(src, wide);
                match byte {
                    0x8B => self.write_reg(reg, b, wide),
                    0x0B => {
                        let r = a | b;
                        self.flags_logic(r);
                        self.write_reg(reg, r, wide);
                    }
                    0x03 => {
                        let r = a.wrapping_add(b);
                        self.flags_logic(r);
                        self.write_reg(reg, r, wide);
                    }
                    0x2B => {
                        self.flags_sub(a, b);
                        let r = a.wrapping_sub(b);
                        self.write_reg(reg, r, wide);
                    }
                    0x3B => self.flags_sub(a, b),
                    _ => unreachable!(),
                }
            }
            // movsxd reg64, r/m32 -- carga un int CON SIGNO
            0x63 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.load(src, false) as u32 as i32 as i64 as u64;
                self.write_reg(reg, v, true);
            }
            // lea reg, [mem]
            0x8D => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                match src {
                    Operand::Mem(a) => self.write_reg(reg, a, wide),
                    Operand::Reg(_) => panic!("lea con operando registro es inválido"),
                }
            }
            // * `imul reg, r/m, imm` -- las dos anchuras del inmediato.
            //
            // Las emite el escalado de un indice cuando el paso no es potencia
            // de dos: `int grid[2][3]` avanza DOCE bytes por fila, y
            // `gammatable[5][256]` doscientos cincuenta y seis.
            //
            // No estaban, y el emulador hizo lo correcto: dio panic con el
            // opcode en la mano en vez de seguir con un valor inventado. Ese
            // panic es el que descubrio que el paso de un array de arrays se
            // calculaba mal -- el compilador emitia `0x6B` y nada lo habia
            // ejecutado nunca.
            //
            // `0x6B` lleva imm8 con signo y `0x69` imm32; el resto es el mismo
            // ModRM de siempre.
            0x69 | 0x6B => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let imm = if byte == 0x6B {
                    self.fetch_u8() as i8 as i64
                } else {
                    self.fetch_u32() as i32 as i64
                };
                let a = self.load(src, wide) as i64;
                let r = a.wrapping_mul(imm) as u64;
                self.banderas_producto(a, imm, wide);
                self.write_reg(reg, r, wide);
            }
            // grupo 1 con imm8: /0 add, /5 sub, /7 cmp, /4 and
            0x83 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as i8 as i64 as u64;
                let a = self.load(dst, wide);
                match ext & 7 {
                    0 => {
                        let r = a.wrapping_add(imm);
                        self.flags_logic(r);
                        self.store(dst, r, ancho);
                    }
                    4 => {
                        let r = a & imm;
                        self.flags_logic(r);
                        self.store(dst, r, ancho);
                    }
                    5 => {
                        self.flags_sub(a, imm);
                        let r = a.wrapping_sub(imm);
                        self.store(dst, r, ancho);
                    }
                    7 => self.flags_sub(a, imm),
                    other => panic!("grupo 83 /{other} no emitido por BMO"),
                }
            }
            // grupo 1 con imm32
            0x81 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u32() as i32 as i64 as u64;
                let a = self.load(dst, wide);
                match ext & 7 {
                    0 => {
                        let r = a.wrapping_add(imm);
                        self.flags_logic(r);
                        self.store(dst, r, ancho);
                    }
                    // AND. Lo emite `and_r64_imm32`, que usa `read_line` para
                    // quedarse con el byte bajo del paquete. Faltaba, y esa
                    // ausencia es la prueba de que `read_line` nunca se habia
                    // EJECUTADO aqui -- solo emitido.
                    4 => {
                        let r = a & imm;
                        self.flags_logic(r);
                        self.store(dst, r, ancho);
                    }
                    5 => {
                        self.flags_sub(a, imm);
                        let r = a.wrapping_sub(imm);
                        self.store(dst, r, ancho);
                    }
                    7 => self.flags_sub(a, imm),
                    other => panic!("grupo 81 /{other} no emitido por BMO"),
                }
            }
            // mov r/m, imm32
            0xC7 => {
                let (_, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u32() as i32 as i64 as u64;
                self.store(dst, imm, ancho);
            }
            // desplazamientos con imm8: /4 shl, /5 shr, /7 sar
            0xC1 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as u32;
                let a = self.load(dst, wide);
                let r = match ext & 7 {
                    4 => a << imm,
                    5 => a >> imm,
                    7 => ((a as i64) >> imm) as u64,
                    other => panic!("grupo C1 /{other} no emitido por BMO"),
                };
                self.flags_logic(r);
                self.store(dst, r, ancho);
            }
            // mov r/m8, r8  -- guarda el byte bajo de un registro
            0x88 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.regs[reg] & 0xFF;
                self.store_u8(dst, v);
            }
            // * mov r8, r/m8 -- CARGA un byte. La pareja de `0x88`, y faltaba.
            //
            // Sin ella, todo lo que recorre bytes de uno en uno --`memcpy`,
            // `strlen`, `strcmp`-- moria en el emulador con "opcode 0x8A no
            // emitido por BMO". El emulador no mentia: es que nadie habia
            // emitido un bucle de bytes hasta ahora. Es el limite honesto de
            // un emulador escrito a medida -- cubre lo que se emite, y crece
            // cuando el codegen aprende algo nuevo.
            //
            // Solo toca el byte bajo del destino: el resto del registro se
            // queda como estaba, que es lo que hace el silicio.
            0x8A => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.load_u8(src) & 0xFF;
                self.regs[reg] = (self.regs[reg] & !0xFF) | v;
            }
            // == LAS INSTRUCCIONES DE CADENA ==========================
            //
            // ## Por que no estaban, y por que estan ahora
            //
            // La cabecera de `memoria.rs` decia *"no hay version vectorizada
            // [...] cuando se note se cambia"*, y `agregados.rs` daba la razon
            // concreta de no usar `rep movsb`: **"el emulador no lo tiene"**.
            // O sea que una carencia del banco de pruebas estaba decidiendo
            // como es el codigo que corre en el Ryzen. Eso es la cola moviendo
            // al perro: el emulador existe para verificar lo que se emite, no
            // para elegirlo.
            //
            // Cuestan veinte lineas. Y ademas ARREGLAN algo: un `rep movsb` es
            // **un paso** del emulador en vez de seis por byte, asi que un
            // programa que copia un buffer grande ya no se come el presupuesto
            // de `run(..., max_steps)`.
            //
            // ## El prefijo `REP` y el contador
            //
            // `F3` delante de una instruccion de cadena es `REP`: repetir
            // mientras `rcx != 0`, decrementando. Con `rcx` a cero **no se
            // ejecuta ni una vez**, que es lo que permite quitar el
            // `test rcx,rcx / jz` que llevaban los bucles a mano.
            //
            // [!] `f3` ya lo consumia el bucle de prefijos de arriba para las
            // formas SSE (`movss`/`cvtss2sd`), asi que aqui solo hay que
            // mirarlo -- y por eso este brazo tiene que ir DESPUES de que el
            // prefijo se haya leido, no dentro del bucle.
            //
            // `movsb`: `[rdi] <- [rsi]`, y los dos punteros avanzan (o
            // retroceden, si `df`).
            0xA4 => {
                let paso: u64 = if self.df { u64::MAX } else { 1 }; // -1 en complemento a dos
                let veces = if f3 { self.regs[RCX] } else { 1 };
                for _ in 0..veces {
                    let b = self.read_u8_mem(self.regs[RSI]);
                    self.mem.insert(self.regs[RDI], b);
                    self.regs[RSI] = self.regs[RSI].wrapping_add(paso);
                    self.regs[RDI] = self.regs[RDI].wrapping_add(paso);
                }
                if f3 {
                    self.regs[RCX] = 0;
                }
            }
            // `stosb`: `[rdi] <- al`, y `rdi` avanza. Es `memset`.
            0xAA => {
                let paso: u64 = if self.df { u64::MAX } else { 1 };
                let veces = if f3 { self.regs[RCX] } else { 1 };
                let v = (self.regs[RAX] & 0xFF) as u8;
                for _ in 0..veces {
                    self.mem.insert(self.regs[RDI], v);
                    self.regs[RDI] = self.regs[RDI].wrapping_add(paso);
                }
                if f3 {
                    self.regs[RCX] = 0;
                }
            }
            // `cld` / `std` -- la bandera de direccion.
            0xFC => self.df = false,
            0xFD => self.df = true,
            // test r/m8, r8 -- la version de un byte de `0x85`.
            0x84 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let a = self.load_u8(dst) & 0xFF;
                let b = self.regs[reg] & 0xFF;
                self.flags_logic(a & b);
            }
            // mov r/m8, imm8
            0xC6 => {
                let (_, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as u64;
                self.store_u8(dst, imm);
            }
            // grupo 1 sobre BYTE con imm8: /7 cmp
            0x80 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as u64;
                let a = self.load_u8(dst);
                match ext & 7 {
                    7 => self.flags_sub(a, imm),
                    other => panic!("grupo 80 /{other} no emitido por BMO"),
                }
            }
            // desplazamientos por `cl`: /4 shl, /5 shr, /7 sar
            0xD3 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let count = (self.regs[RCX] & 0x3F) as u32; // el CPU enmascara a 6 bits
                let a = self.load(dst, wide);
                let r = match ext & 7 {
                    4 => a << count,
                    5 => a >> count,
                    7 => ((a as i64) >> count) as u64,
                    other => panic!("grupo D3 /{other} no emitido por BMO"),
                };
                self.flags_logic(r);
                self.store(dst, r, ancho);
            }
            // grupo 3: /2 not, /3 neg, /6 div, /7 idiv
            0xF7 => {
                let (ext, src) = self.modrm(0, rex_x, rex_b);
                let v = self.load(src, wide);
                match ext & 7 {
                    // `~x`. Faltaba, y el hueco era invisible: el codegen de C
                    // lo emitia BIEN desde siempre --un `.bef` con `~0` se
                    // escribe sin quejarse-- pero **ninguna matriz lo podia
                    // ejecutar**, asi que ni C ni COBOL tenian una fila con
                    // `~`. Lo destapo C++ al escribir la suya desde cero.
                    //
                    // * A diferencia de `neg`, `not` **no toca las banderas**
                    // en x86-64. Llamar a `flags_logic` aqui seria un no-op
                    // silencioso el 99% de las veces y una mentira el 1%: un
                    // `~x` seguido de un salto condicional decidiria por el
                    // resultado del `not` en vez de por la comparacion de
                    // antes, que es lo que el silicio conserva.
                    2 => {
                        let r = !v;
                        self.store(src, r, ancho);
                    }
                    3 => {
                        let r = (self.load(src, wide) as i64).wrapping_neg() as u64;
                        self.flags_logic(r);
                        self.store(src, r, ancho);
                    }
                    // div SIN signo: rdx:rax entre el operando. El emisor
                    // siempre pone rdx=0 antes, asi que basta con rax.
                    6 => {
                        assert_ne!(v, 0, "division por cero en el codigo emitido");
                        assert_eq!(
                            self.regs[RDX], 0,
                            "div de 128 bits: el emisor debe poner rdx=0 antes"
                        );
                        let dividend = self.regs[RAX];
                        self.regs[RAX] = dividend / v;
                        self.regs[RDX] = dividend % v;
                    }
                    7 => {
                        // idiv: dividendo en rdx:rax; aqui basta rax con signo
                        // extendido por cqo, que es lo unico que emitimos.
                        let divisor = v as i64;
                        assert_ne!(divisor, 0, "division por cero en el codigo emitido");
                        let dividend = self.regs[RAX] as i64;
                        self.regs[RAX] = dividend.wrapping_div(divisor) as u64;
                        self.regs[RDX] = dividend.wrapping_rem(divisor) as u64;
                    }
                    other => panic!("grupo F7 /{other} no emitido por BMO"),
                }
            }
            // grupo 5: /0 inc, /1 dec
            0xFF => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let a = self.load(dst, wide);
                // /2 = call indirecto (punteros a funcion)
                if (ext & 7) == 2 {
                    let target = self.load(dst, true);
                    let return_to = self.rip as u64;
                    self.push(return_to);
                    self.rip = target as usize;
                    return;
                }
                // * /6 = `push qword [mem]`. No calcula nada y no toca
                // banderas, asi que sale antes del tronco de inc/dec.
                if (ext & 7) == 6 {
                    let v = self.load(dst, true);
                    self.push(v);
                    return;
                }
                let r = match ext & 7 {
                    0 => a.wrapping_add(1),
                    1 => a.wrapping_sub(1),
                    other => panic!("grupo FF /{other} no emitido por BMO"),
                };
                self.flags_logic(r);
                self.store(dst, r, ancho);
            }
            // cqo -- extiende el signo de rax a rdx
            0x99 => {
                self.regs[RDX] = if (self.regs[RAX] as i64) < 0 {
                    u64::MAX
                } else {
                    0
                };
            }
            0x90 => {} // nop
            // call rel32 / ret -- las funciones de C se llaman asi.
            0xE8 => {
                let rel = self.fetch_u32() as i32;
                let return_to = self.rip as u64;
                self.push(return_to);
                self.rip = (self.rip as i64 + rel as i64) as usize;
            }
            0xC3 => {
                let target = self.pop();
                self.rip = target as usize;
            }
            // ** LAS TRES QUE PARAN EL PROGRAMA: `hlt`, `int3` y `ud2`.
            //
            // Se modelan por la misma regla que las barreras y por la contraria
            // que `rdmsr`: **esto es COMPORTAMIENTO, no un dato**.
            //
            // `hlt` para el CPU hasta la siguiente interrupcion; aqui no hay
            // interrupciones, asi que para y ya -- que es exactamente lo que
            // hace en el silicio cuando no llega ninguna. `int3` y `ud2`
            // levantan una excepcion que, sin manejador, termina el programa.
            //
            // No se inventa ningun valor: `rax` se queda como estaba. Lo unico
            // que se dice es *"aqui se acabo"*, y eso es verdad en las tres.
            //
            // Antes daban panic, y la consecuencia era peor de lo que parece:
            // una tabla de INTI con setenta nombres de maquina no se podia
            // recorrer entera para ver cuales salen, porque la primera parada
            // se llevaba el banco por delante.
            0xF4 | 0xCC => self.exited = true,
            // ** `cli` y `sti` -- la misma regla que las barreras.
            //
            // Aqui NO HAY interrupciones que habilitar ni que apagar, asi que
            // apagarlas es un no-op **de verdad** y no una simplificacion: no
            // hay nada que pudiera llegar y no llegue.
            //
            // Se modelan y `rdmsr` sigue dando panic porque son cosas distintas:
            // esto no contesta nada, y aquello contestaria un dato inventado.
            // La linea del emulador no es "de bajo nivel", es **te estoy
            // devolviendo un valor que me acabo de inventar?**
            0xFA | 0xFB => {}
            // cdqe/cwde -- extiende eax a rax con signo
            0x98 => {
                if wide {
                    self.regs[RAX] = self.regs[RAX] as u32 as i32 as i64 as u64;
                } else {
                    self.regs[RAX] = (self.regs[RAX] as u16 as i16 as i32) as u32 as u64;
                }
            }
            0xE9 => {
                let rel = self.fetch_u32() as i32;
                self.rip = (self.rip as i64 + rel as i64) as usize;
            }
            0xEB => {
                let rel = self.fetch_u8() as i8;
                self.rip = (self.rip as i64 + rel as i64) as usize;
            }
            // jcc rel8
            0x70..=0x7F => {
                let rel = self.fetch_u8() as i8;
                if self.cond(byte & 0x0F) {
                    self.rip = (self.rip as i64 + rel as i64) as usize;
                }
            }
            // `xchg r/m, r` -- intercambia y devuelve lo que habia. Sobre
            // memoria lleva LOCK implicito, y por eso es el cerrojo mas simple
            // que existe.
            0x87 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let a = self.load(dst, wide);
                let b = self.read_reg(reg, wide);
                self.store(dst, b, ancho);
                self.write_reg(reg, a, wide);
            }
            0x0F => {
                let second = self.fetch_u8();
                match second {
                    0x05 => self.do_syscall(),

                    // == SSE ESCALAR ======================================
                    //
                    // Las catorce que BMO C emite para `float` y `double`, y
                    // ni una mas. Hasta hoy **ninguna se ejecutaba**: los 9
                    // tests de coma flotante comparaban ventanas de bytes, que
                    // es el metodo que la cabecera de este archivo declara
                    // insuficiente. La ruta compilaba, daba verde, y ningun
                    // CPU la habia corrido.
                    //
                    // El prefijo decide el ancho, que es como funciona SSE:
                    // `F2` escalar doble, `F3` escalar simple, `66` entero
                    // empaquetado o comparacion ordenada.

                    // movsd/movss xmm, r/m -- CARGA
                    0x10 if f2 || f3 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = match src {
                            Operand::Reg(r) => self.xmm[r],
                            Operand::Mem(a) => {
                                if f3 {
                                    // `movss` carga 32 bits y **pone a cero el
                                    // resto** cuando viene de memoria. Desde
                                    // otro registro no lo haria; aqui solo se
                                    // emite desde memoria.
                                    (self.read_u64(a) & 0xFFFF_FFFF) as u32 as u64
                                } else {
                                    self.read_u64(a)
                                }
                            }
                        };
                        self.xmm[reg] = v;
                    }
                    // movsd/movss r/m, xmm -- ALMACENA
                    0x11 if f2 || f3 => {
                        let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                        let v = self.xmm[reg];
                        match dst {
                            Operand::Reg(r) => self.xmm[r] = v,
                            // * El ancho importa: `movss` escribe CUATRO
                            // bytes. Escribir ocho pisaria el vecino, que es
                            // exactamente el bug que este emulador ya se comio
                            // una vez con `mov [mem], eax`.
                            Operand::Mem(a) => self.store(Operand::Mem(a), v, if f3 { 4 } else { 8 }),
                        }
                    }
                    // ** sqrtsd / minsd / maxsd -- las que un motor grafico
                    // pide y `+ - * /` no dan (2026-08-22).
                    //
                    // La raiz es UNARIA y las otras dos binarias, pero las tres
                    // comparten la forma: destino izquierdo, fuente derecha. Se
                    // modelan aqui y no en un bloque aparte porque separarlas
                    // seria repetir el `modrm` y el orden de los operandos --
                    // que es justo donde este emulador ya se equivoco una vez.
                    //
                    // *** `minsd`/`maxsd` NO son conmutativas ante un NaN: el
                    // silicio devuelve el operando FUENTE si cualquiera de los
                    // dos es NaN. Se modela asi a proposito, aunque sorprenda:
                    // un emulador que "arregla" al procesador es un emulador que
                    // aprueba programas que el metal suspende.
                    0x51 | 0x5D | 0x5F if f2 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let b = f64::from_bits(self.leer_xmm(src));
                        let a = f64::from_bits(self.xmm[reg]);
                        let r = match second {
                            0x51 => b.sqrt(),
                            0x5D => {
                                if a.is_nan() || b.is_nan() || b < a {
                                    b
                                } else {
                                    a
                                }
                            }
                            _ => {
                                if a.is_nan() || b.is_nan() || b > a {
                                    b
                                } else {
                                    a
                                }
                            }
                        };
                        self.xmm[reg] = r.to_bits();
                    }
                    // addsd / mulsd / subsd / divsd
                    0x58 | 0x59 | 0x5C | 0x5E if f2 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let b = f64::from_bits(self.leer_xmm(src));
                        let a = f64::from_bits(self.xmm[reg]);
                        // El orden NO es conmutativo en dos de las cuatro, y
                        // ese fue el bug que el banco de pruebas ya cazo una
                        // vez en los enteros: el destino es el operando
                        // IZQUIERDO.
                        let r = match second {
                            0x58 => a + b,
                            0x59 => a * b,
                            0x5C => a - b,
                            _ => a / b,
                        };
                        self.xmm[reg] = r.to_bits();
                    }
                    // cvtsd2ss (F2) / cvtss2sd (F3) -- cambiar de precision
                    0x5A if f2 || f3 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = self.leer_xmm(src);
                        self.xmm[reg] = if f2 {
                            // double -> float: **se pierde precision aqui**, y
                            // tiene que perderse. Guardar el double en un
                            // `float` y leerlo daria mas digitos de los que
                            // caben, y el test no veria lo que ve el silicio.
                            (f64::from_bits(v) as f32).to_bits() as u64
                        } else {
                            (f32::from_bits(v as u32) as f64).to_bits()
                        };
                    }
                    // comisd -- comparar y dejar el resultado en las BANDERAS
                    0x2F if op16 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let b = f64::from_bits(self.leer_xmm(src));
                        let a = f64::from_bits(self.xmm[reg]);
                        // * `comisd` pone ZF/CF/PF, **no** SF ni OF, y por eso
                        // los saltos que le siguen son los SIN SIGNO (`ja`,
                        // `jb`), no `jg`/`jl`. Modelarlo con SF seria hacer
                        // pasar codigo que en el silicio salta al reves.
                        //
                        // No-ordenado (algun NaN) pone las TRES a 1, `pf`
                        // incluida.
                        //
                        // ** Esto decia "no pasa hoy" hasta que INTI empezo a
                        // comparar flotantes. Ahora pasa, y `pf` es la unica
                        // bandera que distingue un NaN de una igualdad: sin
                        // ella, `a = b` con un NaN dentro contesta que si --
                        // porque el no-ordenado enciende `zf` igual que la
                        // igualdad de verdad.
                        if a.is_nan() || b.is_nan() {
                            self.zf = true;
                            self.cf = true;
                            self.pf = true;
                        } else {
                            self.zf = a == b;
                            self.cf = a < b;
                            self.pf = false;
                        }
                        self.sf = false;
                        self.of = false;
                    }
                    // xorpd xmm, xmm -- el cero de la coma flotante
                    0x57 if op16 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = self.leer_xmm(src);
                        self.xmm[reg] ^= v;
                    }
                    // movq xmm, r64 -- los BITS de un entero, tal cual
                    //
                    // * NO es una conversion: es como BMO C mete un literal
                    // `double` en un registro SSE. El compilador pone los bits
                    // del numero en `rax` con un `mov imm64` y los mueve aqui
                    // sin tocarlos. Confundir esto con `cvtsi2sd` daria
                    // `4614256656552045848.0` donde tiene que haber `3.14`.
                    //
                    // Tambien lo usa la NEGACION, que en coma flotante es un
                    // `xor` con el bit de signo -- no una resta contra cero,
                    // que daria `-0.0` mal para el cero.
                    0x6E if op16 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = self.load(src, wide);
                        self.xmm[reg] = if wide { v } else { v & 0xFFFF_FFFF };
                    }
                    // movq r64, xmm / movd r32, xmm -- el camino de VUELTA
                    //
                    // * La hermana de `0x6E`, y la que faltaba: aquella mete
                    // bits en un registro SSE, esta los saca. Es como BMO C
                    // pasa un `double` a una funcion -- los argumentos van por
                    // la PILA, asi que el valor tiene que bajar de `xmm0` a un
                    // registro entero para poder empujarlo.
                    //
                    // Ojo al reparto de campos: aqui el operando de ModRM que
                    // manda es el `reg`, y **es el XMM**; el destino entero es
                    // el `r/m`. Al reves que en casi todo lo demas, y por eso
                    // se escribe explicito.
                    0x7E if op16 => {
                        let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                        let v = self.xmm[reg];
                        // Sin REX.W son cuatro bytes: un `float`, no un
                        // `double`. Llevarse los ocho seria arrastrar la mitad
                        // alta de la mantisa a un registro que declara 32 bits.
                        let bytes = if wide { 8 } else { 4 };
                        self.store(dst, if wide { v } else { v & 0xFFFF_FFFF }, bytes);
                    }
                    // cvtsi2sd xmm, r64 -- entero con signo a double
                    0x2A if f2 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        // CON SIGNO: `-1` tiene que dar `-1.0` y no
                        // 18446744073709551615.0.
                        let v = self.load(src, true) as i64;
                        self.xmm[reg] = (v as f64).to_bits();
                    }
                    // cvttsd2si r64, xmm -- double a entero, TRUNCANDO
                    0x2C if f2 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = f64::from_bits(self.leer_xmm(src));
                        // `cvtt` trunca hacia cero; `cvt` (0x2D) redondearia.
                        // BMO solo emite el que trunca, que es lo que manda C
                        // para un cast a entero: `(int)2.7` son 2.
                        //
                        // ** Y LO QUE PASA CUANDO NO CABE, que estaba mal.
                        //
                        // Esto escribia `v as i64` a secas, que en Rust
                        // **satura**: 1e30 daba el entero mas grande y un NaN
                        // daba cero. El silicio no hace ninguna de las dos:
                        // devuelve el entero mas NEGATIVO como centinela, para
                        // los dos casos y sin levantar nada.
                        //
                        // La diferencia no es academica. Es la unica senal que
                        // el procesador da de que la conversion no cabia, asi
                        // que **es la que la Regla 12 de INTI tiene que mirar**.
                        // Con la version que satura, un programa que comprueba
                        // el centinela pasaba aqui y atrapaba en metal -- o al
                        // reves, que es peor.
                        //
                        // Es exactamente la clase de fallo que este emulador
                        // existe para no tener: uno donde el banco dice que si
                        // y el Ryzen dice que no.
                        let r = if v.is_nan() || v >= 9223372036854775808.0 || v < -9223372036854775808.0
                        {
                            i64::MIN
                        } else {
                            v as i64
                        };
                        self.write_reg(reg, r as u64, true);
                    }
                    // movsx reg, r/m8 -- carga un char CON signo
                    0xBE => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = self.load_u8(src) as u8 as i8 as i64 as u64;
                        self.write_reg(reg, v, wide);
                    }
                    // movzx reg, r/m16 / movsx reg, r/m16
                    0xB7 | 0xBF => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let raw = (self.load(src, false) & 0xFFFF) as u16;
                        let v = if second == 0xBF {
                            raw as i16 as i64 as u64
                        } else {
                            raw as u64
                        };
                        self.write_reg(reg, v, wide);
                    }
                    // movzx reg, r/m8
                    0xB6 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = match src {
                            Operand::Mem(a) => self.read_u8_mem(a) as u64,
                            Operand::Reg(r) => self.regs[r] & 0xFF,
                        };
                        self.write_reg(reg, v, false);
                    }
                    // -- Los atomicos: lo que se prueba es que devuelvan lo de
                    //    ANTES, que es lo que se escribe al reves sin notarlo --
                    //
                    // `cmpxchg r/m, r`: compara rax con el destino. Si son
                    // iguales, mete el registro fuente; si no, **rax se queda
                    // con lo que habia**. Ese detalle --que en el caso de fallo
                    // rax cambia-- es justo el que permite reintentar sin releer.
                    0xB1 => {
                        let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                        let actual = self.load(dst, wide);
                        let esperado = self.read_reg(RAX, wide);
                        if actual == esperado {
                            let nuevo = self.read_reg(reg, wide);
                            self.store(dst, nuevo, ancho);
                            self.zf = true;
                        } else {
                            self.zf = false;
                        }
                        // En los dos casos rax acaba con lo que habia.
                        self.write_reg(RAX, actual, wide);
                    }
                    // `xadd r/m, r`: suma y deja en el REGISTRO lo anterior.
                    0xC1 => {
                        let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                        let antes = self.load(dst, wide);
                        let suma = self.read_reg(reg, wide);
                        self.store(dst, antes.wrapping_add(suma), ancho);
                        self.write_reg(reg, antes, wide);
                    }
                    // -- Bits --
                    //
                    // `popcnt` lleva F3 obligatorio; `bsf`/`bsr` no lo llevan, y
                    // con el pasan a ser `tzcnt`/`lzcnt`. El mismo opcode con
                    // dos significados segun el prefijo: por eso hace falta
                    // saber si venia `f3`.
                    0xB8 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = self.load(src, wide);
                        self.write_reg(reg, v.count_ones() as u64, wide);
                    }
                    0xBC | 0xBD => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = if wide { self.load(src, true) } else { self.load(src, false) & 0xFFFF_FFFF };
                        let anchura = if wide { 64 } else { 32 };
                        let r = match (second, f3) {
                            // tzcnt / lzcnt: DEFINIDOS en cero, dan la anchura.
                            (0xBC, true) => v.trailing_zeros().min(anchura) as u64,
                            (0xBD, true) => {
                                if wide { v.leading_zeros() as u64 }
                                else { (v as u32).leading_zeros() as u64 }
                            }
                            // bsf / bsr: INDEFINIDOS en cero. Aqui se deja el
                            // destino intacto, que es lo que hace el silicio --
                            // asi un mapa de bits lleno pasado sin comprobar da
                            // el indice de la busqueda ANTERIOR, igual que en
                            // metal, y el test lo puede ver.
                            (0xBC, false) => {
                                if v == 0 { self.read_reg(reg, wide) } else { v.trailing_zeros() as u64 }
                            }
                            _ => {
                                if v == 0 { self.read_reg(reg, wide) }
                                else { (anchura - 1 - if wide { v.leading_zeros() } else { (v as u32).leading_zeros() }) as u64 }
                            }
                        };
                        self.zf = v == 0;
                        self.write_reg(reg, r, wide);
                    }
                    // `bswap r` -- el registro va DENTRO del opcode.
                    0xC8..=0xCF => {
                        let r = ((second & 7) as usize) | (rex_b << 3);
                        let v = self.read_reg(r, wide);
                        let dado_la_vuelta = if wide {
                            v.swap_bytes()
                        } else {
                            (v as u32).swap_bytes() as u64
                        };
                        self.write_reg(r, dado_la_vuelta, wide);
                    }
                    // `0F AE` -- barreras (mfence/lfence/sfence) y `clflush`.
                    //
                    // * Aqui un no-op NO es mentir, y esa distincion importa:
                    // una barrera en un interprete de un solo hilo que ejecuta
                    // en orden **es** un no-op de verdad, no una simplificacion.
                    // Lo que ordena ya estaba ordenado.
                    //
                    // Por eso este opcode se modela y `rdmsr` o `mov rax, cr0`
                    // siguen dando panic: devolver 0 como si fuera el valor de
                    // un MSR seria inventarse un dato, y eso el emulador no lo
                    // hace. Ver VERDAD.md -- hay intrinsecos que solo el metal
                    // puede contestar.
                    0xAE => {
                        let modrm = self.code[self.rip];
                        if modrm >> 6 == 3 {
                            self.rip += 1; // fence: el ModRM es la variante
                        } else {
                            let _ = self.modrm(rex_r, rex_x, rex_b); // clflush
                        }
                    }
                    // `ud2` -- instruccion invalida a proposito. Termina.
                    //
                    // Igual que `int3`: es comportamiento, no un dato. Un
                    // programa que la ejecuta se acaba ahi, en el emulador y en
                    // el silicio.
                    0x0B => self.exited = true,
                    // imul reg, r/m
                    //
                    // ** Y PONE BANDERAS, que es lo que faltaba y era grave.
                    //
                    // Esto multiplicaba y no tocaba `of`, asi que un `jo`
                    // detras **nunca saltaba**. BMO C no lo noto porque C no
                    // comprueba el desbordamiento; INTI si, y su Regla 1 salia
                    // verde aqui y habria atrapado en el Ryzen.
                    //
                    // El silicio enciende `cf` y `of` a la vez cuando el
                    // producto con signo no cabe en el registro -- que es
                    // exactamente la pregunta que la Regla 1 hace.
                    //
                    // Es hermano del hueco de `add` que se encontro el 19-08 con
                    // las mismas palabras: *ningun lenguaje de BMO lo habia
                    // notado porque ninguno emitia un `jo`*.
                    0xAF => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let a = self.read_reg(reg, wide) as i64;
                        let b = self.load(src, wide) as i64;
                        let r = a.wrapping_mul(b) as u64;
                        self.banderas_producto(a, b, wide);
                        self.write_reg(reg, r, wide);
                    }
                    // setcc r/m8 -- deja 0 o 1 segun la condicion
                    0x90..=0x9F => {
                        let (_, dst) = self.modrm(0, rex_x, rex_b);
                        let value = u64::from(self.cond(second & 0x0F));
                        self.store_u8(dst, value);
                    }
                    // jcc rel32
                    0x80..=0x8F => {
                        let rel = self.fetch_u32() as i32;
                        if self.cond(second & 0x0F) {
                            self.rip = (self.rip as i64 + rel as i64) as usize;
                        }
                    }
                    other => panic!("opcode 0F {other:#04X} no emitido por BMO"),
                }
            }
            other => panic!("opcode {other:#04X} no emitido por BMO"),
        }
    }

    /// Evalua el codigo de condicion de un `jcc` (el nibble bajo del opcode).
    fn cond(&self, cc: u8) -> bool {
        match cc {
            0x0 => self.of,
            0x1 => !self.of,
            0x2 => self.cf,
            0x3 => !self.cf,
            0x4 => self.zf,
            0x5 => !self.zf,
            0x6 => self.cf || self.zf,
            0x7 => !self.cf && !self.zf,
            0x8 => self.sf,
            0x9 => !self.sf,
            // ** `p` / `np`: "no comparables" y "comparables". Solo tienen
            // sentido detras de una comparacion de coma flotante, y sin ellas
            // no hay forma de escribir una igualdad que el NaN no engane.
            0xA => self.pf,
            0xB => !self.pf,
            0xC => self.sf != self.of,
            0xD => self.sf == self.of,
            0xE => self.zf || (self.sf != self.of),
            0xF => !self.zf && (self.sf == self.of),
            other => panic!("condicion {other:#x} no emitida por BMO"),
        }
    }
}

#[derive(Clone, Copy)]
enum Operand {
    Reg(usize),
    Mem(u64),
}

/// Ejecuta hasta caer del final del codigo, hasta `EXIT`, o hasta agotar el
/// presupuesto de pasos (un bucle que no termina es un bug, y colgar el test
/// lo esconde en vez de reportarlo).
pub fn run(mut m: Machine, max_steps: usize) -> Machine {
    let mut steps = 0;
    while m.rip < m.code.len() && !m.exited {
        m.step();
        steps += 1;
        assert!(
            steps < max_steps,
            "el codigo emitido no termina (>{max_steps} instrucciones)"
        );
    }
    m
}
