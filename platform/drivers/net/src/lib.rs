//! **RED -- de momento, solo RECONOCER.**
//!
//! ## Que habia aqui antes, y por que se borro
//!
//! 287 lineas de driver de **Intel e1000** que no llamaba nadie. La NIC de esta
//! maquina es `PCI\VEN_10EC&DEV_8168` --una Realtek RTL8111/8168-- y el e1000 es
//! la NIC por defecto de **QEMU**: aquel codigo no habria encendido un LED en el
//! Ryzen. Sigue en el historial (`git log -- platform/drivers/net`).
//!
//! ## El contrato que va a ocupar este sitio
//!
//! Se escribe antes que el driver, que es la regla de la casa:
//!
//! ```text
//!    Ring 0                          Ring 3
//!    ------                          ------
//!    KIND_RED                        ARP, IP, TCP, DNS, TLS
//!    tramas Ethernet crudas          todo lo que tiene versiones
//!    la MAC, el enlace, el DMA       y por tanto se equivoca
//! ```
//!
//! **El kernel no sabe lo que es una IP.** Una pila TCP es la superficie de
//! ataque mas grande de un sistema conectado, y aqui se puede morir sin llevarse
//! la maquina. Windows y Linux la tienen dentro del nucleo porque en 1990 no
//! habia otra forma.
//!
//! ## ** Y por que HOY esto solo mira ==
//!
//! Un anillo de descriptores mal armado no da un fallo: da **el disco de red
//! escribiendo en memoria de otro**, y el sintoma tres arranques despues. Antes
//! de programar un solo DMA hay que contestar tres preguntas que no cuestan
//! nada y que ninguna teoria puede contestar:
//!
//! 1. La tarjeta que encuentra el barrido, **es la que dice Windows**?
//! 2. El BAR que se eligio, **es el que lleva a los registros**?
//! 3. El cable, **esta enchufado**?
//!
//! Las tres se responden con lecturas, sin escribir un byte en el aparato. Y
//! hay algo mejor: **se pueden predecir antes de mirar**. El Windows de esta
//! misma maquina dice `2C-F0-5D-D9-3C-E3`, enlace arriba a 100 Mbps. Si el
//! kernel imprime eso, las tres estan contestadas de golpe y el driver se
//! empieza sobre suelo firme. Si imprime otra cosa, **el numero dice cual de las
//! tres fallo**, que es justo lo que un `no funciona` no dice.
//!
//! Es el metodo de las cinco sondas del `#GP` de julio: predecir, leer, comparar.

// `no_std` en el kernel, `std` en el banco: la interpretacion de los bits SI se
// puede probar en el anfitrion, y es justo la parte que se equivoca en silencio.
#![cfg_attr(not(test), no_std)]

/// Seis bytes. El nombre existe para que una firma no diga `[u8; 6]` y deje al
/// que lee adivinando si son bytes de MAC o de otra cosa.
pub type Mac = [u8; 6];

/// **Lo que un RTL8111/8168 cuenta de si mismo sin que se le configure nada.**
#[derive(Clone, Copy)]
pub struct Identidad {
    /// La direccion fisica, tal como el chip la cargo de su EEPROM al arrancar.
    pub mac: Mac,
    /// El registro `PHYstatus` crudo. Se guarda **sin interpretar** ademas de
    /// interpretado: el dia que un bit no cuadre, el byte entero es la prueba y
    /// las funciones de abajo son la opinion.
    pub phy: u8,
}

/// Registros del RTL8169/8168 que hacen falta para mirar. Del mapa de la
/// familia, el mismo que usa el `r8169` de Linux.
mod reg {
    /// `IDR0..IDR5`: la MAC, cargada de la EEPROM en el reset. Es de solo
    /// lectura hasta que se desbloquea con `9346CR`, y aqui **no se desbloquea
    /// nada**: se lee y se sale.
    pub const IDR0: usize = 0x00;
    /// `PHYstatus`. Un byte, y cuenta el enlace entero.
    pub const PHY_STATUS: usize = 0x6C;
}

/// Bits de `PHYstatus`.
mod phy {
    pub const LINK_UP: u8 = 0x02;
    pub const FULL_DUPLEX: u8 = 0x01;
    pub const M10: u8 = 0x04;
    pub const M100: u8 = 0x08;
    pub const M1000: u8 = 0x10;
}

