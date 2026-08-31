//! `KIND_AUDIO` -- el derecho a HACER RUIDO, como capability.
//!
//! [carril]  AMARILLO  el derecho a hacer ruido
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! ## Que es esto, y que NO es
//!
//! Esto **no es un driver de audio**. Es el contrato: quien puede sonar, quien
//! no, y que pasa con el aparato cuando el que lo tenia se muere. El driver de
//! verdad --HD Audio, con su codec y su DMA-- es la casilla 5.1 de
//! `docs/plan/PLAN_DOOM.md` y es una pieza XL que todavia no existe.
//!
//! Se escribe primero el contrato **a proposito**. La alternativa era escribir
//! el driver y despues preguntarse quien tiene derecho a usarlo, y esa pregunta
//! contestada tarde es como se acaba con un sistema donde cualquier programa
//! puede pitar encima de cualquier otro. La pantalla ya aprendio esto:
//! `KIND_FRAMEBUFFER` existe desde antes de que hubiera un compositor.
//!
//! ## Exclusivo, igual que la pantalla, y por el mismo motivo
//!
//! Un solo proceso lo tiene a la vez. Dos duenos escribiendo en el mismo
//! aparato de sonido no es mezclar: es ruido. Mezclar es un trabajo con nombre
//! --se llama mezclador-- y **le toca a Ring 3**, no al kernel, exactamente
//! igual que componer ventanas.
//!
//! Y cuando alguien lo reclama, **el kernel se calla**: [`kernel_beep`] no
//! suena mientras el aparato tenga dueno. Es el espejo de `info::has_fb()`.
//!
//! ## Lo que suena HOY, dicho sin adornos
//!
//! El altavoz del PC. `platform/drivers/audio` son 109 lineas de `outb` al
//! puerto 0x61 y un retardo por TSC, llevaban meses sin que las llamara nadie
//! --uno de los crates huerfanos de la auditoria de deuda tecnica-- y esto es
//! lo que por fin las conecta a algo.
//!
//! [!] **Y puede que no se oiga nada, y no seria un fallo de este codigo.** El
//! puerto 0x61 existe en todos los PC; el altavoz fisico, no. Muchas placas
//! modernas --entre ellas las que se prueban aqui-- traen el cabezal SPKR sin
//! nada conectado. Por eso [`AUDIO_OP_DEVICES`] contesta que aparatos hay y
//! **no promete que suenen**: el kernel sabe que el puerto esta ahi, no sabe si
//! hay un zumbador al otro lado. Es la ley 11 de `BITACORA.md` -- un `set` sin
//! su `get` es una carta sin acuse de recibo, y aqui no hay acuse posible.
//!
//! ## Y BLOQUEA mientras pita
//!
//! `beep` es un bucle de espera sobre el TSC: mientras dura, este nucleo no
//! hace otra cosa. Es la unica forma con el altavoz del PC --no hay interrupcion
//! que avise de que el tono acabo-- y por eso hay un tope duro en
//! [`MAX_MS`]. Un programa no puede pedir un pitido de diez segundos y llevarse
//! el nucleo con el.
//!
//! Cuando exista el driver HDA esto desaparece: alli se llena un anillo de
//! buffers y el DMA lo consume solo. La forma bloqueante es del altavoz, no del
//! contrato.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::ring0::obj::cap;

/// Nadie lo tiene. Los pid validos son 0..MAX_PROCS, asi que hace falta un
/// centinela que no pueda ser un pid.
const NO_OWNER: u32 = u32::MAX;

static OWNER: AtomicU32 = AtomicU32::new(NO_OWNER);
/// El handle concedido al dueno, para poder revocarlo si lo SUELTA. Vale `0`
/// cuando no hay dueno. Mismo motivo que en `fb.rs`: `cap` no ofrece "revoca
/// todo lo de este tipo".
static HANDLE: AtomicU64 = AtomicU64::new(0);
/// El crate del altavoz necesita la frecuencia del TSC para medir el tiempo, y
/// se la pasamos una vez. Sin esto usa un valor de reserva de 3,7 GHz y los
/// tonos duran lo que no son.
static CALIBRADO: AtomicBool = AtomicBool::new(false);

/// Ya lo tiene otro proceso.
pub const ERROR_BUSY: u32 = 16;

