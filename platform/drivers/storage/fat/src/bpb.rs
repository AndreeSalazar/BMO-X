//! **QUE VOLUMEN ES ESTE?** -- el sector de arranque como FORMATO.
//!
//! Entra un sector ya leido, sale una [`Geometria`] o un motivo por el que no.
//! Aqui no se lee del disco: el bucle que pide sectores vive arriba, igual que
//! en `bmo-particiones`. Esa frontera no es estetica -- es lo que permite que
//! el censo del final de este fichero exista sin encender una maquina.
//!
//! # El fallo que este fichero mata
//!
//! `bmo-fat32` aceptaba un volumen si `BS_BootSig` valia 0x29 o 0x28. **FAT16
//! tambien vale 0x29.** A partir de ahi leia `BPB_FATSz32` y `BPB_RootClus`,
//! que en un FAT16 caen encima de `BS_VolID` y de la etiqueta -- o sea que
//! montaba un volumen apuntando a cualquier parte, y en silencio. Cualquier
//! pendrive pequeno sale FAT16 de fabrica, y UEFI los admite como ESP.
//!
//! La unica forma valida de decidir el tipo es **contar los clusters**. No es
//! una heuristica entre varias: es la definicion, y UEFI la repite prohibiendo
//! explicitamente fiarse de la cadena `BS_FilSysType`. Esta en [`identificar`].
//!
//! # Y por que no hay un `#[repr(C, packed)]` aqui
//!
//! El codigo de antes hacia `unsafe { &*(buf.as_ptr() as *const FatBpb) }`.
//! Eso da por supuesta la alineacion del buffer y el orden de bytes de la
//! maquina, y a cambio no ahorra nada: los campos se leen una vez al montar.
//! Aqui se leen byte a byte en little-endian explicito y la crate puede
//! declarar `forbid(unsafe_code)`.

use core::cmp::max;

/// Cual de los tres. Lo decide la cuenta de clusters y **nada mas**.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tipo {
    Fat12,
    Fat16,
    Fat32,
}

impl Tipo {
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Fat12 => "FAT12",
            Tipo::Fat16 => "FAT16",
            Tipo::Fat32 => "FAT32",
        }
    }

    /// Bits por entrada de la FAT. Lo necesita `tabla.rs` para saber cuanto
    /// leer, y sale del tipo -- que sale de la cuenta de clusters.
    pub fn bits_por_entrada(self) -> u32 {
        match self {
            Tipo::Fat12 => 12,
            Tipo::Fat16 => 16,
            Tipo::Fat32 => 32,
        }
    }
}

/// Donde vive el directorio raiz, que es lo unico que FAT32 hace distinto de
/// verdad: en FAT12/16 es una region de tamano fijo detras de la FAT, y en
/// FAT32 es una cadena de clusters como cualquier otro directorio.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Raiz {
    /// FAT12/16: una region contigua. `sector` es relativo a la particion.
    Region { sector: u32, sectores: u32 },
    /// FAT32: el primer cluster de la cadena.
    Cadena { cluster: u32 },
}

/// Por que este sector NO describe un volumen FAT.
///
/// Cada variante es una comprobacion distinta a proposito. Un `Option::None`
/// obligaba a quien monta a decir "no es FAT" sin poder distinguir un disco
/// vacio de una tabla corrupta, y son dos conversaciones distintas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoEs {
    /// El buffer no llega a 512 bytes: no se puede ni mirar la firma.
    Corto,
    /// Falta `0xAA55` en el offset 510.
    SinFirma,
    /// `BPB_BytsPerSec` no es 512, 1024, 2048 ni 4096.
    SectorImposible,
    /// `BPB_SecPerClus` no es una potencia de dos entre 1 y 128.
    ClusterImposible,
    /// `BPB_RsvdSecCnt` es 0. La FAT empezaria encima del propio BPB.
    SinReservados,
    /// `BPB_NumFATs` es 0. No hay tabla.
    SinFats,
    /// `FATSz` es 0 en las dos anchuras.
    SinTamanoFat,
    /// `TotSec` es 0 en las dos anchuras.
    SinTotal,
    /// Las regiones declaradas no caben dentro del volumen declarado.
    NoCabe,
    /// `BPB_FSVer` no es 0x0000. La especificacion dice **no montar**: es una
    /// version futura cuyo significado no se conoce.
    VersionDesconocida,
    /// FAT32 con `BPB_RootClus` fuera del rango de clusters validos, o
    /// FAT12/16 cuyo directorio raiz no cuadra con el tamano del sector.
    RaizImposible,
}

