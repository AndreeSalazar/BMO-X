//! **El plano del anillo de recepcion, y el corral donde la tarjeta puede
//! escribir.**
//!
//! # Que problema resuelve este fichero
//!
//! Programar DMA es darle a un aparato una direccion de memoria y decirle
//! *"escribe ahi"*. La tarjeta no comprueba nada: escribe donde se le mande, y
//! si la cuenta que produjo esa direccion estaba mal, **escribe en la tabla de
//! paginas, o en la pila de otra tarea, o en el propio codigo del kernel**. No
//! hay fallo, no hay excepcion. Hay un sistema que se rompe tres arranques
//! despues por algo que ya nadie relaciona.
//!
//! Esa mina ya se piso una vez en BMO-X con el PRDT de AHCI.
//!
//! *** ASI QUE LA DIRECCION NO SE CALCULA EN EL KERNEL. Se calcula aqui, donde
//! hay banco de pruebas en el anfitrion, y el kernel se limita a **copiar** lo
//! que este fichero le da despues de que [`Plan::contiene`] lo haya aprobado.
//! Aritmetica de punteros sin tests es la definicion del problema.
//!
//! # EL CORRAL: una sola reserva, y nada sale de ella
//!
//! ```text
//!    +------------------ la arena, contigua ------------------+
//!    | descriptores |          buferes de trama               |
//!    |   256 B      |          16 x 2 KiB = 32 KiB            |
//!    +--------------------------------------------------------+
//!         ^                          ^
//!         RDSAR apunta aqui          y cada descriptor a uno de estos
//! ```
//!
//! ** La tarjeta solo puede escribir donde se le haya dado una direccion. Si
//! **todas** las direcciones que se le dan estan dentro de la arena, entonces un
//! error de cuenta mio corrompe **mi propio bufer de red** -- que es un fallo
//! visible, reproducible y que no se lleva nada por delante. Eso convierte la
//! clase de bug mas cara del sistema en la mas barata.
//!
//! # [!!] Y HAY UN BIT QUE SE ESCAPA DEL CORRAL: `EOR`
//!
//! El corral acota los **buferes**, porque cada uno lleva su direccion y su
//! tamano. Pero el **anillo** se acota de otra forma: la tarjeta lo recorre
//! sola, y lo unico que le dice que vuelva al principio es el bit `EOR` del
//! ultimo descriptor.
//!
//! ```text
//!    sin EOR   la tarjeta sigue leyendo descriptores MAS ALLA del anillo,
//!              interpreta lo que haya como direcciones... y escribe alli
//! ```
//!
//! *** O sea que `EOR` es **el unico bit de todo este fichero cuyo fallo sale
//! del corral**. Por eso no se pone "en el ultimo" y ya: [`Plan::descriptor`] lo
//! decide, y hay un test que comprueba que hay **exactamente uno** en el anillo
//! entero. Un bit que puede tirar el sistema merece su propia prueba.
//!
//! # Por que 2 KiB por trama y no jumbo
//!
//! Una trama Ethernet normal son 1518 bytes con FCS. 2 KiB los cubre con sitio
//! de sobra y **cada bufer cae dentro de una pagina**, asi que ninguno cruza un
//! limite -- lo que importa el dia que esto viva detras de un IOMMU. Los jumbo
//! de 9 KiB pedirian buferes de varias paginas y el reensamblado de varios
//! descriptores por trama, que es codigo que hoy no tiene cliente.

/// Descriptores del anillo. **Es el que ya usaba el driver**, no uno nuevo.
///
/// [!] Y eso es a proposito: este fichero entra a poner un corral alrededor de
/// una aritmetica que ya funcionaba, no a redisenarla. Cambiar el tamano en el
/// mismo movimiento haria que un fallo despues no dijera cual de las dos cosas
/// lo causo. Una cosa cada vez.
pub const ANILLO: usize = crate::RX_RING_LEN;

/// Bytes por bufer de trama. Tambien el que ya habia. Ver la cabecera.
pub const BUFER: u64 = crate::RX_BUF_LEN as u64;

/// Bytes de un descriptor. Contrato de hardware, no eleccion.
pub const DESC: u64 = 16;

/// **`RDSAR` exige que la base del anillo este alineada a 256 bytes.**
///
/// No es una recomendacion del manual: los bits bajos del registro **no
/// existen**, asi que una direccion desalineada no da error -- se guarda
/// truncada, y la tarjeta recorre un anillo que empieza donde nadie puso nada.
pub const ALINEACION: u64 = 256;

