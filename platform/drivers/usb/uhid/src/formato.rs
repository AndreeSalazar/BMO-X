//! **El HID Report Descriptor, leido.** Donde esta cada campo del informe, y de
//! cuantos bits -- dicho por el aparato.
//!
//! === Por que hacia falta ===
//!
//! El driver suponia el informe del protocolo BOOT: cuatro bytes
//! `[botones, dx, dy, rueda]`, todos de 8 bits. Y este raton **ignoro el
//! `SET_PROTOCOL(boot)`** -- lo confeso el mismo con `GET_PROTOCOL`:
//! `protocolo=0x1 (INFORME: el aparato ignoro el BOOT)`.
//!
//! El primer parche fue saltarse un byte, porque en protocolo de informe hay un
//! Report ID delante. Eso arreglo el sintoma visible --el puntero se movia al
//! hacer clic-- y dejo abierta la pregunta de fondo: **los desplazamientos son
//! de 8 bits o de 16?** Un raton de juego manda casi siempre 16, y entonces el
//! byte que se lee como `dy` es la mitad alta de `dx`: mover en horizontal
//! moveria tambien en vertical, y un tiron rapido saldria disparado. La foto
//! decia `raton x=-4332`, que no es un movimiento de mano.
//!
//! La respuesta a esa pregunta estaba --y esta siempre-- en el aparato. Se le
//! pide el Report Descriptor y se lee. Es la ley 11 de la bitacora, la que dejo
//! el episodio 22: **a un dispositivo se le pregunta, no se le supone.** Y esta
//! vez no hay foto que valga: adivinar entre 8 y 16 bits mirando ocho bytes de
//! log es leer el formato en la variable equivocada.
//!
//! === Que se lee, y que no ===
//!
//! Se sacan cuatro campos: botones, X, Y y rueda, con su posicion en BITS y su
//! tamano. Nada mas. No hay tabla de usages completa, ni Feature reports, ni
//! unidades fisicas, ni Push/Pop de estado global -- este parser existe para
//! localizar cuatro numeros en un informe de raton, y todo lo que no sirva a eso
//! es superficie que mantener sin nadie que la use.
//!
//! Lo que si se respeta es la aritmetica real del descriptor: `Report Size` y
//! `Report Count` en bits (no en bytes), el desplazamiento acumulado **por
//! Report ID**, y el reparto de usages entre los elementos de un mismo item
//! (una lista, o un rango `Usage Minimum`..`Usage Maximum`).

/// Un campo dentro del informe: donde empieza, en bits, y cuanto mide.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Campo {
    /// Bit donde empieza, contando desde el principio del informe **sin** el
    /// byte de Report ID.
    pub bit: u16,
    pub bits: u8,
}

impl Campo {
    /// Lee el campo del buffer y lo devuelve **con signo extendido**.
    ///
    /// Los desplazamientos de un raton son relativos y con signo, y ahi es
    /// donde muere un parser descuidado: un `dx` de 12 bits con el valor
    /// `0xFFF` es -1, no 4095. Extender el signo es responsabilidad de quien
    /// conoce el ancho, y el unico que lo conoce es este campo.
    pub fn leer_con_signo(&self, informe: &[u8]) -> i32 {
        let bruto = self.leer_crudo(informe);
        if self.bits == 0 || self.bits >= 32 {
            return bruto as i32;
        }
        let signo = 1u32 << (self.bits - 1);
        if bruto & signo != 0 {
            (bruto | !(signo * 2 - 1)) as i32
        } else {
            bruto as i32
        }
    }

    /// Los bits tal cual, sin interpretar. Para los botones, que son banderas.
    pub fn leer_crudo(&self, informe: &[u8]) -> u32 {
        let mut v = 0u32;
        for i in 0..self.bits as u16 {
            let bit = self.bit + i;
            let byte = (bit / 8) as usize;
            if byte >= informe.len() {
                break;
            }
            if informe[byte] >> (bit % 8) & 1 != 0 {
                v |= 1 << i;
            }
        }
        v
    }
}

