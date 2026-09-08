//! **EL PULSO DEL ESCRITORIO: cuantas vueltas da por segundo, siempre a la vista.**
//!
//! # *** POR QUE EXISTE, y es una leccion sobre diagnosticar
//!
//! El 2026-09-08 el dueno trajo un sintoma exacto:
//!
//! > *"los FPS dependen de un teclado que no tiene sentido... tengo que pulsar
//! > el bloq numerico SOLO para ver 1 frame que cambia"*
//!
//! Y se persiguieron **SEIS hipotesis leyendo codigo**, una detras de otra:
//!
//! ```text
//!    el framebuffer sin write-combining    NO -- `map_page_wc` existe y se usa
//!    el hilo del bus muerto de hambre      NO -- prioridad 2 contra 0
//!    `yield_screen()` bloqueando           NO -- es un yield, deja LISTA
//!    el `sfence` que falta                 NO -- esta al final de compose
//!    el planificador por prioridad         NO -- el bus gana siempre
//!    `gather` esperando una tecla          NO -- sondea, no bloquea
//! ```
//!
//! Seis descartes correctos y **cero avance**, porque todos contestaban a la
//! misma pregunta y ninguno la contestaba de verdad:
//!
//! > **El bucle del escritorio, gira o no gira?**
//!
//! *** Y esa pregunta tenia respuesta desde hacia meses --`Tick::loops_per_second`
//! se calcula con el TSC en cada vuelta-- pero solo se pintaba **dentro de las
//! ventanas de CPU y MEMORIA**, que hay que ABRIR CON UNA TECLA.
//!
//! > El unico numero que dice si el escritorio esta vivo estaba detras de la
//! > cosa cuya muerte habia que diagnosticar.
//!
//! Es la misma leccion que ya obligo a poner la ficha de CABINA siempre en la
//! barra, escrita en `paint.rs`: *"un panel de diagnostico al que solo se llega
//! con el aparato que puede estar roto no es un panel de diagnostico"*. Se
//! aprendio para CABINA y no se aplico al pulso.
//!
//! # Como se lee, y parte el problema EN DOS
//!
//! ```text
//!    la AGUJA gira    el bucle VIVE, sea cual sea el numero
//!    la AGUJA quieta  el bucle NO da vueltas. Ahi se acaba la ambiguedad
//!
//!    miles            gira rapido. El fallo esta en quien marca SUCIO
//!    decenas          gira despacio: algo dentro de la vuelta cuesta
//!    SIN RELOJ        no es el bucle: el kernel contesto 0 a INFO_TSC_HZ,
//!                     y entonces el ritmo entero de esta casa --el cuarto de
//!                     segundo-- se cuenta por VUELTAS, no por tiempo
//! ```
//!
//! # *** EL REPARTO DEL SEGUNDO, y por que hizo falta (2026-09-08)
//!
//! El dueno arranco con la aguja puesta y trajo el numero:
//!
//! > *"el reloj no sale mal, solo que 1 pulso y luego 50 pulso"*
//!
//! ** El bucle VIVE --gira, y hay reloj-- y aun asi da **50 vueltas por
//! segundo**, o sea **20 ms por vuelta**. Y este bucle no tiene freno ninguno:
//! acaba en `yield_screen()` y vuelve. Veinte milisegundos por vuelta no son
//! lentitud, son alguien quedandose el turno.
//!
//! Asi que la pregunta se parte otra vez en dos, porque una vuelta solo tiene
//! dos mitades y **no se arreglan en el mismo sitio**:
//!
//! ```text
//!    cuerpo 900 / puerta 40    el compositor GASTA el segundo
//!                              -> el trabajo esta en Ring 3: quien compone
//!    cuerpo 40 / puerta 900    el compositor ESPERA el segundo
//!                              -> el trabajo esta en Ring 0: quien reparte
//!    los dos bajos             ni gasta ni espera: entonces son POCAS
//!                              vueltas y hay una que dura de mas
//! ```
//!
//! Suman ~1000 porque son los ms de un segundo, y por eso se leen como un
//! reparto y no como dos medidas sueltas.
//!
//! [!] Esa tercera fila se anadio el mismo dia, y por el mismo motivo que la
//! aguja: sin reloj, `loops_per_second` **no se calcula nunca** y se queda en
//! el cero con el que nacio. La caja pintaba `pulso 0/s`, que se lee como *"el
//! bucle esta muerto"* cuando lo que pasa es *"no tengo con que medirlo"*. Un
//! cero es una medida, y esa medida no se tomo. Ver `desktop::Tick::sin_reloj`.
//!
//! [!] La aguja se anadio DESPUES, y por un fallo de este mismo fichero: ver la
//! nota de `AGUJA`. La primera version solo tenia el numero, y un numero que se
//! calcula una vez por segundo **se ve igual vivo que muerto** el resto del
//! tiempo.
//!
//! ** Ese es todo su trabajo. No dice cual es el fallo: dice **en cual de las
//! dos mitades buscarlo**, y eso es lo que faltaba tras seis descartes.
//!
//! # Lo que NO hace
//!
//! ```text
//!    [ ] no mide FPS: mide VUELTAS. Una vuelta sin nada sucio no pinta
//!    [ ] no arregla nada
//!    [ ] y no se puede cerrar, igual que la ficha de CABINA
//! ```

