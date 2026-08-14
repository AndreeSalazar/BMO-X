//! **La tabla de particiones GPT, como FORMATO.** Entran bytes, salen structs.
//!
//! # Por que vive fuera del kernel
//!
//! Salio de `ring0/dev/disk/mod.rs`, que tenia 893 lineas haciendo siete
//! trabajos. El criterio del reparto --`docs/PLAN_ALMACENAMIENTO.md`, seccion
//! 1-- es agrupar por **la pregunta que responde el fichero**, y de las siete
//! preguntas hay dos que **no tocan hardware**. Esta es una de ellas:
//!
//! ```text
//!   donde estan las cosas?   ->  MBR / GPT   ** CERO HARDWARE: es un formato
//! ```
//!
//! Leer una tabla de particiones es exactamente lo mismo que leer una cabecera
//! BEF: un formato ajeno, fijado por escrito por otra gente, del que hay una
//! respuesta correcta. Y como BEF, **admite un censo**: tablas escritas a mano
//! con la respuesta sabida de antemano, y cero discos encendidos.
//!
//! # La frontera, dicha
//!
//! Aqui NO se lee del disco. El bucle que pide sectores se queda en el kernel,
//! que es quien tiene el dispositivo y quien sabe escribir en CABINA. Esta
//! crate recibe **un sector ya leido** y contesta.
//!
//! Esa division no es estetica: es lo que hace que las pruebas de abajo puedan
//! existir. Con el bucle de lectura dentro, la unica forma de probar el parseo
//! seria arrancar la maquina.

#![no_std]

/// Bytes de un sector logico. GPT vive en sectores de 512 en esta maquina.
pub const SECTOR: usize = 512;

/// Cuantas particiones se guardan. La GPT admite 128; aqui interesan las que
/// caben en la pantalla y en el uso real de un disco de BMO.
pub const MAX_PARTS: usize = 8;

/// Una particion, tal y como la declara la tabla.
#[derive(Clone, Copy)]
pub struct Partition {
    pub index: u32,
    pub first_lba: u64,
    pub last_lba: u64,
    /// Primeros 4 bytes del GUID de tipo -- basta para distinguir las que nos
    /// importan sin arrastrar 16 bytes por todos lados.
    pub type_lo: u32,
    /// Nombre de la particion (UTF-16 en disco, aqui solo su parte ASCII).
    pub name: [u8; 36],
    pub name_len: usize,
}

impl Partition {
    pub const VACIA: Partition = Partition {
        index: 0, first_lba: 0, last_lba: 0, type_lo: 0,
        name: [0; 36], name_len: 0,
    };

    pub fn sectors(&self) -> u64 {
        self.last_lba.saturating_sub(self.first_lba) + 1
    }
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
    /// Es la particion de sistema EFI? (GUID C12A7328-...) Ahi vive el
    /// arranque de BMO.
    pub fn is_esp(&self) -> bool { self.type_lo == 0xC12A_7328 }
    /// Datos basicos de Microsoft? (GUID EBD0A0A2-...) BMO-DATA es de este
    /// tipo, y tambien lo son las particiones de Windows -- por eso el tipo
    /// NO basta para decidir donde escribir. Ver el gate de identidad.
    pub fn is_basic_data(&self) -> bool { self.type_lo == 0xEBD0_A0A2 }
}

/// Por que no se pudo leer la tabla.
///
/// Un `bool` no distingue "este disco no tiene GPT" --que es normal y puede
/// ser un disco ajeno-- de "la GPT dice una barbaridad", que es un disco roto
/// o un parser equivocado. Son dos conversaciones distintas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GptError {
    /// El sector no empieza por `EFI PART`. No hay tabla GPT aqui.
    SinFirma,
    /// El sector recibido no mide un sector.
    SectorCorto,
    /// `entry_size` fuera de lo que la especificacion permite (>= 128 y que
    /// quepa en un sector). Con un tamano inventado, el recorrido de entradas
    /// leeria en diagonal y devolveria particiones que no existen.
    TamanoDeEntradaAbsurdo(u32),
}

impl GptError {
    pub fn name(self) -> &'static str {
        match self {
            GptError::SinFirma => "el disco no tiene tabla GPT",
            GptError::SectorCorto => "el sector no mide 512 bytes",
            GptError::TamanoDeEntradaAbsurdo(_) => "tamano de entrada GPT inesperado",
        }
    }
}