/// Por que un plano no se pudo construir. Cada variante es una cosa distinta
/// que hacer, y por eso no es un `bool`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Falta {
    /// La base de la arena no esta alineada a [`ALINEACION`].
    NoAlineada,
    /// La arena no da para el anillo y sus buferes.
    Pequena,
    /// La arena se sale del espacio direccionable al sumarle su tamano.
    Desborda,
}

/// **El plano: donde cae cada cosa dentro de la arena.**
///
/// Se construye una vez y despues **solo se consulta**. No tiene metodos que
/// cambien nada a proposito: un plano que se puede modificar despues de haberse
/// validado no esta validado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Plan {
    /// Direccion **fisica** donde empieza la arena.
    base: u64,
    /// Bytes de la arena.
    bytes: u64,
}

/// Bytes que hace falta reservar. Constante, y por eso es `const fn`: quien
/// reserva puede pedir el numero exacto sin construir nada antes.
pub const fn bytes_necesarios() -> u64 {
    DESC * ANILLO as u64 + BUFER * ANILLO as u64
}

impl Plan {
    /// Comprueba la arena y devuelve el plano, o **por que no**.
    pub fn nuevo(base: u64, bytes: u64) -> Result<Plan, Falta> {
        if base % ALINEACION != 0 {
            return Err(Falta::NoAlineada);
        }
        if base.checked_add(bytes).is_none() {
            return Err(Falta::Desborda);
        }
        if bytes < bytes_necesarios() {
            return Err(Falta::Pequena);
        }
        Ok(Plan { base, bytes })
    }

    /// Donde empieza el anillo de descriptores. **Es lo que va a `RDSAR`.**
    pub fn descriptores(&self) -> u64 {
        self.base
    }

    /// Direccion fisica del bufer `i`.
    ///
    /// [!] Devuelve `None` fuera de rango en vez de calcular una direccion que
    /// no existe. Un `i` malo tiene que parar aqui: mas adelante ya es una
    /// direccion de aspecto normal en un registro de la tarjeta.
    pub fn bufer(&self, i: usize) -> Option<u64> {
        if i >= ANILLO {
            return None;
        }
        Some(self.base + DESC * ANILLO as u64 + BUFER * i as u64)
    }

    /// **Esta este trozo dentro del corral?**
    ///
    /// Es la unica pregunta que separa un error de cuenta de una corrupcion de
    /// memoria ajena, y por eso la contesta una funcion con nombre y con tests
    /// en vez de un `if` escrito en el sitio donde hacia falta.
    ///
    /// ** La suma se hace con `checked_add`: un largo enorme daria la vuelta al
    /// contador y un rango imposible pasaria por bueno. Es el fallo clasico de
    /// toda comprobacion de limites escrita deprisa.
    pub fn contiene(&self, dir: u64, largo: u64) -> bool {
        match dir.checked_add(largo) {
            None => false,
            Some(fin) => dir >= self.base && fin <= self.base + self.bytes,
        }
    }

    /// El descriptor `i` **ya listo para la tarjeta**, o `None` si `i` no existe.
    ///
    /// *** Aqui es donde se pone `EOR`, y es el unico sitio. Ver la cabecera:
    /// es el bit cuyo fallo se sale del corral.
    pub fn descriptor(&self, i: usize) -> Option<super::RxDesc> {
        let buf = self.bufer(i)?;
        // Cinturon: el bufer que acabamos de calcular tiene que caer dentro. Si
        // esto fallara, el plano estaria mal construido y la alternativa seria
        // darle a la tarjeta una direccion de fuera.
        if !self.contiene(buf, BUFER) {
            return None;
        }
        Some(super::RxDesc::to_card(buf, BUFER as u16, i == ANILLO - 1))
    }

