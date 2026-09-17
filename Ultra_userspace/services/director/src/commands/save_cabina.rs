//! **Lo que `save` no decia y CABINA si** (2026-09-17).
//!
//! Eddi: *"Save tiene que decir todo en CABINA... mas organizado por completo"*.
//! Tres cosas que existian y solo se leian en F11 o en `cabina fallos`:
//!
//!   - **usb**: que controlador maneja este kernel, cuantos aparatos quedaron
//!     en el OTRO xHC sin que nadie los mire, y el libro del portero: que llego
//!     por cada puerto y que se hizo con ello. El movil del dueno enchufado al
//!     Ryzen fue lo que lo hizo visible: F11 callaba y `save` no sabia nada.
//!   - **prestamos**: las ventanas. Ofertas vivas, tomadas, huerfanas y las
//!     NEGADAS desde el arranque -- la negativa mas cara de esta casa.
//!   - **avisos**: los ultimos WARNING o peores del anillo, para no tener que
//!     acordarse de `cabina fallos` cuando algo no salio.
//!
//! Vive en su fichero porque `reports.rs` esta en el filo de L6a, y porque son
//! tres tablas que se leen juntas: "que hay enchufado, que se presto, que se
//! quejo".
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! [carril]  VERDE     pinta lo que el kernel contesta; no decide nada
//! [cuesta]  DATO      pregunta a la maquina (INFO y el anillo); una fila mal
//!                     leida engana al que mira, no a la maquina
//! [riesgo]  ESPEJO    desempaqueta bits que empaqueta el kernel; la forma esta
//!                     en `bmo-abi/syscalls/surface/informe.rs` y en un solo
//!                     sitio por campo
//! [consumo] NADA      solo corre cuando alguien escribe `save`

use bmo_userland as bmo;

use super::tabla::{fila, fila_cero, subregla};
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

/// Cuantas fichas del portero se ensenan como mucho. El libro tiene 12.
const FICHAS: u64 = 12;
/// Cuantos avisos recientes se ensenan como mucho.
const AVISOS: u64 = 8;

/// **USB: el controlador y el libro del portero.**
pub(crate) fn report_usb(s: &mut Output) {
    subregla(s, b"usb -- que controlador se maneja, y que llego por cada puerto");
    let censo = bmo::info(bmo::INFO_USB_CENSO);
    let censados = censo & 0xFF;
    let vistos = (censo >> 8) & 0xFF;
    let huerfanos = (censo >> 16) & 0xFF;
    fila(s, b"xHC en la placa", censados, b"", b"controladores censados al arrancar");
    if censo >> 63 == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    ninguno elegido: este kernel no maneja USB en esta sesion\n");
        s.with_ink(INK_PLAIN);
    } else {
        let bdf = (censo >> 24) & 0xFF_FFFF;
        s.text(b"    elegido        ");
        s.hex((bdf >> 16) & 0xFF, 2);
        s.byte(b':');
        s.hex((bdf >> 8) & 0xFF, 2);
        s.byte(b'.');
        s.hex(bdf & 0xFF, 1);
        s.text(b"    bus:dev.func -- el que MAS aparatos veia; es el UNICO que se maneja\n");
        fila(s, b"ve", vistos, b"aparatos", b"en los puertos raiz del elegido");
    }
    // ** La fila que explica un movil mudo. Si no es cero, hay algo enchufado
    // que este kernel no va a mirar jamas, y la solucion esta en la nota.
    fila_cero(s, b"en el OTRO xHC", huerfanos,
              b"aparatos que NO se miran: cambiarlos a un puerto del elegido (S3b.2d)");

    let fichas = bmo::info(bmo::INFO_USB_FICHAS);
    let escritas = fichas & 0xFFFF;
    fila(s, b"fichas", escritas, b"", b"lo que el portero apunto: una por interfaz que llego");
    fila_cero(s, b"sin sitio", (fichas >> 16) & 0xFFFF, b"llegaron mas de las que caben en el libro");
    if escritas == 0 {
        return;
    }
    s.with_ink(INK_ECHO);
    s.text(b"      puerto  vid:pid    que es       que se hizo   (puerto como en F11: el 1 es el primero)\n");
    s.with_ink(INK_PLAIN);
    let mut txt = [0u8; 96];
    // ** Se recorren las `escritas`, sin cortar en un cero: una ficha de un
    // puerto 0 sin vid empaqueta exactamente 0, y el 17-09 el Ryzen enseno
    // `fichas 10` con la tabla VACIA por cortar en la primera.
    for i in 0..escritas.min(FICHAS) {
        let papeles = bmo::info(bmo::INFO_USB_FICHA | (i << 8));
        let veredicto = bmo::info(bmo::INFO_USB_FICHA_VEREDICTO | (i << 8));
        s.text(b"      ");
        s.dec_right(((papeles >> 24) & 0xFF) + 1, 4);
        s.text(b"    ");
        s.hex(papeles >> 48, 4);
        s.byte(b':');
        s.hex((papeles >> 32) & 0xFFFF, 4);
        s.text(b"  ");
        let n = bmo::info_texto(bmo::INFO_TXT_USB_QUE_ES | (i << 8), &mut txt);
        s.text(&txt[..n]);
        for _ in n..13 {
            s.byte(b' ');
        }
        // Teclado, raton y "configurado" son lo bueno; el resto, lo que falta.
        s.with_ink(if veredicto == 1 || veredicto == 2 || veredicto == 11 { INK_GOOD } else { INK_ECHO });
        let n = bmo::info_texto(bmo::INFO_TXT_USB_MOTIVO | (i << 8), &mut txt);
        s.text(&txt[..n]);
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
}

