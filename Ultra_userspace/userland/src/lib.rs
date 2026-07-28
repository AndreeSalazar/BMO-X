//! **El runtime de Ring 3 de BMO.** La cara de userspace de los tres syscalls.
//!
//! Antes esta crate re-exportaba `BootContext` "para que userland tuviera la
//! misma struct que el kernel para el handoff". Eso era de otro sistema y era
//! el modelo equivocado entero: **un proceso Ring 3 no recibe la estructura de
//! arranque del kernel.** No sabe cuánta RAM hay, ni dónde está el
//! framebuffer, ni qué discos existen. Recibe *capabilities*, y cada una es un
//! permiso concreto sobre un objeto concreto. Lo que no le hayan dado, no
//! existe para él. Ese es el trato.
//!
//! ## La superficie, entera
//!
//! Tres syscalls. No hay un cuarto y no va a haberlo:
//!
//! ```text
//!   INVOKE(cap, operacion, a0, a1, a2)   la puerta síncrona
//!   CHANNEL_KICK(cap, secuencia)         avisar al consumidor
//!   WAIT(esperable, visto, timeout_ns)   bloquearse
//! ```
//!
//! Todo lo demás —abrir un endpoint, escribir en consola, reclamar la
//! pantalla— es una *operación* sobre una capability. La API crece por dentro,
//! en la pareja `(tipo de objeto, operación)`, y el ABI no se toca. Añadir
//! "abrir ventana" no es cambiar la frontera: es un número más en una tabla.
//!
//! ## Convención de registros
//!
//! `rax` = número de syscall; argumentos en `rdi, rsi, rdx, r10, r8`.
//!
//! ★ **`r10` y no `rcx`.** En `SYSCALL` el CPU mete el RIP de retorno en `rcx`
//! y las RFLAGS en `r11`: un argumento en `rcx` no sería el dato de nadie,
//! sería la dirección a la que volver. Linux salta a `r10` por lo mismo. Este
//! detalle ya se cobró una tarde en el lado del kernel.
//!
//! De vuelta: `rax` = `codigo | (flags << 32)`, `rdx` = valor.

#![no_std]

use core::arch::asm;

// ── La superficie congelada ─────────────────────────────────────────────

pub const NR_INVOKE: u32 = 0;
pub const NR_CHANNEL_KICK: u32 = 1;
pub const NR_WAIT: u32 = 2;

/// Pseudo-capability que se refiere al proceso que llama. No es un handle
/// concedido: es la forma de pedir lo que uno ya tiene por ser quien es.
pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;

// Operaciones sobre `CURRENT_TASK`.
pub const OP_GET_PID: u32 = 0x01;
pub const OP_GET_TID: u32 = 0x02;
pub const OP_YIELD: u32 = 0x03;
pub const OP_EXIT: u32 = 0x04;
pub const OP_CHANNEL_OPEN: u32 = 0x05;
pub const OP_CONSOLE_WRITE: u32 = 0x06;
pub const OP_ENDPOINT_CREATE: u32 = 0x07;
pub const OP_ENDPOINT_CONNECT: u32 = 0x08;
pub const OP_FRAMEBUFFER_CLAIM: u32 = 0x09;
pub const OP_INPUT_CLAIM: u32 = 0x0A;
pub const OP_RUTA: u32 = 0x0B;
pub const OP_EJECUTAR: u32 = 0x0C;

// Operaciones sobre un handle de pantalla (`KIND_FRAMEBUFFER`).
pub const FB_OP_BASE: u32 = 0x01;
pub const FB_OP_DIMS: u32 = 0x02;
pub const FB_OP_STRIDE: u32 = 0x03;
pub const FB_OP_BYTES: u32 = 0x04;

// Operaciones sobre un handle de entrada (`KIND_INPUT`).
pub const INPUT_OP_PUNTERO: u32 = 0x01;
pub const INPUT_OP_EVENTOS: u32 = 0x02;
pub const INPUT_OP_TECLA: u32 = 0x03;

