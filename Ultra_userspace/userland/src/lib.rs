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
pub const OP_CONSOLA_CREAR: u32 = 0x0D;
pub const OP_DIR_ABRIR: u32 = 0x0E;
pub const OP_CONSOLE_READ: u32 = 0x0F;
pub const OP_ARCHIVO_ABRIR: u32 = 0x10;
pub const OP_ARCHIVO_CREAR: u32 = 0x11;
/// Reiniciar la máquina. No vuelve. Ver [`reiniciar`].
pub const OP_REINICIAR: u32 = 0x12;
/// Un dato del sistema. Ver [`info`] y [`info_texto`].
pub const OP_INFO: u32 = 0x13;
pub const OP_INFO_TEXTO: u32 = 0x14;
/// Pedir un bloque de memoria. Ver [`Memoria`].
pub const OP_MEMORIA_PEDIR: u32 = 0x15;
/// El log del kernel, leído desde Ring 3. Ver `klog_lineas`/`klog_texto`.
pub const OP_KLOG_INFO: u32 = 0x16;
pub const OP_KLOG_TEXTO: u32 = 0x17;
/// **Escribe en el disco.** Ver [`estratos_sellar`].
pub const OP_ESTRATOS_SELLAR: u32 = 0x18;

/// Dónde empieza el bloque, y cuánto se ha entregado a este proceso.
pub const MEM_OP_BASE: u32 = 0x01;
pub const MEM_OP_BYTES: u32 = 0x02;

// Campos de `OP_INFO`. Son una TABLA: añadir un dato es una fila, no una
// operación nueva.
pub const INFO_RAM_TOTAL: u64 = 0x01;
pub const INFO_RAM_LIBRE: u64 = 0x02;
pub const INFO_RAM_MARCOS: u64 = 0x03;
pub const INFO_RAM_MARCOS_LIBRES: u64 = 0x04;
pub const INFO_TSC_HZ: u64 = 0x05;
pub const INFO_CPU_HILOS: u64 = 0x06;
pub const INFO_CPU_NUCLEOS: u64 = 0x07;
pub const INFO_TAREAS_TOTAL: u64 = 0x08;
pub const INFO_TAREAS_LISTAS: u64 = 0x09;
pub const INFO_TAREAS_LIBRES: u64 = 0x0A;
pub const INFO_TICKS: u64 = 0x0B;
pub const INFO_KERNEL_BYTES: u64 = 0x0C;
pub const INFO_PROGRAMAS: u64 = 0x0D;
pub const INFO_PROGRAMAS_OLVIDADOS: u64 = 0x0E;
pub const INFO_DISCO_LISTO: u64 = 0x0F;
pub const INFO_DATOS_MONTADO: u64 = 0x10;
/// ── ESTRATOS ──────────────────────────────────────────────────────
///
/// El volumen de datos grande. Ring 3 los necesita para poder ENSENAR el estado
/// del almacen sin cruzar a Ring 0 por cada dato: son una fila mas de la tabla
/// de `OP_INFO`, que es como crece esta superficie sin tocar el ABI.
pub const INFO_ES_MONTADO: u64 = 0x11;
/// Generacion del superbloque: cuantas transacciones lleva el volumen.
pub const INFO_ES_GENERACION: u64 = 0x12;
pub const INFO_ES_BLOQUES: u64 = 0x13;
pub const INFO_ES_USADOS: u64 = 0x14;
pub const INFO_ES_BLOQUE_TAM: u64 = 0x15;
/// 0 holgado, 1 ambar, 2 rojo, 3 solo lectura. Ver `bmo_estratos::espacio`.
pub const INFO_ES_NIVEL: u64 = 0x16;
/// El gate del §5: 1 si el volumen nacio en ESTE disco.
pub const INFO_ES_IDENTIDAD: u64 = 0x17;
/// 1 si hoy se puede escribir. Hoy siempre 0: falta cablear la E/S.
pub const INFO_ES_ESCRIBIBLE: u64 = 0x18;
/// Bytes que Ring 3 ha PEDIDO con `KIND_MEMORIA` desde el arranque. Es lo
/// único del informe que sólo se mueve si alguien ejerció la capability.
pub const INFO_MEM_ENTREGADA: u64 = 0x19;

// Campos de `OP_INFO_TEXTO`.
pub const INFO_TXT_CPU_VENDOR: u64 = 0x01;
pub const INFO_TXT_CPU_NOMBRE: u64 = 0x02;
pub const INFO_TXT_UARCH: u64 = 0x03;
pub const INFO_TXT_FAMILIA: u64 = 0x04;

// Operaciones sobre un handle de directorio (`KIND_DIRECTORIO`).
pub const DIR_OP_SIGUIENTE: u32 = 0x01;
pub const DIR_OP_NOMBRE: u32 = 0x02;
/// Cierra el directorio y devuelve su ranura. Lo llama `Drop`, no tú.
pub const DIR_OP_CERRAR: u32 = 0x03;

// Operaciones sobre un handle de archivo (`KIND_ARCHIVO`).
pub const ARCH_OP_LEER: u32 = 0x01;
pub const ARCH_OP_ESCRIBIR: u32 = 0x02;
pub const ARCH_OP_TAMANO: u32 = 0x03;
pub const ARCH_OP_CERRAR: u32 = 0x04;

// Operaciones sobre un handle de consola (`KIND_CONSOLE`).
pub const CONSOLA_OP_LEER: u32 = 0x01;
pub const CONSOLA_OP_PERDIDOS: u32 = 0x02;
pub const CONSOLA_OP_ESCRIBIR: u32 = 0x03;
pub const CONSOLA_OP_HAY_HIJO: u32 = 0x04;

// Operaciones sobre un handle de pantalla (`KIND_FRAMEBUFFER`).
pub const FB_OP_BASE: u32 = 0x01;
pub const FB_OP_DIMS: u32 = 0x02;
pub const FB_OP_STRIDE: u32 = 0x03;
pub const FB_OP_BYTES: u32 = 0x04;

// Operaciones sobre un handle de entrada (`KIND_INPUT`).
pub const INPUT_OP_PUNTERO: u32 = 0x01;
pub const INPUT_OP_EVENTOS: u32 = 0x02;
pub const INPUT_OP_TECLA: u32 = 0x03;
pub const INPUT_OP_MODIFICADORES: u32 = 0x04;
pub const INPUT_OP_RUEDA: u32 = 0x05;