impl NoEs {
    pub fn nombre(self) -> &'static str {
        match self {
            NoEs::Corto => "el buffer no llega a un sector",
            NoEs::SinFirma => "no lleva la firma 0xAA55",
            NoEs::SectorImposible => "el tamano de sector no es 512/1024/2048/4096",
            NoEs::ClusterImposible => "el tamano de cluster no es potencia de dos entre 1 y 128",
            NoEs::SinReservados => "declara cero sectores reservados",
            NoEs::SinFats => "declara cero FATs",
            NoEs::SinTamanoFat => "declara una FAT de cero sectores",
            NoEs::SinTotal => "declara cero sectores en total",
            NoEs::NoCabe => "las regiones no caben en el volumen que declara",
            NoEs::VersionDesconocida => "es una version de FAT32 que no se conoce",
            NoEs::RaizImposible => "el directorio raiz que declara no existe",
        }
    }
}

/// Todo lo que hay que saber del volumen para hablar con el, ya calculado.
///
/// Los campos son valores DERIVADOS y no una copia del BPB: quien monta no
/// deberia volver a hacer la resta de las regiones ni acordarse de que en
/// FAT32 `RootDirSectors` es cero. La cuenta se hace una vez, aqui.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Geometria {
    pub tipo: Tipo,
    pub bytes_por_sector: u32,
    pub sectores_por_cluster: u32,
    /// `BPB_RsvdSecCnt`. Tambien es el primer sector de la FAT #0.
    pub reservados: u32,
    pub num_fats: u32,
    /// `FATSz`, ya elegido entre la anchura de 16 y la de 32.
    pub sectores_por_fat: u32,
    /// `TotSec`, ya elegido entre las dos anchuras.
    pub total_sectores: u32,
    /// Sectores del directorio raiz de FAT12/16. Cero en FAT32.
    pub sectores_raiz: u32,
    /// Primer sector de la zona de datos, relativo a la particion. El cluster
    /// numero 2 empieza aqui.
    pub primer_sector_datos: u32,
    /// `CountofClusters`: los clusters que EXISTEN. El primero es el 2.
    pub clusters: u32,
    /// Ultimo numero de cluster valido, o sea `clusters + 1`.
    pub ultimo_cluster: u32,
    pub raiz: Raiz,
    /// Esta encendido el espejo de la FAT? Sale de `BPB_ExtFlags` bit 7, al
    /// reves: el bit PUESTO significa espejo APAGADO.
    pub espejo: bool,
    /// Cual es la FAT activa. Si hay espejo son todas; si no, solo esta.
    pub fat_activa: u32,
    /// Sector del FSInfo, relativo a la particion. Cero = no hay (FAT12/16).
    pub sector_fsinfo: u32,
    /// Sector con la copia del sector de arranque. Cero = no hay.
    pub sector_respaldo: u32,
    /// `BS_VolID`, leido del sitio que corresponda al tipo.
    pub volume_id: u32,
}

impl Geometria {
    /// Primer sector de la copia `n` de la FAT, relativo a la particion.
    pub fn sector_fat(&self, copia: u32) -> u32 {
        self.reservados + copia * self.sectores_por_fat
    }

    /// Primer sector del cluster `c`, relativo a la particion.
    ///
    /// Devuelve `None` para un cluster que no existe. Que el numero 0 y el 1
    /// no sean clusters es la trampa mas vieja de este formato, y no se deja
    /// que la pise quien llama.
    pub fn sector_de_cluster(&self, c: u32) -> Option<u32> {
        if c < 2 || c > self.ultimo_cluster {
            return None;
        }
        Some(self.primer_sector_datos + (c - 2) * self.sectores_por_cluster)
    }

    pub fn bytes_por_cluster(&self) -> u32 {
        self.bytes_por_sector * self.sectores_por_cluster
    }

