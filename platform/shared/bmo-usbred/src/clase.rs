//! **QUE ES una interfaz USB**, por su clase, subclase y protocolo.
//!
//! [carril]  VERDE     una tabla: no toca nada
//! [cuesta]  NADA      una comparacion
//! [riesgo]  SILENCIO  un aparato mal nombrado manda a buscar el driver equivocado
//!
//! El portero ya apuntaba estos tres numeros de CADA interfaz; esto les pone
//! nombre. Un movil en anclaje por USB suele traer DOS interfaces: la de control
//! (la que dice RNDIS o NCM) y la de datos (`0x0A`), y a veces MTP y ADB al lado.

/// Lo que es, dicho con una palabra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tipo {
    /// Red por USB de Microsoft: el anclaje de casi todos los Android.
    Rndis,
    /// Red por USB estandar (CDC NCM).
    Ncm,
    /// Red por USB estandar antigua (CDC ECM).
    Ecm,
    /// La mitad de DATOS de una red CDC: las tramas van por aqui.
    DatosCdc,
    /// Transferir fotos y ficheros de un movil (MTP / PTP).
    Mtp,
    /// Depuracion de Android.
    Adb,
    /// Un disco USB.
    Almacenamiento,
    Hid,
    Audio,
    Video,
    Hub,
    /// Algo que el fabricante no quiso decir (clase `0xFF`).
    DelFabricante,
    Otro,
}

impl Tipo {
    /// Es una de las formas en que llega una red por USB?
    pub fn es_red(self) -> bool {
        matches!(self, Tipo::Rndis | Tipo::Ncm | Tipo::Ecm)
    }

    /// En palabras, para quien mira CABINA con la maquina delante.
    pub fn texto(self) -> &'static str {
        match self {
            Tipo::Rndis => "llego una RED POR USB (RNDIS: anclaje de un movil) -- hoy sin driver: falta BULK en el xHCI",
            Tipo::Ncm => "llego una RED POR USB (NCM) -- hoy sin driver: falta BULK en el xHCI",
            Tipo::Ecm => "llego una RED POR USB (ECM) -- hoy sin driver: falta BULK en el xHCI",
            Tipo::DatosCdc => "llego la mitad de DATOS de una red o serie USB",
            Tipo::Mtp => "llego un MOVIL en modo archivos (MTP): no hay codigo para su clase",
            Tipo::Adb => "llego la depuracion de Android (ADB): no se adopta",
            Tipo::Almacenamiento => "llego un DISCO USB: no hay codigo para su clase",
            Tipo::Hid => "llego un HID",
            Tipo::Audio => "llego AUDIO USB",
            Tipo::Video => "llego VIDEO USB (camara): no hay codigo para su clase",
            Tipo::Hub => "llego un HUB",
            Tipo::DelFabricante => "llego algo del fabricante (clase 0xFF): no hay codigo para su clase",
            Tipo::Otro => "llego algo que NO ES HID: no hay codigo para su clase",
        }
    }

    /// El nombre corto, para una tabla: cabe en una columna y no explica.
    /// (`save` lo pone al lado de cada puerto, 2026-09-17.)
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Rndis => "red RNDIS",
            Tipo::Ncm => "red NCM",
            Tipo::Ecm => "red ECM",
            Tipo::DatosCdc => "datos CDC",
            Tipo::Mtp => "movil MTP",
            Tipo::Adb => "ADB",
            Tipo::Almacenamiento => "disco",
            Tipo::Hid => "HID",
            Tipo::Audio => "audio",
            Tipo::Video => "video",
            Tipo::Hub => "hub",
            Tipo::DelFabricante => "fabricante",
            Tipo::Otro => "otro",
        }
    }
}

/// **Que es**, por los tres numeros del descriptor de interfaz.
pub fn que_es(clase: u8, subclase: u8, protocolo: u8) -> Tipo {
    match (clase, subclase, protocolo) {
        // RNDIS se anuncia de tres maneras segun el movil: como "controlador
        // inalambrico", como "miscelaneo", o como CDC con protocolo del fabricante.
        (0xE0, 0x01, 0x03) | (0xEF, 0x04, 0x01) | (0x02, 0x02, 0xFF) => Tipo::Rndis,
        (0x02, 0x0D, _) => Tipo::Ncm,
        (0x02, 0x06, _) => Tipo::Ecm,
        (0x0A, _, _) => Tipo::DatosCdc,
        (0x06, 0x01, 0x01) => Tipo::Mtp,
        (0xFF, 0x42, 0x01) => Tipo::Adb,
        (0xFF, 0xFF, 0x00) => Tipo::Mtp,
        (0x08, _, _) => Tipo::Almacenamiento,
        (0x03, _, _) => Tipo::Hid,
        (0x01, _, _) => Tipo::Audio,
        (0x0E, _, _) => Tipo::Video,
        (0x09, _, _) => Tipo::Hub,
        (0xFF, _, _) => Tipo::DelFabricante,
        _ => Tipo::Otro,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_tres_caras_de_rndis() {
        for (c, s, p) in [(0xE0, 0x01, 0x03), (0xEF, 0x04, 0x01), (0x02, 0x02, 0xFF)] {
            assert_eq!(que_es(c, s, p), Tipo::Rndis, "{c:02X}/{s:02X}/{p:02X}");
            assert!(que_es(c, s, p).es_red());
        }
    }

    #[test]
    fn un_movil_tipico_se_nombra_entero() {
        assert_eq!(que_es(0x02, 0x0D, 0x00), Tipo::Ncm);
        assert_eq!(que_es(0x0A, 0x00, 0x00), Tipo::DatosCdc);
        assert_eq!(que_es(0x06, 0x01, 0x01), Tipo::Mtp);
        assert_eq!(que_es(0xFF, 0x42, 0x01), Tipo::Adb);
        assert!(!que_es(0x0A, 0x00, 0x00).es_red(), "los datos solos no son una red");
    }

    #[test]
    fn un_modem_serie_no_es_rndis() {
        assert_eq!(que_es(0x02, 0x02, 0x01), Tipo::Otro, "CDC ACM con AT: un modem, no RNDIS");
        assert_eq!(que_es(0x03, 0x01, 0x01), Tipo::Hid);
        assert_eq!(que_es(0xFF, 0x00, 0x00), Tipo::DelFabricante);
    }

    #[test]
    fn todo_tiene_texto() {
        for c in 0..=255u8 {
            assert!(!que_es(c, 0, 0).texto().is_empty());
        }
    }
}