/// Bits de la máscara de modificadores.
pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_ALTGR: u8 = 1 << 3;
pub const MOD_CAPS: u8 = 1 << 4;

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

/// Un dato numérico del sistema. `0` si el kernel no sabe contestar ese campo.
///
/// Cuánta RAM hay, cuántos hilos tiene el CPU, cuántas ranuras de tarea quedan.
/// Esto vivía **sólo** en el shell de Ring 0 —`info`, `cpu`, `mem`— y no porque
/// hiciera falta el privilegio: porque los datos estaban a su alcance. Leer un
/// contador no ejerce ningún poder.
#[inline]
pub fn info(campo: u64) -> u64 {
    invoke(CURRENT_TASK, OP_INFO, campo, 0, 0).value
}

// ── El log del kernel, leído desde aquí ─────────────────────────────────
//
// ★ Esto NO es un salto a Ring 0, y la diferencia importa: no se ejecuta nada
// privilegiado, se piden bytes de texto. El kernel contesta y no cede nada,
// igual que con `info`. Ver `ring0/core/klog.rs`.

/// Cuántas líneas del log del kernel se pueden leer ahora mismo.
pub fn klog_lineas() -> u64 {
    invoke(CURRENT_TASK, OP_KLOG_INFO, 0, 0, 0).value
}

/// Cuántas ha escrito el kernel desde el arranque. La resta con
/// [`klog_lineas`] son las que se cayeron por el borde del anillo — y decirlo
/// es lo que separa "no pasó nada más" de "no cabía".
pub fn klog_total() -> u64 {
    invoke(CURRENT_TASK, OP_KLOG_INFO, 1, 0, 0).value
}

/// **Cierra una transacción vacía en ESTRATOS.** Devuelve la generación nueva,
/// o **0** si no se pudo.
///
/// ★ Es la primera llamada de todo el userland que **ESCRIBE EN EL DISCO**, y
/// lo hace de la forma más pequeña que existe: sin datos, apuntando al mismo
/// estrato, y sobre la copia del superbloque que no manda. Si sale mal, el
/// volumen es exactamente el de antes.
///
/// El motivo del fallo no vuelve por aquí — vuelve por CABINA y se lee con
/// **F11**. Es a propósito: caben más motivos en una línea de log que en un
/// código de retorno, y el que la llama ya tiene la ventana para leerlos.
pub fn estratos_sellar() -> u64 {
    invoke(CURRENT_TASK, OP_ESTRATOS_SELLAR, 0, 0, 0).value
}

/// Una línea del log en `dst`. **`n = 0` es la más reciente.** Devuelve cuántos
/// bytes se escribieron.
pub fn klog_texto(n: u64, dst: &mut [u8]) -> usize {
    let mut escritos = 0usize;
    let mut trozo = 0u64;
    while escritos < dst.len() {
        let w = invoke(CURRENT_TASK, OP_KLOG_TEXTO, n, trozo, 0).value;
        if w == 0 {
            break;
        }
        for k in 0..8 {
            let b = ((w >> (k * 8)) & 0xFF) as u8;
            if b == 0 || escritos >= dst.len() {
                return escritos;
            }
            dst[escritos] = b;
            escritos += 1;
        }
        trozo += 1;
    }
    escritos
}

/// **Un bloque de memoria pedido al kernel.**
///
/// ★ Esto NO es un `malloc` y no lo pretende. Es memoria entregada entera:
/// pides una vez, te dan un bloque contiguo, y **no hay forma de devolverlo**
/// — vive hasta que el proceso muere.
///
/// El asignador se escribe ENCIMA, aquí en Ring 3, con la política que quiera
/// cada uno. Ésa es la razón de que el kernel no traiga uno: un `malloc`
/// general dentro del kernel sería escribir una política que el programa de al
/// lado no usa, y encima cobrársela con un syscall por llamada.
///
/// El caso que lo decidió: DOOM pide ~8 MiB una vez al arrancar y se los
/// administra él con su `Z_Zone`. Para eso, esto es exactamente lo que hace
/// falta y ni un byte más.
pub struct Memoria {
    cap: u64,
    base: u64,
    bytes: u64,
}

impl Memoria {
    /// Pide `bytes`. `None` si no hay RAM contigua, si pasa del tope por
    /// petición (64 MiB) o si este proceso ya gastó sus cuatro peticiones.
    pub fn pedir(bytes: u64) -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_MEMORIA_PEDIR, bytes, 0, 0).valor()?;
        let base = invoke(cap, MEM_OP_BASE, 0, 0, 0).valor()?;
        Some(Self { cap, base, bytes })
    }

    /// La dirección del primer byte.
    ///
    /// Está MAPEADO: a partir de aquí se escribe con `mov` y el kernel no se
    /// entera de nada. Un syscall por byte sería justo lo contrario de
    /// entregar memoria.
    pub fn base(&self) -> *mut u8 {
        self.base as *mut u8
    }

    /// Lo que se pidió.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Lo que el kernel dice que lleva entregado a este proceso — que puede ser
    /// más que `bytes()` si se pidió varias veces, y siempre está redondeado a
    /// páginas enteras.
    pub fn entregado(&self) -> u64 {
        invoke(self.cap, MEM_OP_BYTES, 0, 0, 0).value
    }
}

/// Un campo de TEXTO en `dst`. Devuelve cuántos bytes se escribieron.
///
/// Viaja de 8 en 8 con el cero como final, igual que la ruta de `ejecutar`: en
/// esta superficie no hay punteros de Ring 3 hacia el kernel.
pub fn info_texto(campo: u64, dst: &mut [u8]) -> usize {
    let mut n = 0usize;
    let mut trozo = 0u64;
    while n < dst.len() {
        let w = invoke(CURRENT_TASK, OP_INFO_TEXTO, campo, trozo, 0).value;
        if w == 0 {
            break;
        }
        for k in 0..8 {
            let b = ((w >> (k * 8)) & 0xFF) as u8;
            if b == 0 || n >= dst.len() {
                return n;
            }
            dst[n] = b;
            n += 1;
        }
        trozo += 1;
    }
    n
}