use bmo_userland as bmo;

use super::{chip_box, INK, INK_DIM, TASKBAR};
use crate::text::decimal;

/// La ranura siguiente al testigo del USB. El testigo mide 168 px desde la
/// suya, asi que esto empieza pasado ese ancho.
const TRAS_TESTIGO: u32 = 168 + 8;
/// Lo que ocupa la forma LARGA: `pulso 12345/s  cuerpo 900  puerta 40  |`.
const ANCHO: u32 = 330;
/// Lo que ocupa la forma CORTA: `pulso 12345/s |`, sin el reparto.
///
/// ** Existe porque la alternativa era no pintar nada. La regla vieja era "si
/// no cabe, no se pinta", y con la caja larga eso apagaba el pulso ENTERO en
/// una pantalla estrecha -- se perderia lo que dice si el escritorio vive por
/// no caber lo que dice en que se le va el tiempo. Primero lo que no se puede
/// perder; el reparto es lo que sobra si falta sitio.
const ANCHO_CORTO: u32 = 150;

/// Lo ultimo que se pinto.
static mut ULTIMO: u32 = u32::MAX;

/// **La aguja.** Avanza en CADA cuarto de segundo que este modulo recibe.
///
/// == *** POR QUE NO BASTABA EL NUMERO (2026-09-08) =========================
///
/// La primera version de este fichero pintaba solo `loops_per_second`, y el
/// dueno lo probo en el Ryzen y trajo esto:
///
/// > *"veo pulso una sola vez y se congela"*
///
/// ** Y NO ERA LA MAQUINA: era el instrumento. `loops_per_second` se calcula
/// **una vez por segundo** --asi esta escrito en `Tick::pulse`-- asi que entre
/// dos calculos el numero es CONSTANTE, y esta caja no se repintaba porque no
/// habia cambiado nada. Correcto, y absolutamente inutil:
///
/// ```text
///    el bucle VIVO y el numero quieto      se ve igual
///    el bucle MUERTO                       se ve igual
/// ```
///
/// *** Un medidor cuyo estado sano se ve identico a su estado roto no mide: es
/// un adorno con cifras. Y encima costo una vuelta al metal para descubrirlo.
///
/// Asi que al lado del numero va una AGUJA que gira con cada cuarto de segundo.
/// Cuatro pasos por segundo, y su unico trabajo es que **quieto signifique
/// muerto**:
///
/// ```text
///    la aguja gira      el bucle VIVE, sea cual sea el numero
///    la aguja quieta    el bucle NO da vueltas. Se acabo la ambiguedad
/// ```
static mut AGUJA: u8 = 0;

/// Los cuatro pasos de la aguja. Se eligen ASCII porque las fuentes de esta
/// casa lo son (ver `docs/identidad`), y porque los cuatro se distinguen de un
/// vistazo a la distancia a la que se mira una barra de tareas. El cuarto va
/// por su codigo ASCII --92 es la barra invertida-- porque escaparla dentro de
/// un literal es justo el tipo de detalle que se rompe al copiar el fichero.
const PASOS: [u8; 4] = [b'|', b'/', b'-', 92];

/// **Olvida lo pintado.** Lo llama quien repinta la barra entera: si no, la
/// caja queda tapada y este modulo cree que sigue en pantalla.
pub(crate) fn olvidar() {
    unsafe { ULTIMO = u32::MAX };
}