    /// Bytes de la arena.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn plan() -> Plan {
        Plan::nuevo(0x1_0000_0000, bytes_necesarios()).unwrap()
    }

    #[test]
    fn la_cuenta_de_lo_que_hace_falta() {
        // 16 descriptores de 16 B + 16 buferes de 2 KiB.
        assert_eq!(bytes_necesarios(), 256 + 32768);
        // Y el anillo entero cabe holgadamente en el margen de 256 bytes que
        // `RDSAR` exige: 256 bytes justos, que es una pagina de sobra.
        assert_eq!(DESC * ANILLO as u64, 256);
    }

    /// Una base desalineada se rechaza **en vez de truncarse**, que es lo que
    /// haria el registro.
    #[test]
    fn una_base_desalineada_no_pasa() {
        assert_eq!(Plan::nuevo(0x1_0000_0001, 1 << 30), Err(Falta::NoAlineada));
        assert_eq!(Plan::nuevo(0x1_0000_0080, 1 << 30), Err(Falta::NoAlineada));
        assert!(Plan::nuevo(0x1_0000_0100, 1 << 30).is_ok());
    }

    #[test]
    fn una_arena_corta_no_pasa() {
        assert_eq!(Plan::nuevo(0x1000, bytes_necesarios() - 1), Err(Falta::Pequena));
        assert!(Plan::nuevo(0x1000, bytes_necesarios()).is_ok());
    }

    /// *** TODOS LOS BUFERES DENTRO, y ninguno pisa a otro. Es la propiedad
    /// entera del corral, comprobada sobre el anillo completo y no sobre una
    /// muestra.
    #[test]
    fn ningun_bufer_se_sale_ni_pisa_al_vecino() {
        let p = plan();
        let mut anterior_fin = p.descriptores() + DESC * ANILLO as u64;
        for i in 0..ANILLO {
            let b = p.bufer(i).unwrap();
            assert!(p.contiene(b, BUFER), "el bufer {} se sale del corral", i);
            assert!(b >= anterior_fin, "el bufer {} pisa lo anterior", i);
            anterior_fin = b + BUFER;
        }
        assert_eq!(p.bufer(ANILLO), None, "acepto un indice que no existe");
    }

    /// Y los descriptores no pisan al primer bufer.
    #[test]
    fn los_descriptores_no_pisan_los_buferes() {
        let p = plan();
        let fin_desc = p.descriptores() + DESC * ANILLO as u64;
        assert_eq!(p.bufer(0).unwrap(), fin_desc);
    }

    /// *** EXACTAMENTE UN `EOR` EN TODO EL ANILLO.
    ///
    /// Ninguno y la tarjeta se sale del anillo escribiendo -- el unico fallo de
    /// este fichero que no queda dentro del corral. Dos y da la vuelta antes de
    /// tiempo, que solo desperdicia descriptores. Los dos casos son bugs y solo
    /// uno se ve, asi que se cuenta.
    #[test]
    fn un_solo_eor_y_en_el_ultimo() {
        let p = plan();
        let mut cuantos = 0;
        for i in 0..ANILLO {
            let d = p.descriptor(i).unwrap();
            if d.opts1 & crate::rx::EOR != 0 {
                cuantos += 1;
                assert_eq!(i, ANILLO - 1, "el EOR esta en el descriptor {}", i);
            }
            assert!(d.owned_by_card(), "el descriptor {} nace sin OWN", i);
            assert_eq!(d.opts1 & crate::rx::LEN_MASK, BUFER as u32);
        }
        assert_eq!(cuantos, 1, "EOR aparece {} veces", cuantos);
    }

    /// La direccion del descriptor es la del bufer, partida en dos palabras.
    #[test]
    fn el_descriptor_lleva_la_direccion_del_bufer() {
        let p = plan();
        for i in [0usize, 1, ANILLO / 2, ANILLO - 1] {
            let d = p.descriptor(i).unwrap();
            let dir = ((d.addr_hi as u64) << 32) | d.addr_lo as u64;
            assert_eq!(dir, p.bufer(i).unwrap(), "descriptor {}", i);
        }
    }

    /// [!] Un largo que da la vuelta al contador **no** puede pasar por bueno.
    /// Es el fallo clasico de toda comprobacion de limites, y por eso tiene su
    /// propio caso.
    #[test]
    fn un_largo_que_desborda_se_rechaza() {
        let p = plan();
        assert!(!p.contiene(p.descriptores(), u64::MAX));
        assert!(!p.contiene(u64::MAX, 1));
    }

    /// Un byte fuera es fuera. Los limites se prueban en el limite.
    #[test]
    fn el_borde_exacto() {
        let p = plan();
        let base = p.descriptores();
        let fin = base + p.bytes();
        assert!(p.contiene(base, p.bytes()), "la arena entera tendria que caber");
        assert!(!p.contiene(base, p.bytes() + 1), "acepto un byte de mas");
        assert!(!p.contiene(base - 1, 1), "acepto un byte de menos");
        assert!(p.contiene(fin - 1, 1));
        assert!(!p.contiene(fin, 1), "acepto justo el byte siguiente");
    }
}