/// Lo que devuelve un syscall: un código y un valor.
///
/// `code == 0` es lo único que significa éxito. `flags` lleva pistas del
/// kernel — por ejemplo `NEEDS_CAP`, que distingue "no tienes permiso" de
/// "ese handle no existe".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Status {
    pub code: u32,
    pub flags: u32,
    pub value: u64,
}

impl Status {
    #[inline(always)]
    pub fn ok(self) -> bool {
        self.code == 0
    }
    /// El valor si fue bien, o `None`. Para no comprobar el código a mano
    /// cada vez y acabar olvidándolo una.
    #[inline(always)]
    pub fn valor(self) -> Option<u64> {
        if self.code == 0 {
            Some(self.value)
        } else {
            None
        }
    }
}

#[inline(always)]
fn syscall(nr: u32, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> Status {
    let rax: u64;
    let rdx: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr as u64 => rax,
            in("rdi") a0,
            in("rsi") a1,
            inlateout("rdx") a2 => rdx,
            in("r10") a3,
            in("r8") a4,
            // El CPU los machaca: rcx = RIP de retorno, r11 = RFLAGS.
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    Status {
        code: rax as u32,
        flags: (rax >> 32) as u32,
        value: rdx,
    }
}

/// `INVOKE` — la puerta síncrona.
#[inline(always)]
pub fn invoke(cap: u64, operacion: u32, a0: u64, a1: u64, a2: u64) -> Status {
    syscall(NR_INVOKE, cap, operacion as u64, a0, a1, a2)
}

/// `CHANNEL_KICK` — avisar al consumidor de un estuario.
#[inline(always)]
pub fn channel_kick(cap: u64, secuencia: u64) -> Status {
    syscall(NR_CHANNEL_KICK, cap, secuencia, 0, 0, 0)
}

/// `WAIT` — bloquearse hasta que la secuencia del esperable pase de `visto`,
/// o hasta que venza el plazo. `esperable = 0` es dormir a secas.
#[inline(always)]
pub fn wait(esperable: u64, visto: u64, timeout_ns: u64) -> Status {
    syscall(NR_WAIT, esperable, visto, timeout_ns, 0, 0)
}

// ── Lo que uno tiene por ser quien es ───────────────────────────────────

#[inline]
pub fn pid() -> u64 {
    invoke(CURRENT_TASK, OP_GET_PID, 0, 0, 0).value
}

#[inline]
pub fn tid() -> u64 {
    invoke(CURRENT_TASK, OP_GET_TID, 0, 0, 0).value
}

/// Ceder el turno. Un bucle de espera en Ring 3 que no cede se come el quantum
/// entero sin avanzar nada.
#[inline]
pub fn ceder() {
    invoke(CURRENT_TASK, OP_YIELD, 0, 0, 0);
}

/// Terminar. No vuelve: el kernel revoca las capabilities del proceso y
/// cambia de contexto en el propio borde del syscall.
pub fn salir() -> ! {
    invoke(CURRENT_TASK, OP_EXIT, 0, 0, 0);
    // Si el kernel nos devolviera el control, seguir ejecutando sería peor
    // que quedarse quieto.
    loop {
        ceder();
    }
}

/// Escribir en la consola del kernel.
///
/// La puerta admite 8 bytes empaquetados en little-endian por llamada, con el
/// cero como final. Es deliberadamente pobre: es la consola de arranque, no la
/// salida de nadie en serio. Cuando el compositor exista, el terminal será un
/// proceso Ring 3 y esto quedará para lo que es — decir "estoy vivo" antes de
/// que haya con qué decirlo.
pub fn consola(texto: &str) {
    for trozo in texto.as_bytes().chunks(8) {
        let mut w = [0u8; 8];
        w[..trozo.len()].copy_from_slice(trozo);
        invoke(CURRENT_TASK, OP_CONSOLE_WRITE, u64::from_le_bytes(w), 0, 0);
    }
}

// ── La pantalla ─────────────────────────────────────────────────────────

/// La pantalla, ya mapeada en este proceso.
///
/// No hay un `dibujar()` que cruce el anillo, y no lo va a haber: el
/// framebuffer **es memoria de este proceso**. `base` es un puntero de verdad
/// y escribir en él es un `mov`. Ése es el trato entero de `KIND_FRAMEBUFFER`
/// — el kernel contesta cuatro preguntas al arrancar y después se aparta.
pub struct Pantalla {
    /// Handle de la capability. Hace falta para preguntarle cosas.
    pub cap: u64,
    pub base: *mut u32,
    pub ancho: u32,
    pub alto: u32,
    /// En PÍXELES, no en bytes: es el mismo número que usa el kernel.
    pub stride: u32,
    pub formato: u32,
    pub bytes: u64,
}

impl Pantalla {
    /// Reclamarla. Sólo un proceso puede tenerla a la vez; el kernel deja de
    /// dibujar mientras dure, y la recupera solo si este proceso muere.
    pub fn reclamar() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_FRAMEBUFFER_CLAIM, 0, 0, 0).valor()?;
        let base = invoke(cap, FB_OP_BASE, 0, 0, 0).valor()?;
        let dims = invoke(cap, FB_OP_DIMS, 0, 0, 0).valor()?;
        let stride = invoke(cap, FB_OP_STRIDE, 0, 0, 0).valor()?;
        let bytes = invoke(cap, FB_OP_BYTES, 0, 0, 0).valor()?;
        Some(Self {
            cap,
            base: base as *mut u32,
            ancho: (dims >> 32) as u32,
            alto: dims as u32,
            stride: (stride >> 32) as u32,
            formato: stride as u32,
            bytes,
        })
    }

    /// Píxeles que caben en el área mapeada.
    #[inline]
    pub fn pixeles(&self) -> usize {
        (self.bytes / 4) as usize
    }

    /// Un píxel, sin comprobar nada. Es el camino caliente de un compositor y
    /// no va a llevar una rama dentro.
    ///
    /// # Safety
    /// `x < stride` y `y < alto`.
    #[inline(always)]
    pub unsafe fn punto_sin_comprobar(&self, x: u32, y: u32, color: u32) {
        self.base
            .add((y as usize) * (self.stride as usize) + x as usize)
            .write_volatile(color);
    }

    /// Un píxel, comprobando. Fuera de la pantalla no hace nada.
    #[inline]
    pub fn punto(&self, x: u32, y: u32, color: u32) {
        if x < self.ancho && y < self.alto {
            unsafe { self.punto_sin_comprobar(x, y, color) };
        }
    }

    /// Rellenar la pantalla entera.
    ///
    /// Sólo para el primer pintado. Repetirlo por fotograma sería recorrer
    /// varios MB de memoria de vídeo sin caché: un pase de diapositivas. Lo
    /// que se repinta en un bucle es el DAÑO, no la pantalla.
    pub fn limpiar(&self, color: u32) {
        let n = self.pixeles();
        for i in 0..n {
            unsafe { self.base.add(i).write_volatile(color) };
        }
    }

    /// Un rectángulo, recortado a la pantalla. Es la única primitiva de dibujo
    /// que hay, y con ella se hace un escritorio entero: fondo, barra,
    /// ventanas, bordes. Lo demás son estas mismas llamadas puestas en orden.
    pub fn rect(&self, x: u32, y: u32, ancho: u32, alto: u32, color: u32) {
        let x1 = (x.saturating_add(ancho)).min(self.ancho);
        let y1 = (y.saturating_add(alto)).min(self.alto);
        let mut fila = y;
        while fila < y1 {
            let mut col = x;
            while col < x1 {
                unsafe { self.punto_sin_comprobar(col, fila, color) };
                col += 1;
            }
            fila += 1;
        }
    }
}

