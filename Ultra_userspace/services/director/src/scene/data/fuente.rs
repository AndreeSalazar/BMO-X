//! **LA FUENTE del explorador** -- que volumen se esta mirando (2026-09-13).
//!
//! [consumo] NADA      no corre en reposo: lee una carpeta cuando alguien entra
//!                     en ella, y pintar solo pregunta a lo ya leido (L6h)
//!
//! ## Por que existe
//!
//! Eddi: *"expande ESTRATOS para que vea otros discos como FAT32 ... y EFI ...
//! alli estan mis apps"*. El explorador del F12 hablaba con UNA cosa --el cursor
//! de ESTRATOS-- en 110 llamadas repartidas por doce ficheros. Anadir otro
//! volumen llamada a llamada habria sido un `if` en cada una, y el dia que
//! alguien olvidara uno, un clic en DATOS moveria el cursor de ESTRATOS.
//!
//! ** La pieza: las MISMAS preguntas que el cursor ya contesta --`hijos`,
//! `hijo_nombre`, `entrar`, `subir`, `nivel_*`-- y aqui dentro se decide QUIEN
//! contesta. El arbol, la rejilla y el grafo no saben que volumen pintan.
//!
//! ```text
//!    1 ESTRATOS   el cursor del kernel, como siempre
//!    2 DATOS      la FAT32 de las apps: un cursor en Ring 3 sobre `DIR_ABRIR`
//!    3 EFI        la particion de arranque (`efi:`), SOLO para mirar
//! ```
//!
//! ## El cursor FAT, y por que guarda los niveles
//!
//! Igual que el de ESTRATOS: cada nivel por el que se baja se queda leido. El
//! arbol pinta los hermanos de TODOS los niveles en cada repintado, y releerlos
//! seria un `DIR_ABRIR` por nivel por cada movimiento del raton.
//!
//! [!] Tope de 64 entradas por carpeta y 8 niveles de hondo. Lo que no cabe se
//! DICE (`truncado`): una carpeta recortada en silencio se ve igual que una con
//! pocos ficheros.
//!
//! [!] Lo que es SOLO de ESTRATOS --firma, sellar, historial, la consola que
//! escribe-- no existe en FAT32, y quien lo ofrece pregunta [`es_estratos`]
//! antes. En EFI no se escribe NADA: ni desde aqui ni desde el kernel, que monto
//! esa particion sin escritor.
//!
//! [!] SIN CERROJO, y es correcto: lo usa solo el hilo del DIRECTOR.

use bmo_userland as bmo;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicU8, Ordering};

pub(crate) use bmo::estratos::{ARCHIVO, DIRECTORIO, NINGUNO};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Volumen {
    Estratos = 0,
    Datos = 1,
    Efi = 2,
}

impl Volumen {
    /// En el orden de las pestanas, que es el de las teclas 1, 2 y 3.
    pub(crate) const TODOS: [Volumen; 3] = [Volumen::Estratos, Volumen::Datos, Volumen::Efi];

    pub(crate) fn nombre(self) -> &'static str {
        match self {
            Volumen::Estratos => "ESTRATOS",
            Volumen::Datos => "DATOS",
            Volumen::Efi => "EFI",
        }
    }
}

static ACTIVO: AtomicU8 = AtomicU8::new(0);

pub(crate) fn activo() -> Volumen {
    match ACTIVO.load(Ordering::Relaxed) {
        1 => Volumen::Datos,
        2 => Volumen::Efi,
        _ => Volumen::Estratos,
    }
}

pub(crate) fn es_estratos() -> bool {
    activo() == Volumen::Estratos
}

/// Cambia de volumen y lo deja en su raiz.
pub(crate) fn cambiar(v: Volumen) -> bool {
    ACTIVO.store(v as u8, Ordering::Relaxed);
    a_la_raiz()
}

/// Lo que va delante de una ruta de este volumen. Sin esto, abrir `efi/boot`
/// desde la pestana EFI abriria la carpeta del mismo nombre en DATOS.
pub(crate) fn prefijo() -> &'static [u8] {
    if activo() == Volumen::Efi { b"efi:" } else { b"" }
}