/// Lo que la cabecera GPT dice sobre donde estan las entradas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gpt {
    /// Ultimo LBA utilizable del disco, segun la tabla.
    pub last_lba: u64,
    pub entries_lba: u64,
    pub entry_count: u32,
    pub entry_size: u32,
}

impl Gpt {
    /// Cuantas entradas caben en un sector.
    pub fn por_sector(&self) -> usize {
        SECTOR / self.entry_size as usize
    }
}

/// **Lee la cabecera GPT del LBA 1.**
pub fn cabecera(sec: &[u8]) -> Result<Gpt, GptError> {
    if sec.len() < SECTOR {
        return Err(GptError::SectorCorto);
    }
    if &sec[0..8] != b"EFI PART" {
        return Err(GptError::SinFirma);
    }
    let entry_size = le32(sec, 84);
    // El limite superior importa tanto como el inferior: `por_sector` divide
    // por este numero y el recorrido lo usa como paso.
    if entry_size < 128 || entry_size as usize > SECTOR {
        return Err(GptError::TamanoDeEntradaAbsurdo(entry_size));
    }
    Ok(Gpt {
        last_lba: le64(sec, 48),
        entries_lba: le64(sec, 72),
        entry_count: le32(sec, 80),
        entry_size,
    })
}

/// **Una entrada de la tabla**, o `None` si ese hueco esta libre.
///
/// `off` es el desplazamiento de la entrada DENTRO del sector, e `indice` el
/// numero de particion que le toca (1 para la primera).
pub fn entrada(sec: &[u8], off: usize, indice: u32) -> Option<Partition> {
    if off + 128 > sec.len() {
        return None;
    }
    let type_lo = le32(sec, off);
    // Una entrada con GUID de tipo todo ceros es un hueco.
    let vacia = type_lo == 0
        && le32(sec, off + 4) == 0
        && le32(sec, off + 8) == 0
        && le32(sec, off + 12) == 0;
    if vacia {
        return None;
    }
    let mut p = Partition {
        index: indice,
        first_lba: le64(sec, off + 32),
        last_lba: le64(sec, off + 40),
        type_lo,
        name: [0; 36],
        name_len: 0,
    };
    // El nombre son 36 unidades UTF-16LE. Aqui solo se conserva lo
    // representable en ASCII: el font es de un byte y el objetivo es reconocer
    // "BMO", no renderizar cualquier idioma.
    let mut n = 0usize;
    for k in 0..36 {
        let lo = sec[off + 56 + k * 2];
        let hi = sec[off + 56 + k * 2 + 1];
        if lo == 0 && hi == 0 {
            break;
        }
        if hi == 0 && (0x20..0x7F).contains(&lo) {
            p.name[n] = lo;
            n += 1;
        }
    }
    p.name_len = n;
    Some(p)
}

pub fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

pub fn le64(b: &[u8], o: usize) -> u64 {
    let mut v = 0u64;
    for i in (0..8).rev() {
        v = (v << 8) | b[o + i] as u64;
    }
    v
}

// ============================================================================
// EL CENSO
// ============================================================================
//
// ** Tablas escritas A MANO, con la respuesta sabida antes de compilar. Es el
// mismo metodo que los censos de BMO C: las filas no salen de leer el parser
// --eso mide lo que el parser cree-- sino de la ESPECIFICACION de GPT, que es
// autoridad de fuera.
//
// Esto es lo que el paso 1 del plan compra: con el bucle de lectura dentro del
// kernel, ninguna de estas cinco casillas se podia escribir sin encender la
// maquina.

#[cfg(test)]
mod censo {
    use super::*;

    /// Un sector de cabecera GPT valido, con los campos puestos a mano.
    fn cabecera_buena() -> [u8; SECTOR] {
        let mut s = [0u8; SECTOR];
        s[0..8].copy_from_slice(b"EFI PART");
        s[48..56].copy_from_slice(&1_000_000u64.to_le_bytes()); // last_lba
        s[72..80].copy_from_slice(&2u64.to_le_bytes()); // entries_lba
        s[80..84].copy_from_slice(&128u32.to_le_bytes()); // entry_count
        s[84..88].copy_from_slice(&128u32.to_le_bytes()); // entry_size
        s
    }

    /// Una entrada con tipo, rango y nombre.
    fn entrada_en(s: &mut [u8], off: usize, tipo: u32, primero: u64, ultimo: u64, nombre: &str) {
        s[off..off + 4].copy_from_slice(&tipo.to_le_bytes());
        s[off + 8..off + 12].copy_from_slice(&0x1111_1111u32.to_le_bytes()); // GUID unico
        s[off + 32..off + 40].copy_from_slice(&primero.to_le_bytes());
        s[off + 40..off + 48].copy_from_slice(&ultimo.to_le_bytes());
        for (k, c) in nombre.bytes().enumerate() {
            s[off + 56 + k * 2] = c;
            s[off + 56 + k * 2 + 1] = 0;
        }
    }

