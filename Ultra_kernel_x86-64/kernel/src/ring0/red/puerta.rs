//! **EL GATE RED** -- el pase que se paga una vez, el buzon, el latido de 4 ms
//! y la revocacion en caliente.
//!
//! [carril]  ROJO      mapea memoria en un proceso y se la quita sin pedirle permiso
//! [consumo] LATE      un hilo cada 4 ms MIENTRAS hay un pase; sin pase no existe
//!
//! # E3 de `docs/plan/PLAN_RED_TX.md` (2026-09-13)
//!
//! Eddi: *"una sola vez paga y luego sin burocracia, pero viene con radar de
//! 4 ms"*. Los veredictos viven en `bmo-puerta-red` y tienen banco; aqui solo se
//! juntan los hechos, se mueven paginas y se toca la tarjeta.
//!
//! ```text
//!    RED_OP_ABRIR    INVOKE, UNA vez. `pase::juzgar` pregunta en orden; si dice
//!                    que si: buzon a cero, 7 paginas mapeadas, grifo abierto,
//!                    latido en marcha, y un handle KIND_RED
//!    el buzon        el proceso deja tramas y recoge las recibidas. CERO syscalls
//!    WAIT(handle)    duerme hasta que el latido mete algo o revoca
//!    el latido       cada 4 ms: tarjeta -> buzon; buzon -> COPIA -> grifo ->
//!                    corral -> campana; y el radar decide si sigue
//!    revocar         grifo cerrado, y las 7 paginas apuntan a una LAPIDA
//! ```
//!
//! # *** Por que una LAPIDA y no un hueco
//!
//! Desmapear dejando el hueco convierte la siguiente lectura del proceso en un
//! fallo de pagina, y el proceso que tiene la red es el DIRECTOR: una revocacion
//! se llevaria el escritorio, la misma trampa que `obj/loan.rs` ya evito. La
//! lapida es un marco a ceros con `ESTADO` puesto: el proceso lee "revocado, y
//! por que"; lo que escriba cae donde nadie lee; y el buzon de verdad queda
//! fuera de su alcance desde el latido en que se decidio.
//!
//! # [!] Lo que esto supone, dicho
//!
//! - **UN nucleo corre Ring 3.** El `invlpg` de `map_page` limpia el TLB del
//!   nucleo que lo ejecuta, y en un espacio que no esta cargado lo limpia el
//!   cambio de `CR3` (no hay PCID). El dia que dos nucleos corran procesos, la
//!   revocacion pide un shootdown, y esta es la linea que hay que leer.
//! - **El cerrojo apaga interrupciones.** Con el puesto no entra ni el tick ni
//!   otro hilo; por eso `spawn_kernel` y `wake_by_key`, que toman el cerrojo del
//!   planificador, van SIEMPRE fuera.
//! - **El radar ve tarde**: lo que el grifo dejo salir en un latido ya salio.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use bmo_puerta_red::{buzon, pase, radar};

use super::salida;
use crate::ring0::mm;
use crate::ring0::task::scheduler;

/// La llave de `WAIT` del buzon. "RED" en hexadecimal que se lee.
pub const LLAVE: u64 = 0x52_4544;
/// Donde queda el buzon en el proceso. Lejos de lo prestado (`0x1_0000_0000`
/// mas 16 ventanas de 64 MiB) y de todo lo de debajo de 4 GiB.
pub const BUZON_VA: u64 = 0x0000_0002_0000_0000;
/// La del hilo del bus: por encima de la tarea ociosa, por debajo del trabajo.
const PRIORIDAD_LATIDO: u8 = 2;

const _: () = assert!(buzon::TRAMA_MAXIMA == bmo_net::tx::MAXIMA);
const _: () = assert!(pase::MS_MAX == bmo_net::tx::DURACION_MAX);
const _: () = assert!(pase::CUPO_MAX == bmo_net::tx::CUPO_MAX);
const _: () = assert!(buzon::PAGINA as u64 == mm::PAGE);