// ===================================================================
//  El cursor FAT
// ===================================================================

const NIVELES: usize = 9;
const ENTRADAS: usize = 64;

#[derive(Clone, Copy)]
struct Entrada {
    nom: [u8; 12],
    largo: u8,
    dir: bool,
    bytes: u32,
}

const ENTRADA_VACIA: Entrada = Entrada { nom: [0; 12], largo: 0, dir: false, bytes: 0 };

#[derive(Clone, Copy)]
struct Nivel {
    e: [Entrada; ENTRADAS],
    cuantas: usize,
    truncado: bool,
    /// Por cual se bajo al nivel de debajo. `usize::MAX` = por ninguno.
    elegido: usize,
}

const NIVEL_VACIO: Nivel =
    Nivel { e: [ENTRADA_VACIA; ENTRADAS], cuantas: 0, truncado: false, elegido: usize::MAX };

static mut NIVEL: [Nivel; NIVELES] = [NIVEL_VACIO; NIVELES];
static mut HONDO: usize = 0;
static mut LEGIBLE: bool = false;

fn niveles() -> &'static mut [Nivel; NIVELES] {
    unsafe { &mut *addr_of_mut!(NIVEL) }
}

fn hondo_fat() -> usize {
    unsafe { HONDO }
}

fn entrada(nivel: usize, i: usize) -> Option<Entrada> {
    let n = niveles().get(nivel)?;
    if i < n.cuantas { Some(n.e[i]) } else { None }
}

/// **Lee el nivel `l`**: la carpeta a la que se llega bajando por los elegidos
/// de los niveles de encima.
fn leer(l: usize) -> bool {
    let mut ruta = [0u8; 128];
    let pre = prefijo();
    ruta[..pre.len()].copy_from_slice(pre);
    let mut k = pre.len();
    for n in 0..l {
        let niv = &niveles()[n];
        let Some(e) = niv.e.get(niv.elegido).filter(|_| niv.elegido < niv.cuantas) else {
            return false;
        };
        let m = e.largo as usize;
        if k + m + 1 > ruta.len() {
            return false;
        }
        ruta[k] = b'/';
        k += 1;
        ruta[k..k + m].copy_from_slice(&e.nom[..m]);
        k += m;
    }
    let niv = &mut niveles()[l];
    niv.cuantas = 0;
    niv.truncado = false;
    niv.elegido = usize::MAX;
    // `Directorio` cierra su ranura al soltarse: el kernel solo tiene ocho.
    let Ok(d) = bmo::Directorio::open(&ruta[..k]) else { return false };
    let mut vistas = 0u32;
    while vistas < 512 {
        let Some(ent) = d.next() else { break };
        vistas += 1;
        let mut nom = [0u8; 12];
        let largo = ent.legible(&mut nom);
        if crate::text::is_dot_entry(&nom[..largo]) {
            continue;
        }
        if niv.cuantas == ENTRADAS {
            niv.truncado = true;
            break;
        }
        niv.e[niv.cuantas] = Entrada { nom, largo: largo as u8, dir: ent.es_dir, bytes: ent.bytes };
        niv.cuantas += 1;
    }
    true
}

// ===================================================================
//  Las preguntas, con el mismo nombre que las del cursor de ESTRATOS
// ===================================================================

pub(crate) fn a_la_raiz() -> bool {
    if es_estratos() {
        return bmo::estratos::a_la_raiz();
    }
    let ok = leer(0);
    unsafe {
        HONDO = 0;
        LEGIBLE = ok;
    }
    ok
}

/// Se pudo listar la raiz de este volumen? En ESTRATOS lo dice su propia guarda.
pub(crate) fn legible() -> bool {
    es_estratos() || unsafe { LEGIBLE }
}

pub(crate) fn hijos() -> u64 {
    if es_estratos() {
        return bmo::estratos::hijos();
    }
    niveles()[hondo_fat()].cuantas as u64
}

pub(crate) fn truncado() -> bool {
    if es_estratos() {
        return bmo::estratos::truncado();
    }
    niveles()[hondo_fat()].truncado
}