/// Reiniciar la máquina. No vuelve.
///
/// Reiniciar es tocar puertos de E/S (`0xCF9`, el 8042) y Ring 3 no puede
/// hacerlo: se le pide al kernel, que ya tenía el reinicio de tres pasos para
/// su propio shell. Si volviera —no debería— se cede el turno en vez de seguir
/// como si nada, por la misma razón que en [`salir`].
pub fn reiniciar() -> ! {
    invoke(CURRENT_TASK, OP_REINICIAR, 0, 0, 0);
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
/// framebuffer **es memoria de este proceso**. `lienzo` es un puntero de verdad
/// y escribir en él es un `mov`. Ése es el trato entero de `KIND_FRAMEBUFFER`
/// — el kernel contesta cuatro preguntas al arrancar y después se aparta.
///
/// ═══ ★ EL DOBLE BÚFER ═══
///
/// Se dibuja en **`lienzo`** y se vuelca a **`panel`**. Sin doble búfer los dos
/// punteros son el mismo y todo funciona como siempre; con él, `lienzo` es RAM
/// normal pedida con [`Memoria`] y `panel` es la memoria de vídeo.
///
/// **Por qué**, en orden de lo que más dolía:
///
/// 1. **Mata el ghosting por construcción.** El framebuffer está en
///    write-combining, y leer memoria WC no devuelve lo que acabas de escribir
///    (Ep. 25 de `BITACORA.md`). El *save-under* del cursor es una lectura, y
///    era la única del programa. Leyendo del lienzo el problema **no existe**:
///    es RAM normal, cacheada y coherente consigo misma. Un `sfence` bien puesto
///    lo arregla; esto lo hace imposible.
/// 2. **Mata el tearing.** El escáner de vídeo ya no ve un fotograma a medio
///    pintar: ve el anterior hasta que llega el volcado.
/// 3. **Pintar es más rápido.** Escribir en RAM cacheada no se parece a escribir
///    en memoria de vídeo, ni con WC. El coste se paga una vez, en el volcado, y
///    en la forma que al bus le gusta: ráfagas seguidas.
/// 4. **Es el prerequisito de las superficies.** El día que una ventana sea de
///    otro proceso, lo que ese proceso pinta va a un búfer y alguien lo compone.
///    Esto es esa pieza, con un solo cliente todavía.
///
/// **Y sólo es posible desde que existe `KIND_MEMORIA`**: hasta entonces un
/// proceso recibía su imagen y 64 KiB de pila, y un búfer de pantalla son ~8 MB.
///
/// ═══ Lo sucio, y por qué una caja y no la pantalla entera ═══
///
/// Volcar 8 MB por fotograma contradiría la regla que ya estaba escrita aquí:
/// *lo que se repinta en un bucle es el DAÑO, no la pantalla*. Así que se lleva
/// la **caja envolvente** de todo lo escrito desde el último volcado y sólo se
/// copia eso.
///
/// El precio, dicho: una caja **no** son varias regiones. Tocar la esquina de
/// arriba y la de abajo da una caja que las contiene a las dos, o sea casi todo.
/// Es el caso peor y sigue siendo mucho mejor que volcar siempre entero; si
/// algún día se nota, lo que toca es una lista corta de rectángulos, no
/// abandonar la caja.
pub struct Pantalla {
    /// Handle de la capability. Hace falta para preguntarle cosas.
    pub cap: u64,
    /// **Donde se DIBUJA.** Con doble búfer es RAM normal; sin él, el panel.
    pub lienzo: *mut u32,
    /// **El framebuffer de verdad.** Igual que `lienzo` si no hay doble búfer.
    pub panel: *mut u32,
    pub ancho: u32,
    pub alto: u32,
    /// En PÍXELES, no en bytes: es el mismo número que usa el kernel.
    pub stride: u32,
    pub formato: u32,
    pub bytes: u64,
    /// Caja envolvente de lo escrito desde el último volcado, `(x0,y0,x1,y1)`
    /// con `x1`/`y1` exclusivos. Vacía cuando `x0 >= x1`.
    ///
    /// Es una `Cell` porque dibujar toma `&self` en todo el compositor y
    /// cambiarlo a `&mut self` obligaría a reescribir cada llamada para ganar
    /// nada: esto es un programa de un solo hilo y `Cell` es exactamente la
    /// herramienta para eso.
    sucio: core::cell::Cell<(u32, u32, u32, u32)>,
    /// Lo que ha costado mover píxeles. Ver [`Volcado`]: es el número que
    /// decide si una GPU compra algo o sólo cuesta un año.
    volcado: core::cell::Cell<Volcado>,
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
            lienzo: base as *mut u32,
            panel: base as *mut u32,
            ancho: (dims >> 32) as u32,
            alto: dims as u32,
            stride: (stride >> 32) as u32,
            formato: stride as u32,
            bytes,
            sucio: core::cell::Cell::new(VACIO),
            volcado: core::cell::Cell::new(Volcado {
                fotogramas: 0,
                bytes: 0,
                peor: 0,
                modo: Volcador::Ninguno,
            }),
        })
    }

    /// **Pide el búfer de fondo y empieza a dibujar en él.**
    ///
    /// Devuelve `false` si no lo consigue, y entonces **no pasa nada**: se
    /// sigue dibujando directamente en el panel, que es lo que se hacía antes.
    /// Eso no es un adorno defensivo — el bloque son ~8 MB de RAM **contigua en
    /// físico**, y si la memoria está fragmentada el kernel lo rechaza con su
    /// motivo. Un compositor que se cayera por no conseguir una optimización
    /// sería peor que uno sin la optimización.
    ///
    /// Quien llama decide si lo dice por la consola. Aquí no se decide eso.
    pub fn activar_doble_bufer(&mut self) -> bool {
        if self.lienzo != self.panel {
            return true; // ya está
        }
        // El lienzo tiene el MISMO stride que el panel, no el mismo ancho: así
        // el índice `y*stride + x` vale para los dos y no hay dos aritméticas
        // que mantener en paralelo. Que sobren unos píxeles por fila es más
        // barato que una segunda forma de calcular la misma dirección.
        let bytes = (self.stride as u64) * (self.alto as u64) * 4;
        let Some(m) = Memoria::pedir(bytes) else {
            return false;
        };
        self.lienzo = m.base() as *mut u32;
        // Lo que hay en el panel ahora mismo no está en el lienzo: hasta el
        // primer volcado completo, los dos no dicen lo mismo. Se marca la
        // pantalla entera para que el primer `vaciar` los iguale.
        self.marcar(0, 0, self.ancho, self.alto);
        true
    }

    /// ¿Se está dibujando fuera de la pantalla de vídeo?
    pub fn tiene_doble_bufer(&self) -> bool {
        self.lienzo != self.panel
    }

    /// Píxeles que caben en el área mapeada.
    #[inline]
    pub fn pixeles(&self) -> usize {
        (self.bytes / 4) as usize
    }

    /// **Apunta que esta región ha cambiado.** Sin esto, lo pintado se queda en
    /// el lienzo y no llega nunca al panel.
    ///
    /// Las primitivas de dibujo de aquí lo hacen solas. Es público porque
    /// [`Self::punto_sin_comprobar`] no marca —es el camino caliente y no va a
    /// llevar esto dentro—, así que quien la use tiene que marcar él.
    #[inline]
    pub fn marcar(&self, x: u32, y: u32, ancho: u32, alto: u32) {
        if ancho == 0 || alto == 0 {
            return;
        }
        let (x0, y0, x1, y1) = self.sucio.get();
        let nx1 = (x + ancho).min(self.ancho);
        let ny1 = (y + alto).min(self.alto);
        if x >= nx1 || y >= ny1 {
            return;
        }
        self.sucio.set(if x0 >= x1 {
            (x, y, nx1, ny1)
        } else {
            (x0.min(x), y0.min(y), x1.max(nx1), y1.max(ny1))
        });
    }

    /// Un píxel, sin comprobar nada. Es el camino caliente de un compositor y
    /// no va a llevar una rama dentro.
    ///
    /// # Safety
    /// `x < stride`, `y < alto`, y **quien llame tiene que marcar la región**
    /// con [`Self::marcar`] o lo pintado no llegará al panel. Las primitivas de
    /// aquí lo hacen; de fuera no lo llama nadie.
    #[inline(always)]
    pub unsafe fn punto_sin_comprobar(&self, x: u32, y: u32, color: u32) {
        self.lienzo
            .add((y as usize) * (self.stride as usize) + x as usize)
            .write_volatile(color);
    }

    /// Un píxel, comprobando. Fuera de la pantalla no hace nada.
    #[inline]
    pub fn punto(&self, x: u32, y: u32, color: u32) {
        if x < self.ancho && y < self.alto {
            unsafe { self.punto_sin_comprobar(x, y, color) };
            self.marcar(x, y, 1, 1);
        }
    }

    /// Rellenar la pantalla entera.
    ///
    /// Sólo para el primer pintado. Repetirlo por fotograma sería recorrer
    /// varios MB de memoria sin caché: un pase de diapositivas. Lo que se
    /// repinta en un bucle es el DAÑO, no la pantalla.
    pub fn limpiar(&self, color: u32) {
        // ★ El tope es el MENOR de los dos, y esto no es prudencia de más.
        //
        // `pixeles()` mide el área que mapeó el KERNEL, que puede ser más
        // grande que `stride × alto` (redondeos, padding del firmware). El
        // lienzo mide exactamente `stride × alto`. Recorrer el primero
        // escribiendo en el segundo se sale del bloque de `KIND_MEMORIA` y pisa
        // lo que haya detrás — y como el bloque se pidió contiguo y el
        // asignador da lo siguiente que encuentre, "lo que haya detrás" es
        // memoria de alguien.
        //
        // El mínimo protege en los dos sentidos, que es la razón de que sea un
        // mínimo y no un caso especial.
        let n = self.pixeles().min((self.stride as usize) * (self.alto as usize));
        for i in 0..n {
            unsafe { self.lienzo.add(i).write_volatile(color) };
        }
        self.marcar(0, 0, self.ancho, self.alto);
    }

    /// **Empuja a la pantalla lo que se acaba de pintar.**
    ///
    /// ★ Esto es la otra mitad del write-combining, y sin ella el WC no es una
    /// optimización: es un bug.
    ///
    /// Con memoria WC el CPU **acumula** las escrituras en un búfer y las suelta
    /// cuando se llena o cuando algo le obliga. Eso es lo que hace que pintar
    /// sea rápido — y también lo que hace que lo pintado **no llegue** al panel
    /// si el fotograma acaba con el búfer a medias. El escáner de vídeo lee la
    /// memoria, no el búfer.
    ///
    /// El síntoma, dicho por quien lo sufrió: *"cuando muevo el ratón tengo que
    /// apuntar bien para que me pinte las escrituras"*. No era el ratón: era que
    /// mover el ratón genera más escrituras, el búfer se llenaba, y al vaciarse
    /// aparecía de golpe el texto que se había tecleado antes.
    ///
    /// `sfence` ordena: nada de lo de después se ve antes que lo de antes. Es
    /// una instrucción, se hace **una vez por fotograma**, y convierte el WC en
    /// lo que promete.
    ///
    /// ★ Con doble búfer esto **además vuelca**: primero la copia del lienzo al
    /// panel, después la barrera. Ese orden es el único que sirve — la barrera
    /// tiene que cerrar las escrituras del volcado, no las de antes.
    #[inline]
    pub fn vaciar(&self) {
        self.volcar();
        unsafe { core::arch::asm!("sfence", options(nostack, preserves_flags)) };
    }

    /// **Copia al panel lo sucio del lienzo**, y deja la caja vacía.
    ///
    /// Sin doble búfer no hay nada que copiar: lo pintado ya está en el panel.
    /// Igual se limpia la caja, porque llevarla puesta sin volcar sería mentir
    /// sobre lo que queda pendiente.
    pub fn volcar(&self) {
        let (x0, y0, x1, y1) = self.sucio.replace(VACIO);
        if self.lienzo == self.panel || x0 >= x1 || y0 >= y1 {
            return;
        }
        let stride = self.stride as usize;
        let ancho = (x1 - x0) as usize;
        let mut fila = y0 as usize;
        while fila < y1 as usize {
            let off = fila * stride + x0 as usize;
            // ★ De DOS píxeles por escritura cuando se puede.
            //
            // El panel es write-combining: el CPU junta escrituras seguidas en
            // ráfagas de 64 bytes, y le cuesta menos juntar ocho de 8 bytes que
            // dieciséis de 4. Es la misma cantidad de datos con la mitad de
            // operaciones, y no necesita SSE ni alineación especial más allá de
            // la que ya tiene un framebuffer (base de página, píxeles de 4 B).
            //
            // El píxel suelto del final se copia como siempre. Un bucle que
            // "casi" cubre el ancho es peor que uno que lo cubre.
            let mut i = 0usize;
            unsafe {
                while i + 1 < ancho {
                    let a = self.lienzo.add(off + i).read() as u64;
                    let b = self.lienzo.add(off + i + 1).read() as u64;
                    (self.panel.add(off + i) as *mut u64).write_volatile(a | (b << 32));
                    i += 2;
                }
                while i < ancho {
                    let v = self.lienzo.add(off + i).read();
                    self.panel.add(off + i).write_volatile(v);
                    i += 1;
                }
            }
            fila += 1;
        }

        // La cuenta, para poder contestar "¿hace falta una GPU?" con un número
        // en vez de con una intuición.
        let bytes = (ancho as u64) * ((y1 - y0) as u64) * 4;
        let v = self.volcado.get();
        self.volcado.set(Volcado {
            fotogramas: v.fotogramas + 1,
            bytes: v.bytes + bytes,
            peor: v.peor.max(bytes),
            modo: v.modo,
        });
    }

    /// Lo que lleva costado el volcado. Ver [`Volcado`] y [`Volcador`].
    pub fn volcado(&self) -> Volcado {
        let mut v = self.volcado.get();
        v.modo = if self.lienzo == self.panel { Volcador::Ninguno } else { Volcador::Directo };
        v
    }

    /// **Asegura que lo escrito se puede LEER.** Llamar antes de [`Self::leer`].
    ///
    /// ★ Existe por el Ep. 25, y hace dos cosas distintas según dónde se dibuje:
    ///
    /// - **Con doble búfer**: nada. El lienzo es RAM normal y cacheada, así que
    ///   una lectura ve lo que se acaba de escribir. El problema no existe.
    /// - **Sin doble búfer**: `sfence`. Se está leyendo memoria WC, y una
    ///   lectura de WC **no está ordenada** contra las escrituras pendientes en
    ///   el búfer: sin barrera devuelve la pantalla de hace un fotograma.
    ///
    /// Que sea un no-op en el camino bueno es justo la gracia: el doble búfer no
    /// arregla el ghosting, lo hace **imposible**.
    #[inline]
    pub fn sincronizar_lectura(&self) {
        if self.lienzo == self.panel {
            unsafe { core::arch::asm!("sfence", options(nostack, preserves_flags)) };
        }
    }

    /// Qué hay AHORA en un píxel. Fuera de la pantalla, negro.
    ///
    /// ★ Se lee del LIENZO, que es donde se ha dibujado. Eso es lo que permite
    /// dibujar el cursor del ratón **encima de cualquier cosa**: se guarda lo
    /// que había debajo y se devuelve al moverlo. Sin esto hay que preguntarle a
    /// un modelo de la escena qué debería haber, y ese modelo se queda corto en
    /// cuanto aparece una ventana que no conoce: el cursor deja agujeros con el
    /// color del fondo por donde pasa.
    ///
    /// Sin doble búfer esto lee memoria de vídeo, que es cara y además exige
    /// [`Self::sincronizar_lectura`] antes. Con doble búfer es RAM normal.
    #[inline]
    pub fn leer(&self, x: u32, y: u32) -> u32 {
        if x >= self.ancho || y >= self.alto {
            return 0;
        }
        unsafe {
            self.lienzo
                .add((y as usize) * (self.stride as usize) + x as usize)
                .read_volatile()
        }
    }

    /// Un rectángulo, recortado a la pantalla. Es la única primitiva de dibujo
    /// que hay, y con ella se hace un escritorio entero: fondo, barra,
    /// ventanas, bordes. Lo demás son estas mismas llamadas puestas en orden.
    pub fn rect(&self, x: u32, y: u32, ancho: u32, alto: u32, color: u32) {
        let x1 = (x.saturating_add(ancho)).min(self.ancho);
        let y1 = (y.saturating_add(alto)).min(self.alto);
        // Se marca UNA vez, con las medidas ya recortadas, en vez de un píxel
        // por vuelta: un rectángulo de pantalla completa son millones de
        // llamadas a `marcar` que darían exactamente la misma caja.
        if x < x1 && y < y1 {
            self.marcar(x, y, x1 - x, y1 - y);
        }
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

/// La caja vacía: `x0 >= x1`, así que no hay nada que volcar.
const VACIO: (u32, u32, u32, u32) = (u32::MAX, u32::MAX, 0, 0);

/// **Cómo llegan los píxeles del lienzo al panel.**
///
/// ═══ ★ Ésta es la costura donde entra una GPU ═══
///
/// La idea que la motivó era partir el compositor en `gui_CPU.bex` y
/// `gui_GPU.bex`. Eso sería **bifurcar antes de que exista la segunda
/// implementación**: cada arreglo habría que hacerlo dos veces, que es
/// exactamente el problema que resolvió `refactor(abi): la disposición de
/// agregados estaba escrita TRES veces`.
///
/// El corte correcto es por CAPA, y hay tres:
///
/// ```text
///   POLITICA     que ventana existe, donde va, quien tiene el foco
///                -> no cambia NUNCA con una GPU. Vive en el compositor.
///   DIBUJO       llenar el lienzo: punto, rect, texto
///                -> es CPU SIEMPRE. Una app que pinta su superficie en RAM
///                   la pinta con el CPU, tenga GPU o no.
///   VOLCADO      mover el rectangulo sucio del lienzo al panel
///                -> AQUI, y solo aqui, una GPU cambia algo.
/// ```
///
/// Por eso el contrato es esto y no un trait `Lienzo` entero: `punto` está en
/// el camino caliente y meterle una llamada indirecta costaría en cada píxel
/// para no ganar nada. `volcar` se llama **una vez por fotograma**, así que
/// aquí una rama no se nota — y es donde está el coste de verdad.
///
/// ═══ Y antes de comprar una tarjeta, MEDIR ═══
///
/// La caja de sucio ya evita casi todo el trabajo: escribir una letra vuelca
/// unos pocos KiB, no la pantalla. Ver [`Pantalla::volcado`] — el número va
/// primero, la tarjeta después.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Volcador {
    /// No hay doble búfer: lo pintado ya está en el panel. No hay nada que
    /// mover, y decirlo con su nombre vale más que un `if` suelto.
    Ninguno,
    /// Copia con escrituras normales, de 8 en 8 bytes cuando se puede.
    ///
    /// Es lo que hay hoy y lo que corre en el Ryzen. Dos píxeles por
    /// escritura: el framebuffer es write-combining y agrupa mejor cuantas
    /// menos escrituras sueltas reciba.
    Directo,
}