/// **Preguntale al chip quien es.** No escribe **nada**.
///
/// `mmio` es la base del BAR de memoria, ya en direccion virtual alcanzable.
///
/// # Safety
/// `mmio` tiene que ser el MMIO de una Realtek de la familia 8169/8168 y estar
/// mapeado. Leer estos offsets en otro aparato devuelve lo que haya ahi -- que
/// es exactamente por que quien llama comprueba el vendor **antes**.
pub unsafe fn identificar(mmio: *mut u8) -> Identidad {
    // Dos lecturas de 32 bits en vez de seis de 8.
    //
    // Los `IDR` admiten acceso por byte, pero varios registros de esta familia
    // NO -- y un MMIO al que se le lee un byte donde pide una palabra puede
    // devolver basura sin que nada falle. Dos dwords cubren los seis bytes y
    // dejan la costumbre puesta para los registros que si son quisquillosos.
    let base = mmio as *const u32;
    let lo = core::ptr::read_volatile(base.add(reg::IDR0 / 4));
    let hi = core::ptr::read_volatile(base.add(reg::IDR0 / 4 + 1));
    let mac: Mac = [
        (lo & 0xFF) as u8,
        ((lo >> 8) & 0xFF) as u8,
        ((lo >> 16) & 0xFF) as u8,
        ((lo >> 24) & 0xFF) as u8,
        (hi & 0xFF) as u8,
        ((hi >> 8) & 0xFF) as u8,
    ];
    let phy = core::ptr::read_volatile(mmio.add(reg::PHY_STATUS));
    Identidad { mac, phy }
}

impl Identidad {
    /// Hay cable, y del otro lado contesta alguien?
    pub fn enlace_arriba(&self) -> bool {
        self.phy & phy::LINK_UP != 0
    }

    /// A cuantos megabits negocio. `0` = sin enlace, o el chip no lo dice.
    ///
    /// ** No se inventa un valor por defecto. Un `1000` supuesto en un enlace
    /// que negocio a 100 no da un error: da un sistema que cree ir diez veces
    /// mas rapido de lo que va, y eso sale como paquetes perdidos mucho despues.
    pub fn megabits(&self) -> u16 {
        if !self.enlace_arriba() {
            return 0;
        }
        match self.phy {
            p if p & phy::M1000 != 0 => 1000,
            p if p & phy::M100 != 0 => 100,
            p if p & phy::M10 != 0 => 10,
            _ => 0,
        }
    }

    /// Duplex completo? En un enlace moderno siempre, y por eso mismo un `false`
    /// aqui es una pista de que se esta leyendo el registro equivocado.
    pub fn duplex_completo(&self) -> bool {
        self.phy & phy::FULL_DUPLEX != 0
    }

    /// **La MAC como un solo numero, para poder pintarla.**
    ///
    /// El byte 0 arriba del todo, asi que en hexadecimal sale en el mismo orden
    /// en que se escribe: `2C:F0:5D:D9:3C:E3` -> `2CF05DD93CE3`. Es lo que
    /// permite comparar de un vistazo con lo que dice cualquier otro sistema, y
    /// esa comparacion es toda la prueba de este paso.
    pub fn mac_u64(&self) -> u64 {
        let m = &self.mac;
        ((m[0] as u64) << 40)
            | ((m[1] as u64) << 32)
            | ((m[2] as u64) << 24)
            | ((m[3] as u64) << 16)
            | ((m[4] as u64) << 8)
            | (m[5] as u64)
    }