    /// **La zona reservada que no usa nadie.**
    ///
    /// De los 32 sectores reservados de un FAT32 tipico se usan cuatro: el 0
    /// (el BPB), el del FSInfo, el de la copia del BPB y el siguiente (la
    /// copia del FSInfo). Del ultimo ocupado hasta `RsvdSecCnt` no escribe ni
    /// lee nadie, jamas.
    ///
    /// Son ~24 sectores = 12 KiB en un volumen normal: cabe entero un
    /// superbloque de ESTRATOS (4096 B). Ver `PLAN_FAT32.md` seccion 4.B1.
    ///
    /// [!] **Se calcula, no se supone.** Un volumen formateado por otra
    /// herramienta puede traer `RsvdSecCnt` mas pequeno, y entonces la
    /// respuesta es `None` y quien queria el sitio se entera en vez de
    /// escribir encima de la FAT.
    pub fn zona_reservada(&self) -> Option<(u32, u32)> {
        let ultimo_ocupado = max(
            max(0, self.sector_fsinfo),
            // La copia del BPB se lleva su sector y el siguiente, que por
            // convenio guarda la copia del FSInfo.
            if self.sector_respaldo == 0 { 0 } else { self.sector_respaldo + 1 },
        );
        let primero = ultimo_ocupado + 1;
        if primero >= self.reservados {
            return None;
        }
        Some((primero, self.reservados - primero))
    }
}

// ---------------------------------------------------------------------------
//  LEER
// ---------------------------------------------------------------------------