/// Lo que ha costado el volcado, para poder **perfilar antes de comprar nada**.
///
/// No es telemetría de adorno: la pregunta "¿hace falta una GPU?" sólo se puede
/// contestar con estos dos números. Si los bytes por fotograma son pocos, una
/// tarjeta no compra nada y cuesta un año de trabajo.
#[derive(Clone, Copy)]
pub struct Volcado {
    /// Fotogramas con algo que volcar. Los que no cambian nada no cuentan:
    /// promediar con ellos escondería el caso caro.
    pub fotogramas: u64,
    /// Bytes movidos del lienzo al panel, en total.
    pub bytes: u64,
    /// El fotograma más caro visto. **El peor caso importa más que la media**:
    /// un tirón se nota, y una media buena lo esconde.
    pub peor: u64,
    pub modo: Volcador,
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

    /// Un carácter AMPLIADO por un entero: cada píxel del glifo pasa a ser un
    /// cuadrado de `escala`.
    ///
    /// Entero y con `rect`, no interpolado: ampliar por 4 un glifo de 8x16 da
    /// bloques limpios de 32x64, y esa estética es la que tiene esta máquina.
    /// Una interpolación pediría coma flotante, un buffer intermedio y un gusto
    /// que no es el de aquí — y con la misma fuente que ya está cargada.
    pub fn glifo_escala(&self, x: u32, y: u32, c: u8, color: u32, escala: u32) {
        if escala <= 1 {
            self.glifo(x, y, c, color);
            return;
        }
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
                    self.rect(x + col * escala, y + fila as u32 * escala, escala, escala, color);
                }
            }
        }
    }

    /// Un `&str` ampliado. Devuelve la x donde acabó.
    pub fn texto_escala(&self, x: u32, y: u32, s: &str, color: u32, escala: u32) -> u32 {
        let mut cx = x;
        for &c in s.as_bytes() {
            self.glifo_escala(cx, y, c, color, escala);
            cx += GLIFO_ANCHO * escala;
        }
        cx
    }

    /// Lo que ocupa un texto ampliado, para poder centrarlo sin adivinar.
    pub fn ancho_escala(s: &str, escala: u32) -> u32 {
        s.len() as u32 * GLIFO_ANCHO * escala
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
    ejecutar_en(ruta, 0)
}