/// Tope de duracion de un pitido, en milisegundos.
///
/// No es un numero bonito: es el tiempo que este nucleo se queda sin hacer nada
/// dentro del syscall. Un cuarto de segundo se nota y no cuelga; un segundo
/// entero seria un programa de Ring 3 parando el planificador a voluntad.
pub const MAX_MS: u64 = 250;

// -- Operaciones sobre un handle KIND_AUDIO -------------------------------

/// Que aparatos de sonido hay. Devuelve un mapa de bits, ver [`DEVICE_SPEAKER`].
///
/// Existe para que Ring 3 pueda preguntar en vez de suponer: el dia que haya
/// HDA, el mismo programa se entera sin recompilar nada.
pub const AUDIO_OP_DEVICES: u64 = 0x01;
/// Pitar. `a0` = frecuencia en Hz, `a1` = duracion en ms (topada a [`MAX_MS`]).
/// Devuelve los ms que de verdad sono.
pub const AUDIO_OP_BEEP: u64 = 0x02;
/// Volumen global, `a0` de 0 a 100. Devuelve el que queda puesto.
///
/// En el altavoz del PC el volumen no es volumen: es el modo del PIT --pulsos
/// estrechos suenan mas flojo que una onda cuadrada al 50%-- asi que hay dos
/// escalones, no cien. Se dice aqui para que nadie espere un fundido.
pub const AUDIO_OP_VOLUME: u64 = 0x03;
/// Callar ahora mismo.
pub const AUDIO_OP_SILENCE: u64 = 0x04;
/// El tubo isocrono: abrirlo, armarlo y preguntarle. Ver el ABI.
pub const AUDIO_OP_TUBO: u64 = 0x05;

/// Hay altavoz de PC (o al menos el puerto que lo controla: ver la nota del
/// modulo -- que el puerto exista no prueba que haya un zumbador conectado).
pub const DEVICE_SPEAKER: u64 = 1 << 0;
/// Hay HD Audio con su codec abierto. **Hoy siempre 0**: es la casilla 5.1.
pub const DEVICE_HDA: u64 = 1 << 1;
/// **Hay un audifono USB Audio con control de volumen.** Suena de verdad en
/// esta maquina, al reves que el altavoz del PC.
pub const DEVICE_USB: u64 = 1 << 2;

/// Le pasa al crate del altavoz la frecuencia real del TSC, una sola vez.
fn calibrate() {
    if CALIBRADO.swap(true, Ordering::SeqCst) {
        return;
    }
    bmo_audio::init(crate::ring0::task::scheduler::tsc_freq());
}

/// Concede el audio al proceso `pid`. Devuelve el handle, o el error.
///
/// No mapea nada --a diferencia de la pantalla, aqui no hay memoria que
/// entregar-- asi que lo unico que se concede es el DERECHO. Hoy eso ya vale
/// para algo: sin este handle, `AUDIO_OP_BEEP` no resuelve.
pub fn claim(pid: u32) -> Result<u64, u32> {
    // Un solo dueno. `compare_exchange` y no "leer y luego escribir": dos
    // procesos pidiendolo en el mismo tick no pueden ganar los dos.
    if OWNER
        .compare_exchange(NO_OWNER, pid, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ERROR_BUSY);
    }
    // READ **y** WRITE. No es generosidad: el despachador de `INVOKE` resuelve
    // todo handle exigiendo `RIGHT_READ`, asi que una capability concedida solo
    // con WRITE no resuelve ni para preguntar que aparatos hay -- se lleva un
    // `PERMISSION_DENIED` que no tiene nada que ver con lo que pasa. Y las dos
    // clases de operacion existen de verdad aqui: `APARATO` pregunta y `PITAR`
    // manda.
    let handle = match cap::grant(pid, cap::KIND_AUDIO, cap::RIGHT_READ | cap::RIGHT_WRITE, 0) {
        Some(h) => h,
        None => {
            // La tabla estaba llena. Se devuelve el aparato antes de contestar:
            // quedarse marcado como dueno sin handle seria un audio que nadie
            // puede usar y nadie puede reclamar hasta el proximo reinicio.
            OWNER.store(NO_OWNER, Ordering::SeqCst);
            return Err(cap::ERROR_PERMISSION_DENIED);
        }
    };
    HANDLE.store(handle, Ordering::SeqCst);
    calibrate();
    crate::ring0::cabina::info("audio", "sonido cedido a Ring 3", pid as u64);
    Ok(handle)
}