pub(crate) fn hondo() -> u64 {
    if es_estratos() {
        return bmo::estratos::hondo();
    }
    hondo_fat() as u64
}

/// Que es el nodo donde se esta. En FAT32 siempre una carpeta.
pub(crate) fn tipo() -> u64 {
    if es_estratos() { bmo::estratos::tipo() } else { DIRECTORIO }
}

pub(crate) fn hijo_tipo(i: u64) -> u64 {
    nivel_hijo_tipo(hondo(), i)
}

pub(crate) fn hijo_nombre(i: u64, dst: &mut [u8]) -> usize {
    if es_estratos() {
        return bmo::estratos::hijo_nombre(i, dst);
    }
    nivel_hijo_nombre(hondo(), i, dst)
}

pub(crate) fn hijo_bytes(i: u64) -> u64 {
    if es_estratos() {
        return bmo::estratos::hijo_bytes(i);
    }
    entrada(hondo_fat(), i as usize).map_or(0, |e| e.bytes as u64)
}

/// Baja al hijo `i`. `false` si es un fichero, si no existe o si no se pudo leer
/// -- y en ese caso no se mueve nada.
pub(crate) fn entrar(i: u64) -> bool {
    if es_estratos() {
        return bmo::estratos::entrar(i);
    }
    let h = hondo_fat();
    let Some(e) = entrada(h, i as usize) else { return false };
    if !e.dir || h + 1 >= NIVELES {
        return false;
    }
    niveles()[h].elegido = i as usize;
    if leer(h + 1) {
        unsafe { HONDO = h + 1 };
        true
    } else {
        niveles()[h].elegido = usize::MAX;
        false
    }
}

/// Sube al padre. No relee nada: el nivel de encima sigue leido.
pub(crate) fn subir() -> bool {
    if es_estratos() {
        return bmo::estratos::subir();
    }
    let h = hondo_fat();
    if h == 0 {
        return false;
    }
    unsafe { HONDO = h - 1 };
    niveles()[h - 1].elegido = usize::MAX;
    true
}

/// El nombre del tramo `nivel` de la ruta (1 = el primero bajo la raiz).
pub(crate) fn nombre_nivel(nivel: u64, dst: &mut [u8]) -> usize {
    if es_estratos() {
        return bmo::estratos::nombre_nivel(nivel, dst);
    }
    if nivel == 0 || nivel as usize > hondo_fat() {
        return 0;
    }
    let n = &niveles()[nivel as usize - 1];
    nivel_hijo_nombre(nivel - 1, n.elegido as u64, dst)
}

pub(crate) fn nivel_hijos(nivel: u64) -> u64 {
    if es_estratos() {
        return bmo::estratos::nivel_hijos(nivel);
    }
    if nivel as usize > hondo_fat() { 0 } else { niveles()[nivel as usize].cuantas as u64 }
}

pub(crate) fn nivel_hijo_tipo(nivel: u64, i: u64) -> u64 {
    if es_estratos() {
        return if nivel == bmo::estratos::hondo() {
            bmo::estratos::hijo_tipo(i)
        } else {
            bmo::estratos::nivel_hijo_tipo(nivel, i)
        };
    }
    match entrada(nivel as usize, i as usize) {
        Some(e) if nivel as usize <= hondo_fat() => if e.dir { DIRECTORIO } else { ARCHIVO },
        _ => bmo::estratos::NOTHING,
    }
}

pub(crate) fn nivel_hijo_nombre(nivel: u64, i: u64, dst: &mut [u8]) -> usize {
    if es_estratos() {
        return bmo::estratos::nivel_hijo_nombre(nivel, i, dst);
    }
    match entrada(nivel as usize, i as usize) {
        Some(e) if nivel as usize <= hondo_fat() => {
            let m = (e.largo as usize).min(dst.len());
            dst[..m].copy_from_slice(&e.nom[..m]);
            m
        }
        _ => 0,
    }
}

pub(crate) fn nivel_elegido(nivel: u64) -> u64 {
    if es_estratos() {
        return bmo::estratos::nivel_elegido(nivel);
    }
    let n = nivel as usize;
    if n >= hondo_fat() { NINGUNO } else { niveles()[n].elegido as u64 }
}
