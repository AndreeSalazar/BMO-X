//! Emulador x86-64 mínimo — el banco de pruebas del toolchain.
//!
//! # Por qué existe
//!
//! Un emisor de código máquina que solo se testea comparando bytes contra
//! bytes escritos a mano no prueba nada: si el autor entendió mal una
//! codificación, el test la repite y pasa igual de mal. Peor aún: un
//! `IF` que emite un salto con desplazamiento cero **parece** código
//! correcto en un volcado de bytes, compila, valida el BEF, y ejecuta las
//! dos ramas en hardware. Esa clase de mentira solo la caza la ejecución.
//!
//! Así que este módulo no compara: **ejecuta**. Corre el código emitido y
//! modela la puerta del kernel (`uconsole::write_packed`: 8 bytes LE,
//! NUL-stop) para reconstruir el texto que aparecería en pantalla. El test
//! compara ese texto con lo que el programa debería imprimir.
//!
//! Modela también dos cosas que el silicio hace y es fácil olvidar:
//! - `syscall` destruye `rcx` y `r11` → aquí se llenan de veneno, para que
//!   cualquier código que dependa de ellos falle en el test y no en el metal.
//! - Escribir un registro de 32 bits pone a cero la mitad alta del de 64.
//!
//! # Alcance
//!
//! Cubre el subconjunto que emiten los frontends de BMO: movimientos,
//! aritmética entera con signo, `imul`/`idiv`, pila, direccionamiento
//! `[rbp+disp]` y `[rsp]`, comparaciones, saltos condicionales e
//! incondicionales, y `syscall`. **No** es un emulador general: ante un
//! opcode que ningún emisor de BMO produce hace panic con el byte, que es
//! la respuesta correcta — significa que alguien emitió algo sin pensar en
//! cómo lo iba a verificar.
//!
//! Se activa con la feature `emulator` para que no viaje en las builds
//! normales del toolchain.

use std::collections::HashMap;

/// Dirección base del área de datos que carga el test.
pub const DATA_BASE: u64 = 0x1_0000;
/// Tope de pila inicial. Alineado a 64 como pide el contrato de BMO.
pub const STACK_TOP: u64 = 0x7000_0000;

const POISON: u64 = 0xDEAD_BEEF_DEAD_BEEF;

/// El handle que devuelve `TASK_OP_INPUT_CLAIM` aquí dentro. Lejos del rango
/// de los archivos (1..n) a propósito: un programa que confunda los dos
/// handles tiene que fallar en la prueba, no acertar por casualidad.
const CAP_ENTRADA: u64 = 0x0001_0001;

const RAX: usize = 0;
const RCX: usize = 1;
const RDX: usize = 2;
const RSP: usize = 4;
const RSI: usize = 6;
const RDI: usize = 7;
const R11: usize = 11;