/// Igual, pero la salida del hijo va a `consola` en vez de al panel del kernel.
///
/// Esto es lo que separa un lanzador de un TERMINAL: sin la consola el programa
/// se ejecuta y su salida cae donde no la ves; con ella, aterriza en un anillo
/// que tú drenas. `consola = 0` es el comportamiento de siempre.
pub fn ejecutar_en(ruta: &[u8], consola: u64) -> Result<u64, u32> {
    for trozo in ruta.chunks(8) {
        let mut w = [0u8; 8];
        w[..trozo.len()].copy_from_slice(trozo);
        invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
    }
    let st = invoke(CURRENT_TASK, OP_EJECUTAR, 0, consola, 0);
    if st.ok() {
        Ok(st.value)
    } else {
        Err(st.code)
    }
}

// ── La consola ──────────────────────────────────────────────────────────

/// La salida de los programas que este proceso lance.
///
/// Cierra la última asimetría del sistema: la pantalla y la entrada ya eran
/// capabilities, y la consola era un global fijo — por eso un terminal no podía
/// leer lo que imprimía su propio hijo. Ahora la crea quien va a leerla.
pub struct Consola {
    pub cap: u64,
}

impl Consola {
    pub fn crear() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_CONSOLA_CREAR, 0, 0, 0).valor()?;
        Some(Self { cap })
    }

    /// Hasta **7** bytes de salida. Devuelve cuántos son válidos; `0` = no hay
    /// nada. **No bloquea**, igual que `tecla()` y por la misma razón: un
    /// terminal tiene bucle de fotograma y dormirse aquí congelaría el cursor.
    ///
    /// Siete y no ocho porque el octavo lleva el contador — ver
    /// `CONSOLA_OP_LEER` en el kernel.
    pub fn leer(&self, dst: &mut [u8; 8]) -> usize {
        let v = invoke(self.cap, CONSOLA_OP_LEER, 0, 0, 0).value;
        *dst = v.to_le_bytes();
        let n = (v >> 56) as usize;
        dst[7] = 0;
        n.min(7)
    }

    /// Mete texto en la ENTRADA de esta consola: lo que el terminal teclea
    /// PARA su hijo. El otro sentido del canal.
    pub fn escribir(&self, texto: &[u8]) {
        for trozo in texto.chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            invoke(self.cap, CONSOLA_OP_ESCRIBIR, u64::from_le_bytes(w), 0, 0);
        }
    }

    /// ¿Hay un proceso vivo escribiendo aquí? El terminal lo pregunta para
    /// saber si lo que se teclea es un comando suyo o entrada para el hijo.
    pub fn hay_hijo(&self) -> bool {
        invoke(self.cap, CONSOLA_OP_HAY_HIJO, 0, 0, 0).value != 0
    }

    /// Bytes descartados por anillo lleno. Un terminal que va lento tiene
    /// derecho a saber que está perdiendo salida en vez de creerse completo.
    pub fn perdidos(&self) -> u64 {
        invoke(self.cap, CONSOLA_OP_PERDIDOS, 0, 0, 0).value
    }
}

