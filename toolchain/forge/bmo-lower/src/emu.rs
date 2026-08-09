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
/// falta nombrarlo: `AUDIO_OP_PITAR` lleva la duracion ahi.
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
    /// Todo lo que sono, en orden: `(hz, ms)`.
    audio_partitura: Vec<(u64, u64)>,
    data_len: u64,
    zf: bool,
    sf: bool,
    of: bool,
    cf: bool,
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
            lotes: Vec::new(),
            rueda: 0,
            puntero: (0, 0, 0),
            eventos_hid: 0,
            modificadores: 0,
            mem_cursor: MEMORIA_VA_BASE,
            mem_peticiones: 0,
            mem_entregados: 0,
            mem_bloques: Vec::new(),
            mem: HashMap::new(),
            audio_dueno: false,
            audio_volumen: 50,
            audio_partitura: Vec::new(),
            data_len: 0,
            zf: false,
            sf: false,
            of: false,
            cf: false,
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
    }

    fn flags_logic(&mut self, r: u64) {
        self.zf = r == 0;
        self.sf = (r as i64) < 0;
        self.cf = false;
        self.of = false;
    }

    /// Siembra lo que el terminal habria tecleado. El `\n` final hace falta:
    /// `read_line` espera verlo para dar la linea por cerrada, exactamente
    /// igual que en la maquina.
    pub fn poner_entrada(&mut self, texto: &str) {
        self.entrada.extend_from_slice(texto.as_bytes());
    }

    /// Siembra un archivo antes de ejecutar. Es el disco de la prueba.
    pub fn poner_archivo(&mut self, ruta: &str, datos: &[u8]) {
        self.archivos.insert(ruta.to_string(), datos.to_vec());
    }

    /// Hace que guardar ESA ruta falle: el `CLOSE` contestara `0` y en el disco
    /// no quedara nada.
    ///
    /// Es el disco diciendo que no, que es lo unico que un programa puede
    /// observar. Sirve para probar que el programa **se entera** -- un `CLOSE`
    /// que siempre dice que si deja el camino del fallo sin pisar, y ese es
    /// justo el que decide si un fichero se perdio en silencio.
    pub fn fallar_al_guardar(&mut self, ruta: &str) {
        self.fallo_al_guardar.insert(ruta.to_string());
    }

    /// Lo que hay en el disco al terminar. `None` si ese archivo no existe --
    /// que es distinto de existir vacio, y en un batch bancario esa diferencia
    /// es la que separa "no se escribio" de "se escribio cero registros".
    pub fn archivo(&self, ruta: &str) -> Option<&[u8]> {
        self.archivos.get(ruta).map(|v| v.as_slice())
    }

    /// Igual, pero como texto. Comodidad para los tests.
    pub fn archivo_texto(&self, ruta: &str) -> Option<String> {
        self.archivo(ruta).map(|b| String::from_utf8_lossy(b).into_owned())
    }

    // -- Sembrar la entrada ----------------------------------------------

    /// Concede la entrada: a partir de aqui `TASK_OP_INPUT_CLAIM` funciona.
    ///
    /// Hay que pedirlo a proposito porque la entrada es **exclusiva**: sin
    /// esto, la prueba ve lo mismo que un programa lanzado mientras el
    /// compositor la tiene tomada, que es el caso que mas se equivoca al
    /// escribirlo.
    pub fn ceder_entrada(&mut self) {
        self.entrada_cedida = true;
    }

    /// Teclas que el programa ira recogiendo con `INPUT_OP_TECLA`, una por
    /// llamada. Los bytes son Latin-1 ya resueltos; para las que no tienen
    /// glifo, las constantes `TECLA_*` de `bmo_abi::syscalls::surface`.
    pub fn poner_teclas(&mut self, teclas: &[u8]) {
        self.teclas.extend_from_slice(teclas);
    }

    /// Teclas repartidas EN EL TIEMPO: un lote por fotograma, entendiendo por
    /// fotograma cada `YIELD` que haga el programa.
    ///
    /// Es la diferencia entre probar un programa interactivo y probar una
    /// rafaga: con todo disponible de golpe, un bucle que drena el teclado ve
    /// la sesion entera en la primera vuelta y nunca llega a repintar entre
    /// pulsacion y pulsacion -- que es justo la conducta que se quiere mirar.
    ///
    /// El primer lote llega tras el primer `YIELD`; lo que deba estar ahi
    /// desde el principio va en [`Machine::poner_teclas`].
    pub fn poner_teclas_por_fotograma(&mut self, lotes: &[&[u8]]) {
        // Se guardan al reves para poder sacar el siguiente por el final, que
        // es O(1). El orden que ve el programa es el de la lista.
        for lote in lotes.iter().rev() {
            self.lotes.push(lote.to_vec());
        }
    }

    /// Suma muescas de rueda. Positivo = hacia arriba. Se acumulan hasta que
    /// alguien las lea, y leerlas las vacia.
    pub fn poner_rueda(&mut self, muescas: i32) {
        self.rueda += muescas;
        self.eventos_hid += muescas.unsigned_abs() as u64;
    }

    /// Coloca el puntero y sube el pulsometro de informes HID.
    pub fn poner_puntero(&mut self, x: u32, y: u32, botones: u8) {
        self.puntero = (x, y, botones);
        self.eventos_hid += 1;
    }

    /// Modificadores pulsados AHORA (`MOD_SHIFT`, `MOD_CTRL`...). Es estado: se
    /// queda puesto hasta que se cambie.
    pub fn poner_modificadores(&mut self, mascara: u8) {
        self.modificadores = mascara;
    }

    /// Muescas de rueda que quedan sin leer. Un programa que se olvida de
    /// drenarla las deja aqui, y la prueba puede decirlo.
    pub fn rueda_pendiente(&self) -> i32 {
        self.rueda
    }

    /// La PARTITURA: todo lo que el programa mando sonar, `(hz, ms)` en orden.
    ///
    /// Es lo unico que un banco de pruebas puede mirar de una libreria de
    /// musica, y es suficiente: si `LA4` en negra a 120 pulsos no son 440 Hz
    /// durante 425 ms, la libreria esta mal, suene el altavoz o no.
    pub fn partitura(&self) -> &[(u64, u64)] {
        &self.audio_partitura
    }

    /// Milisegundos totales que el programa dejo el altavoz sonando (sin contar
    /// los silencios). Sirve para comprobar articulacion y tempo de una frase
    /// entera sin enumerar nota por nota.
    pub fn audio_ms_sonando(&self) -> u64 {
        self.audio_partitura.iter().filter(|p| p.0 != 0).map(|p| p.1).sum()
    }

    /// Volumen que quedo puesto. 50 si nadie lo toco, igual que el crate.
    pub fn audio_volumen(&self) -> u64 {
        self.audio_volumen
    }

    /// Despacho de la capability de sonido. Copia la semantica de
    /// `ring0/obj/audio.rs` -- sobre todo la que se nota: **el tope recorta**.
    fn audio_op(&mut self, op: u64, a0: u64, a1: u64) -> u64 {
        use bmo_abi::syscalls::surface::{
            APARATO_ALTAVOZ, AUDIO_OP_APARATO, AUDIO_OP_CALLAR, AUDIO_OP_PITAR, AUDIO_OP_VOLUMEN,
        };
        match op {
            // Solo el altavoz. HDA sigue sin existir, y decir aqui que si lo
            // hay seria darle al programa una respuesta que el Ryzen no da.
            AUDIO_OP_APARATO => APARATO_ALTAVOZ,
            AUDIO_OP_PITAR => {
                let hz = a0.min(20_000);
                let ms = a1.min(AUDIO_MAX_MS);
                self.audio_partitura.push((hz, ms));
                ms
            }
            AUDIO_OP_VOLUMEN => {
                self.audio_volumen = a0.min(100);
                self.audio_volumen
            }
            AUDIO_OP_CALLAR => {
                self.audio_partitura.push((0, 0));
                0
            }
            _ => 0,
        }
    }

    /// Despacho de la capability de entrada. Copia la semantica de
    /// `ring0/obj/input.rs` -- sobre todo la que se nota: la rueda CONSUME.
    fn entrada_op(&mut self, op: u64) -> u64 {
        use bmo_abi::syscalls::surface::{
            INPUT_OP_EVENTOS, INPUT_OP_MODIFICADORES, INPUT_OP_PUNTERO, INPUT_OP_RUEDA,
            INPUT_OP_TECLA,
        };
        match op {
            INPUT_OP_PUNTERO => {
                let (x, y, b) = self.puntero;
                ((x as u64) << 32) | ((y as u64) << 16) | b as u64
            }
            INPUT_OP_EVENTOS => self.eventos_hid,
            // `0x100 | byte` cuando hay una; `0` cuando no. El bit 8 es lo que
            // distingue "llego el byte 0" de "no llego nada".
            INPUT_OP_TECLA => {
                if self.teclas_cursor < self.teclas.len() {
                    let b = self.teclas[self.teclas_cursor];
                    self.teclas_cursor += 1;
                    0x100 | b as u64
                } else {
                    0
                }
            }
            INPUT_OP_MODIFICADORES => self.modificadores as u64,
            // * Consume. Dos lecturas seguidas sin girar dan cero la segunda.
            INPUT_OP_RUEDA => {
                let v = self.rueda;
                self.rueda = 0;
                v as i64 as u64
            }
            _ => 0,
        }
    }

    /// Abre o crea. Devuelve el handle (el indice + 1, para que 0 no sea uno
    /// valido) o 0 si no se pudo.
    fn archivo_abrir(&mut self, escribe: bool) -> u64 {
        let ruta = String::from_utf8_lossy(&self.ruta).into_owned();
        self.ruta.clear();
        if ruta.is_empty() {
            return 0;
        }
        let datos = if escribe {
            Vec::new()
        } else {
            match self.archivos.get(&ruta) {
                Some(d) => d.clone(),
                // Abrir para leer lo que no existe FALLA. En el kernel es
                // `ERROR_NOT_THERE`; aqui es un handle nulo. Devolver uno vacio
                // haria que un `READ` de un fichero que falta pareciera un
                // fichero sin registros.
                None => return 0,
            }
        };
        self.abiertos.push(Abierto { ruta, datos, cursor: 0, escribe, vivo: true });
        self.abiertos.len() as u64
    }

    fn archivo_op(&mut self, handle: u64, op: u64, arg0: u64) -> u64 {
        use bmo_abi::syscalls::surface::{
            ARCH_OP_CERRAR, ARCH_OP_ESCRIBIR, ARCH_OP_LEER, ARCH_OP_LEER_LINEA, ARCH_OP_TAMANO,
        };
        let i = match (handle as usize).checked_sub(1) {
            Some(i) if i < self.abiertos.len() => i,
            _ => return 0,
        };
        if !self.abiertos[i].vivo {
            return 0;
        }
        match op {
            ARCH_OP_LEER if !self.abiertos[i].escribe => {
                let a = &mut self.abiertos[i];
                let mut w = [0u8; 8];
                let mut n = 0usize;
                while n < 7 && a.cursor < a.datos.len() {
                    w[n] = a.datos[a.cursor];
                    a.cursor += 1;
                    n += 1;
                }
                ((n as u64) << 56) | u64::from_le_bytes(w)
            }
            // Se para en el salto y lo consume. Modela EXACTAMENTE lo que
            // hace `ring0/archivo.rs`: si el emulador entregara los bytes de
            // detras del salto, un fichero de varios registros pasaria los
            // tests y daria basura en la maquina.
            ARCH_OP_LEER_LINEA if !self.abiertos[i].escribe => {
                let a = &mut self.abiertos[i];
                let mut w = [0u8; 8];
                let mut n = 0usize;
                let mut fin = 0u64;
                while n < 7 && a.cursor < a.datos.len() {
                    let b = a.datos[a.cursor];
                    a.cursor += 1;
                    if b == b'\n' {
                        fin = 1;
                        break;
                    }
                    w[n] = b;
                    n += 1;
                }
                (fin << 63) | ((n as u64) << 56) | u64::from_le_bytes(w)
            }
            ARCH_OP_ESCRIBIR if self.abiertos[i].escribe => {
                let n = (((arg0 >> 56) & 0xFF) as usize).min(7);
                let b = arg0.to_le_bytes();
                let a = &mut self.abiertos[i];
                for k in 0..n {
                    a.datos.push(b[k]);
                }
                n as u64
            }
            ARCH_OP_TAMANO => {
                let a = &self.abiertos[i];
                if a.escribe { a.datos.len() as u64 } else { (a.datos.len() - a.cursor) as u64 }
            }
            ARCH_OP_CERRAR => {
                let a = &mut self.abiertos[i];
                a.vivo = false;
                if a.escribe {
                    let (ruta, datos) = (a.ruta.clone(), a.datos.clone());
                    // El disco dice que no: no se escribe NADA y se contesta
                    // `0`. No se guarda un trozo -- un archivo a medias se
                    // parece demasiado a uno entero, que es la misma regla que
                    // sigue `close` en `ring0/archivo.rs`.
                    if self.fallo_al_guardar.contains(&ruta) {
                        return 0;
                    }
                    // * AQUI es donde llega al disco, y solo aqui. Igual que
                    // en el kernel.
                    self.archivos.insert(ruta, datos);
                }
                1
            }
            // El modo manda: pedirle bytes a uno de escritura no es un error
            // de permisos, es una pregunta que ese objeto no responde.
            _ => 0,
        }
    }

    /// `TASK_OP_MEMORIA_PEDIR` -- el bloque, o el motivo por el que no.
    ///
    /// Los dos rechazos que un programa puede provocar SOLO son los mismos que
    /// los del kernel y **con sus mismos codigos**: pedir cero o pasarse del
    /// tope (`0xE001`), y pedir una quinta vez (`0xE003`).
    ///
    /// Los otros dos no se modelan, y por el mismo motivo los dos: **aqui solo
    /// corre un proceso y la memoria es infinita**. `ERROR_NO_RAM` necesitaria
    /// RAM que fragmentar y `ERROR_NO_SLOT` necesitaria 16 procesos vivos a
    /// la vez. Fingirlos seria inventarse fallos que este emulador no puede
    /// reproducir de forma repetible -- y son exactamente el tipo de cosa que el
    /// eje 2 de la seccion FIDELIDAD dice que hay que probar en el Ryzen.
    fn memoria_pedir(&mut self, bytes: u64) -> Result<u64, u64> {
        const ERROR_TOO_BIG: u64 = 0xE001;
        const ERROR_TOO_MANY: u64 = 0xE003;

        if bytes == 0 || bytes > MEMORIA_MAX_BYTES {
            return Err(ERROR_TOO_BIG);
        }
        if self.mem_peticiones >= MEMORIA_MAX_PETICIONES {
            return Err(ERROR_TOO_MANY);
        }
        // Redondeo a paginas ARRIBA: pedir 1024 bytes entrega 4096, y el
        // siguiente bloque empieza detras de los 4096. Si esto redondeara hacia
        // abajo, dos bloques se solaparian y el emulador --memoria dispersa-- no
        // se quejaria nunca. Por eso el programa de prueba compara las bases.
        let paginas = (bytes + MEMORIA_PAGE - 1) / MEMORIA_PAGE;
        let base = self.mem_cursor;
        self.mem_cursor += paginas * MEMORIA_PAGE;
        self.mem_entregados += paginas * MEMORIA_PAGE;
        self.mem_peticiones += 1;
        self.mem_bloques.push(base);
        Ok(CAP_MEMORIA + (self.mem_bloques.len() as u64 - 1))
    }

    /// Las dos preguntas que responde un handle de memoria.
    fn memoria_op(&self, handle: u64, op: u64) -> u64 {
        use bmo_abi::syscalls::surface::{MEM_OP_BASE, MEM_OP_BYTES};
        let i = (handle - CAP_MEMORIA) as usize;
        match op {
            MEM_OP_BASE => self.mem_bloques.get(i).copied().unwrap_or(0),
            // Lo entregado al PROCESO entero, no a este bloque: es lo que
            // contesta el kernel, que lleva la cuenta por pid.
            MEM_OP_BYTES => self.mem_entregados,
            _ => 0,
        }
    }

    /// Cuantos bytes de `KIND_MEMORIA` se han entregado. Para que un test pueda
    /// comprobar lo que el programa pidio sin creerse lo que el programa dice.
    pub fn memoria_entregada(&self) -> u64 {
        self.mem_entregados
    }

    /// La puerta del kernel, modelada.
    fn do_syscall(&mut self) {
        use bmo_abi::syscalls::surface::{
            CURRENT_TASK, NR_INVOKE, TASK_OP_ARCHIVO_ABRIR, TASK_OP_ARCHIVO_CREAR,
            TASK_OP_AUDIO_RECLAMAR, TASK_OP_AUDIO_SOLTAR, TASK_OP_CONSOLE_READ,
            TASK_OP_CONSOLE_WRITE, TASK_OP_EXIT, TASK_OP_INPUT_CLAIM, TASK_OP_MEMORIA_PEDIR,
            TASK_OP_RUTA, TASK_OP_YIELD,
        };

        let call = ObservedSyscall {
            nr: self.regs[RAX],
            capability: self.regs[RDI],
            operation: self.regs[RSI],
            arg0: self.regs[RDX],
        };
        self.syscalls.push(call);

        assert_eq!(
            call.nr, NR_INVOKE as u64,
            "solo INVOKE cruza esta puerta (rax={:#x})",
            call.nr
        );

        if call.capability == CURRENT_TASK {
            match call.operation {
                op if op == TASK_OP_CONSOLE_WRITE => {
                    for i in 0..8 {
                        let b = ((call.arg0 >> (i * 8)) & 0xFF) as u8;
                        if b == 0 {
                            break; // NUL-stop: identico al kernel
                        }
                        self.console.push(b as char);
                    }
                }
                op if op == TASK_OP_EXIT => self.exited = true,
                // La ruta se acumula de 8 en 8 y se corta en el primer cero,
                // igual que en el kernel: un chunk final corto viene relleno.
                op if op == TASK_OP_RUTA => {
                    for i in 0..8 {
                        let b = ((call.arg0 >> (i * 8)) & 0xFF) as u8;
                        if b == 0 {
                            break;
                        }
                        self.ruta.push(b);
                    }
                }
                // La consola AL REVES: lo que el terminal habria tecleado. Se
                // siembra con `poner_entrada` y sale de 7 en 7, como en el
                // kernel. Es lo que hace testeable el `ACCEPT` de COBOL.
                op if op == TASK_OP_CONSOLE_READ => {
                    let mut w = [0u8; 8];
                    let mut n = 0usize;
                    while n < 7 && self.entrada_cursor < self.entrada.len() {
                        w[n] = self.entrada[self.entrada_cursor];
                        self.entrada_cursor += 1;
                        n += 1;
                    }
                    let v = ((n as u64) << 56) | u64::from_le_bytes(w);
                    self.finalizar_syscall(v);
                    return;
                }
                op if op == TASK_OP_ARCHIVO_ABRIR => {
                    let h = self.archivo_abrir(false);
                    self.finalizar_syscall(h);
                    return;
                }
                op if op == TASK_OP_ARCHIVO_CREAR => {
                    let h = self.archivo_abrir(true);
                    self.finalizar_syscall(h);
                    return;
                }
                // Reclamar la entrada. Sin `ceder_entrada()` devuelve 0, que
                // es el handle nulo: exactamente lo que ve un programa cuando
                // otro proceso la tiene tomada.
                op if op == TASK_OP_INPUT_CLAIM => {
                    let h = if self.entrada_cedida { CAP_ENTRADA } else { 0 };
                    self.finalizar_syscall(h);
                    return;
                }
                // Pedir memoria. Un rechazo NO es "handle 0": es un **codigo de
                // error en rax**, y esa es la diferencia que el emulador tiene
                // que respetar. `malloc` mira `rax` primero (`test eax,eax`), y
                // un modelo que devolviera siempre codigo 0 dejaria sin probar
                // justo la rama que decide si el tope se cumple.
                op if op == TASK_OP_MEMORIA_PEDIR => {
                    match self.memoria_pedir(call.arg0) {
                        Ok(h) => self.finalizar_syscall(h),
                        Err(code) => self.fallar_syscall(code),
                    }
                    return;
                }
                // Ceder el turno es el borde del fotograma: aqui es donde
                // "llega" lo que el usuario tecleo mientras tanto.
                // El SONIDO. Reclamarlo dos veces sin soltar tiene que fallar:
                // es la propiedad entera de un aparato exclusivo, y modelarla
                // aqui es lo que permite probarla sin encender el Ryzen.
                op if op == TASK_OP_AUDIO_RECLAMAR => {
                    let h = if self.audio_dueno { 0 } else { CAP_AUDIO };
                    self.audio_dueno = true;
                    self.finalizar_syscall(h);
                    return;
                }
                op if op == TASK_OP_AUDIO_SOLTAR => {
                    if self.audio_dueno {
                        self.audio_dueno = false;
                        self.finalizar_syscall(0);
                    } else {
                        // No era suyo. El kernel contesta ERROR_BUSY, no OK:
                        // un "si" a quien no era dueno le haria creer que lo
                        // solto.
                        self.fallar_syscall(16);
                    }
                    return;
                }
                op if op == TASK_OP_YIELD => {
                    if let Some(lote) = self.lotes.pop() {
                        self.teclas.extend_from_slice(&lote);
                    }
                }
                _ => {}
            }
        } else if call.capability == CAP_AUDIO {
            // [!] Y **solo si sigue siendo suyo**. Un handle que funciona
            // despues de soltarlo es un uso-despues-de-liberar con otro nombre,
            // y en el kernel de verdad no resuelve porque la generacion cambio.
            // Si el emulador no modelara esto, la prueba que lo comprueba
            // pasaria con el kernel roto.
            if !self.audio_dueno {
                self.fallar_syscall(2); // ERROR_INVALID_HANDLE
                return;
            }
            let v = self.audio_op(call.operation, call.arg0, self.regs[R10]);
            self.finalizar_syscall(v);
            return;
        } else if call.capability == CAP_ENTRADA {
            let v = self.entrada_op(call.operation);
            self.finalizar_syscall(v);
            return;
        } else if call.capability >= CAP_MEMORIA {
            let v = self.memoria_op(call.capability, call.operation);
            self.finalizar_syscall(v);
            return;
        } else if call.capability != 0 {
            // Cualquier otro handle: aqui solo existen los de archivo. El
            // emulador no modela la pantalla ni el raton porque ningun codigo
            // EMITIDO los toca -- los usa el compositor, que es Rust normal.
            let v = self.archivo_op(call.capability, call.operation, call.arg0);
            self.finalizar_syscall(v);
            return;
        }

        self.finalizar_syscall(0);
    }

    /// El epilogo comun de toda llamada.
    ///
    /// * El valor vuelve en **rdx**, no en rax. `BmoStatus` es
    /// `{code, flags, value}`: rax trae el codigo y las banderas, rdx trae el
    /// valor. Se puede leer en el stub de `userland::syscall`.
    ///
    /// Esto estaba MAL modelado: el emulador ponia `rax = 0` y no tocaba rdx,
    /// asi que ahi seguia el argumento de entrada. Por eso `console::read_line`
    /// --la puerta de `ACCEPT`-- no tiene ni un test: en el emulador habria
    /// visto siempre "no hay nada" y girado para siempre. El emulador mentia
    /// sobre la puerta, que es justo lo que no puede hacer.
    fn finalizar_syscall(&mut self, valor: u64) {
        // El silicio destruye estos dos.
        self.regs[RCX] = POISON;
        self.regs[R11] = POISON;
        self.regs[RAX] = 0; // code = 0 (ok), flags = 0
        self.regs[RDX] = valor;
    }

    /// El epilogo de una llamada que el kernel RECHAZA: codigo en `rax` y
    /// **valor envenenado** en `rdx`.
    ///
    /// Lo segundo es a proposito. Un programa que se salta la comprobacion del
    /// codigo y usa el valor igual tiene que estropearse aqui, en un test, y no
    /// en el Ryzen -- donde `rdx` traeria lo que hubiera quedado y funcionaria
    /// por casualidad las primeras veces.
    fn fallar_syscall(&mut self, code: u64) {
        self.regs[RCX] = POISON;
        self.regs[R11] = POISON;
        self.regs[RAX] = code;
        self.regs[RDX] = POISON;
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
                        self.flags_logic(r);
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
                        // No-ordenado (algun NaN) pone las tres a 1. No pasa
                        // hoy, y esta dicho para que el dia que pase no
                        // parezca "menor que".
                        if a.is_nan() || b.is_nan() {
                            self.zf = true;
                            self.cf = true;
                        } else {
                            self.zf = a == b;
                            self.cf = a < b;
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
                        self.write_reg(reg, (v as i64) as u64, true);
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
                    // imul reg, r/m
                    0xAF => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let a = self.read_reg(reg, wide) as i64;
                        let b = self.load(src, wide) as i64;
                        let r = a.wrapping_mul(b) as u64;
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