// == EL CERROJO =================================================================

static OCUPADO: AtomicBool = AtomicBool::new(false);

/// Interrupciones fuera y una bandera. Con UN nucleo corriendo trabajo, apagar
/// interrupciones ya es exclusion; la bandera es para el dia que no.
struct Cerrojo(u64);

impl Cerrojo {
    fn tomar() -> Self {
        let rflags: u64;
        unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) rflags) };
        while OCUPADO.swap(true, Ordering::AcqRel) {
            core::hint::spin_loop();
        }
        Cerrojo(rflags)
    }
}

impl Drop for Cerrojo {
    fn drop(&mut self) {
        OCUPADO.store(false, Ordering::Release);
        if self.0 & (1 << 9) != 0 {
            unsafe { core::arch::asm!("sti") };
        }
    }
}

// == LA MEMORIA =================================================================

/// Siete marcos contiguos para el buzon. Se piden una vez y no se sueltan: un
/// marco que estuvo mapeado en un proceso no vuelve al asignador mientras ese
/// proceso pueda seguir teniendolo en su tabla.
static mut BUZON_FIS: u64 = 0;
/// Un marco para la lapida.
static mut LAPIDA_FIS: u64 = 0;

/// El buzon visto desde el espejo del kernel, byte a byte y con `volatile`: el
/// proceso puede estar escribiendo mientras se lee.
struct Fisica {
    base: *mut u8,
}

impl Fisica {
    fn buzon() -> Self {
        Fisica { base: mm::phys_to_virt(unsafe { BUZON_FIS }) as *mut u8 }
    }
}

impl buzon::Memoria for Fisica {
    fn tam(&self) -> usize {
        buzon::BYTES
    }
    fn leer8(&self, off: usize) -> u8 {
        if off >= buzon::BYTES {
            return 0;
        }
        unsafe { core::ptr::read_volatile(self.base.add(off)) }
    }
    fn escribir8(&mut self, off: usize, v: u8) {
        if off < buzon::BYTES {
            unsafe { core::ptr::write_volatile(self.base.add(off), v) };
        }
    }
}

fn reservar() -> bool {
    use crate::ring0::mm::phys::{alloc_frames_contig_de, Titular};
    unsafe {
        // ** `Bufer`: memoria del kernel que un proceso ve, como un fichero reflejado.
        if BUZON_FIS == 0 {
            match alloc_frames_contig_de(buzon::PAGINAS as u64, Titular::Bufer) {
                Some(p) => BUZON_FIS = p,
                None => return false,
            }
        }
        if LAPIDA_FIS == 0 {
            match alloc_frames_contig_de(1, Titular::Bufer) {
                Some(p) => LAPIDA_FIS = p,
                None => return false,
            }
        }
    }
    true
}

fn sellar_lapida(motivo: u32) {
    let p = unsafe { LAPIDA_FIS };
    if p == 0 {
        return;
    }
    let base = mm::phys_to_virt(p) as *mut u8;
    unsafe {
        for i in 0..mm::PAGE as usize {
            core::ptr::write_volatile(base.add(i), 0);
        }
        for (k, b) in motivo.to_le_bytes().iter().enumerate() {
            core::ptr::write_volatile(base.add(buzon::campo::ESTADO + k), *b);
        }
    }
}

/// Las 7 paginas del proceso pasan a la lapida. Si una no se puede, se QUITA:
/// un hueco es peor para el proceso, pero el buzon real no puede quedar a mano.
fn a_la_lapida(aspace: u64) {
    let lapida = unsafe { LAPIDA_FIS };
    for k in 0..buzon::PAGINAS as u64 {
        let va = BUZON_VA + k * mm::PAGE;
        if lapida == 0 || crate::ring0::mm::vmm::map_page(aspace, va, lapida, true, true).is_err() {
            crate::ring0::mm::vmm::unmap_page(aspace, va);
        }
    }
}