/// Soltar el audio siendo su dueno y seguir vivo.
///
/// Va desde el primer dia, y no por simetria: la pantalla vivio meses sin su
/// `release` y el resultado fue que el escritorio no podia prestarla ni
/// queriendo, porque **la unica forma de soltarla era morir**. El mismo agujero
/// aqui significaria que el primer programa que pite se queda el altavoz para
/// siempre.
///
/// Se calla el aparato antes de soltarlo: un tono que sigue sonando despues de
/// que su dueno lo devolvio es del sistema, y el sistema no pidio ese tono.
pub fn release(pid: u32) -> Result<(), u32> {
    if OWNER.load(Ordering::SeqCst) != pid {
        // No es suyo. Se dice en vez de contestar OK: un "si" a quien no era
        // dueno le haria creer que lo cedio.
        return Err(ERROR_BUSY);
    }
    bmo_audio::beep_ex(0, 0, 0);
    let h = HANDLE.swap(0, Ordering::SeqCst);
    if h != 0 {
        cap::revoke(pid, h);
    }
    OWNER.store(NO_OWNER, Ordering::SeqCst);
    crate::ring0::cabina::info("audio", "sonido SOLTADO por su dueno", pid as u64);
    Ok(())
}

/// El proceso `pid` murio (o salio). Si era el dueno, el kernel recupera el
/// audio. Lo llama `cap::revoke_all`, que corre en TODAS las salidas --EXIT
/// voluntario y muerte por fault.
///
/// [!] **Lo primero que hace es CALLAR el aparato.** Un proceso que muere en
/// mitad de un tono deja el altavoz sonando: el bit del puerto 0x61 se queda
/// puesto y no hay nadie vivo a quien pedirle que lo quite. Un pitido continuo
/// que solo para reiniciando es, literalmente, la maquina de rehen -- la misma
/// forma que el raycaster con el teclado, con otro aparato.
pub fn process_died(pid: u32) {
    if OWNER
        .compare_exchange(pid, NO_OWNER, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        bmo_audio::beep_ex(0, 0, 0);
        HANDLE.store(0, Ordering::SeqCst);
        crate::ring0::cabina::warn(
            "audio",
            "el dueno del sonido MURIO: el kernel calla el aparato",
            pid as u64,
        );
    }
}

/// Pid del dueno actual, o `None`. Lo lee la autopsia para contar fugas.
pub fn owner() -> Option<u32> {
    match OWNER.load(Ordering::SeqCst) {
        NO_OWNER => None,
        pid => Some(pid),
    }
}

/// Que aparatos hay. Publico porque `informe.rs` lo puede querer sin handle:
/// preguntar QUE HAY no es lo mismo que tener derecho a usarlo.
pub fn devices() -> u64 {
    // Se busca aqui porque es la primera pregunta que hace todo programa de
    // sonido: si el audifono esta, que salga en la respuesta desde el principio
    // y no en la segunda llamada.
    crate::ring0::dev::uaudio::buscar();
    // El puerto del altavoz existe en todo x86. Que haya algo conectado al
    // otro lado no se puede saber desde aqui, y por eso este bit dice "hay
    // camino", no "vas a oir algo".
    let mut mapa = DEVICE_SPEAKER;
    // ** Y EL AUDIFONO, que es el unico de los dos que se oye en esta maquina.
    //
    // Este bit faltaba: `DEVICE_USB` estaba declarado, la ventana de F10 ya
    // sabia pintar `+ audifono USB` y cambiar su frase por *"el volumen manda
    // sobre el audifono USB de verdad"*, y **nadie lo encendia nunca**. O sea
    // que la rama existia y era inalcanzable -- el aparato podia estar
    // enchufado, localizado y obedeciendo, y la pantalla seguia diciendo que
    // ahi solo habia un altavoz de PC.
    if crate::ring0::dev::uaudio::hay() {
        mapa |= DEVICE_USB;
    }
    mapa
}

