//! **EL CENTINELA: la prueba que viaja con el trabajo de verdad.**
//!
//! [carril]  AMARILLO  vigila UNA pagina que es suya, y avisa con nombre
//! [consumo] NADA      `mirar` lo llama quien vigila; no arma nada
//!
//! [cuesta]  TAREA -- una falsa alarma no rompe nada, pero CABINA guarda 82
//!           eventos: un centinela que grita sin motivo **vacia el anillo** y se
//!           lleva por delante las lineas que explican otra cosa (L6e)
//!
//! [riesgo]  SILENCIO -- lo que este fichero puede perder es una ALARMA, y una
//!           alarma perdida no se nota nunca. Es el motivo de que la ventana
//!           que mira sea el BORDE y no una muestra al azar: en el borde, o
//!           esta o no esta (L6f)
//!
//! # *** QUE PIDE UN APARATO CON DMA, DE VERDAD
//!
//! Pregunta del dueno, 2026-09-10: *"en DMA que se solicita para aplicar? no lo
//! tipico, sino en que se basa y que quiere"*.
//!
//! Un aparato no pide MEMORIA. Pide **una promesa de cuatro partes**, y ninguna
//! de las cuatro se la puede dar el:
//!
//! ```text
//!    1. UNA DIRECCION QUE ENTIENDA EL BUS   no virtual. Y "fisica" solo por
//!                                           casualidad: en esta placa la del
//!                                           bus y la fisica coinciden, y eso
//!                                           es una PROPIEDAD, no una ley
//!    2. UN TRAMO SEGUIDO EN ESA VISTA       el aparato avanza SUMANDO. No
//!                                           tiene tablas de paginas: la
//!                                           contiguidad es lo unico que el
//!                                           CPU dejo de necesitar hace
//!                                           cuarenta anios y el aparato no
//!    3. UNA VENTANA DE TIEMPO               nadie se lo quita mientras
//!                                           escribe. **Nadie escribe esto**
//!    4. UNA FORMA DE DECIR QUE ACABO        la unica en la que el aparato
//!                                           contesta en vez de pedir
//! ```
//!
//! ** Y ahi esta la respuesta a *"en que se basa"*: **las cuatro son promesas
//! sobre el FUTURO, hechas por un sistema que reparte memoria.** Por eso el DMA
//! no es una API con cuatro funciones: es una disciplina. El mecanismo son tres
//! lineas --una direccion en un registro y una campana-- y todo lo demas existe
//! porque el que promete tambien reasigna.
//!
//! > El aparato no quiere permiso. Quiere que nadie le mueva el sitio. Eso no
//! > es un permiso: es propiedad con horario.
//!
//! # Y LO QUE EL APARATO QUIERE DE VERDAD ES **NO TENER QUE PREGUNTAR**
//!
//! Todo el DMA existe para sacar al CPU del camino del dato. Asi que **cada
//! comprobacion que anadimos es una que el aparato no pidio**: son para
//! nosotros. El aparato pide cuatro cosas; el sistema necesita saber siete. La
//! distancia entre el cuatro y el siete es la materia entera.
//!
//! # ** POR QUE EN USO Y NO CON UNA SONDA
//!
//! Una sonda contesta *"funciono cuando la corri"*. Y el fallo que este fichero
//! busca --que el aparato escriba MAS ALLA de lo que declaro-- no aparece
//! cuando uno mira: aparece con un tamano raro, un lote raro, o un dia raro.
//!
//! *** Asi que esto no es una sonda: **es una comprobacion que viaja pegada al
//! trabajo real**, como `pisados` y `choques`. Corre en cada rebote, con los
//! tamanos de verdad, todo el rato, y no hay que acordarse de lanzarla.
//!
//! # LO INTELIGENTE: SOLO SE MIRA EL BORDE
//!
//! Comparar los 4 KiB del centinela costaria como el `memcpy` del rebote: seria
//! **duplicar el precio del camino lento** para vigilarlo.
//!
//! ```text
//!    un desbordamiento tiene que CRUZAR la frontera para existir
//!    -> con vigilar la frontera basta
//! ```
//!
//! ** Se miran los primeros 64 bytes de la pagina de al lado. Un aparato que se
//! pasa escribe **desde el principio de lo que sigue**, asi que cualquier
//! desbordamiento de un byte o mas cae dentro de esa ventana. Ocho lecturas de
//! `u64` por rebote, contra los miles de bytes que ya se copian.
//!
//! # [!] Y A QUIEN VIGILA DE VERDAD: A NOSOTROS
//!
//! Es facil leer esto como *"por si el HBA miente"*, y esa es la mitad menos
//! util. La otra es que la longitud del PRDT **la calculamos aqui**:
//!
//! ```text
//!    PER_BATCH, MAX_POR_COMANDO, tres `min()` encadenados, un `got` que vuelve
//! ```
//!
//! *** Esa aritmetica es exactamente la que se equivoca por uno. Con el
//! centinela, equivocarse por uno se paga **en una pagina nuestra y se dice**;
//! sin el, se paga en la memoria de otro y no se dice nunca.
//!
//! Cuesta 4 KiB de RAM, una vez, para siempre.