// == EL PASE ====================================================================

#[derive(Clone, Copy)]
struct Pase {
    pid: u32,
    aspace: u64,
    hasta_ms: u64,
    lado: buzon::Lado,
    radar: radar::Radar,
}

static mut PASE: Option<Pase> = None;
static ABIERTO: AtomicBool = AtomicBool::new(false);
static DUENO: AtomicU32 = AtomicU32::new(0);
static GENERACION: AtomicU64 = AtomicU64::new(0);
static SECUENCIA: AtomicU64 = AtomicU64::new(0);
static LATIDO_VIVO: AtomicBool = AtomicBool::new(false);
static ULTIMO_MOTIVO: AtomicU32 = AtomicU32::new(0);

/// Hay un pase abierto?
pub fn abierto() -> bool {
    ABIERTO.load(Ordering::Acquire)
}

/// El testigo de `WAIT`. Lo lee el planificador CON su cerrojo: es un atomico y
/// no toma el de aqui.
pub fn secuencia() -> u64 {
    SECUENCIA.load(Ordering::Acquire)
}

/// Sigue valiendo el pase que nombra este handle?
pub fn vigente(pid: u32, generacion: u64) -> bool {
    abierto() && DUENO.load(Ordering::Acquire) == pid && GENERACION.load(Ordering::Acquire) == generacion
}

/// El motivo del ultimo cierre, `0` si nunca se cerro ninguno.
pub fn ultimo_motivo() -> u32 {
    ULTIMO_MOTIVO.load(Ordering::Acquire)
}

/// **LA PUERTA.** Devuelve la generacion del pase (el objeto del handle) o POR
/// QUE no. Se llama desde el syscall, con el `CR3` del que pide cargado.
pub fn abrir(pid: u32, aspace: u64, autoridad: bool, ms: u64, cupo: u32) -> Result<u64, pase::NoPase> {
    let ahora = crate::ring0::reloj::ticks();
    let generacion = {
        let _c = Cerrojo::tomar();
        let id = super::releer();
        let hechos = pase::Hechos {
            autoridad,
            hay_tarjeta: id.is_some_and(|i| i.creible()),
            enlace: id.is_some_and(|i| i.enlace_arriba()),
            receptor: super::rx_activo(),
            ocupado: abierto(),
            cerrandose: LATIDO_VIVO.load(Ordering::Acquire),
            pide_ms: ms,
            pide_cupo: cupo,
        };
        let c = pase::juzgar(&hechos)?;
        if !reservar() {
            return Err(pase::NoPase::SinMemoria);
        }
        if !salida::armar(unsafe { super::MMIO }) {
            return Err(pase::NoPase::SalidaNoArma);
        }
        // ** A CERO ANTES DE MAPEAR: el proceso no ve nunca lo del pase anterior.
        let mut lado = buzon::Lado::nuevo();
        if lado.preparar(&mut Fisica::buzon()).is_err() {
            return Err(pase::NoPase::SinMemoria);
        }
        let base = unsafe { BUZON_FIS };
        for k in 0..buzon::PAGINAS as u64 {
            let va = BUZON_VA + k * mm::PAGE;
            if crate::ring0::mm::vmm::map_page(aspace, va, base + k * mm::PAGE, true, true).is_err() {
                a_la_lapida(aspace);
                return Err(pase::NoPase::NoSeMapea);
            }
        }
        unsafe {
            PASE = Some(Pase { pid, aspace, hasta_ms: ahora + c.ms, lado, radar: radar::Radar::nuevo() });
        }
        salida::abrir_grifo(ahora, c.ms, c.cupo);
        SECUENCIA.store(0, Ordering::Release);
        ULTIMO_MOTIVO.store(0, Ordering::Release);
        DUENO.store(pid, Ordering::Release);
        ABIERTO.store(true, Ordering::Release);
        LATIDO_VIVO.store(true, Ordering::Release);
        GENERACION.fetch_add(1, Ordering::AcqRel) + 1
    };

    // FUERA del cerrojo: `spawn_kernel` toma el del planificador.
    if scheduler::spawn_kernel(latir as *const () as usize as u64, 0, PRIORIDAD_LATIDO).is_none() {
        LATIDO_VIVO.store(false, Ordering::Release);
        {
            let _c = Cerrojo::tomar();
            revocar(radar::Motivo::BuzonRoto);
        }
        crate::ring0::cabina::fault("red", "GATE RED: no se pudo lanzar el latido de 4 ms", pid as u64);
        return Err(pase::NoPase::SinMemoria);
    }
    crate::ring0::cabina::info("red", "GATE RED: pase ABIERTO para el pid", pid as u64);
    crate::ring0::cabina::count("red", "  ...por milisegundos", ms.min(pase::MS_MAX));
    Ok(generacion)
}