/// El formato del informe de un raton, sacado de su Report Descriptor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Formato {
    /// El Report ID que precede al informe, o 0 si el aparato no usa ninguno.
    ///
    /// Cero no es "el informe 0": un descriptor sin `Report ID` **no manda el
    /// byte**, y esa es exactamente la diferencia que corria todo un byte.
    pub report_id: u8,
    pub botones: Option<Campo>,
    pub x: Option<Campo>,
    pub y: Option<Campo>,
    pub rueda: Option<Campo>,
    /// Longitud del informe en bits, sin contar el Report ID.
    pub bits: u16,
}

impl Formato {
    /// Cuantos bytes hay que saltarse al principio del buffer.
    pub fn desplazamiento(&self) -> usize {
        if self.report_id != 0 { 1 } else { 0 }
    }

    /// El informe BOOT de toda la vida: `[botones, dx, dy, rueda]`, 8 bits cada
    /// uno y sin Report ID.
    ///
    /// Es el reserva para cuando no hay descriptor que leer o no se entiende.
    /// Que exista no es pereza: un aparato que cumple el protocolo BOOT tiene
    /// **prohibido** mandar otra cosa, asi que esto es un formato correcto y
    /// no una suposicion -- la suposicion era aplicarlo sin preguntar.
    pub const fn boot() -> Self {
        Self {
            report_id: 0,
            botones: Some(Campo { bit: 0, bits: 8 }),
            x: Some(Campo { bit: 8, bits: 8 }),
            y: Some(Campo { bit: 16, bits: 8 }),
            rueda: Some(Campo { bit: 24, bits: 8 }),
            bits: 32,
        }
    }

    /// El formato BOOT, con o sin el byte de Report ID delante.
    ///
    /// Es el reserva de cuando el Report Descriptor no se puede leer. El
    /// `report_id` que se pone aqui es **1 como cualquier otro numero**: lo
    /// unico que se sabe es que hay un byte delante, porque `GET_PROTOCOL`
    /// contesto "protocolo de informe". No se sabe cual es el informe, y este
    /// campo solo se usa para [`Self::desplazamiento`].
    pub const fn boot_con_id(hay_report_id: bool) -> Self {
        let mut f = Self::boot();
        if hay_report_id {
            f.report_id = 1;
        }
        f
    }

    /// Los desplazamientos son de mas de un byte? Es LA pregunta que abrio
    /// este modulo, y ahora tiene una respuesta que no depende de mirar un log.
    pub fn ejes_anchos(&self) -> bool {
        self.x.map_or(false, |c| c.bits > 8) || self.y.map_or(false, |c| c.bits > 8)
    }
}

// -- Usages que este parser reconoce -------------------------------------
const PAGINA_GENERIC_DESKTOP: u32 = 0x01;
const PAGINA_BOTONES: u32 = 0x09;
const USO_X: u32 = 0x30;
const USO_Y: u32 = 0x31;
const USO_RUEDA: u32 = 0x38;

/// Cuantos Report ID distintos se siguen a la vez. Un raton usa uno o dos (el
/// informe y a veces uno de consumidor); dieciseis es holgura, no ambicion.
const MAX_IDS: usize = 16;

/// Cuantos usages locales se recuerdan por item. Un raton declara tres ejes y
/// cinco botones; ocho cubre cualquier informe sensato y acota el estado.
const MAX_USOS: usize = 8;

/// **Tope de elementos que un solo item Input puede declarar.**
///
/// # *** EL HUECO QUE ESTO TAPA (auditoria 2026-08-24)
///
/// `Report Count` **lo pone el aparato** y es un entero de 32 bits. Sin tope:
///
/// ```text
///    un descriptor con Report Count = 0xFFFF_FFFF
///      -> `for k in 0..4_294_967_295`
///      -> y eso corre DENTRO de la enumeracion USB, en el kernel
/// ```
///
/// ** No corrompe nada: cuelga. La maquina se queda parseando el descriptor de
/// un raton, y el unico sintoma es que BMO-X deja de arrancar cuando ese
/// aparato esta enchufado. **Un cuelgue no da autopsia**, asi que ni siquiera
/// dice quien lo causo.
///
/// [!] Y no hace falta un atacante: un aparato con el firmware roto declara
/// basura igual. La ley 11 dice que a un dispositivo se le pregunta -- **pero
/// lo que conteste sigue siendo suyo, no nuestro.**
///
/// *** 256 y no un numero mayor porque un item real no pasa de decenas: los
/// botones de un raton de juego son ocho o dieciseis, y el teclado BOOT declara
/// seis. Un tope que solo corta lo imposible no protege de nada.
const MAX_POR_ITEM: u32 = 256;