/// Una llamada observada cruzando CPL3→CPL0.
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
/// programa que se olvida del `CLOSE` pasaría los tests y perdería el fichero
/// en la máquina real — que es exactamente la clase de mentira que este módulo
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
    pub code: Vec<u8>,
    pub rip: usize,
    /// Texto que el kernel habría pintado.
    pub console: String,
    /// Toda llamada observada, en orden.
    pub syscalls: Vec<ObservedSyscall>,
    /// True cuando el programa invocó `TASK_OP_EXIT`.
    pub exited: bool,
    /// El disco, modelado: ruta → contenido.
    ///
    /// Sin esto el File I/O de COBOL no se podría probar de ninguna forma —
    /// `OPEN`/`READ`/`WRITE` sólo se distinguen de un no-op **ejecutándolos**,
    /// que es la lección entera de este módulo. Las pruebas siembran los
    /// archivos con [`Machine::poner_archivo`] y leen lo escrito con
    /// [`Machine::archivo`].
    pub archivos: HashMap<String, Vec<u8>>,
    /// Lo que el terminal habría tecleado para este proceso. Lo siembra
    /// [`Machine::poner_entrada`] y lo drena `TASK_OP_CONSOLE_READ`.
    entrada: Vec<u8>,
    entrada_cursor: usize,
    /// El renglón donde se acumula una ruta byte a byte (`TASK_OP_RUTA`),
    /// igual que en el kernel: la superficie no acepta punteros.
    ruta: Vec<u8>,
    /// Archivos abiertos: `(ruta, contenido, cursor, escribe)`.
    abiertos: Vec<Abierto>,
    /// ── La entrada, modelada ────────────────────────────────────────────
    ///
    /// Esto no estaba, y el comentario que lo justificaba —"ningún código
    /// emitido toca el ratón, lo usa el compositor, que es Rust normal"— dejó
    /// de ser verdad en cuanto un frontend pudo emitir la puerta. Mientras no
    /// estuvo, **la rueda sólo se podía probar en el Ryzen**: un `INPUT_OP_RUEDA`
    /// que devuelve siempre lo mismo se ve idéntico a uno que consume, y ésa
    /// es justo la diferencia que decide si un scroll se mueve solo.
    ///
    /// El ratón se declara AUSENTE por defecto (`entrada_cedida = false`), que
    /// es lo que ve un programa cuando otro proceso ya reclamó la entrada.
    entrada_cedida: bool,
    /// Teclas pendientes, en orden. Las siembra [`Machine::poner_teclas`] y
    /// las drena `INPUT_OP_TECLA`, una por llamada.
    teclas: Vec<u8>,
    teclas_cursor: usize,
    /// Teclas que aún no han LLEGADO: un lote por fotograma.
    ///
    /// Sin esto, todo lo sembrado está disponible en la primera vuelta del
    /// bucle, y un programa que drena el teclado hasta vaciarlo —que es lo
    /// correcto— ve la sesión entera de golpe. Un ESC al final de la lista
    /// mata el programa antes de que llegue a reaccionar a nada.
    ///
    /// El reloj es `YIELD`, y no es una convención inventada: un bucle de
    /// fotograma que no cede se come el quantum, así que ceder **es** el borde
    /// del fotograma. Ver [`Machine::poner_teclas_por_fotograma`].
    lotes: Vec<Vec<u8>>,
    /// Muescas de rueda acumuladas. **Leerlas las vacía**, igual que el kernel.
    rueda: i32,
    /// `(x, y, botones)` y el pulsómetro de informes HID.
    puntero: (u32, u32, u8),
    eventos_hid: u64,
    modificadores: u8,
    mem: HashMap<u64, u8>,
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
            code,
            rip: 0,
            console: String::new(),
            syscalls: Vec::new(),
            exited: false,
            archivos: HashMap::new(),
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
            mem: HashMap::new(),
            data_len: 0,
            zf: false,
            sf: false,
            of: false,
            cf: false,
        };
        m.regs[RSP] = STACK_TOP;
        m
    }

    /// Coloca bytes en memoria y devuelve su dirección.
    pub fn load_data(&mut self, bytes: &[u8]) -> u64 {
        let addr = DATA_BASE + self.data_len;
        for (i, b) in bytes.iter().enumerate() {
            self.mem.insert(addr + i as u64, *b);
        }
        self.data_len += bytes.len() as u64;
        addr
    }

    /// Un byte de memoria, para que los tests puedan mirar si el emisor
    /// escribió donde no debía. Sin esto, un desbordamiento de buffer sólo se
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
    /// Si nadie escribió ahí, cae a la propia imagen: los frontends colocan
    /// las cadenas y los globales DENTRO de la sección de código, justo
    /// detrás de las instrucciones, y los alcanzan con `lea [rip+disp]`. Un
    /// `%s` leería ceros si el emulador no modelara eso. Fuera de la imagen
    /// devuelve cero, que es lo que hace el kernel con una página nueva.
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

    /// Siembra lo que el terminal habría tecleado. El `\n` final hace falta:
    /// `read_line` espera verlo para dar la línea por cerrada, exactamente
    /// igual que en la máquina.
    pub fn poner_entrada(&mut self, texto: &str) {
        self.entrada.extend_from_slice(texto.as_bytes());
    }

    /// Siembra un archivo antes de ejecutar. Es el disco de la prueba.
    pub fn poner_archivo(&mut self, ruta: &str, datos: &[u8]) {
        self.archivos.insert(ruta.to_string(), datos.to_vec());
    }

    /// Lo que hay en el disco al terminar. `None` si ese archivo no existe —
    /// que es distinto de existir vacío, y en un batch bancario esa diferencia
    /// es la que separa "no se escribió" de "se escribió cero registros".
    pub fn archivo(&self, ruta: &str) -> Option<&[u8]> {
        self.archivos.get(ruta).map(|v| v.as_slice())
    }

    /// Igual, pero como texto. Comodidad para los tests.
    pub fn archivo_texto(&self, ruta: &str) -> Option<String> {
        self.archivo(ruta).map(|b| String::from_utf8_lossy(b).into_owned())
    }

    // ── Sembrar la entrada ──────────────────────────────────────────────

    /// Concede la entrada: a partir de aquí `TASK_OP_INPUT_CLAIM` funciona.
    ///
    /// Hay que pedirlo a propósito porque la entrada es **exclusiva**: sin
    /// esto, la prueba ve lo mismo que un programa lanzado mientras el
    /// compositor la tiene tomada, que es el caso que más se equivoca al
    /// escribirlo.
    pub fn ceder_entrada(&mut self) {
        self.entrada_cedida = true;
    }

    /// Teclas que el programa irá recogiendo con `INPUT_OP_TECLA`, una por
    /// llamada. Los bytes son Latin-1 ya resueltos; para las que no tienen
    /// glifo, las constantes `TECLA_*` de `bmo_abi::syscalls::surface`.
    pub fn poner_teclas(&mut self, teclas: &[u8]) {
        self.teclas.extend_from_slice(teclas);
    }

    /// Teclas repartidas EN EL TIEMPO: un lote por fotograma, entendiendo por
    /// fotograma cada `YIELD` que haga el programa.
    ///
    /// Es la diferencia entre probar un programa interactivo y probar una
    /// ráfaga: con todo disponible de golpe, un bucle que drena el teclado ve
    /// la sesión entera en la primera vuelta y nunca llega a repintar entre
    /// pulsación y pulsación — que es justo la conducta que se quiere mirar.
    ///
    /// El primer lote llega tras el primer `YIELD`; lo que deba estar ahí
    /// desde el principio va en [`Machine::poner_teclas`].
    pub fn poner_teclas_por_fotograma(&mut self, lotes: &[&[u8]]) {
        // Se guardan al revés para poder sacar el siguiente por el final, que
        // es O(1). El orden que ve el programa es el de la lista.
        for lote in lotes.iter().rev() {
            self.lotes.push(lote.to_vec());
        }
    }

    /// Suma muescas de rueda. Positivo = hacia arriba. Se acumulan hasta que
    /// alguien las lea, y leerlas las vacía.
    pub fn poner_rueda(&mut self, muescas: i32) {
        self.rueda += muescas;
        self.eventos_hid += muescas.unsigned_abs() as u64;
    }

    /// Coloca el puntero y sube el pulsómetro de informes HID.
    pub fn poner_puntero(&mut self, x: u32, y: u32, botones: u8) {
        self.puntero = (x, y, botones);
        self.eventos_hid += 1;
    }

    /// Modificadores pulsados AHORA (`MOD_SHIFT`, `MOD_CTRL`…). Es estado: se
    /// queda puesto hasta que se cambie.
    pub fn poner_modificadores(&mut self, mascara: u8) {
        self.modificadores = mascara;
    }

    /// Muescas de rueda que quedan sin leer. Un programa que se olvida de
    /// drenarla las deja aquí, y la prueba puede decirlo.
    pub fn rueda_pendiente(&self) -> i32 {
        self.rueda
    }

    /// Despacho de la capability de entrada. Copia la semántica de
    /// `ring0/obj/input.rs` — sobre todo la que se nota: la rueda CONSUME.
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
            // distingue "llegó el byte 0" de "no llegó nada".
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
            // ★ Consume. Dos lecturas seguidas sin girar dan cero la segunda.
            INPUT_OP_RUEDA => {
                let v = self.rueda;
                self.rueda = 0;
                v as i64 as u64
            }
            _ => 0,
        }
    }

    /// Abre o crea. Devuelve el handle (el índice + 1, para que 0 no sea uno
    /// válido) o 0 si no se pudo.
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
                // `ERROR_NO_ESTA`; aquí es un handle nulo. Devolver uno vacío
                // haría que un `READ` de un fichero que falta pareciera un
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
                    // ★ AQUI es donde llega al disco, y sólo aquí. Igual que
                    // en el kernel.
                    let (ruta, datos) = (a.ruta.clone(), a.datos.clone());
                    self.archivos.insert(ruta, datos);
                }
                1
            }
            // El modo manda: pedirle bytes a uno de escritura no es un error
            // de permisos, es una pregunta que ese objeto no responde.
            _ => 0,
        }
    }

    /// La puerta del kernel, modelada.
    fn do_syscall(&mut self) {
        use bmo_abi::syscalls::surface::{
            CURRENT_TASK, NR_INVOKE, TASK_OP_ARCHIVO_ABRIR, TASK_OP_ARCHIVO_CREAR,
            TASK_OP_CONSOLE_READ, TASK_OP_CONSOLE_WRITE, TASK_OP_EXIT, TASK_OP_INPUT_CLAIM,
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
                            break; // NUL-stop: idéntico al kernel
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
                // Ceder el turno es el borde del fotograma: aquí es donde
                // "llega" lo que el usuario tecleó mientras tanto.
                op if op == TASK_OP_YIELD => {
                    if let Some(lote) = self.lotes.pop() {
                        self.teclas.extend_from_slice(&lote);
                    }
                }
                _ => {}
            }
        } else if call.capability == CAP_ENTRADA {
            let v = self.entrada_op(call.operation);
            self.finalizar_syscall(v);
            return;
        } else if call.capability != 0 {
            // Cualquier otro handle: aqui solo existen los de archivo. El
            // emulador no modela la pantalla ni el raton porque ningun codigo
            // EMITIDO los toca — los usa el compositor, que es Rust normal.
            let v = self.archivo_op(call.capability, call.operation, call.arg0);
            self.finalizar_syscall(v);
            return;
        }

        self.finalizar_syscall(0);
    }

    /// El epílogo comun de toda llamada.
    ///
    /// ★ El valor vuelve en **rdx**, no en rax. `BmoStatus` es
    /// `{code, flags, value}`: rax trae el codigo y las banderas, rdx trae el
    /// valor. Se puede leer en el stub de `userland::syscall`.
    ///
    /// Esto estaba MAL modelado: el emulador ponia `rax = 0` y no tocaba rdx,
    /// asi que ahi seguia el argumento de entrada. Por eso `console::read_line`
    /// —la puerta de `ACCEPT`— no tiene ni un test: en el emulador habria
    /// visto siempre "no hay nada" y girado para siempre. El emulador mentia
    /// sobre la puerta, que es justo lo que no puede hacer.
    fn finalizar_syscall(&mut self, valor: u64) {
        // El silicio destruye estos dos.
        self.regs[RCX] = POISON;
        self.regs[R11] = POISON;
        self.regs[RAX] = 0; // code = 0 (ok), flags = 0
        self.regs[RDX] = valor;
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
        // cadenas y variables globales (`lea rax, [rip+disp]`), así que sin
        // esto el emulador se comía los 4 bytes del desplazamiento como si
        // fueran instrucciones y descarrilaba.
        if md == 0 && rm == 0b101 {
            let disp = self.fetch_u32() as i32 as i64;
            let addr = (self.rip as i64 + disp) as u64;
            return (reg, Operand::Mem(addr));
        }

        // Base (+ índice si hay SIB).
        let (base, index, scale) = if rm == 0b100 {
            let sib = self.fetch_u8();
            let idx = (((sib >> 3) & 7) as usize) | (rex_x << 3);
            let base = ((sib & 7) as usize) | (rex_b << 3);
            // índice 4 sin REX.X significa "sin índice".
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

    /// Lee un solo byte del operando. En registro es el byte BAJO — con
    /// REX presente `dl`/`sil` son eso y no los registros altos heredados.
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

    fn store(&mut self, op: Operand, value: u64, wide: bool) {
        match op {
            Operand::Reg(r) => self.write_reg(r, value, wide),
            Operand::Mem(a) => {
                if wide {
                    self.write_u64(a, value);
                } else {
                    self.write_u64(a, value as u32 as u64);
                }
            }
        }
    }

    fn step(&mut self) {
        let mut byte = self.fetch_u8();
        let mut rex = 0u8;
        // Prefijos que emitimos: F3 (pause) se trata aparte más abajo.
        if (0x40..=0x4F).contains(&byte) {
            rex = byte;
            byte = self.fetch_u8();
        }
        let wide = rex & 0x08 != 0;
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
                    0x89 => self.store(dst, b, wide),
                    0x09 => {
                        let r = a | b;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    0x21 => {
                        let r = a & b;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    0x31 => {
                        let r = a ^ b;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    0x01 => {
                        let r = a.wrapping_add(b);
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    0x29 => {
                        self.flags_sub(a, b);
                        let r = a.wrapping_sub(b);
                        self.store(dst, r, wide);
                    }
                    0x39 => self.flags_sub(a, b), // cmp
                    0x85 => self.flags_logic(a & b), // test
                    _ => unreachable!(),
                }
            }
            // ALU  reg, r/m  (dirección contraria)
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
            // movsxd reg64, r/m32 — carga un int CON SIGNO
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
            // grupo 1 con imm8: /0 add, /5 sub, /7 cmp, /4 and
            0x83 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as i8 as i64 as u64;
                let a = self.load(dst, wide);
                match ext & 7 {
                    0 => {
                        let r = a.wrapping_add(imm);
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    4 => {
                        let r = a & imm;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    5 => {
                        self.flags_sub(a, imm);
                        let r = a.wrapping_sub(imm);
                        self.store(dst, r, wide);
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
                        self.store(dst, r, wide);
                    }
                    // AND. Lo emite `and_r64_imm32`, que usa `read_line` para
                    // quedarse con el byte bajo del paquete. Faltaba, y esa
                    // ausencia es la prueba de que `read_line` nunca se habia
                    // EJECUTADO aqui — solo emitido.
                    4 => {
                        let r = a & imm;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    5 => {
                        self.flags_sub(a, imm);
                        let r = a.wrapping_sub(imm);
                        self.store(dst, r, wide);
                    }
                    7 => self.flags_sub(a, imm),
                    other => panic!("grupo 81 /{other} no emitido por BMO"),
                }
            }
            // mov r/m, imm32
            0xC7 => {
                let (_, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u32() as i32 as i64 as u64;
                self.store(dst, imm, wide);
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
                self.store(dst, r, wide);
            }
            // mov r/m8, r8  — guarda el byte bajo de un registro
            0x88 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.regs[reg] & 0xFF;
                self.store_u8(dst, v);
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
                self.store(dst, r, wide);
            }
            // grupo 3: /3 neg, /6 div, /7 idiv
            0xF7 => {
                let (ext, src) = self.modrm(0, rex_x, rex_b);
                let v = self.load(src, wide);
                match ext & 7 {
                    3 => {
                        let r = (self.load(src, wide) as i64).wrapping_neg() as u64;
                        self.flags_logic(r);
                        self.store(src, r, wide);
                    }
                    // div SIN signo: rdx:rax entre el operando. El emisor
                    // siempre pone rdx=0 antes, así que basta con rax.
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
                        // idiv: dividendo en rdx:rax; aquí basta rax con signo
                        // extendido por cqo, que es lo único que emitimos.
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
                let r = match ext & 7 {
                    0 => a.wrapping_add(1),
                    1 => a.wrapping_sub(1),
                    other => panic!("grupo FF /{other} no emitido por BMO"),
                };
                self.flags_logic(r);
                self.store(dst, r, wide);
            }
            // cqo — extiende el signo de rax a rdx
            0x99 => {
                self.regs[RDX] = if (self.regs[RAX] as i64) < 0 {
                    u64::MAX
                } else {
                    0
                };
            }
            0x90 => {} // nop
            // call rel32 / ret — las funciones de C se llaman así.
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
            // cdqe/cwde — extiende eax a rax con signo
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
            0xF3 => {
                let nop = self.fetch_u8();
                assert_eq!(nop, 0x90, "solo se emite PAUSE con prefijo F3");
            }
            0x0F => {
                let second = self.fetch_u8();
                match second {
                    0x05 => self.do_syscall(),
                    // movsx reg, r/m8 — carga un char CON signo
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
                    // imul reg, r/m
                    0xAF => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let a = self.read_reg(reg, wide) as i64;
                        let b = self.load(src, wide) as i64;
                        let r = a.wrapping_mul(b) as u64;
                        self.write_reg(reg, r, wide);
                    }
                    // setcc r/m8 — deja 0 o 1 según la condición
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

    /// Evalúa el código de condición de un `jcc` (el nibble bajo del opcode).
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

/// Ejecuta hasta caer del final del código, hasta `EXIT`, o hasta agotar el
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