/// Los códigos que devuelve `ejecutar`. Son pocos a propósito: un proceso no
/// necesita saber de FAT32, pero sí distinguir las tres cosas que le hacen
/// cambiar de conducta.
/// Los motivos por los que no se pudo abrir o crear un archivo.
///
/// Son varios y no uno porque cada uno manda a hacer algo DISTINTO: crear la
/// carpeta, acortar el nombre, o mirar si te equivocaste de sitio. Un unico
/// "no se pudo" manda a buscar donde no es.
pub const ERROR_ARCH_SIN_HUECO: u32 = 27;
pub const ERROR_ARCH_NO_ESTA: u32 = 28;
pub const ERROR_ARCH_GRANDE: u32 = 29;
pub const ERROR_ARCH_NOMBRE: u32 = 30;
pub const ERROR_ARCH_SOLO_LECTURA: u32 = 31;
pub const ERROR_ARCH_CARPETA: u32 = 32;
pub const ERROR_ARCH_ES_CARPETA: u32 = 33;

pub const ERROR_NO_ESTA: u32 = 20;
pub const ERROR_GATE: u32 = 21;
pub const ERROR_OCUPADO: u32 = 22;
pub const ERROR_NO_ADMITIDO: u32 = 23;

/// Lee de la consola de ESTE proceso — lo que el terminal le manda.
///
/// Hasta 7 bytes; `0` = no hay nada. **No bloquea**: un programa que espera
/// entrada cede el turno entre intentos en vez de quedarse con el CPU.
///
/// Es la pareja de `consola()`: se escribe por una y se escucha por la otra,
/// sobre el mismo objeto. Es lo que permite un `ACCEPT` en un proceso que no
/// tiene —ni debe tener— la capability del teclado.
pub fn leer_consola(dst: &mut [u8; 8]) -> usize {
    let v = invoke(CURRENT_TASK, OP_CONSOLE_READ, 0, 0, 0).value;
    *dst = v.to_le_bytes();
    let n = (v >> 56) as usize;
    dst[7] = 0;
    n.min(7)
}

