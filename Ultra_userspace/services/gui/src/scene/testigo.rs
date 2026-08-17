//! **EL TESTIGO DEL BUS**: la luz que dice si el teclado esta vivo, encendida
//! en la barra y sin abrir nada.
//!
//! Es la mitad de Ring 3 de la sexta exigencia (E6) de
//! `docs/EL_TECLADO_EXIGE.md`. La otra mitad la contesta el kernel en
//! `dev/usb/salud.rs`.
//!
//! === El dia que lo pidio ===
//!
//! *"funciona un rato y se muere... el teclado sufre mas, el raton no"*. El
//! kernel lo sabia: tiene cinco contadores, uno por exigencia, y hasta un
//! vigilante que grita `el hilo del bus DEJO DE LATIR`. Pero:
//!
//! ```text
//!    el aviso se dice UNA vez        (de-dup por bandera, y hace bien)
//!    y va a un panel que hay que ABRIR
//!    y el panel se abre... escribiendo
//! ```
//!
//! > ** LA REGLA (R-USB6): UNA AVERIA VIVA ES UN ESTADO, NO UN EVENTO.
//! >
//! > Un aviso informa a quien ya estaba mirando. Una averia que **sigue
//! > ocurriendo** necesita una luz encendida mientras dure, y en el sitio donde
//! > vive el dueno.
//!
//! === Por que se pinta SIEMPRE, tambien cuando todo va bien ===
//!
//! Porque una luz que solo aparece cuando hay averia **no se distingue de una
//! luz que no funciona**. Si la primera vez que se ve es el dia malo, lo que
//! dice no se puede creer: nadie sabe que aspecto tenia cuando el sistema
//! estaba sano. Encendida en verde apagado todos los dias, el rojo del dia malo
//! es informacion.
//!
//! Y por eso vive AL LADO de la ficha de CABINA --en la ranura siguiente, con
//! su misma geometria--, que ya esta siempre por la misma razon escrita con
//! otras palabras: *"un panel de diagnostico al que solo se llega con el
//! aparato que puede estar roto no es un panel de diagnostico"*. La luz dice
//! **que** pasa; la ficha de al lado es **donde** se mira entero, y con el
//! raton.
//!
//! === Lo que NO hace ===
//!
//! No arregla nada ni pide nada: **lee dos numeros y elige un color**. Toda
//! decision sobre el bus --resucitar un endpoint, soltar un puerto-- vive en el
//! kernel y sigue viviendo alli. Esto es una ventana, como `usb/panel.rs`.

use bmo_userland as bmo;

use super::{chip_box, INK, INK_DIM, TASKBAR};
use crate::text::decimal;

/// Ancho de la caja del testigo. Da para el punto y ~19 letras, que es lo que
/// mide el mensaje mas largo de los de abajo.
const TESTIGO_W: u32 = 168;

/// Cuantas fichas hay antes de el. Se coloca **en la ranura siguiente a la de
/// CABINA** y con la geometria de una ficha, no con numeros propios: asi se
/// mueve solo si algun dia cambia el tamano de las fichas.
///
/// [!] Y no contra el borde derecho, que era el sitio obvio: alli pinta el
/// arranque `SIN ENTRADA: teclado y raton son de otro`, que son cuarenta letras
/// colocadas por su largo real. Dos cosas en el mismo sitio es una que tapa a la
/// otra, y la tapada seria justo el aviso del dia en que la entrada no se pudo
/// reclamar.
const TESTIGO_RANURA: u32 = 3;

/// Verde apagado: sano se ve, pero no llama. Un verde brillante permanente
/// convierte la barra en un arbol de navidad y entrena al ojo a no mirarla.
const LUZ_BIEN: u32 = 0x0034_9E58;
/// Ambar: funciona, pero se esta reparando solo. No es una alarma; es un aviso
/// de que algo esta costando.
const LUZ_DESGASTE: u32 = 0x00F5_9E0B;
/// Rojo: hoy no se puede escribir por aqui.
const LUZ_CAIDO: u32 = 0x00EF_4444;

/// Cuantos milisegundos sin latido del hilo del bus se consideran "parado".
///
/// El hilo late cada 4 ms. Cien es veinticinco vueltas perdidas: lo bastante
/// alto para que un pico de carga --un frame gordo, una lectura de disco-- no
/// encienda la luz, y lo bastante bajo para que un teclado muerto se vea antes
/// de que el dueno termine de preguntarse por que no escribe.
const LATIDO_LIMITE_MS: u64 = 100;