    #[test]
    fn una_cabecera_valida_da_sus_cuatro_numeros() {
        let g = cabecera(&cabecera_buena()).expect("tiene firma y tamano legal");
        assert_eq!(g.last_lba, 1_000_000);
        assert_eq!(g.entries_lba, 2);
        assert_eq!(g.entry_count, 128);
        assert_eq!(g.entry_size, 128);
        assert_eq!(g.por_sector(), 4);
    }

    #[test]
    fn sin_la_firma_se_dice_que_no_hay_gpt_y_no_es_un_error_de_disco() {
        let mut s = cabecera_buena();
        s[0] = b'X';
        assert_eq!(cabecera(&s), Err(GptError::SinFirma));
    }

    /// ** La casilla que justifica que `entry_size` se valide.
    ///
    /// Con un tamano de 7, `por_sector()` daria 73 y el recorrido leeria las
    /// entradas EN DIAGONAL: cada una empezaria a mitad de la anterior y
    /// saldrian particiones que no existen, con rangos inventados. Un disco
    /// con basura en ese campo se convertiria en un mapa de un disco que no es.
    #[test]
    fn un_tamano_de_entrada_absurdo_se_rechaza_en_vez_de_leer_en_diagonal() {
        let mut s = cabecera_buena();
        s[84..88].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(cabecera(&s), Err(GptError::TamanoDeEntradaAbsurdo(7)));

        // Y por arriba tambien: 4096 no cabe en un sector de 512.
        let mut g = cabecera_buena();
        g[84..88].copy_from_slice(&4096u32.to_le_bytes());
        assert_eq!(cabecera(&g), Err(GptError::TamanoDeEntradaAbsurdo(4096)));
    }

    #[test]
    fn un_hueco_no_es_una_particion() {
        let s = [0u8; SECTOR];
        assert!(entrada(&s, 0, 1).is_none(), "GUID de tipo a cero es un hueco");
    }

    #[test]
    fn la_esp_y_los_datos_se_reconocen_por_su_tipo() {
        let mut s = [0u8; SECTOR];
        entrada_en(&mut s, 0, 0xC12A_7328, 2048, 206_847, "EFI");
        entrada_en(&mut s, 128, 0xEBD0_A0A2, 206_848, 1_000_000, "BMO-DATA");

        let esp = entrada(&s, 0, 1).expect("la primera entrada existe");
        assert!(esp.is_esp(), "C12A7328 es la particion de sistema EFI");
        assert!(!esp.is_basic_data());
        assert_eq!(esp.name_str(), "EFI");
        assert_eq!(esp.index, 1);
        assert_eq!(esp.sectors(), 206_847 - 2048 + 1);

        let datos = entrada(&s, 128, 2).expect("la segunda entrada existe");
        assert!(datos.is_basic_data());
        assert_eq!(datos.name_str(), "BMO-DATA");
        assert_eq!(datos.first_lba, 206_848);
    }

    /// El nombre en disco es UTF-16LE y el font de BMO es de un byte. Lo que
    /// no sea ASCII imprimible **se salta**, y no corta el nombre: cortar
    /// convertiria "BMO-DATA" en "BMO" el dia que alguien meta un acento.
    #[test]
    fn el_nombre_se_queda_con_lo_ascii_y_no_corta_en_el_primer_raro() {
        let mut s = [0u8; SECTOR];
        entrada_en(&mut s, 0, 0xEBD0_A0A2, 1, 2, "AB");
        // Una unidad UTF-16 fuera de ASCII en medio, y otra letra detras.
        s[0 + 56 + 2 * 2] = 0xE9;
        s[0 + 56 + 2 * 2 + 1] = 0x00;
        s[0 + 56 + 3 * 2] = b'C';
        s[0 + 56 + 3 * 2 + 1] = 0x00;

        let p = entrada(&s, 0, 1).unwrap();
        assert_eq!(p.name_str(), "ABC", "el byte raro se salta, la C sobrevive");
    }

    #[test]
    fn un_sector_corto_no_se_parsea_a_medias() {
        let s = [0u8; 64];
        assert_eq!(cabecera(&s), Err(GptError::SectorCorto));
        assert!(entrada(&s, 0, 1).is_none());
    }
}