// ── Las letras ──────────────────────────────────────────────────────────

/// La fuente 8x16 de BMO, la MISMA que pinta el kernel.
///
/// ## Por qué está copiada aquí y no se pide por syscall
///
/// La alternativa era una operación `DIBUJAR_TEXTO` sobre el framebuffer, y
/// eso es exactamente la línea que `KIND_FRAMEBUFFER` existe para no cruzar: el
/// kernel contesta cuatro preguntas y se aparta. Si Ring 0 dibujara letras
/// tendría que saber de tipografía, de kerning y de colores — decisiones de
/// aspecto, ninguna suya. Es el mismo argumento que dejó el cursor en Ring 3.
///
/// Así que aquí hay 4 KiB de tabla duplicada. Sale del mismo generador
/// (`toolchain/tools/fontgen`), así que no son dos fuentes que puedan
/// divergir: son dos copias de una, y regenerar actualiza las dos.
static FONT16: [[u8; 16]; 120] = include!("font16_data.rs");
static FONT_EXTRA: [u8; 25] = include!("font16_extra.rs");
const ASCII_GLYPHS: usize = 95;

/// Ancho y alto de un glifo, en píxeles. El avance horizontal ES el ancho: la
/// fuente ya trae su propio espaciado dentro del mapa de bits.
pub const GLIFO_ANCHO: u32 = 8;
pub const GLIFO_ALTO: u32 = 16;