/// Que esta pasando, en un solo dato. **Es un HECHO nombrado, no un consejo**:
/// el testigo no dice que hacer, dice que ve.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Luz {
    /// No hay controlador xHCI. Ni sano ni roto: no hay bus que mirar.
    SinBus,
    /// El controlador dice que esta muerto (HSE/HCE en `USBSTS`).
    XhcMuerto,
    /// El hilo del bus no ha latido en [`LATIDO_LIMITE_MS`]. E1 caida: nadie
    /// esta preguntando, y un endpoint de interrupcion al que no se pregunta
    /// no manda nada aunque este perfecto.
    BusParado,
    /// Hay bus, pero ningun teclado USB adoptado. Es lo que se ve al
    /// desenchufarlo, y es un hecho: puede que se este escribiendo por otro
    /// sitio.
    SinTeclado,
    /// Adoptado y **sin escuchar**: o no tiene transferencia encolada, o su
    /// endpoint no esta en `Running`. Esta es la muerte silenciosa que este
    /// documento entero existe para hacer visible.
    TecladoParado,
    /// Vivo, pero con contadores que deberian ser cero. El numero dice cual.
    Desgaste(Desgaste, u64),
    /// Todo lo que se puede comprobar, comprobado.
    Bien,
}

/// Cual de los cuatro contadores de `INFO_USB_AVERIAS` esta sucio. En el orden
/// en que importan: el primero mata endpoints, el ultimo solo cuesta tiempo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Desgaste {
    /// Eventos perdidos del aparcadero (E2). El evento ES el permiso para
    /// volver a encolar: perder uno deja el endpoint mudo para siempre.
    Perdidos,
    /// Se intento resucitar un endpoint y no salio (E3).
    Fallidas,
    /// Endpoints resucitados (E3). El sistema funciona; el bus da errores.
    Reparados,
    /// Barridos que repararon algo (E5): se estan perdiendo avisos de puerto.
    Avisos,
}

/// Lo ultimo que se pinto, para no repintar lo mismo sesenta veces por segundo.
///
/// ** Vive en un `static` --o sea en `.bss`-- y no en el `Desktop`, y no por
/// pereza: es memoria de ESTA luz, no del escritorio, y nadie mas la mira.
/// Meterla en el struct obligaria a pasarla por la firma de `compose` para no
/// ganar nada. Lo que si respeta es la regla que costo el desbordamiento del
/// 2026-08-14: **el estado que vive todo el programa no va a la pila**.
static mut ULTIMA: Option<Luz> = None;

/// **Olvida lo pintado.** Se llama cuando algo repinto la barra por debajo --el
/// fondo entero al volver de prestar la pantalla, por ejemplo--: el testigo
/// sigue creyendo que su luz esta ahi, y lo que hay es barra vacia.
///
/// Sin esto, el unico momento en que se repintaria seria al CAMBIAR de estado.
/// Una luz sana que se borro y no vuelve es peor que no tenerla: el hueco vacio
/// se lee como "no hay problema".
pub(crate) fn olvidar() {
    unsafe { ULTIMA = None };
}

/// Lee la salud del bus y pinta la luz **si cambio algo**.
///
/// Se llama una vez por fotograma. Las dos lecturas de `OP_INFO` cuestan dos
/// puertas de ~884 ciclos, y aun asi no se hacen siempre: `cada` las espacia.
/// No porque duelan --a 60 fps serian 106.000 ciclos por segundo, nada-- sino
/// porque un estado que cambia despacio no se aprende mirandolo mas rapido.
pub(crate) fn refrescar(p: &bmo::Pantalla, frames: u32, cada: u32) {
    // ** Se cuenta LA DISTANCIA al ultimo refresco, no `frames % cada`. El
    // modulo parece lo mismo y no lo es: quien llama solo lo hace en los
    // fotogramas que pintan, asi que un `% 15` puede caer sistematicamente en
    // vueltas que no pintan y dejar la luz sin mirar durante mucho rato. La
    // distancia no depende de con que fotogramas coincida.
    static mut VISTO_EN: u32 = 0;
    // Se COPIA el valor en vez de preguntarle al `static`: un `ULTIMA.is_none()`
    // toma una referencia compartida a un `static mut`, que es UB aunque
    // compile y el compilador lo avisa. `Luz` es `Copy`, asi que leerlo cuesta
    // lo mismo y no crea ninguna referencia.
    let previa = unsafe { ULTIMA };
    if previa.is_some() && frames.wrapping_sub(unsafe { VISTO_EN }) < cada {
        return;
    }
    unsafe { VISTO_EN = frames };
    let luz = leer();
    if previa == Some(luz) {
        return;
    }
    unsafe { ULTIMA = Some(luz) };
    pintar(p, luz);
}