/// **Cierra el pase de `pid`**, si es suyo. `false` si no tenia.
pub fn cerrar(pid: u32) -> bool {
    cerrar_por(pid, radar::Motivo::CerradoPorElDueno)
}

/// Lo llama `cap::revoke_all`. El espacio del muerto se destruye despues, asi
/// que aqui no se toca su tabla de paginas.
pub fn process_died(pid: u32) {
    cerrar_por(pid, radar::Motivo::DuenoMurio);
}

fn cerrar_por(pid: u32, motivo: radar::Motivo) -> bool {
    let hecho = {
        let _c = Cerrojo::tomar();
        match unsafe { PASE } {
            Some(p) if p.pid == pid => {
                revocar(motivo);
                true
            }
            _ => false,
        }
    };
    if hecho {
        scheduler::wake_by_key(LLAVE);
    }
    hecho
}

/// **REVOCA.** Con el cerrojo puesto. El latido ve `PASE == None` y se va.
fn revocar(motivo: radar::Motivo) {
    let Some(p) = (unsafe { PASE }) else { return };
    salida::cerrar_grifo();
    let codigo = motivo.codigo();
    buzon::escribir32(&mut Fisica::buzon(), buzon::campo::ESTADO, codigo);
    sellar_lapida(codigo);
    if motivo != radar::Motivo::DuenoMurio {
        a_la_lapida(p.aspace);
    }
    unsafe { PASE = None };
    ABIERTO.store(false, Ordering::Release);
    DUENO.store(0, Ordering::Release);
    ULTIMO_MOTIVO.store(codigo, Ordering::Release);
    // El que duerme en WAIT despierta y lee el porque en la lapida.
    SECUENCIA.fetch_add(1, Ordering::AcqRel);
    if motivo.es_normal() {
        crate::ring0::cabina::info("red", motivo.texto(), p.pid as u64);
    } else {
        crate::ring0::cabina::fault("red", motivo.texto(), p.pid as u64);
    }
}

/// `(abierto << 63) | (motivo << 56) | (ultimo no << 48) | (negadas << 24) | salieron`.
pub fn estado() -> u64 {
    let _c = Cerrojo::tomar();
    let (sal, neg, ult) = salida::contadores();
    ((abierto() as u64) << 63)
        | (((ultimo_motivo() & 0x7F) as u64) << 56)
        | (((ult & 0xFF) as u64) << 48)
        | ((neg & 0xFF_FFFF) << 24)
        | (sal & 0xFF_FFFF)
}

/// El sondeo de siempre, cuando NO hay pase. Con pase, las tramas son del buzon
/// y las recoge el latido: sondear aqui se las comeria.
pub fn sondear_sin_pase() -> u32 {
    let _c = Cerrojo::tomar();
    if abierto() {
        return 0;
    }
    super::rx_poll_con(&mut |_| {})
}

// == EL LATIDO ==================================================================