fn u16le(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

fn u32le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// **Identifica el volumen que empieza en este sector.**
///
/// El orden de las comprobaciones importa y es el de la especificacion:
///
/// 1. La firma `0xAA55`, que es lo unico que se puede mirar sin fiarse de nada.
/// 2. Los campos de los que depende la ARITMETICA -- sector, cluster,
///    reservados, FATs. Sin ellos las restas de abajo dan cualquier cosa.
/// 3. **La cuenta de clusters**, que es la que decide el tipo.
/// 4. Y solo ENTONCES los campos que dependen del tipo, porque en FAT12/16 y
///    en FAT32 viven en offsets distintos: el `BS_VolID` de un FAT16 esta en
///    el 39 y el de un FAT32 en el 67. Leerlos antes de saber el tipo es
///    exactamente el fallo que este fichero existe para matar.
pub fn identificar(sector: &[u8]) -> Result<Geometria, NoEs> {
    if sector.len() < 512 {
        return Err(NoEs::Corto);
    }

    // 1. La firma vive en el 510 aunque el sector sea de 4096: es un offset
    //    fijo del formato, no "el final del sector".
    if u16le(sector, 510) != 0xAA55 {
        return Err(NoEs::SinFirma);
    }

    // 2. La aritmetica.
    let bytes_por_sector = u16le(sector, 11) as u32;
    if !matches!(bytes_por_sector, 512 | 1024 | 2048 | 4096) {
        return Err(NoEs::SectorImposible);
    }
    let sectores_por_cluster = sector[13] as u32;
    if sectores_por_cluster == 0
        || sectores_por_cluster > 128
        || !sectores_por_cluster.is_power_of_two()
    {
        return Err(NoEs::ClusterImposible);
    }
    let reservados = u16le(sector, 14) as u32;
    if reservados == 0 {
        return Err(NoEs::SinReservados);
    }
    let num_fats = sector[16] as u32;
    if num_fats == 0 {
        return Err(NoEs::SinFats);
    }

    let root_ent_cnt = u16le(sector, 17) as u32;
    let fat_sz_16 = u16le(sector, 22) as u32;
    let tot_sec_16 = u16le(sector, 19) as u32;
    let fat_sz_32 = u32le(sector, 36);
    let tot_sec_32 = u32le(sector, 32);

    // La regla de las dos anchuras: el campo de 16 manda si no es cero. Es
    // literal de la especificacion y por eso se escribe asi de plana.
    let sectores_por_fat = if fat_sz_16 != 0 { fat_sz_16 } else { fat_sz_32 };
    if sectores_por_fat == 0 {
        return Err(NoEs::SinTamanoFat);
    }
    let total_sectores = if tot_sec_16 != 0 { tot_sec_16 } else { tot_sec_32 };
    if total_sectores == 0 {
        return Err(NoEs::SinTotal);
    }

    // El directorio raiz de FAT12/16 son entradas de 32 bytes redondeadas
    // hacia arriba al sector. En FAT32 `RootEntCnt` es 0 y esto da 0 solo.
    let bytes_raiz = root_ent_cnt * 32;
    if !bytes_raiz.is_multiple_of(bytes_por_sector) && root_ent_cnt != 0 {
        // La especificacion pide que ocupe sectores enteros. Uno que no lo
        // haga desplaza la zona de datos medio sector y lo estropea todo.
        return Err(NoEs::RaizImposible);
    }
    let sectores_raiz = bytes_raiz.div_ceil(bytes_por_sector);

    // 3. LA CUENTA. Todo lo de arriba existe para poder hacer estas dos restas.
    let metadatos = reservados
        .checked_add(num_fats.checked_mul(sectores_por_fat).ok_or(NoEs::NoCabe)?)
        .and_then(|v| v.checked_add(sectores_raiz))
        .ok_or(NoEs::NoCabe)?;
    if metadatos >= total_sectores {
        return Err(NoEs::NoCabe);
    }
    let sectores_datos = total_sectores - metadatos;
    let clusters = sectores_datos / sectores_por_cluster;
    if clusters == 0 {
        return Err(NoEs::NoCabe);
    }

    // Los dos umbrales, y son estos y no otros. `4085` y `65525` estan
    // elegidos para que el numero de clusters nunca llegue a los valores
    // reservados de la anchura de abajo; equivocarse por uno aqui es el bug
    // clasico de los drivers de FAT.
    let tipo = if clusters < 4085 {
        Tipo::Fat12
    } else if clusters < 65525 {
        Tipo::Fat16
    } else {
        Tipo::Fat32
    };

    let primer_sector_datos = metadatos;
    let ultimo_cluster = clusters + 1;

    // 4. Y AHORA lo que depende del tipo.
    let (raiz, espejo, fat_activa, sector_fsinfo, sector_respaldo, volume_id) = match tipo {
        Tipo::Fat32 => {
            if u16le(sector, 42) != 0 {
                return Err(NoEs::VersionDesconocida);
            }
            let root_clus = u32le(sector, 44);
            if root_clus < 2 || root_clus > ultimo_cluster {
                return Err(NoEs::RaizImposible);
            }
            // `BPB_ExtFlags`: el bit 7 PUESTO significa que el espejo esta
            // APAGADO, y entonces los bits 0..3 dicen cual es la unica FAT
            // viva. Leerlo del reves --que es lo que invita el nombre-- hace
            // que se escriba en la tabla muerta.
            let ext = u16le(sector, 40);
            let espejo = (ext & 0x0080) == 0;
            let activa = if espejo { 0 } else { (ext & 0x000F) as u32 };
            // Una FAT activa que no existe es una geometria rota, no un
            // detalle: se cae al espejo, que es lo conservador.
            let activa = if activa < num_fats { activa } else { 0 };
            let fsinfo = u16le(sector, 48) as u32;
            let respaldo = u16le(sector, 50) as u32;
            // Los dos tienen que caer DENTRO de la zona reservada. Un
            // `BkBootSec` que apunte a la FAT convierte la comprobacion de
            // integridad en una corrupcion.
            let fsinfo = if fsinfo < reservados { fsinfo } else { 0 };
            let respaldo = if respaldo != 0 && respaldo < reservados { respaldo } else { 0 };
            (
                Raiz::Cadena { cluster: root_clus },
                espejo,
                activa,
                fsinfo,
                respaldo,
                u32le(sector, 67),
            )
        }
        Tipo::Fat12 | Tipo::Fat16 => {
            if root_ent_cnt == 0 {
                // Un FAT12/16 sin directorio raiz no tiene donde empezar.
                return Err(NoEs::RaizImposible);
            }
            (
                Raiz::Region {
                    sector: reservados + num_fats * sectores_por_fat,
                    sectores: sectores_raiz,
                },
                // FAT12/16 no tienen `ExtFlags`: el espejo esta siempre puesto.
                true,
                0,
                0,
                0,
                u32le(sector, 39),
            )
        }
    };

    Ok(Geometria {
        tipo,
        bytes_por_sector,
        sectores_por_cluster,
        reservados,
        num_fats,
        sectores_por_fat,
        total_sectores,
        sectores_raiz,
        primer_sector_datos,
        clusters,
        ultimo_cluster,
        raiz,
        espejo,
        fat_activa,
        sector_fsinfo,
        sector_respaldo,
        volume_id,
    })
}

/// **Cuadra el sector de arranque con su copia?**
///
/// Todo volumen FAT32 lleva una copia del sector 0 en el que diga
/// `BPB_BkBootSec` (el 6, casi siempre). **Nadie la compara nunca**, y es
/// integridad que el formato ya esta pagando: si el BPB principal se piso, un
/// driver que no mire la copia se va a leer sectores que no son.
///
/// Se comparan los 90 bytes del BPB y la firma, no el sector entero: los 420
/// bytes de en medio son codigo de arranque de 16 bits que a nadie de aqui le
/// importa y que algunos formateadores dejan distinto en la copia.
///
/// Devuelve `None` si este volumen no declara copia.
pub fn cuadra_con_respaldo(principal: &[u8], respaldo: &[u8]) -> Option<bool> {
    if principal.len() < 512 || respaldo.len() < 512 {
        return None;
    }
    let g = identificar(principal).ok()?;
    if g.sector_respaldo == 0 {
        return None;
    }
    let iguales = principal[11..90] == respaldo[11..90] && u16le(respaldo, 510) == 0xAA55;
    Some(iguales)
}

/// El FSInfo del sector 1: el atajo del espacio libre.
///
/// Son **pistas, no verdad** -- la FAT manda--, y por eso los dos campos son
/// `Option`: `0xFFFFFFFF` significa literalmente "no se sabe" y colapsarlo a
/// cero seria decir "no queda sitio".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FsInfo {
    /// Clusters libres, si el volumen lo sabe.
    pub libres: Option<u32>,
    /// Por donde seguir buscando. Es la diferencia entre asignar en O(1) y
    /// barrer la FAT entera por cada cluster.
    pub siguiente_libre: Option<u32>,
}