/// Las dos preguntas al kernel, convertidas en un solo hecho.
///
/// ** El orden de las preguntas ES el diagnostico, y va de fuera hacia dentro:
/// sin controlador no hay bus, sin bus que lata no hay pregunta que llegue al
/// teclado, y sin preguntas no significa nada que el endpoint parezca sano. Al
/// reves --mirando primero el teclado-- se diria "teclado parado" cuando lo que
/// se paro fue el hilo, que es un diagnostico que manda a mirar el aparato
/// equivocado.
fn leer() -> Luz {
    let salud = bmo::info(bmo::INFO_USB_SALUD);
    let bits = salud & 0xFFFF;
    let edad = (salud >> bmo::USB_SALUD_EDAD_SHIFT) & bmo::USB_SALUD_EDAD_MASK;

    if bits & bmo::USB_SALUD_XHCI == 0 {
        return Luz::SinBus;
    }
    if bits & bmo::USB_SALUD_XHC_AVERIADO != 0 {
        return Luz::XhcMuerto;
    }
    if edad >= LATIDO_LIMITE_MS {
        return Luz::BusParado;
    }
    if bits & bmo::USB_SALUD_KBD == 0 {
        return Luz::SinTeclado;
    }
    if bits & bmo::USB_SALUD_KBD_BOMBA == 0 || bits & bmo::USB_SALUD_KBD_CORRE == 0 {
        return Luz::TecladoParado;
    }

    // Vivo. Queda mirar lo que tendria que ser cero -- y se mira aunque el
    // teclado responda, porque estos numeros son el aviso ANTES de la averia:
    // el aparcadero que empieza a perder eventos es el que se lleva por delante
    // el endpoint del que menos habla, y el que menos habla es el teclado.
    let averias = bmo::info(bmo::INFO_USB_AVERIAS);
    let perdidos = averias & 0xFFFF;
    let fallidas = (averias >> 16) & 0xFFFF;
    let reparados = (averias >> 32) & 0xFFFF;
    let avisos = (averias >> 48) & 0xFFFF;
    if perdidos > 0 {
        Luz::Desgaste(Desgaste::Perdidos, perdidos)
    } else if fallidas > 0 {
        Luz::Desgaste(Desgaste::Fallidas, fallidas)
    } else if reparados > 0 {
        Luz::Desgaste(Desgaste::Reparados, reparados)
    } else if avisos > 0 {
        Luz::Desgaste(Desgaste::Avisos, avisos)
    } else {
        Luz::Bien
    }
}

/// El color y las palabras de cada estado. **Se dice el hecho, no el
/// veredicto**: "BUS PARADO" y no "teclado roto" -- el bus parado tambien deja
/// mudo a un teclado perfecto, y mandar a mirar el cable seria mandar al sitio
/// equivocado.
fn pintar(p: &bmo::Pantalla, luz: Luz) {
    let (color, texto, numero) = match luz {
        Luz::SinBus => (INK_DIM, "SIN BUS USB", None),
        Luz::XhcMuerto => (LUZ_CAIDO, "xHC MUERTO", None),
        Luz::BusParado => (LUZ_CAIDO, "BUS PARADO", None),
        Luz::SinTeclado => (LUZ_CAIDO, "SIN TECLADO USB", None),
        Luz::TecladoParado => (LUZ_CAIDO, "TECLADO PARADO", None),
        Luz::Desgaste(d, n) => {
            let t = match d {
                Desgaste::Perdidos => "EVT PERDIDOS ",
                Desgaste::Fallidas => "NO RESUCITA ",
                Desgaste::Reparados => "REPARADO x",
                Desgaste::Avisos => "AVISOS PERD ",
            };
            (LUZ_DESGASTE, t, Some(n))
        }
        Luz::Bien => (LUZ_BIEN, "TECLADO", None),
    };

    let (x, y, _, h) = chip_box(TESTIGO_RANURA);
    if x + TESTIGO_W >= p.ancho {
        // En una pantalla estrecha no cabe, y se prefiere no pintarlo a pintarlo
        // encima de otra cosa: una luz a medias en el sitio equivocado es peor
        // que ninguna.
        return;
    }
    // El hueco entero primero: los mensajes miden distinto y sin borrar
    // quedarian letras del anterior asomando por la derecha -- que es como se
    // lee "TECLADO PARADOO".
    p.rect(x, y, TESTIGO_W, h, TASKBAR);
    p.rect(x, y + (h - 8) / 2, 8, 8, color);
    // El texto en gris salvo cuando esta caido: en rojo, la palabra tiene que
    // llegar antes que el punto.
    let tinta = match luz {
        Luz::XhcMuerto | Luz::BusParado | Luz::SinTeclado | Luz::TecladoParado => INK,
        _ => INK_DIM,
    };
    let tx = x + 16;
    let ancho = p.texto(tx, y + (24 - bmo::GLIFO_ALTO) / 2, texto, tinta);
    if let Some(n) = numero {
        let mut buf = [0u8; 10];
        let k = decimal(n, &mut buf);
        p.texto_bytes(tx + ancho, y + (24 - bmo::GLIFO_ALTO) / 2, &buf[..k], tinta);
    }
}