extern "C" fn latir(_arg: u64) -> ! {
    crate::ring0::cabina::info("red", "GATE RED: el latido de 4 ms EMPIEZA", 0);
    loop {
        let (seguir, despertar) = latido();
        if despertar {
            scheduler::wake_by_key(LLAVE);
        }
        if !seguir {
            crate::ring0::cabina::info("red", "GATE RED: el latido TERMINA", 0);
            LATIDO_VIVO.store(false, Ordering::Release);
            scheduler::exit_and_park();
        }
        let hz = scheduler::tsc_freq();
        if hz == 0 {
            scheduler::yield_current();
            continue;
        }
        scheduler::park_until(scheduler::rdtsc() + hz / 1000 * radar::LATIDO_MS);
    }
}

/// Una vuelta. `(sigue abierto, hay que despertar a quien espera)`.
fn latido() -> (bool, bool) {
    let _c = Cerrojo::tomar();
    let Some(mut p) = (unsafe { PASE }) else {
        return (false, false);
    };
    let ahora = crate::ring0::reloj::ticks();
    let antes = p.lado.secuencia();
    let mmio = unsafe { super::MMIO };
    let id = super::releer();
    let mi_mac = id.map_or([0u8; 6], |i| i.mac);
    let mut v = radar::Vuelta {
        ahora_ms: ahora,
        hasta_ms: p.hasta_ms,
        enlace: id.is_some_and(|i| i.enlace_arriba()),
        ..radar::Vuelta::default()
    };
    let mut m = Fisica::buzon();

    // 1. LO QUE LLEGO, al buzon.
    {
        let lado = &mut p.lado;
        let mut mal = None;
        super::rx_poll_con(&mut |t: &[u8]| {
            if mal.is_none() {
                if let Err(e) = lado.meter(&mut m, t) {
                    mal = Some(e);
                }
            }
        });
        v.mal = mal;
    }

    // 2. Lo que la tarjeta ya envio, de vuelta.
    salida::recoger(ahora);

    // 3. LO QUE DEJO EL PROCESO: COPIA a memoria del kernel, grifo, campana.
    if v.mal.is_none() {
        let mut copia = [0u8; buzon::TRAMA_MAXIMA];
        for _ in 0..buzon::CASILLAS {
            if !salida::hay_sitio() {
                break;
            }
            match p.lado.sacar(&mut m, &mut copia) {
                Ok(None) => break,
                Err(e) => {
                    v.mal = Some(e);
                    break;
                }
                Ok(Some(n)) => match salida::enviar(mmio, &copia[..n], mi_mac, ahora) {
                    Ok(_) => {}
                    Err(salida::Fallo::Grifo(no)) => clasificar(&mut v, no),
                    Err(_) => break,
                },
            }
        }
    }

    let (sal, neg, ult) = salida::contadores();
    p.lado.publicar(&mut m, sal, neg, ult);
    v.cupo_restante = salida::cupo();

    // 4. EL RADAR.
    let veredicto = p.radar.mirar(&v);
    let despues = p.lado.secuencia();
    unsafe { PASE = Some(p) };
    SECUENCIA.store(despues, Ordering::Release);
    match veredicto {
        None => (true, despues != antes),
        Some(motivo) => {
            revocar(motivo);
            (false, true)
        }
    }
}

/// Cada no del grifo, a su casilla del radar.
fn clasificar(v: &mut radar::Vuelta, no: bmo_net::tx::NoSale) {
    use bmo_net::tx::NoSale;
    match no {
        NoSale::OrigenAjeno => v.origen_ajeno += 1,
        NoSale::Ritmo => v.ritmo += 1,
        NoSale::Corta | NoSale::Larga | NoSale::DestinoImposible | NoSale::Tipo => v.malformadas += 1,
        // Plazo y cupo los mira el radar por su cuenta; sin sitio no es culpa
        // del proceso.
        NoSale::Cerrado | NoSale::Caducado | NoSale::SinCupo | NoSale::SinSitio => {}
    }
}