// ── El disco ────────────────────────────────────────────────────────────

/// Una entrada de directorio, ya leída.
///
/// `EntradaDir` y no `Entrada` porque `Entrada` ya es el ratón+teclado. Dos
/// cosas distintas no pueden llamarse igual aunque en castellano suenen igual.
pub struct EntradaDir {
    /// Nombre 8.3 CRUDO, con sus espacios de relleno: `"COBOL   BEX"`.
    /// Convertirlo a `COBOL.BEX` es presentación, y eso es cosa de quien pinta.
    pub nombre: [u8; 11],
    pub es_dir: bool,
    pub bytes: u32,
}

impl EntradaDir {
    /// El nombre en forma legible, `cobol.bex`, escrito en `dst`. Devuelve
    /// cuántos bytes ocupó.
    ///
    /// ★ En MINÚSCULA. FAT32 los guarda en mayúscula porque el formato es de
    /// 1980 y no distinguía; eso es un detalle del disco, no del nombre. Un
    /// listado a gritos se lee peor, y el kernel acepta las dos formas al
    /// abrir — así que no se pierde nada bajándolos aquí, que es donde se
    /// decide cómo se ve.
    pub fn legible(&self, dst: &mut [u8; 12]) -> usize {
        let baja = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
        let mut n = 0;
        for i in 0..8 {
            if self.nombre[i] == b' ' { break; }
            dst[n] = baja(self.nombre[i]);
            n += 1;
        }
        if self.nombre[8] != b' ' {
            dst[n] = b'.';
            n += 1;
            for i in 8..11 {
                if self.nombre[i] == b' ' { break; }
                dst[n] = baja(self.nombre[i]);
                n += 1;
            }
        }
        n
    }
}

/// Un directorio abierto.
///
/// ★ Esto NO es "una ruta que cualquiera puede escribir". Es un handle que te
/// concedieron: lo que no te hayan dado no existe para este proceso. Es la
/// misma disciplina que la pantalla y la entrada — un nombre es adivinable, un
/// permiso no.
pub struct Directorio {
    pub cap: u64,
}

impl Directorio {
    /// Abre un directorio del volumen de datos. Ruta vacía = la raíz.
    pub fn abrir(ruta: &[u8]) -> Result<Self, u32> {
        for trozo in ruta.chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
        }
        let st = invoke(CURRENT_TASK, OP_DIR_ABRIR, 0, 0, 0);
        if st.ok() { Ok(Self { cap: st.value }) } else { Err(st.code) }
    }

    /// La siguiente entrada, o `None` cuando se acaba.
    pub fn siguiente(&self) -> Option<EntradaDir> {
        let v = invoke(self.cap, DIR_OP_SIGUIENTE, 0, 0, 0).value;
        if v >> 63 == 0 {
            return None;
        }
        let es_dir = (v >> 62) & 1 != 0;
        let bytes = v as u32;
        // El nombre viene aparte: son 11 bytes y no caben con lo demás.
        let mut nombre = [b' '; 11];
        let mut puesto = 0usize;
        for desde in [0u64, 7] {
            let w = invoke(self.cap, DIR_OP_NOMBRE, desde, 0, 0).value;
            let n = (w >> 56) as usize;
            let b = w.to_le_bytes();
            for k in 0..n.min(7) {
                if puesto < 11 {
                    nombre[puesto] = b[k];
                    puesto += 1;
                }
            }
        }
        Some(EntradaDir { nombre, es_dir, bytes })
    }
}

/// **Cerrar es del `Drop`, no de quien llama.**
///
/// ═══ El bug que esto arregla ═══
///
/// No había forma de cerrar un directorio, y la tabla del kernel son OCHO
/// ranuras que sólo se liberaban al **morir el proceso**. El cliente es el
/// compositor, que **no muere nunca** — es el escritorio.
///
/// Así que cada `ls` se quedaba una ranura para siempre y al noveno la tabla
/// estaba llena: `ls` empezaba a contestar *"no puedo abrir esa carpeta"* y ya
/// no se recuperaba hasta reiniciar. Un fallo que aparece **después de un rato
/// de uso normal** y no se puede reproducir recién arrancado, que es de los
/// peores de encontrar.
///
/// Y va en `Drop` y no en un método `cerrar()` a propósito: un cierre que hay
/// que acordarse de llamar es un cierre que un día no se llama. Aquí el
/// compositor no cambia ni una línea — el `Directorio` sale de ámbito al
/// terminar el `ls` y la ranura vuelve sola.
impl Drop for Directorio {
    fn drop(&mut self) {
        invoke(self.cap, DIR_OP_CERRAR, 0, 0, 0);
    }
}