use super::super::super::mm;

/// El patron. Se elige para que **no se parezca a nada que se escriba solo**:
/// ni ceros (memoria recien puesta), ni `0xFF` (un registro que no contesta),
/// ni ASCII, ni una direccion plausible.
const PATRON: u64 = 0xCE47_1E1A_CE47_1E1A;

/// Cuantos `u64` del borde se miran. 64 bytes.
///
/// ** El numero sale de que un desbordamiento tiene que empezar en el byte 0
/// del centinela: con mirar el principio sobra. Sesenta y cuatro y no ocho por
/// si el aparato escribiera en trozos alineados a 64.
const BORDE: usize = 8;

/// Donde vive el centinela. Cero = no hay, y entonces no se mira nada.
static mut PAGINA: u64 = 0;
/// Veces que se ha mirado.
static mut MIRADAS: u64 = 0;
/// Veces que estaba ROTO. **Tiene que ser CERO.**
static mut ROTAS: u64 = 0;
/// Veces que el disco dijo haber movido MAS de lo que se le pidio.
static mut MAS_DE_LO_PEDIDO: u64 = 0;

/// **Poner el centinela.** Se llama al reservar la pagina de rebote, con la
/// pagina de AL LADO.
pub fn sembrar(phys: u64) {
    if phys == 0 {
        return;
    }
    unsafe {
        PAGINA = phys;
        rellenar(phys);
    }
    crate::ring0::cabina::info("disco", "centinela puesto detras de la pagina de rebote", phys);
}

fn rellenar(phys: u64) {
    let p = mm::phys_to_virt(phys) as *mut u64;
    // Solo el borde: es lo unico que se mira, y rellenar 4 KiB en cada siembra
    // seria pagar por una vigilancia que no se hace.
    for i in 0..BORDE {
        unsafe { core::ptr::write_volatile(p.add(i), PATRON) };
    }
}

/// **Sigue entero?** Se llama DESPUES de cada rebote.
///
/// Devuelve `false` y lo dice UNA vez por rotura. Vuelve a sembrar, para que la
/// siguiente respuesta signifique algo -- un centinela roto que se queda roto
/// convierte todas las respuestas siguientes en la misma, y deja de informar.
pub fn mirar() -> bool {
    let phys = unsafe { PAGINA };
    if phys == 0 {
        return true;
    }
    let p = mm::phys_to_virt(phys) as *const u64;
    unsafe { MIRADAS = MIRADAS.wrapping_add(1) };
    let mut roto = false;
    for i in 0..BORDE {
        if unsafe { core::ptr::read_volatile(p.add(i)) } != PATRON {
            roto = true;
            break;
        }
    }
    if !roto {
        return true;
    }
    unsafe { ROTAS = ROTAS.wrapping_add(1) };
    crate::ring0::cabina::fault(
        "disco",
        "EL DISCO ESCRIBIO MAS ALLA DE LO QUE DECLARO (centinela roto)",
        phys,
    );
    rellenar(phys);
    false
}

/// **El disco dijo haber movido mas sectores de los que se le pidieron.**
///
/// [!] Esto no puede pasar, y por eso se comprueba: `got` sale del `PRDBC`, que
/// es un contador del HBA. Un `got > pedido` significa que el contador de bytes
/// no cuadra con la longitud que se puso en el PRDT -- o sea que **la memoria
/// de detras del bufer ya esta escrita** cuando se lee esto.
///
/// ** Cuesta UNA comparacion en el camino del arranque. Es la comprobacion mas
/// barata de todo el DMA y no existia.
pub fn cuadra_la_cuenta(pedido: u16, got: u16) -> u16 {
    if got <= pedido {
        return got;
    }
    unsafe { MAS_DE_LO_PEDIDO = MAS_DE_LO_PEDIDO.wrapping_add(1) };
    crate::ring0::cabina::fault(
        "disco",
        "el HBA dice haber movido MAS sectores de los pedidos (PRDBC)",
        ((pedido as u64) << 32) | got as u64,
    );
    // ** Se devuelve lo PEDIDO y no lo dicho. Creerle al aparato aqui haria que
    // el que llamo copiara de mas -- o sea propagar el fallo en vez de pararlo.
    pedido
}

/// `(miradas, rotas, mas de lo pedido)`. Las dos ultimas, CERO.
pub fn cuentas() -> (u64, u64, u64) {
    unsafe { (MIRADAS, ROTAS, MAS_DE_LO_PEDIDO) }
}