/// Estado que un item `Input` consume y que los items globales/locales llenan.
struct Estado {
    pagina: u32,
    report_size: u32,
    report_count: u32,
    report_id: u8,
    usos: [u32; MAX_USOS],
    n_usos: usize,
    uso_min: Option<u32>,
    uso_max: Option<u32>,
}

impl Estado {
    fn nuevo() -> Self {
        Self {
            pagina: 0,
            report_size: 0,
            report_count: 0,
            report_id: 0,
            usos: [0; MAX_USOS],
            n_usos: 0,
            uso_min: None,
            uso_max: None,
        }
    }

    /// El usage del elemento `k` de un item con `n` elementos.
    ///
    /// Tres formas, y las tres aparecen en ratones reales:
    /// - lista (`Usage X`, `Usage Y`): el k-esimo, y **el ultimo se repite** si
    ///   hay mas elementos que usages, que es lo que dice la especificacion.
    /// - rango (`Usage Minimum 1`, `Usage Maximum 5`): min + k. Asi declaran
    ///   los botones todos los ratones.
    /// - ninguno: relleno (`Input (Cnst)`), que ocupa bits y no significa nada.
    fn uso_de(&self, k: usize) -> Option<u32> {
        if let (Some(min), Some(max)) = (self.uso_min, self.uso_max) {
            let u = min + k as u32;
            return if u <= max { Some(u) } else { None };
        }
        if self.n_usos == 0 {
            return None;
        }
        Some(self.usos[k.min(self.n_usos - 1)])
    }

    /// Los locales se olvidan al consumir un Main item. Olvidarlos es parte de
    /// la especificacion, no una limpieza: sin esto, los usages de los botones
    /// se aplicarian tambien al relleno que va detras.
    fn olvidar_locales(&mut self) {
        self.n_usos = 0;
        self.uso_min = None;
        self.uso_max = None;
    }
}