// ── Un archivo ──────────────────────────────────────────────────────────

/// Un archivo abierto del volumen de datos.
///
/// Hermano de [`Directorio`]: aquel deja PREGUNTAR qué hay, éste deja mover
/// los bytes de dentro. Y la misma disciplina — no es una ruta que cualquiera
/// escriba, es un handle que te concedieron.
///
/// El MODO se fija al abrir: [`Archivo::leer_de`] da uno de lectura y
/// [`Archivo::crear`] uno de escritura. Pedirle bytes a uno de escritura no
/// devuelve un error de permisos: devuelve que esa pregunta no existe para ese
/// objeto.
///
/// **Límite de hoy**: 4 KiB por archivo. Los bytes cruzan de 7 en 7 (la
/// superficie congelada no acepta punteros) y hace falta un buffer en el
/// kernel donde juntarlos. Ver `ring0/archivo.rs`; lo que lo quitará es un
/// escritor por sectores en `bmo_fat32`, que es otra pieza.
pub struct Archivo {
    pub cap: u64,
    escribe: bool,
}

impl Archivo {
    fn con_ruta(ruta: &[u8], op: u32, escribe: bool) -> Result<Self, u32> {
        // El mismo renglón que usan `ejecutar` y `Directorio::abrir`. No hay
        // un segundo mecanismo para lo mismo.
        for trozo in ruta.chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
        }
        let st = invoke(CURRENT_TASK, op, 0, 0, 0);
        if st.ok() { Ok(Self { cap: st.value, escribe }) } else { Err(st.code) }
    }

    /// Abre un archivo para LEER. Se trae entero al abrir, así que a partir de
    /// aquí una lectura no puede fallar a mitad por un error de disco.
    pub fn leer_de(ruta: &[u8]) -> Result<Self, u32> {
        Self::con_ruta(ruta, OP_ARCHIVO_ABRIR, false)
    }

    /// Abre un archivo para ESCRIBIR. Acepta subdirectorios
    /// (`datos/movim.dat`); el nombre tiene que ser un 8.3 válido.
    ///
    /// **Nada llega al disco hasta [`Archivo::cerrar`]**. Un proceso que muere
    /// a medias no deja un archivo a medias: no deja nada.
    pub fn crear(ruta: &[u8]) -> Result<Self, u32> {
        Self::con_ruta(ruta, OP_ARCHIVO_CREAR, true)
    }

    /// Llena `dst` con lo que quede. Devuelve cuántos bytes se leyeron; `0` =
    /// se acabó el archivo.
    pub fn leer(&self, dst: &mut [u8]) -> usize {
        if self.escribe {
            return 0;
        }
        let mut puestos = 0usize;
        while puestos < dst.len() {
            let v = invoke(self.cap, ARCH_OP_LEER, 0, 0, 0).value;
            let n = (v >> 56) as usize;
            if n == 0 {
                break;
            }
            let b = v.to_le_bytes();
            for k in 0..n.min(7) {
                if puestos < dst.len() {
                    dst[puestos] = b[k];
                    puestos += 1;
                }
            }
        }
        puestos
    }

    /// Añade bytes. Devuelve cuántos se aceptaron — menos de los pedidos
    /// significa que se llenó, y entonces `cerrar` devolverá `false`.
    ///
    /// Los bytes viajan de 7 en 7 con su cuenta en el byte alto, no cortando
    /// en el primer cero: un archivo no es texto y un `\0` en medio es un dato
    /// como cualquier otro.
    pub fn escribir(&self, datos: &[u8]) -> usize {
        if !self.escribe {
            return 0;
        }
        let mut puestos = 0usize;
        for trozo in datos.chunks(7) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            w[7] = trozo.len() as u8;
            let n = invoke(self.cap, ARCH_OP_ESCRIBIR, u64::from_le_bytes(w), 0, 0).value;
            puestos += n as usize;
            if (n as usize) < trozo.len() {
                break;
            }
        }
        puestos
    }

    /// Bytes que quedan por leer, o bytes acumulados si es de escritura.
    pub fn tamano(&self) -> u64 {
        invoke(self.cap, ARCH_OP_TAMANO, 0, 0, 0).value
    }

    /// Cierra. En uno de escritura es **donde el contenido llega al disco**:
    /// `false` significa que no se guardó nada, no que se guardara a medias.
    pub fn cerrar(self) -> bool {
        invoke(self.cap, ARCH_OP_CERRAR, 0, 0, 0).value != 0
    }
}

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

    /// Las vueltas de rueda desde la última vez. Positivo = hacia arriba.
    ///
    /// **Consume**: dos llamadas seguidas sin girar dan cero la segunda. Así el
    /// llamante no tiene que guardar el valor anterior y restar — que es donde
    /// se cuela el scroll que se mueve solo.
    pub fn rueda(&self) -> i32 {
        invoke(self.cap, INPUT_OP_RUEDA, 0, 0, 0).value as i32
    }

    /// Qué modificadores están pulsados AHORA. No consume nada: es estado.
    ///
    /// Existe porque `tecla()` da un byte ya resuelto y hay combinaciones que
    /// no producen carácter — `Ctrl+Alt` a secas no es ninguna letra.
    ///
    /// ★ En la distribución española `Ctrl+Alt` **es** `AltGr`: lo que produce
    /// `@`, `#`, `[`, `]`, `\`, `|` y `€`. Un atajo que dispare al PULSARLOS
    /// rompe escribir todo eso. Si lo usas como atajo, dispara al SOLTAR y sólo
    /// si no llegó ningún carácter mientras estaban pulsados.
    pub fn modificadores(&self) -> u8 {
        invoke(self.cap, INPUT_OP_MODIFICADORES, 0, 0, 0).value as u8
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