/// Byte → índice de glifo. ASCII directo; para el español (ñ, á, ¿...) se
/// busca el byte Latin-1 en la tabla de extras.
///
/// **Latin-1 y no UTF-8, igual que en Ring 0.** Un carácter es UN byte, así el
/// teclado, la caja y la fuente hablan el mismo idioma sin decodificador de por
/// medio. Un `&str` de Rust es UTF-8, así que una `ñ` escrita en el código
/// fuente llega como dos bytes y no se dibuja: para eso están `texto_bytes` y
/// el hecho de que lo que se teclea ya viene en Latin-1 del kernel.
fn indice_glifo(c: u8) -> Option<usize> {
    if (32..=126).contains(&c) {
        return Some(c as usize - 32);
    }
    let mut i = 0;
    while i < FONT_EXTRA.len() {
        if FONT_EXTRA[i] == c {
            return Some(ASCII_GLYPHS + i);
        }
        i += 1;
    }
    None
}

impl Pantalla {
    /// Un carácter. Sólo pinta los píxeles encendidos: el fondo se respeta,
    /// que es lo que permite escribir encima de lo que ya hay sin recuadros.
    pub fn glifo(&self, x: u32, y: u32, c: u8, color: u32) {
        let idx = match indice_glifo(c) {
            Some(i) => i,
            None => return,
        };
        let g = &FONT16[idx];
        for (fila, &bits) in g.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            for col in 0..8u32 {
                if bits & (0x80 >> col) != 0 {
                    self.punto(x + col, y + fila as u32, color);
                }
            }
        }
    }

    /// Una tira de bytes Latin-1. Devuelve la x donde acabó, para encadenar.
    pub fn texto_bytes(&self, x: u32, y: u32, s: &[u8], color: u32) -> u32 {
        let mut cx = x;
        for &c in s {
            self.glifo(cx, y, c, color);
            cx += GLIFO_ANCHO;
        }
        cx
    }

    /// Un `&str`. Los bytes que no sean ASCII se saltan en vez de salir como
    /// basura: un literal con acentos viene en UTF-8 y esta fuente es Latin-1.
    pub fn texto(&self, x: u32, y: u32, s: &str, color: u32) -> u32 {
        self.texto_bytes(x, y, s.as_bytes(), color)
    }
}

// ── Lanzar un programa ──────────────────────────────────────────────────