/// Lo que este medidor ensena en una vuelta. Va junta y no como cuatro
/// parametros sueltos: son **una sola lectura** --el mismo instante del mismo
/// segundo-- y repartirla en la firma invita a pintar la mitad de un segundo
/// con la mitad de otro.
pub(crate) struct Lectura {
    /// Vueltas del bucle en el ultimo segundo cerrado.
    pub vueltas: u32,
    /// El kernel no publica reloj de referencia: `vueltas` no significa nada.
    pub sin_reloj: bool,
    /// De ese segundo, ms dentro de la vuelta. Ver `desktop::Tick::cuerpo_ms`.
    pub cuerpo_ms: u32,
    /// De ese segundo, ms esperando el turno.
    pub puerta_ms: u32,
}

/// **Pinta el pulso.** Se llama en las vueltas del cuarto de segundo.
pub(crate) fn refrescar(p: &bmo::Pantalla, l: &Lectura) {
    // ** LA AGUJA AVANZA SIEMPRE, y por eso este modulo repinta SIEMPRE que le
    // llega un cuarto. Es lo contrario de lo que hace el testigo --que se calla
    // si no cambio nada-- y es a proposito: aqui lo que se ensena no es el
    // valor, es que **haya latido**.
    let paso = unsafe {
        AGUJA = AGUJA.wrapping_add(1);
        ULTIMO = l.vueltas;
        PASOS[(AGUJA as usize) % PASOS.len()]
    };
    let (x0, y, _, h) = chip_box(super::testigo::RANURA);
    let x = x0 + TRAS_TESTIGO;
    // ** SE ENCOGE ANTES DE CALLARSE. Si no cabe el reparto se pinta el pulso a
    // secas, y solo si tampoco cabe ese se deja el sitio en paz -- pintar
    // encima de otra cosa es peor que no pintar. Misma regla que el testigo.
    let ancho = if x + ANCHO < p.ancho {
        ANCHO
    } else if x + ANCHO_CORTO < p.ancho {
        ANCHO_CORTO
    } else {
        return;
    };
    p.rect(x, y, ancho, h, TASKBAR);
    let ty = y + (h.saturating_sub(bmo::GLIFO_ALTO)) / 2;
    let tx = p.texto(x + 4, ty, "pulso ", INK_DIM);
    // ** SIN RELOJ NO SE PINTA UN NUMERO, y esa es toda la regla. El numero no
    // existe --nadie lo calculo-- asi que ponerlo seria inventarlo. La aguja
    // sigue girando debajo, porque ella no necesita reloj: cuenta vueltas.
    let mut tx = if l.sin_reloj {
        p.texto(tx, ty, "SIN RELOJ ", INK)
    } else {
        let mut buf = [0u8; 10];
        let n = decimal(l.vueltas as u64, &mut buf);
        // ** EN BLANCO SI ES BAJO. Un pulso de dos digitos no es un detalle de
        // rendimiento: es el bucle bloqueado, y tiene que llamar la atencion sin
        // que nadie sepa que numero esperar.
        let tinta = if l.vueltas < 100 { INK } else { INK_DIM };
        let tx = p.texto_bytes(tx, ty, &buf[..n], tinta);
        p.texto(tx, ty, "/s ", INK_DIM)
    };
    // ** EL REPARTO, y solo con reloj: son ms, y sin reloj no hay ms.
    //
    // La mitad GRANDE va en blanco. No es adorno: es la respuesta -- dice de
    // cual de los dos lados hay que tirar, y tiene que verse sin leer los
    // numeros.
    if !l.sin_reloj && ancho == ANCHO {
        let manda_cuerpo = l.cuerpo_ms >= l.puerta_ms;
        tx = renglon(p, tx, ty, " cuerpo ", l.cuerpo_ms, manda_cuerpo);
        tx = renglon(p, tx, ty, " puerta ", l.puerta_ms, !manda_cuerpo);
    }
    // La aguja al final, en blanco: es lo unico de esta caja que tiene que
    // verse desde lejos sin leer.
    p.texto_bytes(tx + 4, ty, &[paso], INK);
}

/// Una mitad del reparto: su nombre en gris y su numero en blanco si es la que
/// se queda el segundo.
fn renglon(p: &bmo::Pantalla, x: u32, y: u32, nombre: &str, ms: u32, manda: bool) -> u32 {
    let tx = p.texto(x, y, nombre, INK_DIM);
    let mut buf = [0u8; 10];
    let n = decimal(ms as u64, &mut buf);
    p.texto_bytes(tx, y, &buf[..n], if manda { INK } else { INK_DIM })
}