/// **Lee el Report Descriptor y saca el formato del informe de raton.**
///
/// Devuelve el formato del primer informe que declare X e Y en la pagina
/// Generic Desktop -- que es lo que define "esto es un raton" mucho mejor que
/// el `bInterfaceProtocol`, del que ya sabemos que miente.
///
/// `None` si el descriptor no se entiende o no hay tal informe. El llamante
/// vuelve al formato BOOT y **lo dice**: un formato adivinado en silencio es
/// como se llego hasta aqui.
pub fn raton(desc: &[u8]) -> Option<Formato> {
    // Desplazamiento acumulado en bits, por Report ID. El indice 0 es "sin
    // Report ID", que es un informe distinto de todos los demas.
    let mut bits_por_id = [0u16; MAX_IDS];
    let mut e = Estado::nuevo();

    // El informe que se esta construyendo: se rellena a medida que aparecen los
    // Input items, y solo se devuelve si acaba teniendo X e Y.
    let mut id_elegido: Option<u8> = None;
    let mut botones = None;
    let mut x = None;
    let mut y = None;
    let mut rueda = None;

    let mut i = 0usize;
    while i < desc.len() {
        let prefijo = desc[i];
        i += 1;

        // Long item (0xFE): nadie los usa, pero hay que saber saltarlos o el
        // resto del descriptor se lee desalineado y sale cualquier cosa.
        if prefijo == 0xFE {
            if i + 1 >= desc.len() {
                return None;
            }
            let tam = desc[i] as usize;
            i += 2 + tam;
            continue;
        }

        let tam = match prefijo & 0x03 {
            3 => 4, // 3 significa CUATRO bytes, no tres. La trampa clasica.
            n => n as usize,
        };
        if i + tam > desc.len() {
            return None;
        }
        let mut datos = 0u32;
        for k in 0..tam {
            datos |= (desc[i + k] as u32) << (k * 8);
        }
        i += tam;

        let tipo = (prefijo >> 2) & 0x03;
        let tag = prefijo >> 4;

        match (tipo, tag) {
            // -- Global --
            (1, 0x0) => e.pagina = datos,
            (1, 0x7) => e.report_size = datos,
            (1, 0x8) => e.report_id = datos as u8,
            (1, 0x9) => e.report_count = datos,
            // -- Local --
            (2, 0x0) => {
                // Un usage de 4 bytes trae la pagina en la mitad alta.
                let u = if tam == 4 { datos & 0xFFFF } else { datos };
                if e.n_usos < MAX_USOS {
                    e.usos[e.n_usos] = u;
                    e.n_usos += 1;
                }
            }
            (2, 0x1) => e.uso_min = Some(datos),
            (2, 0x2) => e.uso_max = Some(datos),
            // -- Main: Input --
            (0, 0x8) => {
                let idx = (e.report_id as usize).min(MAX_IDS - 1);
                let constante = datos & 1 != 0;
                // ** EL TOPE, y va aqui y no al guardar `report_count`: el
                // valor crudo sigue siendo el que el aparato dijo --y eso es lo
                // que hay que poder ver si algun dia se imprime-- pero el bucle
                // no lo obedece mas alla de lo posible. Ver `MAX_POR_ITEM`.
                for k in 0..e.report_count.min(MAX_POR_ITEM) as usize {
                    let bit = bits_por_id[idx];
                    let campo = Campo { bit, bits: e.report_size.min(255) as u8 };
                    bits_por_id[idx] = bit.saturating_add(e.report_size.min(u16::MAX as u32) as u16);

                    if constante {
                        continue; // relleno: ocupa sitio y no dice nada
                    }
                    let Some(uso) = e.uso_de(k) else { continue };
                    // Solo se mira el informe del raton: el primero que declara
                    // ejes manda, y los demas Report ID se ignoran enteros.
                    let de_este_informe =
                        id_elegido.is_none() || id_elegido == Some(e.report_id);

                    match (e.pagina, uso) {
                        (PAGINA_BOTONES, _) if de_este_informe => {
                            // Los botones vienen como N campos de 1 bit
                            // seguidos. Se guardan como UN campo de N bits, que
                            // es como los usa quien lee: una mascara.
                            botones = Some(match botones {
                                Some(Campo { bit: b, bits: n })
                                    if b as u32 + n as u32 == bit as u32 =>
                                {
                                    Campo { bit: b, bits: n.saturating_add(campo.bits) }
                                }
                                Some(previo) => previo,
                                None => campo,
                            });
                        }
                        (PAGINA_GENERIC_DESKTOP, USO_X) => {
                            if id_elegido.is_none() {
                                id_elegido = Some(e.report_id);
                            }
                            if id_elegido == Some(e.report_id) {
                                x = Some(campo);
                            }
                        }
                        (PAGINA_GENERIC_DESKTOP, USO_Y) if de_este_informe => {
                            if id_elegido.is_none() {
                                id_elegido = Some(e.report_id);
                            }
                            y = Some(campo);
                        }
                        (PAGINA_GENERIC_DESKTOP, USO_RUEDA) if de_este_informe => {
                            rueda = Some(campo)
                        }
                        _ => {}
                    }
                }
                e.olvidar_locales();
            }
            // Output, Feature, Collection y End Collection: los locales se
            // olvidan igual. Sus bits NO cuentan para el informe de entrada --
            // un Output que sumara desplazamiento correria todo lo de detras.
            (0, _) => e.olvidar_locales(),
            _ => {}
        }
    }

    let id = id_elegido?;
    // Sin los dos ejes esto no es un raton, y devolver medio formato seria
    // peor que no devolver ninguno: el llamante creeria que pregunto.
    x?;
    y?;
    Some(Formato {
        report_id: id,
        botones,
        x,
        y,
        rueda,
        bits: bits_por_id[(id as usize).min(MAX_IDS - 1)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El descriptor de raton del apendice E de la especificacion HID: tres
    /// botones, X e Y de 8 bits, sin Report ID. Lo manda cualquier raton de
    /// oficina, y es el formato que el driver suponia para todos.
    const BOOT: &[u8] = &[
        0x05, 0x01, //  Usage Page (Generic Desktop)
        0x09, 0x02, //  Usage (Mouse)
        0xA1, 0x01, //  Collection (Application)
        0x09, 0x01, //    Usage (Pointer)
        0xA1, 0x00, //    Collection (Physical)
        0x05, 0x09, //      Usage Page (Button)
        0x19, 0x01, //      Usage Minimum (1)
        0x29, 0x03, //      Usage Maximum (3)
        0x15, 0x00, //      Logical Minimum (0)
        0x25, 0x01, //      Logical Maximum (1)
        0x95, 0x03, //      Report Count (3)
        0x75, 0x01, //      Report Size (1)
        0x81, 0x02, //      Input (Data,Var,Abs)
        0x95, 0x01, //      Report Count (1)
        0x75, 0x05, //      Report Size (5)
        0x81, 0x01, //      Input (Cnst)          <- relleno
        0x05, 0x01, //      Usage Page (Generic Desktop)
        0x09, 0x30, //      Usage (X)
        0x09, 0x31, //      Usage (Y)
        0x15, 0x81, //      Logical Minimum (-127)
        0x25, 0x7F, //      Logical Maximum (127)
        0x75, 0x08, //      Report Size (8)
        0x95, 0x02, //      Report Count (2)
        0x81, 0x06, //      Input (Data,Var,Rel)
        0xC0, //          End Collection
        0xC0, //        End Collection
    ];

    /// El caso que abrio el modulo: **Report ID de por medio y ejes de 16
    /// bits**, que es lo que manda un raton de juego en protocolo de informe.
    const JUEGO_16: &[u8] = &[
        0x05, 0x01, //  Usage Page (Generic Desktop)
        0x09, 0x02, //  Usage (Mouse)
        0xA1, 0x01, //  Collection (Application)
        0x85, 0x01, //    Report ID (1)
        0x09, 0x01, //    Usage (Pointer)
        0xA1, 0x00, //    Collection (Physical)
        0x05, 0x09, //      Usage Page (Button)
        0x19, 0x01, //      Usage Minimum (1)
        0x29, 0x05, //      Usage Maximum (5)
        0x15, 0x00, //      Logical Minimum (0)
        0x25, 0x01, //      Logical Maximum (1)
        0x95, 0x05, //      Report Count (5)
        0x75, 0x01, //      Report Size (1)
        0x81, 0x02, //      Input (Data,Var,Abs)
        0x95, 0x01, //      Report Count (1)
        0x75, 0x03, //      Report Size (3)
        0x81, 0x01, //      Input (Cnst)
        0x05, 0x01, //      Usage Page (Generic Desktop)
        0x09, 0x30, //      Usage (X)
        0x09, 0x31, //      Usage (Y)
        0x16, 0x00, 0x80, // Logical Minimum (-32768)
        0x26, 0xFF, 0x7F, // Logical Maximum (32767)
        0x75, 0x10, //      Report Size (16)
        0x95, 0x02, //      Report Count (2)
        0x81, 0x06, //      Input (Data,Var,Rel)
        0x09, 0x38, //      Usage (Wheel)
        0x15, 0x81, //      Logical Minimum (-127)
        0x25, 0x7F, //      Logical Maximum (127)
        0x75, 0x08, //      Report Size (8)
        0x95, 0x01, //      Report Count (1)
        0x81, 0x06, //      Input (Data,Var,Rel)
        0xC0, //          End Collection
        0xC0, //        End Collection
    ];

    #[test]
    fn el_raton_boot_sale_como_el_formato_de_siempre() {
        let f = raton(BOOT).expect("es un raton");
        assert_eq!(f.report_id, 0, "sin Report ID: no hay byte que saltarse");
        assert_eq!(f.desplazamiento(), 0);
        assert_eq!(f.botones, Some(Campo { bit: 0, bits: 3 }));
        assert_eq!(f.x, Some(Campo { bit: 8, bits: 8 }));
        assert_eq!(f.y, Some(Campo { bit: 16, bits: 8 }));
        assert!(!f.ejes_anchos());
        assert_eq!(f.bits, 24);
    }

    /// **La pregunta que abrio el modulo, contestada por el aparato.**
    ///
    /// Con este descriptor, leer `dy` en el byte 2 --como hacia el driver-- es
    /// leer la mitad ALTA de `dx`. Por eso mover en horizontal movia en
    /// vertical, y por eso salio `x=-4332` en una foto.
    #[test]
    fn el_raton_de_juego_declara_ejes_de_16_bits_detras_de_un_report_id() {
        let f = raton(JUEGO_16).expect("es un raton");
        assert_eq!(f.report_id, 1);
        assert_eq!(f.desplazamiento(), 1, "hay un byte de Report ID delante");
        assert_eq!(f.botones, Some(Campo { bit: 0, bits: 5 }));
        assert_eq!(f.x, Some(Campo { bit: 8, bits: 16 }));
        assert_eq!(f.y, Some(Campo { bit: 24, bits: 16 }));
        assert_eq!(f.rueda, Some(Campo { bit: 40, bits: 8 }));
        assert!(f.ejes_anchos());
        assert_eq!(f.bits, 48, "6 bytes de informe, mas el Report ID");
    }

    /// El relleno ocupa bits y no es un campo. Si `Input (Cnst)` se tratara
    /// como dato, los tres bits de relleno del raton de juego se llevarian los
    /// usages de los botones y X empezaria en el bit equivocado.
    #[test]
    fn el_relleno_ocupa_sitio_y_no_es_un_campo() {
        let f = raton(BOOT).unwrap();
        // 3 bits de boton + 5 de relleno = X empieza en el bit 8, no en el 3.
        assert_eq!(f.x.unwrap().bit, 8);
    }

    /// Los desplazamientos son **con signo**, y ese es el bug que un parser
    /// nuevo repite: un `dx` de 16 bits con `0xFFFF` es -1, no 65535.
    #[test]
    fn los_ejes_se_leen_con_signo() {
        let f = raton(JUEGO_16).unwrap();
        // Informe sin el Report ID: botones=0, dx=-1, dy=+300, rueda=-1
        let informe = [0x00, 0xFF, 0xFF, 0x2C, 0x01, 0xFF];
        assert_eq!(f.x.unwrap().leer_con_signo(&informe), -1);
        assert_eq!(f.y.unwrap().leer_con_signo(&informe), 300);
        assert_eq!(f.rueda.unwrap().leer_con_signo(&informe), -1);
        assert_eq!(f.botones.unwrap().leer_crudo(&informe), 0);
    }

    /// Un campo de 8 bits con el bit alto puesto es negativo. Es el caso de
    /// todos los dias en un raton boot, y el que se romperia si alguien
    /// "simplificara" la extension de signo.
    #[test]
    fn un_eje_de_8_bits_tambien_lleva_signo() {
        let f = raton(BOOT).unwrap();
        let informe = [0x01, 0xFB, 0x05]; // boton 1, dx=-5, dy=+5
        assert_eq!(f.botones.unwrap().leer_crudo(&informe), 1);
        assert_eq!(f.x.unwrap().leer_con_signo(&informe), -5);
        assert_eq!(f.y.unwrap().leer_con_signo(&informe), 5);
    }

    /// Un descriptor cortado a la mitad no puede dar un formato a medias: eso
    /// seria un formato inventado con aire de dato. Se dice que no y el
    /// llamante vuelve al BOOT.
    #[test]
    fn un_descriptor_truncado_no_da_formato() {
        assert!(raton(&JUEGO_16[..7]).is_none());
        assert!(raton(&[]).is_none());
        // Un item que promete 4 bytes de datos y no los trae.
        assert!(raton(&[0x07, 0x01]).is_none());
    }

    /// Un teclado no es un raton, aunque sea HID y este en la misma pagina.
    #[test]
    fn un_teclado_no_pasa_por_raton() {
        let teclado: &[u8] = &[
            0x05, 0x01, // Usage Page (Generic Desktop)
            0x09, 0x06, // Usage (Keyboard)
            0xA1, 0x01, // Collection (Application)
            0x05, 0x07, //   Usage Page (Keyboard)
            0x19, 0xE0, //   Usage Minimum (224)
            0x29, 0xE7, //   Usage Maximum (231)
            0x75, 0x01, //   Report Size (1)
            0x95, 0x08, //   Report Count (8)
            0x81, 0x02, //   Input (Data,Var,Abs)
            0xC0,
        ];
        assert!(raton(teclado).is_none());
    }

    /// El formato de reserva es el del protocolo BOOT, y tiene que coincidir
    /// **exactamente** con lo que saca el parser de un descriptor boot salvo en
    /// los botones (el descriptor declara 3; el protocolo reserva 8 bits).
    #[test]
    fn el_formato_de_reserva_coincide_con_el_protocolo_boot() {
        let b = Formato::boot();
        let f = raton(BOOT).unwrap();
        assert_eq!(b.report_id, f.report_id);
        assert_eq!(b.x, f.x);
        assert_eq!(b.y, f.y);
        assert_eq!(b.botones.unwrap().bit, f.botones.unwrap().bit);
    }
}

// ------------------------------------------------------------------------
//  *** DESCRIPTORES HOSTILES (auditoria 2026-08-24)
// ------------------------------------------------------------------------
//
// ** Estos no son "casos raros": son lo que llega cuando el aparato del otro
// lado no colabora. Y a un driver de USB le llega SIEMPRE antes de que nadie
// haya podido decidir si ese aparato es de fiar.

#[cfg(test)]
mod hostiles {
    use super::*;

    /// *** UN `Report Count` ENORME NO PUEDE COLGAR LA ENUMERACION.
    ///
    /// Sin tope esto son 4.294.967.295 vueltas dentro del kernel, y el sintoma
    /// es que la maquina no arranca con ese aparato enchufado. **Un cuelgue no
    /// da autopsia**: ni siquiera dice quien lo causo.
    ///
    /// El test vale por lo que TARDA: si el tope desapareciera, no falla --
    /// se queda colgado, que es exactamente el fallo que describe.
    #[test]
    fn un_report_count_imposible_no_cuelga() {
        let desc = [
            0x05, 0x01, // Usage Page (Generic Desktop)
            0x09, 0x02, // Usage (Mouse)
            0xA1, 0x01, // Collection (Application)
            0x75, 0x08, // Report Size (8)
            0x95, 0xFF, 0xFF, 0xFF, 0xFF, // ** Report Count = 0xFFFFFFFF
            0x81, 0x02, // Input (Data,Var,Abs)
            0xC0, // End Collection
        ];
        // Lo que importa es que VUELVA.
        let _ = raton(&desc);
    }

    /// Y un `Report Size` de cero con cuenta enorme: cada vuelta suma 0 bits,
    /// asi que ni siquiera el saturado de `bits_por_id` cortaria el bucle.
    #[test]
    fn tamano_cero_con_cuenta_enorme_tampoco() {
        let desc = [
            0x05, 0x01, 0x09, 0x02, 0xA1, 0x01,
            0x75, 0x00, // Report Size (0)
            0x95, 0xFF, 0xFF, 0xFF, 0xFF, // Report Count = 0xFFFFFFFF
            0x81, 0x02, 0xC0,
        ];
        let _ = raton(&desc);
    }

    /// [!] Un item largo (0xFE) que declara mas de lo que hay no puede leer
    /// fuera del descriptor.
    #[test]
    fn un_item_largo_mentiroso_no_se_sale() {
        let desc = [0xFE, 0xFF, 0x00];
        assert_eq!(raton(&desc), None);
        // Y truncado justo donde deja de haber bytes.
        assert_eq!(raton(&[0xFE]), None);
    }

    /// Un item corto que dice traer cuatro bytes de datos y trae uno.
    #[test]
    fn un_item_corto_truncado_no_se_sale() {
        // prefijo 0x07 = tipo Global, tag 0, tam 3 -> CUATRO bytes de datos.
        assert_eq!(raton(&[0x07, 0x01]), None);
    }

    /// El descriptor vacio, que es el caso que siempre falta.
    #[test]
    fn el_descriptor_vacio_contesta_que_no() {
        assert_eq!(raton(&[]), None);
    }
}