/// Lanza el `.bex` que hay en `ruta`. Devuelve el tid, o el código del fallo.
///
/// La ruta viaja en trozos de 8 bytes, igual que la consola y por la misma
/// razón: los argumentos van en registros y el kernel no tiene todavía forma de
/// leer un puntero de Ring 3 con seguridad. Ver `TASK_OP_RUTA` en el kernel.
///
/// El gate de firma de ESTRATOS se aplica al otro lado: un binario sin firma
/// buena no se ejecuta por mucho que se pida desde aquí.
pub fn ejecutar(ruta: &[u8]) -> Result<u64, u32> {
    for trozo in ruta.chunks(8) {
        let mut w = [0u8; 8];
        w[..trozo.len()].copy_from_slice(trozo);
        invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
    }
    let st = invoke(CURRENT_TASK, OP_EJECUTAR, 0, 0, 0);
    if st.ok() {
        Ok(st.value)
    } else {
        Err(st.code)
    }
}

/// Los códigos que devuelve `ejecutar`. Son pocos a propósito: un proceso no
/// necesita saber de FAT32, pero sí distinguir las tres cosas que le hacen
/// cambiar de conducta.
pub const ERROR_NO_ESTA: u32 = 20;
pub const ERROR_GATE: u32 = 21;
pub const ERROR_OCUPADO: u32 = 22;
pub const ERROR_NO_ADMITIDO: u32 = 23;

// ── La entrada ──────────────────────────────────────────────────────────

/// Dónde está el puntero y qué botones tiene pulsados.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Punto {
    pub x: u32,
    pub y: u32,
    pub botones: u8,
}

/// La entrada —ratón **y teclado**— cedida a este proceso.
///
/// El kernel lee el HID —transferencias xHCI, endpoints, reintentos— porque
/// eso es tocar hardware y todavía no hay otro sitio donde pueda vivir. Lo que
/// entrega son coordenadas ya recortadas al panel, una máscara de botones y los
/// bytes que van saliendo del teclado.
///
/// **El cursor no sale de aquí, y las letras tampoco.** Su forma, su color y su
/// contorno son decisiones de aspecto, y ninguna de ésas tiene nada que hacer
/// en Ring 0.
///
/// ★ Reclamarla es EXCLUSIVO y tiene consecuencia: mientras este proceso la
/// tenga, el shell de Ring 0 deja de leer el teclado físico. No es un reparto,
/// es una cesión — si los dos leyeran la misma cola se repartirían las letras.
pub struct Entrada {
    pub cap: u64,
}

impl Entrada {
    pub fn reclamar() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_INPUT_CLAIM, 0, 0, 0).valor()?;
        Some(Self { cap })
    }

    /// Una llamada por fotograma: los tres datos vienen empaquetados.
    pub fn puntero(&self) -> Punto {
        let v = invoke(self.cap, INPUT_OP_PUNTERO, 0, 0, 0).value;
        Punto {
            x: (v >> 32) as u32,
            y: ((v >> 16) & 0xFFFF) as u32,
            botones: (v & 0xFF) as u8,
        }
    }

    /// Cuántos reportes HID se han visto. Distingue "el ratón no se mueve" de
    /// "el ratón no llega": si esto no sube, el problema está en el USB.
    pub fn eventos(&self) -> u64 {
        invoke(self.cap, INPUT_OP_EVENTOS, 0, 0, 0).value
    }

    /// La siguiente tecla, si hay alguna. **No bloquea**: devuelve `None`
    /// cuando no hay nada esperando.
    ///
    /// No bloquea a propósito. Un compositor tiene un bucle de fotograma; si
    /// se durmiera en el teclado, el cursor se congelaría entre tecla y tecla —
    /// el ratón dejaría de moverse mientras nadie escribe, que es exactamente
    /// al revés de lo que uno quiere.
    ///
    /// El byte es **Latin-1**: la `ñ` llega como `0xF1`, que es justo el índice
    /// que entiende la fuente. Sin decodificador de por medio.
    pub fn tecla(&self) -> Option<u8> {
        let v = invoke(self.cap, INPUT_OP_TECLA, 0, 0, 0).value;
        if v & 0x100 != 0 {
            Some((v & 0xFF) as u8)
        } else {
            None
        }
    }
}