    /// **Es una MAC creible?** Ni todo ceros ni todo unos.
    ///
    /// Los dos son lo que devuelve un MMIO que no lleva a ningun sitio: ceros si
    /// la lectura cae en un agujero, `FF` si el aparato no contesta al ciclo.
    /// O sea que esto no comprueba la tarjeta -- **comprueba el BAR**, y es la
    /// diferencia entre "la NIC esta rota" y "le estoy leyendo el sitio que no".
    ///
    /// Tampoco puede ser multicast: el bit 0 del primer byte puesto significa
    /// "grupo", y ninguna tarjeta tiene de fabrica una direccion de grupo.
    pub fn creible(&self) -> bool {
        let ceros = self.mac.iter().all(|&b| b == 0x00);
        let unos = self.mac.iter().all(|&b| b == 0xFF);
        !ceros && !unos && (self.mac[0] & 0x01) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un bloque de registros de mentira, **alineado**. Sin la alineacion, leer
    /// un `u32` de un `[u8]` cualquiera es comportamiento indefinido en la
    /// prueba y correcto en el kernel -- que es la peor combinacion posible.
    #[repr(C, align(4))]
    struct Registros([u8; 256]);

    /// La tarjeta de esta maquina, segun su otro sistema operativo. La prueba
    /// lleva el numero real a proposito: **es la prediccion contra la que se va
    /// a comparar la foto del arranque**.
    const MAC_DEL_RYZEN: [u8; 6] = [0x2C, 0xF0, 0x5D, 0xD9, 0x3C, 0xE3];

    fn con(mac: [u8; 6], phy: u8) -> Identidad {
        let mut r = Registros([0u8; 256]);
        r.0[0..6].copy_from_slice(&mac);
        r.0[0x6C] = phy;
        unsafe { identificar(r.0.as_mut_ptr()) }
    }

    /// ** EL ORDEN DE LOS BYTES, que es lo unico que aqui se puede torcer sin
    /// que nada falle.
    ///
    /// La MAC sale de **dos lecturas de 32 bits**, y en little-endian el byte 0
    /// del registro es el byte BAJO de la palabra. Equivocarse invierte la
    /// direccion, y una MAC invertida es perfectamente creible: seis bytes que
    /// no son ceros ni unos. Se imprimiria, se compararia con Windows, no
    /// cuadraria, y el sospechoso seria el BAR -- que estaria bien.
    #[test]
    fn la_mac_sale_en_el_orden_en_que_se_escribe() {
        let id = con(MAC_DEL_RYZEN, 0);
        assert_eq!(id.mac, MAC_DEL_RYZEN, "los seis bytes no salen en orden");
        assert_eq!(
            id.mac_u64(),
            0x2CF0_5DD9_3CE3,
            "en hexadecimal tiene que leerse igual que se escribe: 2C:F0:5D:D9:3C:E3"
        );
    }

    /// ** CREIBLE COMPRUEBA EL BAR, NO LA TARJETA.
    ///
    /// Ceros y unos son lo que devuelve un MMIO que no lleva a ningun sitio: los
    /// primeros si la lectura cae en un agujero, los segundos si nadie contesta
    /// al ciclo de bus. Confundir eso con "la NIC esta rota" manda a cambiar de
    /// tarjeta cuando lo que hay que cambiar es un indice de BAR.
    #[test]
    fn una_mac_imposible_delata_el_bar_y_no_la_tarjeta() {
        assert!(!con([0x00; 6], 0).creible(), "todo ceros: el MMIO no lleva a los registros");
        assert!(!con([0xFF; 6], 0).creible(), "todo unos: el aparato no contesta");
        // Bit 0 del primer byte = direccion de GRUPO. Ninguna tarjeta trae de
        // fabrica una MAC multicast, asi que si sale una, se leyo otra cosa.
        assert!(!con([0x01, 0x00, 0x5E, 0x11, 0x22, 0x33], 0).creible(), "una MAC multicast no es de fabrica");
        assert!(con(MAC_DEL_RYZEN, 0).creible(), "y la de verdad si es creible");
    }

    /// `PHYstatus` traducido. El valor que se espera del Ryzen es `0x0B`
    /// --enlace + 100 Mbps + duplex completo-- porque su Windows dice
    /// exactamente eso: `Up`, `100 Mbps`.
    #[test]
    fn el_enlace_se_lee_del_phystatus() {
        let cien = con(MAC_DEL_RYZEN, 0x0B);
        assert!(cien.enlace_arriba());
        assert_eq!(cien.megabits(), 100, "0x0B es enlace + 100M + full");
        assert!(cien.duplex_completo());

        assert_eq!(con(MAC_DEL_RYZEN, 0x13).megabits(), 1000, "0x13 lleva el bit de giga");
        assert_eq!(con(MAC_DEL_RYZEN, 0x07).megabits(), 10, "0x07 es 10M");
    }

    /// ** SIN CABLE, CERO MEGABITS -- y no el ultimo valor que hubiera.
    ///
    /// Un `1000` supuesto en un enlace caido no da un error: da un sistema que
    /// cree ir diez veces mas rapido de lo que va, y eso sale como paquetes
    /// perdidos mucho despues y muy lejos de aqui. Es el patron del cero
    /// silencioso, en el sitio donde parece inofensivo.
    #[test]
    fn sin_enlace_no_se_inventa_velocidad() {
        let sin = con(MAC_DEL_RYZEN, 0x00);
        assert!(!sin.enlace_arriba());
        assert_eq!(sin.megabits(), 0, "sin enlace la velocidad es 0, no la que hubiera");
        // Y aunque el chip deje puesto el bit de 1000 con el enlace caido --que
        // pasa al desenchufar-- sigue siendo 0: manda el enlace.
        assert_eq!(con(MAC_DEL_RYZEN, 0x10).megabits(), 0, "el bit de velocidad sin enlace no vale");
    }
}