/// El kernel pita **solo si nadie tiene el aparato**.
///
/// Es el espejo exacto de `info::has_fb()`: cuando la pantalla es de Ring 3, el
/// kernel deja de dibujar; cuando el sonido es de Ring 3, el kernel deja de
/// sonar. Un kernel que pita encima del programa que tiene el audio es la
/// version sonora de dos duenos pintando el mismo framebuffer.
pub fn kernel_beep(freq_hz: u32, ms: u32) {
    if OWNER.load(Ordering::SeqCst) != NO_OWNER {
        return;
    }
    calibrate();
    bmo_audio::beep(freq_hz, ms.min(MAX_MS as u32));
}

/// Despacho de las operaciones sobre la capability ya resuelta.
///
/// `a0`/`a1` son los argumentos del `INVOKE`. Devuelve `None` para una
/// operacion que no existe, que es lo que el syscall traduce a "no soportado".
pub fn operation(operation: u64, a0: u64, a1: u64) -> Option<u64> {
    calibrate();
    match operation {
        AUDIO_OP_DEVICES => Some(devices()),
        AUDIO_OP_BEEP => {
            // Frecuencia 0 = silencio, y es legal: es la forma de callar sin
            // gastar otra operacion. Por arriba se corta en 20 kHz porque el
            // divisor del PIT desborda mucho antes de ser util.
            let freq = a0.min(20_000) as u32;
            let ms = a1.min(MAX_MS);
            bmo_audio::beep(freq, ms as u32);
            Some(ms)
        }
        AUDIO_OP_VOLUME => {
            let v = a0.min(100) as u8;
            bmo_audio::set_volume(v);
            // ** Y AL AUDIFONO USB, si lo hay.
            //
            // Los dos, no uno u otro: son aparatos distintos y el programa no
            // tiene por que saber cual esta enchufado. En esta maquina el
            // altavoz del PC no suena --la placa no trae zumbador-- asi que
            // esta linea es la unica de las dos que se puede OIR.
            //
            // Y el volumen del USB no es el mismo numero: alli va en 1/256 dB
            // con signo y dentro del rango que declaro el aparato. La
            // conversion vive en `bmo-uaudio`, que se prueba sin hardware.
            crate::ring0::dev::uaudio::set_volume(v);
            Some(v as u64)
        }
        // ** EL TUBO. Todo por `arg0` y no por cinco operaciones nuevas: la
        // superficie esta congelada y esto son preguntas sobre UN aparato, que
        // es exactamente lo que un handle de `KIND_AUDIO` ya representa.
        AUDIO_OP_TUBO => {
            use crate::ring0::dev::usb::audio as tubo;
            let (encoladas, tarde) = tubo::cuentas();
            Some(match a0 {
                0 => tubo::tubo().is_some() as u64,
                1 => tubo::armar_silencio(true) as u64,
                2 => { tubo::armar_silencio(false); 1 }
                3 => tubo::tubo().map(|t| t.bytes_por_trama as u64).unwrap_or(0),
                4 => tubo::tubo().map(|t| t.frecuencia as u64).unwrap_or(0),
                5 => encoladas,
                6 => tarde,
                7 => tubo::armado() as u64,
                // ** A4: el bufer prestado. `a1` lleva el dato cuando hace falta.
                8 => {
                    // Ofrecer: `a1` es la VA del bloque. Los bytes los dice el
                    // propio bloque -- preguntarselos a la app seria dejar que
                    // ella declare un tamano que no tiene.
                    let pid = crate::ring0::task::scheduler::current_pid();
                    match crate::ring0::obj::memory::bytes_de_bloque(pid, a1) {
                        Some(n) => tubo::ofrecer(pid, a1, n) as u64,
                        None => 0,
                    }
                }
                9 => {
                    let pid = crate::ring0::task::scheduler::current_pid();
                    tubo::escrito(pid, a1) as u64
                }
                10 => tubo::leido(),
                11 => tubo::pendientes(),
                12 => tubo::huecos(),
                13 => {
                    let pid = crate::ring0::task::scheduler::current_pid();
                    tubo::soltar(pid);
                    1
                }
                _ => 0,
            })
        }
        AUDIO_OP_SILENCE => {
            bmo_audio::beep_ex(0, 0, 0);
            Some(0)
        }
        _ => None,
    }
}