/// **Los prestamos: las ventanas.**
pub(crate) fn report_prestamos(s: &mut Output) {
    subregla(s, b"prestamos -- la memoria que una app OFRECE y el escritorio TOMA (las ventanas)");
    let p = bmo::info(bmo::INFO_PRESTAMOS);
    fila(s, b"vivas", p & 0xFF, b"ofertas", b"bloques ofrecidos ahora mismo");
    fila(s, b"tomadas", (p >> 8) & 0xFF, b"", b"de esas, las que el escritorio ya compone");
    fila_cero(s, b"huerfanas", (p >> 16) & 0xFF, b"el dueno murio con la oferta viva");
    fila_cero(s, b"negadas", p >> 32,
              b"ofertas rechazadas desde el arranque: el motivo, en `cabina fallos`");
}

/// **Los ultimos avisos del anillo**, WARNING o peor.
///
/// Es `cabina fallos` recortado a los ultimos [`AVISOS`]: lo justo para que
/// un `save` pegado en un mensaje traiga tambien lo que se quejo.
pub(crate) fn report_avisos(s: &mut Output) {
    subregla(s, b"avisos -- lo ultimo que se quejo (entero: `cabina fallos`)");
    let hay = bmo::cabina_disponibles();
    // Primera pasada: cuantos avisos hay, para quedarse con los ULTIMOS.
    let mut cuantos = 0u64;
    for n in 0..hay {
        let Some(sev) = bmo::cabina_campo(bmo::CABINA_SEVERIDAD, n) else {
            break;
        };
        if sev >= bmo::SEV_WARNING {
            cuantos += 1;
        }
    }
    if cuantos == 0 {
        s.with_ink(INK_GOOD);
        s.text(b"    ni un aviso ni un fallo en todo el anillo\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let saltar = cuantos.saturating_sub(AVISOS);
    let mut vistos = 0u64;
    let mut modulo = [0u8; 24];
    let mut mensaje = [0u8; 96];
    for n in 0..hay {
        let Some(sev) = bmo::cabina_campo(bmo::CABINA_SEVERIDAD, n) else {
            break;
        };
        if sev < bmo::SEV_WARNING {
            continue;
        }
        vistos += 1;
        if vistos <= saltar {
            continue;
        }
        let valor = bmo::cabina_campo(bmo::CABINA_VALOR, n).unwrap_or(0);
        let nm = bmo::cabina_texto(n, bmo::CABINA_TXT_MODULO, &mut modulo);
        let nx = bmo::cabina_texto(n, bmo::CABINA_TXT_MENSAJE, &mut mensaje);
        s.with_ink(if sev >= bmo::SEV_FAULT { INK_ERR } else { INK_GOOD });
        s.text(if sev >= bmo::SEV_FAULT { b"    [X] " } else { b"    [!] " });
        s.text(&modulo[..nm]);
        for _ in nm..10 {
            s.byte(b' ');
        }
        s.text(&mensaje[..nx]);
        if valor != 0 {
            s.text(b" =");
            s.dec(valor);
        }
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
    }
    if cuantos > AVISOS {
        s.with_ink(INK_ECHO);
        s.text(b"    ...y ");
        s.dec(cuantos - AVISOS);
        s.text(b" mas antes de estos\n");
        s.with_ink(INK_PLAIN);
    }
}