/// Lee el FSInfo. Las tres firmas se comprueban: un sector que solo cumpla una
/// puede ser cualquier cosa, y creerse su `Free_Count` es peor que no tenerlo.
pub fn leer_fsinfo(sector: &[u8]) -> Option<FsInfo> {
    if sector.len() < 512 {
        return None;
    }
    if u32le(sector, 0) != 0x4161_5252
        || u32le(sector, 484) != 0x6141_7272
        || u32le(sector, 508) != 0xAA55_0000
    {
        return None;
    }
    let crudo = |v: u32| if v == 0xFFFF_FFFF { None } else { Some(v) };
    Some(FsInfo {
        libres: crudo(u32le(sector, 488)),
        siguiente_libre: crudo(u32le(sector, 492)),
    })
}

// ===========================================================================
//  EL CENSO -- sectores de arranque escritos a mano, cero discos encendidos
// ===========================================================================
//
// Mismo metodo que el censo de `bmo-particiones` y que el de C: la respuesta
// se sabe de antemano porque el sector se construye aqui. Lo que se comprueba
// no es "el driver no se cayo", es **el numero exacto**.
#[cfg(test)]
mod censo {
    use super::*;

    /// **Cual de las dos colas del sector de arranque se escribe.**
    ///
    /// No es una comodidad del banco de pruebas: es que **no caben las dos**.
    /// En FAT12/16 el `BS_BootSig` esta en el 38 y el `BS_VolID` en el 39; en
    /// FAT32 esos mismos bytes son la mitad alta de `BPB_FATSz32`, y los tres
    /// siguientes son `BPB_ExtFlags` y `BPB_FSVer`.
    ///
    /// Escribir las dos --que es lo que hacia la primera version de este
    /// censo-- convertia una FAT de 512 sectores en una de 2.687.488 y todas
    /// las pruebas de FAT32 contestaban `NoCabe`. El banco de pruebas se
    /// equivoco exactamente igual que se equivocaba el driver: dando por
    /// supuesto que los campos de los dos tipos conviven.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Cola {
        /// `BS_BootSig` en el 38, `BS_VolID` en el 39.
        Corta,
        /// `BS_BootSig` en el 66, `BS_VolID` en el 67.
        Larga,
    }

    /// Un sector de arranque a medida. Los valores por defecto son los de un
    /// FAT32 normal; cada prueba cambia lo que quiere mirar.
    struct Bpb {
        cola: Cola,
        bytes_por_sector: u16,
        sectores_por_cluster: u8,
        reservados: u16,
        num_fats: u8,
        root_ent_cnt: u16,
        tot_sec_16: u16,
        fat_sz_16: u16,
        tot_sec_32: u32,
        fat_sz_32: u32,
        ext_flags: u16,
        fs_ver: u16,
        root_clus: u32,
        fsinfo: u16,
        respaldo: u16,
        firma: u16,
    }

    impl Bpb {
        /// Un FAT32 de 65.525 clusters exactos: el primero que ya NO es FAT16.
        fn fat32() -> Bpb {
            Bpb {
                cola: Cola::Larga,
                bytes_por_sector: 512,
                sectores_por_cluster: 1,
                reservados: 32,
                num_fats: 2,
                root_ent_cnt: 0,
                tot_sec_16: 0,
                fat_sz_16: 0,
                // 65.525 clusters + 32 reservados + 2 x 512 de FAT
                tot_sec_32: 65_525 + 32 + 2 * 512,
                fat_sz_32: 512,
                ext_flags: 0,
                fs_ver: 0,
                root_clus: 2,
                fsinfo: 1,
                respaldo: 6,
                firma: 0xAA55,
            }
        }

        /// Un FAT16 con 4.085 clusters: el primero que ya NO es FAT12.
        fn fat16() -> Bpb {
            Bpb {
                cola: Cola::Corta,
                bytes_por_sector: 512,
                sectores_por_cluster: 1,
                reservados: 1,
                num_fats: 1,
                root_ent_cnt: 512, // = 32 sectores de raiz
                tot_sec_16: 0,
                fat_sz_16: 16,
                // 4.085 clusters + 1 reservado + 16 de FAT + 32 de raiz
                tot_sec_32: 4_085 + 1 + 16 + 32,
                fat_sz_32: 0,
                ext_flags: 0,
                fs_ver: 0,
                root_clus: 0,
                fsinfo: 0,
                respaldo: 0,
                firma: 0xAA55,
            }
        }

        fn bytes(&self) -> [u8; 512] {
            let mut s = [0u8; 512];
            s[0..3].copy_from_slice(&[0xEB, 0x58, 0x90]);
            s[3..11].copy_from_slice(b"BMO     ");
            s[11..13].copy_from_slice(&self.bytes_por_sector.to_le_bytes());
            s[13] = self.sectores_por_cluster;
            s[14..16].copy_from_slice(&self.reservados.to_le_bytes());
            s[16] = self.num_fats;
            s[17..19].copy_from_slice(&self.root_ent_cnt.to_le_bytes());
            s[19..21].copy_from_slice(&self.tot_sec_16.to_le_bytes());
            s[21] = 0xF8;
            s[22..24].copy_from_slice(&self.fat_sz_16.to_le_bytes());
            s[32..36].copy_from_slice(&self.tot_sec_32.to_le_bytes());
            match self.cola {
                Cola::Larga => {
                    s[36..40].copy_from_slice(&self.fat_sz_32.to_le_bytes());
                    s[40..42].copy_from_slice(&self.ext_flags.to_le_bytes());
                    s[42..44].copy_from_slice(&self.fs_ver.to_le_bytes());
                    s[44..48].copy_from_slice(&self.root_clus.to_le_bytes());
                    s[48..50].copy_from_slice(&self.fsinfo.to_le_bytes());
                    s[50..52].copy_from_slice(&self.respaldo.to_le_bytes());
                    s[66] = 0x29;
                    s[67..71].copy_from_slice(&0x1234_5678u32.to_le_bytes());
                }
                Cola::Corta => {
                    s[38] = 0x29;
                    s[39..43].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
                    // ** Y ADEMAS el 0x29 en el sitio de FAT32, que en un
                    // FAT12/16 cae dentro del codigo de arranque y no significa
                    // nada. Esta puesto a proposito: es la trampa exacta en la
                    // que cayo `bmo-fat32`, y si algun dia alguien vuelve a
                    // decidir el tipo mirando el byte 66, esta prueba lo caza.
                    s[66] = 0x29;
                }
            }
            s[510..512].copy_from_slice(&self.firma.to_le_bytes());
            s
        }
    }

    // -- Lo que este fichero existe para arreglar ---------------------------

    #[test]
    fn un_fat16_no_se_identifica_como_fat32() {
        let g = identificar(&Bpb::fat16().bytes()).expect("es un volumen valido");
        assert_eq!(g.tipo, Tipo::Fat16, "EL FALLO 3.1: 0x29 no decide el tipo");
        assert_eq!(g.clusters, 4_085);
        assert_eq!(g.volume_id, 0xDEAD_BEEF, "el VolID de FAT16 vive en el 39");
    }

    #[test]
    fn un_fat32_se_identifica_y_su_volid_sale_del_otro_sitio() {
        let g = identificar(&Bpb::fat32().bytes()).expect("es un volumen valido");
        assert_eq!(g.tipo, Tipo::Fat32);
        assert_eq!(g.clusters, 65_525);
        assert_eq!(g.volume_id, 0x1234_5678, "el VolID de FAT32 vive en el 67");
        assert_eq!(g.raiz, Raiz::Cadena { cluster: 2 });
    }

    // -- Las fronteras, que es donde vive el bug clasico --------------------

    #[test]
    fn cuatro_mil_ochenta_y_cuatro_clusters_son_fat12() {
        let mut b = Bpb::fat16();
        b.tot_sec_32 -= 1; // 4.084
        let g = identificar(&b.bytes()).unwrap();
        assert_eq!(g.clusters, 4_084);
        assert_eq!(g.tipo, Tipo::Fat12, "4.084 es el ultimo FAT12");
    }

    #[test]
    fn sesenta_y_cinco_mil_quinientos_veinticuatro_son_fat16() {
        let mut b = Bpb::fat32();
        b.tot_sec_32 -= 1; // 65.524
        // Un volumen de ese tamano con RootEntCnt=0 no es un FAT16 legal, asi
        // que se le da raiz: lo que se mide es EL UMBRAL, no la validez.
        b.root_ent_cnt = 512;
        b.tot_sec_32 += 32; // los sectores que se lleva la raiz
        let g = identificar(&b.bytes()).unwrap();
        assert_eq!(g.clusters, 65_524);
        assert_eq!(g.tipo, Tipo::Fat16, "65.524 es el ultimo FAT16");
    }

    // -- Las guardas --------------------------------------------------------

    #[test]
    fn sin_firma_no_hay_volumen() {
        let mut b = Bpb::fat32();
        b.firma = 0x1234;
        assert_eq!(identificar(&b.bytes()), Err(NoEs::SinFirma));
    }

    #[test]
    fn una_version_de_fat32_que_no_se_conoce_no_se_monta() {
        let mut b = Bpb::fat32();
        b.fs_ver = 0x0001;
        assert_eq!(identificar(&b.bytes()), Err(NoEs::VersionDesconocida));
    }

    #[test]
    fn un_buffer_corto_se_dice_en_vez_de_indexar_fuera() {
        assert_eq!(identificar(&[0u8; 100]), Err(NoEs::Corto));
    }

    #[test]
    fn un_cluster_que_no_es_potencia_de_dos_se_rechaza() {
        let mut b = Bpb::fat32();
        b.sectores_por_cluster = 3;
        assert_eq!(identificar(&b.bytes()), Err(NoEs::ClusterImposible));
        b.sectores_por_cluster = 0;
        assert_eq!(identificar(&b.bytes()), Err(NoEs::ClusterImposible));
    }

    #[test]
    fn una_raiz_fuera_del_volumen_se_rechaza() {
        let mut b = Bpb::fat32();
        b.root_clus = 999_999;
        assert_eq!(identificar(&b.bytes()), Err(NoEs::RaizImposible));
        b.root_clus = 1;
        assert_eq!(identificar(&b.bytes()), Err(NoEs::RaizImposible));
    }

    #[test]
    fn unas_regiones_que_no_caben_se_rechazan() {
        let mut b = Bpb::fat32();
        b.tot_sec_32 = 10; // menos que los reservados + las dos FAT
        assert_eq!(identificar(&b.bytes()), Err(NoEs::NoCabe));
    }

    // -- Las perillas de la seccion 4 del plan ------------------------------

    #[test]
    fn un_sector_de_4096_es_legal_y_sube_el_techo_del_volumen() {
        let mut b = Bpb::fat32();
        b.bytes_por_sector = 4096;
        let g = identificar(&b.bytes()).expect("4096 esta en la especificacion");
        assert_eq!(g.bytes_por_sector, 4096);
        assert_eq!(g.bytes_por_cluster(), 4096, "1 sector x 4096");
    }

    #[test]
    fn un_sector_de_mil_no_lo_esta() {
        let mut b = Bpb::fat32();
        b.bytes_por_sector = 1000;
        assert_eq!(identificar(&b.bytes()), Err(NoEs::SectorImposible));
    }

    #[test]
    fn una_sola_fat_es_legal() {
        let mut b = Bpb::fat32();
        b.num_fats = 1;
        b.tot_sec_32 -= 512; // se va una copia de la FAT
        let g = identificar(&b.bytes()).expect("NumFATs=1 esta en la especificacion");
        assert_eq!(g.num_fats, 1);
        assert_eq!(g.clusters, 65_525, "sigue siendo el mismo volumen util");
    }

    #[test]
    fn con_el_espejo_apagado_se_dice_cual_es_la_fat_viva() {
        let mut b = Bpb::fat32();
        b.ext_flags = 0x0081; // bit 7 = sin espejo, FAT numero 1
        let g = identificar(&b.bytes()).unwrap();
        assert!(!g.espejo, "EL FALLO 3.3: el bit 7 PUESTO significa SIN espejo");
        assert_eq!(g.fat_activa, 1);
        assert_eq!(g.sector_fat(1), 32 + 512);
    }

    #[test]
    fn una_fat_activa_que_no_existe_cae_a_la_cero() {
        let mut b = Bpb::fat32();
        b.ext_flags = 0x0087; // sin espejo, y dice que la FAT viva es la 7
        let g = identificar(&b.bytes()).unwrap();
        assert_eq!(g.fat_activa, 0, "de 7 FATs solo hay 2: no se escribe a ciegas");
    }

    #[test]
    fn la_zona_reservada_libre_de_un_fat32_normal_son_24_sectores() {
        let g = identificar(&Bpb::fat32().bytes()).unwrap();
        // Ocupados: 0 (BPB), 1 (FSInfo), 6 (copia del BPB), 7 (copia del
        // FSInfo). Libres: del 8 al 31.
        assert_eq!(g.zona_reservada(), Some((8, 24)));
        assert_eq!(24 * 512, 12_288, "12 KiB: cabe un bloque de ESTRATOS y sobra");
    }

    #[test]
    fn un_volumen_apretado_no_ofrece_zona_reservada() {
        let mut b = Bpb::fat32();
        b.reservados = 8; // justo hasta la copia del FSInfo
        let g = identificar(&b.bytes()).unwrap();
        assert_eq!(g.zona_reservada(), None, "no hay sitio, y se dice");
    }

    #[test]
    fn un_respaldo_que_apunta_fuera_de_la_zona_reservada_se_ignora() {
        let mut b = Bpb::fat32();
        b.respaldo = 40; // mas alla de los 32 reservados: caeria en la FAT
        let g = identificar(&b.bytes()).unwrap();
        assert_eq!(g.sector_respaldo, 0, "un BkBootSec en la FAT no es un respaldo");
    }

    // -- Aritmetica de clusters ---------------------------------------------

    #[test]
    fn el_cluster_dos_empieza_en_la_zona_de_datos_y_el_uno_no_existe() {
        let g = identificar(&Bpb::fat32().bytes()).unwrap();
        assert_eq!(g.primer_sector_datos, 32 + 2 * 512);
        assert_eq!(g.sector_de_cluster(2), Some(g.primer_sector_datos));
        assert_eq!(g.sector_de_cluster(1), None, "el 0 y el 1 no son clusters");
        assert_eq!(g.sector_de_cluster(0), None);
        assert_eq!(g.sector_de_cluster(g.ultimo_cluster + 1), None);
    }

    // -- La copia del sector de arranque (4.A5) -----------------------------

    #[test]
    fn el_respaldo_cuadra_consigo_mismo_y_deja_de_cuadrar_si_se_pisa() {
        let s = Bpb::fat32().bytes();
        assert_eq!(cuadra_con_respaldo(&s, &s), Some(true));

        let mut roto = s;
        roto[13] = 8; // otro tamano de cluster en la copia
        assert_eq!(cuadra_con_respaldo(&s, &roto), Some(false));
    }

    #[test]
    fn un_volumen_sin_respaldo_declarado_no_opina() {
        let mut b = Bpb::fat32();
        b.respaldo = 0;
        let s = b.bytes();
        assert_eq!(cuadra_con_respaldo(&s, &s), None);
    }

    // -- FSInfo -------------------------------------------------------------

    #[test]
    fn el_fsinfo_distingue_no_se_sabe_de_cero() {
        let mut s = [0u8; 512];
        s[0..4].copy_from_slice(&0x4161_5252u32.to_le_bytes());
        s[484..488].copy_from_slice(&0x6141_7272u32.to_le_bytes());
        s[508..512].copy_from_slice(&0xAA55_0000u32.to_le_bytes());
        s[488..492].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        s[492..496].copy_from_slice(&7u32.to_le_bytes());

        let f = leer_fsinfo(&s).expect("las tres firmas estan");
        assert_eq!(f.libres, None, "0xFFFFFFFF es 'no se sabe', no 'cero libres'");
        assert_eq!(f.siguiente_libre, Some(7));
    }

    #[test]
    fn un_fsinfo_con_una_firma_mala_no_se_cree() {
        let mut s = [0u8; 512];
        s[0..4].copy_from_slice(&0x4161_5252u32.to_le_bytes());
        s[484..488].copy_from_slice(&0x6141_7272u32.to_le_bytes());
        // falta la de la cola
        assert_eq!(leer_fsinfo(&s), None);
    }
}
