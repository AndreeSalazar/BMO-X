//! FAT32 and exFAT filesystem reader/writer -- minimal implementation.
//!
//! Supports both FAT32 (S: FASTOS-EFI) and exFAT (T: FastOS-Data, X: Commit-Real).
//! Reads BPB, locates root directory, finds files by 8.3 name,
//! and reads clusters via the FAT chain. El almacenamiento entra por el
//! contrato `BlockReader`/`BlockWriter`: no sabe si debajo hay SATA o NVMe.

// `no_std` en la maquina, `std` en las pruebas. Es el mismo patron que
// `bmo-estratos`, y existe porque un driver que escribe en el disco de alguien
// **tiene que poder probarse en el anfitrion**: la alternativa era verificarlo
// flasheando, o sea arriesgando el volumen para saber si el codigo lo respeta.
#![cfg_attr(not(test), no_std)]

/// Filesystem type detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType {
    Fat32,
    ExFat,
}

/// exFAT BIOS Parameter Block at sector 0, offset 0.
/// exFAT has a different layout than FAT32 -- see exFAT spec section 3.1.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatBpb {
    pub jump: [u8; 3],
    pub fs_name: [u8; 8],       // "EXFAT   "
    pub must_be_zero: [u8; 53],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster_of_root_directory: u32,
    pub volume_serial_number: u32,
    pub fs_revision: u16,
    pub volume_flags: u16,
    pub bytes_per_sector_shift: u8,
    pub sectors_per_cluster_shift: u8,
    pub number_of_fats: u8,
    pub drive_select: u8,
    pub percent_in_use: u8,
    pub reserved: [u8; 7],
    pub boot_code: [u8; 390],
    pub boot_signature: u16,
}

/// FAT32 BIOS Parameter Block at sector 0, offset 11.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FatBpb {
    pub jmp: [u8; 3],
    pub oem: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    _root_entries: u16,
    _total_sectors16: u16,
    pub media: u8,
    _fat_size16: u16,
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors: u32,
    pub fat_size: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
    _reserved: [u8; 12],
    pub drive_number: u8,
    _reserved1: u8,
    pub boot_sig: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    _nt_reserved: u8,
    _create_time_tenth: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub last_access: u16,
    pub first_cluster_hi: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_lo: u16,
    pub file_size: u32,
}

/// Una entrada de directorio YA LOCALIZADA: donde estan sus 32 bytes en el
/// disco, ademas de lo que dicen.
///
/// No es un `DirEntry`: aquel son los bytes del formato, este es *el sitio*.
/// La diferencia importa al reemplazar -- para apuntar un nombre a otra cadena
/// hay que reescribir el sector donde vive, y eso solo se sabe habiendolo
/// encontrado.
#[derive(Debug, Clone, Copy)]
pub struct EntradaDir {
    /// LBA relativo a la particion del sector que la contiene.
    pub lba: u64,
    /// Byte de esa entrada dentro del sector. Siempre multiplo de 32.
    pub offset: usize,
    pub first_cluster: u32,
    pub size: u32,
}

/// exFAT File Directory Entry (type 0x85)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatFileEntry {
    pub entry_type: u8,      // 0x85
    pub secondary_count: u8,
    pub set_checksum: u16,
    pub file_attributes: u16,
    _reserved1: u16,
    pub create_timestamp: u32,
    pub last_modified_timestamp: u32,
    pub last_accessed_timestamp: u32,
    _create_millis: u8,
    _last_modified_millis: u8,
    _create_utc_offset: u8,
    _last_modified_utc_offset: u8,
    _last_accessed_utc_offset: u8,
    _reserved2: [u8; 7],
}

/// exFAT Stream Extension Entry (type 0xC0) -- follows File Entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatStreamEntry {
    pub entry_type: u8,      // 0xC0
    pub general_secondary_flags: u8,
    _reserved1: u8,
    _reserved2: u8,
    pub name_length: u8,
    pub name_hash: u16,
    _reserved3: u16,
    pub valid_data_length: u64,
    _reserved4: u32,
    pub first_cluster: u32,
    pub data_length: u64,
}

/// exFAT Filename Entry (type 0xC1) -- follows Stream Entry
/// Contains up to 15 UTF-16 characters of the filename
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatNameEntry {
    pub entry_type: u8,      // 0xC1
    pub general_secondary_flags: u8,
    pub name_string: [u16; 15],  // UTF-16LE filename (up to 15 chars)
}

/// Lee `count` sectores de 512 B desde `lba` ABSOLUTO del dispositivo.
///
/// Es TODO lo que este sistema de ficheros necesita saber del almacenamiento.
/// No sabe si debajo hay SATA, NVMe o un disco en RAM, y no debe saberlo:
/// antes estaba soldado a `bmo_ahci` y por tanto no habria podido leer jamas
/// un NVMe. Un puntero a funcion en vez de un trait porque en Ring 0 no hay
/// alloc y no hace falta mas.
pub type BlockReader = fn(lba: u64, count: u16, buf: &mut [u8]) -> bool;
/// Escribe sectores. `None` al montar = volumen de SOLO LECTURA, y entonces
/// la imposibilidad de escribir es ESTRUCTURAL, no una promesa.
pub type BlockWriter = fn(lba: u64, count: u16, data: &[u8]) -> bool;

/// Cual de los dos buffers internos usa una operacion. Existe para que el
/// prestamo del buffer y el del dispositivo no se pisen: se copia el puntero
/// a funcion primero y el buffer se toma despues.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
enum Buf { buf, fat_cache }

pub struct FatVolume {
    read: BlockReader,
    write: Option<BlockWriter>,
    /// Primer LBA de la PARTICION dentro del disco. El sistema de ficheros
    /// piensa en sectores relativos a su volumen y no sabe que existe una
    /// tabla de particiones; aqui se suma. Sin esto, `mount` leia el sector 0
    /// del DISCO --la GPT-- creyendo que era el arranque del volumen.
    part_lba: u64,
    pub fs_type: FsType,
    #[allow(dead_code)]
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    num_fats: u8,
    fat_start: u32,
    fat_size_sectors: u32,
    data_start: u32,
    root_cluster: u32,
    /// Ultimo numero de cluster que EXISTE en la zona de datos.
    ///
    /// La FAT casi siempre tiene mas entradas que clusters reales: se
    /// dimensiona en sectores enteros y el final sobra. Ese sobrante esta a
    /// cero, o sea que parece "libre". Buscar un hueco sin este tope devuelve
    /// clusters que no existen, y `cluster_to_lba` de un cluster inexistente
    /// da un LBA FUERA del volumen. Es la diferencia entre "no cabe" y
    /// "escribir en la particion del vecino".
    max_cluster: u32,
    /// * **Operaciones que fallaron y NO cambiaron el resultado.**
    ///
    /// Este driver tiene sitios donde un fallo del dispositivo no puede
    /// convertirse en un `false` sin mentir al reves: rellenar de ceros la cola
    /// de un cluster cuyos datos YA se escribieron bien, o soltar el resto de
    /// una cadena de clusters. Ahi el fallo no cambia lo que se le contesta al
    /// llamante -- y antes de esto tampoco dejaba rastro en ningun sitio.
    ///
    /// Un disco que empieza a fallar lo hace primero en operaciones asi. Si
    /// esta cuenta no es cero, el volumen esta peor de lo que dice cualquier
    /// codigo de retorno. Se lee con [`FatVolume::fallos_mudos`].
    fallos_mudos: u32,
    buf: [u8; 512],
    fat_cache: [u8; 512],
    /// **Que sector de la FAT hay en [`FatVolume::fat_cache`].** `SIN_CACHE` =
    /// ninguno.
    ///
    /// === Por que esto no es una optimizacion, es la diferencia entre que algo
    /// funcione o no ===
    ///
    /// `fat_cache` se llamaba cache y no lo era: cada `read_fat_entry` leia el
    /// sector **otra vez**, aunque fuera el mismo de la llamada anterior. Y una
    /// entrada de FAT32 son cuatro bytes, o sea que en un sector caben **128
    /// entradas seguidas** -- exactamente lo que recorre quien sigue una cadena.
    ///
    /// Mientras lo unico que recorria cadenas era cargar un programa de una vez,
    /// eso se pagaba una vez y no se notaba. Desde el 2026-08-11 un archivo
    /// abierto se lee por rangos y **volver atras es normal** (ver
    /// `ring0::obj::archivo`): cada salto hacia atras en un WAD de 4 MiB son mil
    /// entradas de FAT, y sin esto serian **mil comandos al disco por lump**.
    /// Con esto son ocho.
    ///
    /// > Un buffer al que se le llama cache y no recuerda lo que trajo es un
    /// > coste que nadie ve hasta que el patron de acceso cambia.
    ///
    /// Lo mantiene [`FatVolume::read_sector`], que es el unico sitio que llena
    /// este buffer, y lo refresca `write_sector`: despues de escribirlo, lo que
    /// hay en memoria es lo que hay en el disco.
    fat_cache_lba: u64,
}

/// No hay ningun sector cargado en `fat_cache`. No es un LBA posible.
const SIN_CACHE: u64 = u64::MAX;

/// Por que fallo una escritura. Un `false` pelado no dice si el disco esta
/// lleno, si el volumen es de solo lectura o si el nombre ya existia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteError {
    /// El volumen se monto sin `BlockWriter`.
    ReadOnly,
    /// Ya hay un archivo con ese nombre en ese directorio.
    Exists,
    /// No quedan clusters libres para todos los datos.
    NoSpace,
    /// El directorio no tiene entradas libres y no se pudo extender.
    DirFull,
    /// El dispositivo fallo al leer o escribir un sector.
    Io,
    /// Crear archivos no esta implementado para este formato.
    Unsupported,
}

impl WriteError {
    pub fn name(self) -> &'static str {
        match self {
            WriteError::ReadOnly => "el volumen es de solo lectura",
            WriteError::Exists => "ya existe un archivo con ese nombre",
            WriteError::NoSpace => "no quedan clusters libres",
            WriteError::DirFull => "el directorio esta lleno",
            WriteError::Io => "el disco fallo al leer o escribir",
            WriteError::Unsupported => "crear archivos no soportado en este formato",
        }
    }
}

/// Monta el volumen que empieza en `part_lba` del dispositivo.
///
/// `write = None` monta en SOLO LECTURA: no es una politica que alguien deba
/// recordar respetar, es que no hay con que escribir.
pub fn mount(read: BlockReader, write: Option<BlockWriter>, part_lba: u64) -> Option<FatVolume> {
    let mut buf = [0u8; 512];
    if !read(part_lba, 1, &mut buf) { return None; }

    // Check for exFAT signature ("EXFAT   ") at offset 3
    let fs_name = &buf[3..11];
    if fs_name == b"EXFAT   " {
        return mount_exfat(read, write, part_lba, &buf);
    }

    // Otherwise try FAT32
    let bpb = unsafe { &*(buf.as_ptr() as *const FatBpb) };
    if bpb.bytes_per_sector != 512 { return None; }
    if bpb.boot_sig != 0x29 && bpb.boot_sig != 0x28 { return None; }
    let fat_start = bpb.reserved_sectors as u32;
    let fat_size_sectors = bpb.fat_size;
    let num_fats = bpb.num_fats;
    let data_start = fat_start + (num_fats as u32) * fat_size_sectors;
    let spc = bpb.sectors_per_cluster;
    if spc == 0 { return None; }
    // Clusters que EXISTEN de verdad: los sectores de datos divididos entre el
    // tamano de cluster. La numeracion empieza en 2, asi que el ultimo valido
    // es cuenta+1.
    let total = bpb.total_sectors;
    if total <= data_start { return None; }
    let max_cluster = (total - data_start) / spc as u32 + 1;
    Some(FatVolume { read, write, part_lba, fs_type: FsType::Fat32, bytes_per_sector: bpb.bytes_per_sector, sectors_per_cluster: spc,
        num_fats, fat_start, fat_size_sectors, data_start, root_cluster: bpb.root_cluster, max_cluster, fallos_mudos: 0, buf: [0; 512], fat_cache: [0; 512], fat_cache_lba: SIN_CACHE })
}

fn mount_exfat(read: BlockReader, write: Option<BlockWriter>, part_lba: u64, buf: &[u8; 512]) -> Option<FatVolume> {
    let epb = unsafe { &*(buf.as_ptr() as *const ExFatBpb) };
    if epb.boot_signature != 0xAA55 { return None; }
    let bps_shift = epb.bytes_per_sector_shift;
    let bytes_per_sector: u16 = 1u16 << bps_shift;
    let spc_shift = epb.sectors_per_cluster_shift;
    let sectors_per_cluster: u8 = 1u8 << spc_shift;
    let fat_start = epb.fat_offset;
    let fat_size_sectors = epb.fat_length;
    let data_start = epb.cluster_heap_offset;
    let root_cluster = epb.first_cluster_of_root_directory;
    let num_fats = epb.number_of_fats;


    // exFAT lo dice en su propio BPB, sin tener que deducirlo.
    let max_cluster = epb.cluster_count + 1;
    Some(FatVolume { read, write, part_lba, fs_type: FsType::ExFat, bytes_per_sector, sectors_per_cluster,
        num_fats, fat_start, fat_size_sectors, data_start, root_cluster, max_cluster, fallos_mudos: 0, buf: [0; 512], fat_cache: [0; 512], fat_cache_lba: SIN_CACHE })
}

/// **UN CURSOR DENTRO DE UN ARCHIVO.** Sabe por que cluster va y en que byte del
/// archivo empieza ese cluster.
///
/// === Por que hace falta uno, y por que solo va hacia adelante ===
///
/// `read_file` y `leer_tramo` leen desde el principio o desde una frontera de
/// cluster. Para que el disco escriba **cada seccion de un `.bex` directamente en
/// los marcos del proceso** hace falta empezar en un byte cualquiera: la seccion
/// `Data` empieza donde el fichero diga, no donde caiga un cluster.
///
/// Y hace falta que sea un CURSOR y no un parametro, porque en FAT32 llegar al
/// byte `N` es **seguir la cadena** desde el principio. Con cursor, leer un
/// fichero por trozos cuesta UN recorrido; sin el, uno por trozo -- cuadratico, y
/// justo en el fichero mas grande.
///
/// ** Solo avanza, y eso no es una limitacion que haya que recordar: el cargador
/// aterriza las secciones **en orden de offset de fichero**, decidido el
/// 2026-08-10 exactamente para esto (ver la nota del orden de aterrizaje en
/// `ring0/task/proc.rs`). Pedir hacia atras devuelve `false` en vez de recorrer la
/// cadena en silencio -- un cursor que retrocede sin decirlo convierte un bucle
/// barato en uno cuadratico sin que nada avise.
#[derive(Clone, Copy)]
pub struct Cursor {
    /// El cluster por el que va.
    cluster: u32,
    /// En que byte del ARCHIVO empieza ese cluster.
    base: usize,
}

impl Cursor {
    /// **Por que cluster va.** Para poder DECIRLO.
    ///
    /// Un cargador que falla al leer tiene que poder contar de donde estaba
    /// leyendo: el 2026-08-11 el sistema supo decir *que* bytes llegaron --y que
    /// no eran los del fichero-- pero no **de que sector**, y esas son las dos
    /// mitades de la misma pregunta. Con una sola no se distingue "el mapa esta
    /// mal" de "el disco tiene otra cosa ahi".
    pub fn cluster(&self) -> u32 {
        self.cluster
    }

    /// **En que byte del archivo empieza el cluster por el que va.**
    ///
    /// Es el suelo de lo que este cursor todavia puede leer: pedirle un offset
    /// por debajo es pedirle que retroceda, y contesta que no. Se expone para
    /// que quien llama pueda **distinguir** ese "no" de un fallo del disco --
    /// son el mismo `0` y mandan a sitios opuestos.
    pub fn base(&self) -> usize {
        self.base
    }

    /// Un cursor que no apunta a nada: toda lectura contesta cero.
    ///
    /// Existe para que quien no encuentre un archivo pueda devolver **algo** y
    /// dejar que el motivo lo de la lectura --que sabe decir por que-- en vez de
    /// obligar a cada llamante a desenvolver un `Option` para acabar en el mismo
    /// sitio. El cluster `0` no es valido en FAT32, asi que no puede confundirse
    /// con uno de verdad.
    ///
    /// Es `const` para que una tabla de cursores --una por archivo abierto--
    /// pueda nacer en `.bss` sin codigo de arranque que la rellene. Ver
    /// `ring0::obj::archivo`.
    pub const fn vacio() -> Self {
        Cursor { cluster: 0, base: 0 }
    }
}

impl FatVolume {
    /// Fallos del dispositivo que no cambiaron ningun codigo de retorno.
    ///
    /// **Tiene que ser cero.** Si no lo es, el volumen ha fallado en sitios
    /// donde nadie se entera por la via normal, y eso precede a fallar donde si
    /// se nota. Ver el campo.
    pub fn fallos_mudos(&self) -> u32 { self.fallos_mudos }

    /// **DEL VOLUMEN AL DISCO. La unica traduccion, y por eso esta sola.**
    ///
    /// Todo este driver piensa en sectores relativos al volumen: `data_start`,
    /// `fat_start` y `cluster_to_lba` cuentan desde el sector 0 de la particion,
    /// que no sabe que existe una tabla de particiones. El disco no. Entre las
    /// dos numeraciones hay una suma, y **hasta el 2026-08-11 esa suma estaba
    /// escrita cuatro veces**: tres en los helpers de sector y ninguna en el
    /// camino directo que se estreno en el escalon 3.
    ///
    /// El resultado fue el fallo mas caro de esta semana. Los directorios y la
    /// FAT se leen sector a sector --por los helpers, o sea traducidos-- y los
    /// DATOS iban directos: el sistema encontraba el archivo, sabia su tamano
    /// exacto, y traia los bytes de `lba` **sin sumar nada**, o sea de otra
    /// particion. Con `part_lba = 1230848`, un `.bex` del volumen de datos se
    /// leia de dentro de la ESP.
    ///
    /// > **Una asimetria de "el directorio se lee bien y el contenido no" apunta
    /// > SIEMPRE aqui**: son los dos unicos caminos que este driver tiene al
    /// > disco, y lo unico que puede diferenciarlos es la traduccion.
    ///
    /// Y no se vio en las pruebas porque las dos montan con `part_lba = 0`,
    /// donde la suma que falta vale cero. Ver `volumen_con_base`.
    fn abs(&self, lba: u64) -> u64 {
        self.part_lba + lba
    }

    /// **Lee sectores del volumen DIRECTAMENTE al buffer del llamante.**
    ///
    /// Es el camino del escalon 3 --el HBA escribe en el marco del proceso sin
    /// pagina de rebote-- y existe como metodo, y no como una llamada suelta a
    /// `self.read`, por una sola razon: **para que pase por `abs`**. Un puntero
    /// a funcion invocado a mano se salta la traduccion sin que nada avise.
    fn leer_directo(&self, lba: u64, count: u16, dst: &mut [u8]) -> bool {
        (self.read)(self.abs(lba), count, dst)
    }

    /// Lee un sector del VOLUMEN a uno de los buffers internos.
    ///
    /// El puntero a funcion se copia ANTES de tomar el buffer: si no, seria un
    /// doble prestamo de `self` y no compilaria.
    fn read_sector(&mut self, lba: u64, which: Buf) -> bool {
        let rd = self.read;
        let abs = self.abs(lba);
        match which {
            Buf::buf => rd(abs, 1, &mut self.buf),
            Buf::fat_cache => {
                // ** Y AQUI SI SE RECUERDA. Ver el campo `fat_cache_lba`: en un
                // sector de FAT caben 128 entradas seguidas, que son justo las
                // que recorre quien sigue una cadena. Sin esta linea, seguir
                // una cadena de mil clusters son mil comandos al disco.
                if self.fat_cache_lba == lba {
                    return true;
                }
                let ok = rd(abs, 1, &mut self.fat_cache);
                // Si la lectura fallo, lo que hay en el buffer es del sector
                // ANTERIOR. Decir que es de este seria servir las entradas de
                // otro sitio de la FAT como si fueran de aqui.
                self.fat_cache_lba = if ok { lba } else { SIN_CACHE };
                ok
            }
        }
    }

    /// Escribe uno de los buffers internos. `false` si el volumen se monto en
    /// solo lectura -- no hay writer que llamar.
    fn write_sector(&mut self, lba: u64, which: Buf) -> bool {
        let wr = match self.write { Some(w) => w, None => return false };
        let abs = self.abs(lba);
        match which {
            Buf::buf => wr(abs, 1, &self.buf),
            Buf::fat_cache => {
                let ok = wr(abs, 1, &self.fat_cache);
                // Lo que queda en memoria es lo que acaba de irse al disco, asi
                // que el buffer sigue valiendo para este sector -- y si la
                // escritura fallo, lo que hay en el disco ya no se sabe: se
                // olvida, que es la unica respuesta honesta.
                self.fat_cache_lba = if ok { lba } else { SIN_CACHE };
                ok
            }
        }
    }

    /// Escribe datos externos (un sector ya armado por el llamante).
    fn write_from(&mut self, lba: u64, data: &[u8]) -> bool {
        let wr = match self.write { Some(w) => w, None => return false };
        wr(self.abs(lba), 1, data)
    }

    /// Primer LBA de la particion montada, por si alguien de arriba lo
    /// necesita para diagnostico.
    pub fn partition_lba(&self) -> u64 { self.part_lba }

    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        self.data_start as u64 + (cluster as u64 - 2) * self.sectors_per_cluster as u64
    }

    fn read_fat_entry(&mut self, cluster: u32) -> Option<u32> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start + (fat_offset / 512);
        let fat_index = (fat_offset % 512) as usize;
        unsafe {
            if !self.read_sector(fat_sector as u64, Buf::fat_cache) { return None; }
        }
        let entry = u32::from_le_bytes([self.fat_cache[fat_index], self.fat_cache[fat_index+1],
            self.fat_cache[fat_index+2], self.fat_cache[fat_index+3]]) & 0x0FFF_FFFF;
        match entry {
            0 => None,
            n if n >= 0x0FFF_FFF7 => None,
            n => Some(n),
        }
    }

    pub fn find_file(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        match self.fs_type {
            FsType::Fat32 => self.find_file_fat32(name),
            FsType::ExFat => self.find_file_exfat(name),
        }
    }

    /// Busca un archivo DENTRO de un directorio ya localizado.
    ///
    /// Existe porque `find_file` mira solo la raiz, y en un volumen de
    /// arranque real lo que interesa vive en `EFI/BOOT`. Encontrar el
    /// directorio y luego buscar el archivo en la raiz de todas formas es el
    /// error que se comio el primer intento.
    pub fn find_file_in(&mut self, name: &[u8], dir_cluster: u32) -> Option<(u32, u32)> {
        match self.fs_type {
            FsType::Fat32 => self.find_file_fat32_from(name, dir_cluster),
            FsType::ExFat => self.find_file_exfat(name),
        }
    }

    fn find_file_fat32(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        let root = self.root_cluster;
        self.find_file_fat32_from(name, root)
    }

    fn find_file_fat32_from(&mut self, name: &[u8], start_cluster: u32) -> Option<(u32, u32)> {
        self.find_entry_fat32_from(name, start_cluster).map(|e| (e.first_cluster, e.size))
    }

    /// Igual que [`Self::find_file_fat32_from`], pero devolviendo **donde vive
    /// la entrada de directorio**, no solo lo que dice.
    ///
    /// * Hace falta para REEMPLAZAR. Buscar el archivo por su nombre contesta
    /// "empieza en el cluster N y mide M"; para apuntarlo a otra cadena hay que
    /// volver a escribir esos 32 bytes, y para eso hay que saber en que sector
    /// estan. La version que solo devuelve el par obliga a recorrer el
    /// directorio **dos veces** y a esperar que la segunda pasada encuentre lo
    /// mismo que la primera.
    fn find_entry_fat32_from(&mut self, name: &[u8], start_cluster: u32) -> Option<EntradaDir> {
        let mut cluster = start_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512/32) {
                    let de = unsafe { &*entries.add(i) };
                    if de.name[0] == 0 { return None; }
                    if de.name[0] == 0xE5 { continue; }
                    if name_match(&de.name, name) {
                        return Some(EntradaDir {
                            lba: lba + s,
                            offset: i * 32,
                            first_cluster: (de.first_cluster_hi as u32) << 16
                                | de.first_cluster_lo as u32,
                            size: de.file_size,
                        });
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    fn find_file_exfat(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        let mut cluster = self.root_cluster;
        let spc = self.sectors_per_cluster as u64;
        let _entry_buf = [0u8; 32];
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                // Scan 16 entries per 512-byte sector (each entry = 32 bytes)
                for i in 0..16 {
                    let entry_offset = i * 32;
                    let entry_type = self.buf[entry_offset];
                    if entry_type == 0x00 { return None; } // end of directory
                    if entry_type == 0x05 { continue; }   // deleted
                    if entry_type == 0x85 {
                        // File Entry -- next entries are Stream + Filename
                        let file_entry = unsafe {
                            &*(self.buf[entry_offset..].as_ptr() as *const ExFatFileEntry)
                        };
                        let secondary_count = file_entry.secondary_count;
                        // Walk secondary entries in subsequent slots
                        for sec in 1..=secondary_count {
                            let sec_offset = entry_offset + (sec as usize) * 32;
                            if sec_offset + 32 > 512 { break; }
                            let sec_type = self.buf[sec_offset];
                            if sec_type == 0xC0 {
                                // Stream Extension -- has first_cluster and name_length
                                let stream = unsafe {
                                    &*(self.buf[sec_offset..].as_ptr() as *const ExFatStreamEntry)
                                };
                                let first_cluster = stream.first_cluster;
                                let name_len = stream.name_length as usize;
                                let data_len = stream.valid_data_length as u32;
                                // Next entry should be Filename (0xC1)
                                if sec + 1 <= secondary_count {
                                    let name_offset = entry_offset + ((sec + 1) as usize) * 32;
                                    if name_offset + 32 <= 512 && self.buf[name_offset] == 0xC1 {
                                        let name_entry = unsafe {
                                            &*(self.buf[name_offset..].as_ptr() as *const ExFatNameEntry)
                                        };
                                        // Convert UTF-16LE name to 8.3 for comparison
                                        let mut fat_name = [0u8; 11];
                                        let mut pos = 0;
                                        for ci in 0..name_len.min(15) {
                                            let ch = name_entry.name_string[ci] as u8;
                                            if ch == b'.' {
                                                // Handle extension
                                                while pos < 8 { fat_name[pos] = b' '; pos += 1; }
                                                continue;
                                            }
                                            if pos < 11 {
                                                fat_name[pos] = ch.to_ascii_uppercase();
                                                pos += 1;
                                            }
                                        }
                                        while pos < 11 { fat_name[pos] = b' '; pos += 1; }
                                        if name_match(&fat_name, name) {
                                            return Some((first_cluster, data_len));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// Trae el archivo entero a `dst`. Devuelve **cuantos bytes llegaron DE
    /// VERDAD**, que puede ser menos que `file_size` si la lectura se corto.
    ///
    /// == ** Un sector que NO se pudo leer PARA la lectura ==
    ///
    /// Aqui habia un `if self.read_sector(...) { copiar }` **sin `else`**, y el
    /// `offset += count` de despues corria igual. O sea que un sector que
    /// fallaba dejaba su trozo de `dst` con lo que hubiera antes --basura, o
    /// peor: los bytes del programa anterior-- y esta funcion contestaba el
    /// tamano completo, como si todo hubiera ido bien.
    ///
    /// Para un `.txt` de dos lineas eso es un caracter raro. Para un `.bex` de
    /// 814 KiB son **1.591 lecturas de sector** y basta con que una falle: el
    /// cargador recibe una imagen del tamano correcto **con un agujero dentro**,
    /// y lo que rechaza despues no se parece en nada a la causa.
    ///
    /// Cortar y devolver lo que se tiene convierte ese fallo mudo en uno que se
    /// cuenta: el que llama compara con el tamano que pidio. Y para el `.bex`,
    /// ademas, la imagen declara su propio tamano en la cabecera, asi que el
    /// cargador lo caza con nombre (`BexError::ImagenIncompleta`).
    ///
    /// [!] Lo que esto NO caza es un sector que se lee "bien" y trae datos
    /// equivocados. Para eso hace falta el HASH por seccion (`SectionHash`, en
    /// `bef/signing.rs`), que existe escrito y todavia no lo cablea nadie.
    /// == ** DE UN SECTOR POR COMANDO A UN CLUSTER POR COMANDO (2026-08-10) ==
    ///
    /// Escalon 3 de `LA_RAM.md`, la mitad que vive aqui. Esto leia **de 512 en
    /// 512** y siempre a `self.buf`, para copiar de ahi a `dst`. Con un `.bex`
    /// de 813 KB eso son **1.590 comandos al disco y 1.590 copias**, y cada
    /// comando es armar el FIS, tocar MMIO y esperar a que el HBA conteste.
    ///
    /// El contrato de almacenamiento ya aceptaba varios sectores de una vez
    /// --`BlockReader` recibe `count`-- y nadie lo usaba. Otra vez el mecanismo
    /// escrito y sin lector.
    ///
    /// Ahora se lee **el tramo entero que quepa, directo a `dst`**: un comando
    /// por cluster en vez de uno por sector, y **cero copias intermedias**. El
    /// rebote solo queda para el ultimo sector cuando el fichero no acaba en
    /// frontera de 512, que es donde de verdad hace falta: ahi el disco entrega
    /// 512 bytes y el llamante solo quiere una parte.
    /// **Lee un TROZO y dice por donde iba.** `(bytes leidos, cluster siguiente)`.
    ///
    /// === Por que hace falta que la lectura se pueda parar a la mitad ===
    ///
    /// `read_file` no vuelve hasta que el archivo entero esta en RAM. Para un
    /// `.bex` de 813 KB eso es el kernel dentro de una funcion durante toda la
    /// lectura, y el que la pidio no existe mientras tanto.
    ///
    /// ** Esto la parte en pasos. Cada llamada trae como mucho `tope` bytes y
    /// devuelve **el cluster por el que iba**, asi que la siguiente sigue donde
    /// esta lo dejo. Entre paso y paso el que pidio puede dormirse, y el resto
    /// del sistema corre.
    ///
    /// El cursor es el CLUSTER y no un offset: seguir la cadena desde el
    /// principio en cada llamada seria recorrer el archivo entero por cada
    /// trozo -- cuadratico, y justo en el caso que se queria arreglar.
    ///
    /// `siguiente == 0` significa que se acabo: o la cadena o el archivo.
    pub fn leer_tramo(
        &mut self,
        cluster: u32,
        ya: usize,
        file_size: u32,
        dst: &mut [u8],
        tope: usize,
    ) -> (usize, u32) {
        let mut cluster = cluster;
        let mut offset = ya;
        let spc = self.sectors_per_cluster as usize;
        let del_cluster = spc * 512;
        let fin = (file_size as usize).min(dst.len());
        // [!] CONTRATO: `ya` cae en frontera de cluster. Cada vuelta consume un
        // cluster ENTERO --o la cola del archivo, que lo termina-- y por eso el
        // cursor puede ser un solo numero. Empezar a mitad de cluster pediria
        // llevar tambien el desplazamiento dentro de el, y dos cursores que
        // tienen que cuadrar son dos cursores que un dia no cuadran.
        debug_assert!(ya % del_cluster == 0, "leer_tramo empieza en frontera de cluster");
        while offset < fin {
            // El presupuesto se mira ANTES de empezar un cluster, no en medio:
            // asi nunca se deja uno a medias y el cursor sigue siendo el cluster.
            if offset >= ya + tope {
                return (offset - ya, cluster);
            }
            let lba = self.cluster_to_lba(cluster);
            let de_este = (fin - offset).min(del_cluster);
            let enteros = de_este / 512;
            if enteros > 0 {
                let n = enteros * 512;
                if !self.leer_directo(lba, enteros as u16, &mut dst[offset..offset + n]) {
                    return (offset - ya, 0);
                }
            }
            // El rabo, con la MISMA guarda que `read_file`: solo si queda sector
            // en este cluster. Ver alli por que sin ella se lee el sector fisico
            // siguiente, que no tiene por que ser el de la cadena.
            let rabo = de_este - enteros * 512;
            if rabo > 0 && enteros < spc {
                let desde = offset + enteros * 512;
                let ok = unsafe {
                    if self.read_sector(lba + enteros as u64, Buf::buf) {
                        dst[desde..desde + rabo].copy_from_slice(&self.buf[..rabo]);
                        true
                    } else {
                        false
                    }
                };
                if !ok {
                    return (offset - ya, 0);
                }
            }
            offset += de_este;
            if offset >= fin {
                // Se acabo el archivo. `0` = no hay por donde seguir, que es mas
                // honesto que devolver un cluster que ya no vale para nada.
                return (offset - ya, 0);
            }
            cluster = match self.read_fat_entry(cluster) {
                Some(c) => c,
                None => return (offset - ya, 0),
            };
        }
        (offset - ya, 0)
    }

    /// **En que LBA DEL DISCO empieza un cluster.** La misma cuenta que usa toda
    /// lectura --traduccion incluida-- expuesta para poder decirla.
    ///
    /// [!] Decia "absoluto" y devolvia el relativo al volumen. La foto del
    /// 2026-08-11 lo enseno y nadie lo leyo asi: `LBA =0x11040` con `la particion
    /// empieza en =0x12C800` es un sector **anterior al principio de su propia
    /// particion**, o sea imposible -- y aun asi paso por "razonable" porque la
    /// linea prometia un numero que no daba.
    ///
    /// > Un diagnostico que miente cuesta mas que no tenerlo: manda a buscar el
    /// > fallo al otro lado del mapa. Ahora este numero se puede comparar con el
    /// > de `disk` y con el de la GPT sin sumar nada de cabeza.
    pub fn lba_de_cluster(&self, cluster: u32) -> u64 {
        self.abs(self.cluster_to_lba(cluster))
    }

    /// Abre un cursor al principio de un archivo.
    pub fn cursor(&self, primer_cluster: u32) -> Cursor {
        Cursor { cluster: primer_cluster, base: 0 }
    }

    /// **Coloca el cursor en el cluster que contiene `offset`.**
    ///
    /// `false` si se pide hacia atras o si la cadena se acaba antes. Lo segundo
    /// es un fichero mas corto de lo que su entrada de directorio dice, que es
    /// una FAT rota -- y se contesta que no en vez de leer un cluster ajeno.
    pub fn situar(&mut self, cur: &mut Cursor, offset: usize) -> bool {
        let del_cluster = self.sectors_per_cluster as usize * 512;
        if del_cluster == 0 || offset < cur.base {
            return false;
        }
        while offset >= cur.base + del_cluster {
            match self.read_fat_entry(cur.cluster) {
                Some(c) if c >= 2 => {
                    cur.cluster = c;
                    cur.base += del_cluster;
                }
                _ => return false,
            }
        }
        true
    }

    /// **Lee `dst.len()` bytes del archivo empezando en `offset`.** Devuelve
    /// cuantos entraron.
    ///
    /// `size` es lo que mide el archivo: se para ahi aunque `dst` sea mas grande.
    ///
    /// == Los tres trozos, y por que se cuentan ==
    ///
    /// Un rango cualquiera dentro de un cluster tiene hasta tres partes:
    ///
    /// ```text
    ///    |<-- cabeza -->|<---- sectores enteros ---->|<-- cola -->|
    ///    ^ empieza a mitad de sector      acaba a mitad de sector ^
    /// ```
    ///
    /// La cabeza y la cola pasan por el sector de rebote y se copian; **los
    /// enteros van directos a `dst`**, que es el camino que la pieza B usa para
    /// que el disco escriba en el marco sin intermediario.
    ///
    /// ** Y por eso las secciones cargables se alinean a 512 en el fichero desde
    /// el 2026-08-10: con esa alineacion, una seccion **no tiene cabeza**, y sus
    /// marcos son todos sectores enteros. Sin ella este camino funciona igual,
    /// solo que rebotando -- que es el mismo trato de siempre: correcto siempre,
    /// rapido cuando el formato ayuda.
    pub fn leer_en(
        &mut self,
        cur: &mut Cursor,
        offset: usize,
        size: u32,
        dst: &mut [u8],
    ) -> usize {
        let del_cluster = self.sectors_per_cluster as usize * 512;
        if del_cluster == 0 {
            return 0;
        }
        let fin = (size as usize).min(offset.saturating_add(dst.len()));
        if offset >= fin || !self.situar(cur, offset) {
            return 0;
        }

        let mut pos = offset;
        while pos < fin {
            if !self.situar(cur, pos) {
                return pos - offset;
            }
            let dentro = pos - cur.base; // donde caemos DENTRO del cluster
            let de_este = (fin - pos).min(del_cluster - dentro);
            let lba = self.cluster_to_lba(cur.cluster);

            // -- La cabeza: lo que va desde mitad de sector hasta su final --
            let sector = dentro / 512;
            let en_sector = dentro % 512;
            let mut hecho = 0usize;
            if en_sector != 0 {
                let n = (512 - en_sector).min(de_este);
                let ok = unsafe {
                    if self.read_sector(lba + sector as u64, Buf::buf) {
                        let d = pos - offset;
                        dst[d..d + n].copy_from_slice(&self.buf[en_sector..en_sector + n]);
                        true
                    } else {
                        false
                    }
                };
                if !ok {
                    return pos - offset;
                }
                hecho += n;
            }

            // -- Los sectores ENTEROS, directos a `dst` --
            let enteros = (de_este - hecho) / 512;
            if enteros > 0 {
                let d = pos - offset + hecho;
                let n = enteros * 512;
                let sec = (dentro + hecho) / 512;
                if !self.leer_directo(lba + sec as u64, enteros as u16, &mut dst[d..d + n]) {
                    return pos - offset + hecho;
                }
                hecho += n;
            }

            // -- La cola: lo que sobra sin llegar a un sector --
            let cola = de_este - hecho;
            if cola > 0 {
                let sec = (dentro + hecho) / 512;
                let ok = unsafe {
                    if self.read_sector(lba + sec as u64, Buf::buf) {
                        let d = pos - offset + hecho;
                        dst[d..d + cola].copy_from_slice(&self.buf[..cola]);
                        true
                    } else {
                        false
                    }
                };
                if !ok {
                    return pos - offset + hecho;
                }
                hecho += cola;
            }

            pos += hecho;
            if hecho == 0 {
                // Ni un byte en una vuelta entera es un bucle infinito esperando.
                // No deberia poder pasar --`de_este` es siempre > 0 aqui-- y por
                // eso se corta en vez de confiarlo.
                return pos - offset;
            }
        }
        pos - offset
    }

    pub fn read_file(&mut self, first_cluster: u32, file_size: u32, dst: &mut [u8]) -> usize {
        let mut cluster = first_cluster;
        let mut offset = 0;
        let spc = self.sectors_per_cluster as usize;
        let tope = (file_size as usize).min(dst.len());
        while offset < tope {
            let lba = self.cluster_to_lba(cluster);
            // Lo que queda de este cluster, y lo que queda por leer.
            let del_cluster = spc * 512;
            let queda = tope - offset;
            // Sectores ENTEROS que caben en los dos: son los que pueden ir
            // directos. `dst` recibe exactamente lo que el disco entrega.
            let enteros = (queda.min(del_cluster)) / 512;
            if enteros > 0 {
                let n = enteros * 512;
                let leidos = self.leer_directo(lba, enteros as u16, &mut dst[offset..offset + n]);
                if !leidos {
                    // Se para aqui. Lo leido hasta ahora es bueno; lo que sigue
                    // no se sabe, y no saberlo se dice devolviendo menos, no
                    // rellenando el hueco con lo que hubiera.
                    return offset;
                }
                offset += n;
            }
            // El rabo: menos de un sector. Aqui SI hace falta el rebote, porque
            // el disco entrega 512 bytes y solo se quieren los primeros.
            //
            // El sector es `enteros`: cada vuelta consume UN cluster completo o
            // termina, asi que al entrar `offset` siempre cae en frontera de
            // cluster y los sectores ya leidos de este son exactamente `enteros`.
            //
            // [!] Y **solo si queda sector en ESTE cluster** (`enteros < spc`).
            // Sin esa condicion, un `dst` que se acaba justo en frontera de
            // cluster leeria `lba + spc`, que es el primer sector del SIGUIENTE
            // cluster en el disco -- y el siguiente cluster de la cadena no
            // tiene por que estar ahi. Devolveria bytes de otro archivo sin que
            // nada fallara.
            let queda = tope - offset;
            if queda > 0 && queda < 512 && enteros < spc {
                let sector_en_cluster = enteros as u64;
                let ok = unsafe {
                    if self.read_sector(lba + sector_en_cluster, Buf::buf) {
                        dst[offset..offset + queda].copy_from_slice(&self.buf[..queda]);
                        true
                    } else {
                        false
                    }
                };
                if !ok {
                    return offset;
                }
                offset += queda;
            }
            if offset >= tope {
                break;
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => break };
        }
        offset
    }

    /// Find a free cluster in the FAT.
    /// Busca un cluster libre DENTRO de los que existen.
    ///
    /// El tope `max_cluster` no es cosmetico: sin el, el relleno a cero del
    /// final de la FAT se lee como espacio libre y se acaba escribiendo fuera
    /// del volumen. Ver la nota del campo.
    fn find_free_cluster(&mut self) -> Option<u32> {
        for sector in 0..self.fat_size_sectors {
            unsafe {
                if !self.read_sector((self.fat_start + sector) as u64, Buf::fat_cache) { continue; }
            }
            for i in 0..(512/4) {
                let cluster = sector * (512/4) as u32 + i as u32;
                if cluster < 2 { continue; }
                if cluster > self.max_cluster { return None; }
                let entry = u32::from_le_bytes([
                    self.fat_cache[i*4], self.fat_cache[i*4+1],
                    self.fat_cache[i*4+2], self.fat_cache[i*4+3],
                ]) & 0x0FFF_FFFF;
                if entry == 0 { return Some(cluster); }
            }
        }
        None
    }

    /// Lee la entrada de la FAT tal cual, sin interpretar. `read_fat_entry`
    /// traduce "0" y "fin de cadena" a `None`, que sirve para RECORRER una
    /// cadena pero no para saber si un cluster esta libre.
    fn raw_fat_entry(&mut self, cluster: u32) -> Option<u32> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start + (fat_offset / 512);
        let idx = (fat_offset % 512) as usize;
        unsafe {
            if !self.read_sector(fat_sector as u64, Buf::fat_cache) { return None; }
        }
        Some(u32::from_le_bytes([self.fat_cache[idx], self.fat_cache[idx+1],
            self.fat_cache[idx+2], self.fat_cache[idx+3]]) & 0x0FFF_FFFF)
    }

    /// Escribe una entrada de la FAT en TODAS las copias.
    ///
    /// Actualizar solo la primera deja el volumen incoherente: cualquier
    /// sistema que lea la segunda copia --o un chequeo de disco-- vera una
    /// cadena distinta de la real.
    fn set_fat_entry(&mut self, cluster: u32, value: u32) -> bool {
        if cluster < 2 || cluster > self.max_cluster { return false; }
        let fat_offset = cluster * 4;
        let idx = (fat_offset % 512) as usize;
        let sectors_from_fat_start = fat_offset / 512;
        let v = value & 0x0FFF_FFFF;

        for copy in 0..self.num_fats as u32 {
            let fat_sector = self.fat_start + copy * self.fat_size_sectors + sectors_from_fat_start;
            unsafe {
                if !self.read_sector(fat_sector as u64, Buf::fat_cache) { return false; }
            }
            self.fat_cache[idx]   = v as u8;
            self.fat_cache[idx+1] = (v >> 8) as u8;
            self.fat_cache[idx+2] = (v >> 16) as u8;
            self.fat_cache[idx+3] = (v >> 24) as u8;
            unsafe {
                if !self.write_sector(fat_sector as u64, Buf::fat_cache) { return false; }
            }
        }
        true
    }

    /// Marca un cluster como fin de cadena en todas las copias de la FAT.
    fn mark_cluster_eoc(&mut self, cluster: u32) -> bool {
        self.set_fat_entry(cluster, 0x0FFF_FFFF)
    }

    /// Suelta una cadena de clusters entera. Se usa para deshacer una reserva
    /// a medias: si el disco se llena en mitad de un archivo, lo ya cogido se
    /// devuelve en vez de quedar perdido para siempre.
    fn free_chain(&mut self, first: u32) {
        let mut c = first;
        let mut guard = 0u32;
        while c >= 2 && c <= self.max_cluster {
            // * `unwrap_or(0)` hacia que esta funcion hiciera LO CONTRARIO de
            // lo que existe para hacer, y en silencio: si la FAT no se podia
            // leer, `next` valia 0, la comprobacion de abajo lo tomaba por fin
            // de cadena y se salia dejando perdidos justo los clusters que
            // venia a devolver. "No se pudo leer" y "aqui se acaba la cadena"
            // no son lo mismo y ya no comparten valor.
            let next = match self.raw_fat_entry(c) {
                Some(n) => n,
                None => {
                    self.fallos_mudos = self.fallos_mudos.saturating_add(1);
                    return;
                }
            };
            if !self.set_fat_entry(c, 0) { return; }
            if next < 2 || next >= 0x0FFF_FFF7 { return; }
            c = next;
            // Una FAT corrupta puede tener un ciclo; no se gira para siempre.
            guard += 1;
            if guard > self.max_cluster { return; }
        }
    }

    /// Find a free directory entry in a directory (by first cluster).
    /// Returns (sector_lba, byte_offset_in_sector).
    fn find_free_dir_entry_in(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        match self.fs_type {
            FsType::Fat32 => self.find_free_dir_entry_fat32(dir_cluster),
            FsType::ExFat => self.find_free_dir_entry_exfat(dir_cluster),
        }
    }

    fn find_free_dir_entry_fat32(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512/32) {
                    unsafe {
                        let de = &*entries.add(i);
                        if de.name[0] == 0 || de.name[0] == 0xE5 {
                            return Some((lba + s, i * 32));
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// exFAT: find 3 consecutive free entry slots for File + Stream + Filename
    fn find_free_dir_entry_exfat(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                // Need 3 consecutive free slots (File=0x85, Stream=0xC0, Name=0xC1)
                for i in 0..(512/32 - 2) {
                    let offset = i * 32;
                    let t0 = self.buf[offset];
                    let t1 = self.buf[offset + 32];
                    let t2 = self.buf[offset + 64];
                    if (t0 == 0x00 || t0 == 0x05) && (t1 == 0x00 || t1 == 0x05) && (t2 == 0x00 || t2 == 0x05) {
                        return Some((lba + s, offset));
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// Find a subdirectory by name in the root directory.
    /// Returns the first cluster of the subdirectory.
    pub fn find_subdir(&mut self, name: &[u8]) -> Option<u32> {
        self.find_subdir_in(name, self.root_cluster)
    }

    /// Find a subdirectory by name in a specific directory (by first cluster).
    pub fn find_subdir_in(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        match self.fs_type {
            FsType::Fat32 => self.find_subdir_fat32(name, dir_cluster),
            FsType::ExFat => self.find_subdir_exfat(name, dir_cluster),
        }
    }

    fn find_subdir_fat32(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512/32) {
                    unsafe {
                        let de = &*entries.add(i);
                        if de.name[0] == 0 { return None; }
                        if de.name[0] == 0xE5 { continue; }
                        if de.attr & 0x10 == 0 { continue; } // not a directory
                        if name_match(&de.name, name) {
                            let fc = (de.first_cluster_hi as u32) << 16 | de.first_cluster_lo as u32;
                            return Some(fc);
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    fn find_subdir_exfat(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                for i in 0..16 {
                    let entry_offset = i * 32;
                    let entry_type = self.buf[entry_offset];
                    if entry_type == 0x00 { return None; }
                    if entry_type == 0x05 { continue; }
                    if entry_type == 0x85 {
                        let file_entry = unsafe {
                            &*(self.buf[entry_offset..].as_ptr() as *const ExFatFileEntry)
                        };
                        let secondary_count = file_entry.secondary_count;
                        let is_dir = file_entry.file_attributes & 0x10 != 0;
                        for sec in 1..=secondary_count {
                            let sec_offset = entry_offset + (sec as usize) * 32;
                            if sec_offset + 32 > 512 { break; }
                            let sec_type = self.buf[sec_offset];
                            if sec_type == 0xC0 {
                                let stream = unsafe {
                                    &*(self.buf[sec_offset..].as_ptr() as *const ExFatStreamEntry)
                                };
                                let first_cluster = stream.first_cluster;
                                let name_len = stream.name_length as usize;
                                if sec + 1 <= secondary_count {
                                    let name_offset = entry_offset + ((sec + 1) as usize) * 32;
                                    if name_offset + 32 <= 512 && self.buf[name_offset] == 0xC1 {
                                        let name_entry = unsafe {
                                            &*(self.buf[name_offset..].as_ptr() as *const ExFatNameEntry)
                                        };
                                        let mut fat_name = [0u8; 11];
                                        let mut pos = 0;
                                        for ci in 0..name_len.min(15) {
                                            let ch = name_entry.name_string[ci] as u8;
                                            if ch == b'.' {
                                                while pos < 8 { fat_name[pos] = b' '; pos += 1; }
                                                continue;
                                            }
                                            if pos < 11 {
                                                fat_name[pos] = ch.to_ascii_uppercase();
                                                pos += 1;
                                            }
                                        }
                                        while pos < 11 { fat_name[pos] = b' '; pos += 1; }
                                        if is_dir && name_match(&fat_name, name) {
                                            return Some(first_cluster);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// Get the root directory's first cluster.
    pub fn root_cluster(&self) -> u32 { self.root_cluster }

    /// La entrada numero `n` de un directorio: `(name 8.3, es_dir, tamano)`.
    ///
    /// Devuelve `None` cuando se acaban. Existia `find_file_in` --buscar un
    /// nombre que ya conoces-- pero no habia forma de PREGUNTAR QUE HAY, y sin
    /// eso no puede haber un `ls` ni iconos de carpeta: hay que saberse los
    /// nombres de memoria.
    ///
    /// Se salta las borradas (0xE5), las entradas de nombre largo (attr 0x0F)
    /// y la etiqueta de volumen (0x08). Los nombres salen en 8.3 CRUDO, con
    /// sus espacios de relleno: convertirlos a algo legible es decision de
    /// presentacion y no le toca a un driver de disco.
    ///
    /// Indexar por numero en vez de llevar un cursor es O(n) por llamada, y
    /// listar un directorio entero sale O(n^2). Con directorios de decenas de
    /// entradas eso es irrelevante, y a cambio el driver se queda SIN ESTADO:
    /// dos listados a la vez no se pisan, y una entrada que desaparece no deja
    /// un cursor apuntando al vacio.
    pub fn entry_at(&mut self, dir_cluster: u32, n: usize) -> Option<([u8; 11], bool, u32)> {
        if !matches!(self.fs_type, FsType::Fat32) {
            return None;
        }
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        let mut vistas = 0usize;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if !self.read_sector(lba + s, Buf::buf) { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512 / 32) {
                    let de = unsafe { &*entries.add(i) };
                    // 0x00 = fin del directorio: no hay nada mas, nunca.
                    if de.name[0] == 0 { return None; }
                    if de.name[0] == 0xE5 { continue; }
                    let attr = de.attr;
                    if attr & 0x0F == 0x0F { continue; } // fragmento de nombre largo
                    if attr & 0x08 != 0 { continue; }    // etiqueta de volumen
                    if vistas == n {
                        return Some((de.name, attr & 0x10 != 0, de.file_size));
                    }
                    vistas += 1;
                }
            }
            cluster = self.read_fat_entry(cluster)?;
        }
    }

    /// Crea un archivo dentro de un directorio, dado su primer cluster.
    ///
    /// `name_8_3` son once bytes: ocho de nombre y tres de extension, rellenos
    /// con espacios. Es feo y es lo que hay, FAT lo guarda asi.
    ///
    /// Devuelve el MOTIVO cuando falla. La version anterior devolvia `bool` y
    /// ademas mentia: escribia como mucho UN cluster y apuntaba en el
    /// directorio el tamano completo, asi que cualquier archivo mas grande que
    /// un cluster quedaba registrado con un tamano que sus datos no
    /// respaldaban. Eso no es "incompleto", es un archivo corrupto que parece
    /// bueno hasta que alguien lo lee.
    pub fn create_file_in_dir(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8])
        -> Result<(), WriteError>
    {
        if self.write.is_none() { return Err(WriteError::ReadOnly); }
        match self.fs_type {
            FsType::Fat32 => self.create_file_fat32(dir_cluster, name_8_3, data),
            // El creador de exFAT arrastra las mismas costuras que tenia el de
            // FAT32 y no se ha revisado contra la spec. Se dice, no se
            // disimula: BMO escribe FAT32 hoy.
            FsType::ExFat => Err(WriteError::Unsupported),
        }
    }

    /// **Guarda, exista o no**: crea si el nombre esta libre y REEMPLAZA si ya
    /// esta. Es lo que significa `OPEN OUTPUT` y lo que hace un `>` de shell.
    ///
    /// === Por que no es `create_file_in_dir` con un flag ===
    ///
    /// Porque son dos operaciones con riesgos distintos y quien llama tiene
    /// derecho a elegir. Crear un archivo nuevo no puede destruir nada;
    /// reemplazar SI -- se lleva por delante lo que hubiera. Un `bool` al final
    /// de la lista de argumentos es la forma clasica de borrar un fichero
    /// creyendo que lo estabas creando.
    ///
    /// `create_file_in_dir` sigue rechazando con `Exists`, y quien quiera
    /// pisar tiene que decirlo llamando aqui.
    pub fn save_file_in_dir(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8])
        -> Result<(), WriteError>
    {
        if self.write.is_none() { return Err(WriteError::ReadOnly); }
        match self.fs_type {
            FsType::Fat32 => match self.find_entry_fat32_from(name_8_3, dir_cluster) {
                Some(vieja) => self.replace_file_fat32(vieja, data),
                None => self.create_file_fat32(dir_cluster, name_8_3, data),
            },
            FsType::ExFat => Err(WriteError::Unsupported),
        }
    }

    /// Apunta una entrada que YA EXISTE a un contenido nuevo.
    ///
    /// === * El ORDEN, que es lo unico que importa aqui ===
    ///
    /// Reemplazar tiene tres pasos y **solo un orden es seguro**:
    ///
    /// 1. Escribir la cadena NUEVA entera, sin tocar la vieja.
    /// 2. Apuntar la entrada de directorio a la nueva (**un solo sector**).
    /// 3. Y AHORA soltar la cadena vieja.
    ///
    /// Un corte de corriente entre 1 y 2 deja unos clusters marcados como
    /// ocupados que no son de nadie --una fuga, molesta y recuperable-- y el
    /// archivo **sigue entero con su contenido de antes**. Un corte entre 2 y 3
    /// deja la fuga al reves, con el contenido nuevo ya en pie. En ningun
    /// instante hay un nombre apuntando a datos a medias.
    ///
    /// El orden tentador --soltar lo viejo primero para tener sitio-- es el que
    /// convierte un corte de luz en un archivo perdido. **Escribir encima de la
    /// cadena vieja es peor todavia**: durante la escritura el archivo no es ni
    /// el de antes ni el de ahora, y si falla a mitad no queda ninguno de los
    /// dos.
    ///
    /// === Lo que esto CUESTA, dicho ===
    ///
    /// Durante el paso 1 el volumen aguanta **las dos copias a la vez**.
    /// Reemplazar un archivo de 1 GiB pide 1 GiB libre aunque el archivo final
    /// mida lo mismo que el que sustituye. Es el precio de no poder perderlo, y
    /// se paga a gusto: la alternativa barata es la que pierde datos.
    fn replace_file_fat32(&mut self, vieja: EntradaDir, data: &[u8]) -> Result<(), WriteError> {
        // 1. La cadena nueva, entera y antes de tocar nada.
        let nueva = self.escribir_cadena(data)?;

        // 2. La entrada, de un solo sector. Se relee para no pisar a los
        //    vecinos con lo que hubiera quedado en el buffer.
        // Se RELEE el sector aunque `find_entry_fat32_from` lo dejara en `buf`
        // hace un momento. Apoyarse en eso seria un acoplamiento invisible:
        // `escribir_cadena` no tiene ninguna obligacion de respetar `buf` --hoy
        // usa `fat_cache`, y el dia que eso cambie el reemplazo escribiria en
        // el directorio lo que hubiera quedado en el buffer. Releer cuesta un
        // sector; el fallo que evita se lleva a los quince archivos vecinos.
        unsafe {
            if !self.read_sector(vieja.lba, Buf::buf) {
                self.free_chain(nueva);
                return Err(WriteError::Io);
            }
        }
        // Solo cambian el puntero y el tamano: el nombre y los atributos son
        // los que ya habia, y reescribirlos seria inventarse una entrada nueva
        // encima de una que ya estaba bien.
        let de = unsafe { &mut *(self.buf.as_mut_ptr().add(vieja.offset) as *mut DirEntry) };
        de.first_cluster_hi = (nueva >> 16) as u16;
        de.first_cluster_lo = (nueva & 0xFFFF) as u16;
        de.file_size = data.len() as u32;

        if !unsafe { self.write_sector(vieja.lba, Buf::buf) } {
            // El directorio sigue apuntando a lo viejo, que sigue entero. Lo
            // nuevo se suelta y no ha pasado nada.
            self.free_chain(nueva);
            return Err(WriteError::Io);
        }

        // 3. Y ahora si. Si esto falla a medias es una fuga de clusters, no una
        //    perdida de datos: el archivo ya es el nuevo y esta completo.
        if vieja.first_cluster >= 2 {
            self.free_chain(vieja.first_cluster);
        }
        Ok(())
    }

    /// Reserva y escribe la cadena de clusters de `data`. Devuelve el primero.
    ///
    /// Cada cluster se marca como fin de cadena en cuanto se coge: asi la
    /// siguiente busqueda de hueco ya no lo ve libre y no se entrega dos veces.
    /// Si algo falla a mitad, se suelta lo cogido -- un archivo a medias es un
    /// error; unos clusters marcados como ocupados que ya no pertenecen a nadie
    /// son una fuga permanente.
    ///
    /// **No toca el directorio.** Quien llama decide si el nombre que va a
    /// apuntar aqui es uno nuevo (`create`) o uno que ya existia (`replace`), y
    /// esa diferencia es toda la que hay entre las dos.
    fn escribir_cadena(&mut self, data: &[u8]) -> Result<u32, WriteError> {
        let spc = self.sectors_per_cluster as usize;
        if spc == 0 { return Err(WriteError::Io); }
        let cluster_bytes = spc * 512;
        let clusters_needed = if data.is_empty() { 1 } else { data.len().div_ceil(cluster_bytes) };

        let first = match self.find_free_cluster() {
            Some(c) => c, None => return Err(WriteError::NoSpace),
        };
        if !self.mark_cluster_eoc(first) { return Err(WriteError::Io); }

        let mut prev = first;
        for i in 0..clusters_needed {
            let cluster = if i == 0 { first } else {
                let c = match self.find_free_cluster() {
                    Some(c) => c,
                    None => { self.free_chain(first); return Err(WriteError::NoSpace); }
                };
                if !self.mark_cluster_eoc(c) || !self.set_fat_entry(prev, c) {
                    self.free_chain(first);
                    return Err(WriteError::Io);
                }
                prev = c;
                c
            };

            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                // El buffer se reinicia a CEROS en cada sector. Reutilizar uno
                // sucio dejaba la cola del ultimo sector --y el resto del
                // cluster-- llena de los datos anteriores, justo donde el
                // comentario prometia ceros.
                let mut temp = [0u8; 512];
                let off = i * cluster_bytes + s * 512;
                if off < data.len() {
                    let n = core::cmp::min(512, data.len() - off);
                    temp[..n].copy_from_slice(&data[off..off + n]);
                }
                if !self.write_from(lba + s as u64, &temp) {
                    self.free_chain(first);
                    return Err(WriteError::Io);
                }
            }
        }
        Ok(first)
    }

    fn create_file_fat32(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8])
        -> Result<(), WriteError>
    {
        // Un nombre repetido deja dos entradas iguales en el directorio: la
        // segunda es inalcanzable y sus clusters, perdidos.
        if self.find_file_in(name_8_3, dir_cluster).is_some() {
            return Err(WriteError::Exists);
        }

        let first = self.escribir_cadena(data)?;

        // -- La entrada de directorio, lo ultimo --
        //
        // Se apunta cuando los datos YA estan en el disco. Al reves, un corte
        // entre ambos pasos dejaria un nombre visible apuntando a basura.
        let (dir_lba, dir_off) = match self.find_free_dir_entry_in(dir_cluster) {
            Some(v) => v,
            None => { self.free_chain(first); return Err(WriteError::DirFull); }
        };

        unsafe {
            if !self.read_sector(dir_lba, Buf::buf) {
                self.free_chain(first);
                return Err(WriteError::Io);
            }
        }
        let cluster = first;

        // Write directory entry
        let de = unsafe { &mut *(self.buf.as_mut_ptr().add(dir_off) as *mut DirEntry) };
        de.name = *name_8_3;
        de.attr = 0x20; // Archive
        de._nt_reserved = 0;
        de._create_time_tenth = 0;
        de.create_time = 0;
        de.create_date = 0;
        de.last_access = 0;
        de.first_cluster_hi = (cluster >> 16) as u16;
        de.write_time = 0;
        de.write_date = 0;
        de.first_cluster_lo = (cluster & 0xFFFF) as u16;
        de.file_size = data.len() as u32;

        let written = unsafe { self.write_sector(dir_lba, Buf::buf) };
        if !written {
            self.free_chain(first);
            return Err(WriteError::Io);
        }
        Ok(())
    }

    /// exFAT: create file with 3 entries: File(0x85) + Stream(0xC0) + Filename(0xC1)
    ///
    /// SIN CABLEAR: `create_file_in_dir` devuelve `Unsupported` para exFAT. Se
    /// conserva porque la estructura de las tres entradas es trabajo hecho y
    /// correcto, pero arrastra la misma limitacion de un solo cluster que se
    /// acaba de corregir en FAT32. Cablearlo = darle el mismo repaso.
    #[allow(dead_code)]
    fn create_file_exfat(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8]) -> bool {
        let cluster = match self.find_free_cluster() {
            Some(c) => c, None => return false,
        };

        // Write data to cluster
        let lba = self.cluster_to_lba(cluster);
        let spc = self.sectors_per_cluster as u64;
        let total_sectors = (data.len() as u64 + 511) / 512;
        let write_n = total_sectors.min(spc);

        let mut temp = [0u8; 512];
        for s in 0..write_n {
            let off = (s * 512) as usize;
            let count = core::cmp::min(512, data.len().saturating_sub(off));
            temp[..count].copy_from_slice(&data[off..off + count]);
            unsafe {
                if !self.write_from(lba + s, &temp) { return false; }
            }
        }
        // El relleno de la cola del cluster. Aqui un fallo NO puede devolver
        // `false` --los datos del archivo ya estan escritos y el llamante
        // creeria que no--, pero tampoco puede desaparecer: era un `let _ =`.
        // Se cuenta, y `fallos_mudos` es lo que hay que mirar cuando el disco
        // "va bien" y algo no cuadra.
        for s in write_n..spc {
            let ok = unsafe { self.write_from(lba + s, &temp) };
            if !ok {
                self.fallos_mudos = self.fallos_mudos.saturating_add(1);
            }
        }

        if !self.mark_cluster_eoc(cluster) { return false; }

        // Find 3 consecutive free slots
        let (dir_lba, dir_off) = match self.find_free_dir_entry_in(dir_cluster) {
            Some(v) => v, None => return false,
        };

        // Read directory sector
        unsafe {
            if !self.read_sector(dir_lba, Buf::buf) { return false; }
        }

        // Convert 8.3 name to UTF-16LE (up to 15 chars)
        let mut utf16_name = [0u16; 15];
        let mut name_len: usize = 0;
        for &b in name_8_3.iter() {
            if b == b' ' || b == 0 { break; }
            utf16_name[name_len] = b as u16;
            name_len += 1;
        }

        let _zero32 = [0u8; 32];

        // Entry 1: File Directory Entry (0x85)
        let file_entry = ExFatFileEntry {
            entry_type: 0x85,
            secondary_count: 2,
            set_checksum: 0,
            file_attributes: 0x20, // Archive
            _reserved1: 0,
            create_timestamp: 0,
            last_modified_timestamp: 0,
            last_accessed_timestamp: 0,
            _create_millis: 0,
            _last_modified_millis: 0,
            _create_utc_offset: 0,
            _last_modified_utc_offset: 0,
            _last_accessed_utc_offset: 0,
            _reserved2: [0; 7],
        };
        self.buf[dir_off..dir_off + 32].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&file_entry as *const _ as *const u8, 32)
        });

        // Entry 2: Stream Extension Entry (0xC0)
        let stream_entry = ExFatStreamEntry {
            entry_type: 0xC0,
            general_secondary_flags: 0x01,
            _reserved1: 0,
            _reserved2: 0,
            name_length: name_len as u8,
            name_hash: 0,
            _reserved3: 0,
            valid_data_length: data.len() as u64,
            _reserved4: 0,
            first_cluster: cluster,
            data_length: data.len() as u64,
        };
        self.buf[dir_off + 32..dir_off + 64].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&stream_entry as *const _ as *const u8, 32)
        });

        // Entry 3: Filename Entry (0xC1)
        let name_entry = ExFatNameEntry {
            entry_type: 0xC1,
            general_secondary_flags: 0x01,
            name_string: utf16_name,
        };
        self.buf[dir_off + 64..dir_off + 96].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&name_entry as *const _ as *const u8, 32)
        });

        unsafe {
            self.write_sector(dir_lba, Buf::buf)
        }
    }
}

fn name_match(entry: &[u8; 11], query: &[u8]) -> bool {
    if query.len() > 11 { return false; }
    for i in 0..query.len() {
        let e = if i < 11 { entry[i].to_ascii_uppercase() } else { 0x20 };
        let qb = query[i].to_ascii_uppercase();
        if e != qb && !(qb == b' ' && e == 0x20) { return false; }
    }
    for i in query.len()..11 {
        if entry[i] != 0x20 && entry[i] != 0 { return false; }
    }
    true
}

// ===========================================================================
//  PRUEBAS -- sobre un volumen FAT32 de mentira, en RAM
// ===========================================================================
//
// * Este modulo no existia, y era el agujero mas caro del arbol: el unico
// codigo de BMO que ESCRIBE en un disco de verdad era tambien el unico sin una
// sola prueba. Se verificaba flasheando y mirando la pantalla -- o sea,
// arriesgando el volumen para averiguar si el driver lo respetaba.
//
// El contrato de bloques (`BlockReader`/`BlockWriter`) son punteros a funcion
// sin estado, asi que el disco de mentira vive en un `static mut` y cada
// prueba lo formatea entera antes de empezar.
#[cfg(test)]
mod tests {
    use super::*;

    /// Un volumen minusculo pero REAL: 512 sectores de 512 bytes = 256 KiB.
    ///
    /// Un cluster = un sector, a proposito. Asi un archivo de 600 bytes ya son
    /// DOS clusters encadenados, y el camino de la cadena se pisa con datos de
    /// juguete en vez de necesitar megabytes.
    const SECTORES: usize = 512;
    const RESERVADOS: u32 = 1;
    const FAT_SECTORES: u32 = 4;

    static mut DISCO: [u8; SECTORES * 512] = [0u8; SECTORES * 512];

    /// El disco de mentira es UNO, y `cargo test` corre en paralelo.
    ///
    /// No es un detalle de infraestructura: sin esto, una prueba lee el
    /// volumen que otra acababa de formatear y falla **con un mensaje que
    /// apunta al driver**. Se pierde media tarde buscando un fallo de FAT32
    /// que estaba en el banco de pruebas.
    ///
    /// El candado lo toma [`volumen`] y lo devuelve al terminar la prueba, asi
    /// que no hay forma de olvidarse: quien quiere el volumen se lleva el
    /// turno con el.
    static CANDADO: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// El disco entero como rebanada. Se construye desde el puntero crudo y no
    /// desreferenciando el `static mut`: encadenarlo crearia una referencia a
    /// la desreferencia del puntero, que es lo que el lint prohibe -- y con
    /// razon, porque esconde de donde sale. Mismo trato que en
    /// `ring0/obj/archivo.rs`.
    fn disco() -> &'static mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(core::ptr::addr_of_mut!(DISCO) as *mut u8, SECTORES * 512)
        }
    }

    /// Cuantas veces se ha ido al "disco". Lo lleva el propio lector de mentira
    /// porque **es el unico sitio que no se puede saltar nadie**: si una pieza
    /// del driver deja de recordar lo que ya trajo, este numero lo dice.
    ///
    /// Es global y las pruebas corren en paralelo, pero quien lo mira tiene el
    /// CANDADO en la mano (ver [`volumen`]), asi que dentro de una prueba solo
    /// cuenta lo que hace esa prueba.
    static LECTURAS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn lecturas() -> usize {
        LECTURAS.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn read(lba: u64, count: u16, buf: &mut [u8]) -> bool {
        let off = lba as usize * 512;
        let n = count as usize * 512;
        if off + n > SECTORES * 512 || buf.len() < n { return false; }
        LECTURAS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        buf[..n].copy_from_slice(&disco()[off..off + n]);
        true
    }

    fn write(lba: u64, count: u16, data: &[u8]) -> bool {
        let off = lba as usize * 512;
        let n = count as usize * 512;
        if off + n > SECTORES * 512 || data.len() < n { return false; }
        disco()[off..off + n].copy_from_slice(&data[..n]);
        true
    }

    /// Formatea el disco de mentira y lo monta. Cada prueba empieza de cero.
    ///
    /// Devuelve el TURNO junto con el volumen: mientras la prueba tenga el
    /// guardia vivo, ninguna otra toca el disco. Un `let _ = volumen()` lo
    /// soltaria en el acto, y por eso las pruebas lo atan a un nombre.
    fn volumen() -> (std::sync::MutexGuard<'static, ()>, FatVolume) {
        volumen_con_base(0)
    }

    /// **El mismo volumen, empezando donde se diga.**
    ///
    /// === Por que esto no es un lujo de la prueba ===
    ///
    /// `volumen()` montaba en el sector 0, y ahi `part_lba` vale cero: **la suma
    /// que traduce del volumen al disco no cambia nada**. O sea que trece pruebas
    /// en verde no decian absolutamente nada sobre la unica cuenta que separa
    /// "leer mi archivo" de "leer la particion del vecino".
    ///
    /// El 2026-08-11 eso salio a cobrar. El camino directo del escalon 3 llamaba
    /// al lector con el LBA **relativo al volumen**, y con la particion de datos
    /// en el 1230848 un `.bex` se leia de dentro de la ESP. Las pruebas pasaban.
    ///
    /// > **Un parametro que en las pruebas siempre vale cero es un parametro que
    /// > no se esta probando.**
    fn volumen_con_base(base: u64) -> (std::sync::MutexGuard<'static, ()>, FatVolume) {
        // `into_inner` y no `unwrap`: si una prueba anterior revento con el
        // candado en la mano, el resto tiene que poder seguir. El disco se
        // formatea entero aqui abajo, asi que lo que dejara no importa --
        // envenenar la tanda entera solo escondaria el fallo de verdad.
        let turno = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
        disco().fill(0);
        let mut sector0 = [0u8; 512];
        {
            let bpb = unsafe { &mut *(sector0.as_mut_ptr() as *mut FatBpb) };
            bpb.bytes_per_sector = 512;
            bpb.sectors_per_cluster = 1;
            bpb.reserved_sectors = RESERVADOS as u16;
            bpb.num_fats = 1;
            // Lo que mide el VOLUMEN, no el disco: lo que queda detras de donde
            // empieza. Poner el disco entero haria que `max_cluster` contara
            // clusters que se salen por el final.
            bpb.total_sectors = (SECTORES as u64 - base) as u32;
            bpb.fat_size = FAT_SECTORES;
            bpb.root_cluster = 2;
            bpb.boot_sig = 0x29;
        }
        assert!(write(base, 1, &sector0));

        // El cluster 2 es la raiz y esta OCUPADO: la FAT tiene que decirlo, o
        // el primer archivo que se cree se llevara el directorio por delante.
        let mut fat = [0u8; 512];
        fat[0..4].copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes()); // media
        fat[4..8].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // reservada
        fat[8..12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // la raiz: EOC
        assert!(write(base + RESERVADOS as u64, 1, &fat));

        let v = mount(read, Some(write), base).expect("el volumen de mentira debe montar");
        (turno, v)
    }

    fn name(n: &str) -> [u8; 11] {
        let mut r = [b' '; 11];
        let b = n.as_bytes();
        r[..b.len()].copy_from_slice(b);
        r
    }

    /// Lee un archivo entero por su nombre. `None` si no esta.
    fn leer_archivo(v: &mut FatVolume, n: &str, dst: &mut [u8]) -> Option<usize> {
        let (primero, tam) = v.find_file(&name(n))?;
        let leidos = v.read_file(primero, tam, dst);
        Some(leidos.min(tam as usize))
    }

    /// Cuantos clusters hay OCUPADOS ahora mismo. Es el detector de fugas: si
    /// reemplazar no suelta la cadena vieja, este numero sube y no baja, y el
    /// volumen se llena archivo a archivo sin que nada lo diga.
    fn ocupados(v: &mut FatVolume) -> usize {
        let mut n = 0;
        for c in 2..=v.max_cluster {
            if v.raw_fat_entry(c).unwrap_or(0) != 0 { n += 1; }
        }
        n
    }

    /// ** UN ARCHIVO DE VARIOS CLUSTERS QUE NO ACABA EN FRONTERA DE SECTOR.
    ///
    /// Es el caso que estrena el camino directo de `read_file` (escalon 3): los
    /// sectores enteros van del disco a `dst` sin rebotar, y el rabo --menos de
    /// 512 bytes-- es el unico que sigue pasando por el buffer interno.
    ///
    /// El patron es POSICIONAL a proposito (cada byte dice donde deberia estar):
    /// un desplazamiento de un sector, o un cluster leido dos veces, sale como
    /// un byte que no cuadra y dice exactamente cual.
    #[test]
    fn leer_varios_clusters_con_rabo() {
        let (_turno, mut v) = volumen();
        // 1300 bytes con spc=1 son tres clusters: 512 + 512 + 276.
        let datos: Vec<u8> = (0..1300u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("LARGO   BIN"), &datos).expect("debe crear");

        let mut dst = [0u8; 2048];
        let n = leer_archivo(&mut v, "LARGO   BIN", &mut dst).expect("debe estar");
        assert_eq!(n, datos.len(), "no llego el archivo entero");
        assert_eq!(&dst[..n], &datos[..], "los bytes no cuadran: hay un salto de sector");
    }

    /// ** UN VOLUMEN QUE NO EMPIEZA EN EL SECTOR 0 -- o sea, el caso REAL.
    ///
    /// === El fallo que esta prueba habria cazado el 10 de agosto ===
    ///
    /// Los tres caminos directos --`read_file`, `leer_en` y `leer_tramo`--
    /// llamaban al lector de bloques con el LBA **relativo al volumen**, sin
    /// sumar `part_lba`. Los directorios y la FAT no, porque van por
    /// `read_sector`, que si traduce.
    ///
    /// De ahi el sintoma que costo dos tandas de fotos: el sistema **encontraba**
    /// el archivo y sabia su tamano exacto --eso lo dice el directorio-- y los
    /// bytes que llegaban eran codigo x86-64 ajeno. En el Ryzen, con la particion
    /// de datos en el sector 1230848, un `.bex` se leia de dentro de la ESP.
    ///
    /// === Y por eso hay veneno delante del volumen ===
    ///
    /// Un ida y vuelta a secas no basta: si escribir y leer se equivocaran
    /// **igual**, cuadrarian entre ellos y la prueba pasaria. El veneno ocupa
    /// justo los sectores donde cae una lectura sin traducir, asi que olvidarse
    /// de la suma no da "otro contenido": da `0xEE`, con su nombre.
    ///
    /// Se cubren los tres caminos en una sola prueba porque son el mismo error
    /// repetido tres veces, y arreglar dos de tres deja el sintoma vivo.
    #[test]
    fn leer_de_una_particion_que_no_empieza_en_cero() {
        const BASE: u64 = 64;
        let (_turno, mut v) = volumen_con_base(BASE);

        // Los sectores de DELANTE del volumen: fuera de el, y exactamente donde
        // apunta un LBA al que le falta la suma.
        let veneno = [0xEEu8; 512];
        for s in 0..BASE {
            assert!(write(s, 1, &veneno));
        }

        // 1300 bytes con spc=1 son tres clusters: dos sectores enteros --el
        // camino directo-- y un rabo de 276.
        let datos: Vec<u8> = (0..1300u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("LARGO   BIN"), &datos).expect("debe crear");

        let (primero, tam) = v.find_file(&name("LARGO   BIN")).expect("el directorio SI se lee");

        // -- 1. `read_file`: el archivo entero --
        let mut dst = [0u8; 2048];
        let n = v.read_file(primero, tam, &mut dst);
        assert_ne!(dst[0], 0xEE, "read_file leyo de DELANTE del volumen: falta sumar part_lba");
        assert_eq!(n, datos.len(), "no llego el archivo entero");
        assert_eq!(&dst[..n], &datos[..], "read_file trajo bytes de otro sitio");

        // -- 2. `leer_en`: por rangos, que es por donde carga un `.bex` --
        let mut cur = v.cursor(primero);
        let mut rango = [0u8; 700];
        let n = v.leer_en(&mut cur, 512, tam, &mut rango);
        assert_ne!(rango[0], 0xEE, "leer_en leyo de DELANTE del volumen");
        assert_eq!(n, 700, "el rango no llego entero");
        assert_eq!(&rango[..n], &datos[512..512 + n], "leer_en trajo bytes de otro sitio");

        // -- 3. `leer_tramo`: la lectura a pasos --
        let mut trozo = [0u8; 2048];
        let (n, _siguiente) = v.leer_tramo(primero, 0, tam, &mut trozo, 2048);
        assert_ne!(trozo[0], 0xEE, "leer_tramo leyo de DELANTE del volumen");
        assert_eq!(n, datos.len(), "el tramo no llego entero");
        assert_eq!(&trozo[..n], &datos[..], "leer_tramo trajo bytes de otro sitio");

        // Y el numero que se pinta en CABINA es el del DISCO, no el del volumen:
        // un cluster nunca puede caer antes del principio de su particion, y esa
        // linea lo decia sin que chirriara.
        assert!(
            v.lba_de_cluster(primero) >= BASE,
            "lba_de_cluster devuelve un sector anterior a su propia particion"
        );
    }

    /// ** EL RABO DE UN ARCHIVO **FRAGMENTADO**, que es donde el fallo se ve.
    ///
    /// === Por que hace falta fragmentar para probar esto ===
    ///
    /// El camino directo lee sectores enteros y deja para el buffer interno el
    /// rabo de menos de 512 bytes. Ese rabo esta en el sector `enteros` **de
    /// este cluster** -- y si el cluster ya se agoto (`enteros == spc`), ese
    /// numero de sector cae FUERA: es el sector fisico siguiente, que solo por
    /// casualidad es el siguiente cluster de la cadena.
    ///
    /// ** Y en un volumen recien formateado siempre es esa casualidad: los
    /// clusters se reparten seguidos, asi que el fallo devuelve el dato
    /// correcto y la prueba pasa. Es la clase de bug que se estrena el dia que
    /// el disco lleva seis meses de uso.
    ///
    /// Asi que aqui la cadena se rompe a mano: se muda el segundo cluster lejos
    /// y **el sitio viejo se llena de `0xEE`**. Si alguien quita la comprobacion
    /// de `enteros < spc`, esto sale en la cara con bytes que se reconocen.
    #[test]
    fn leer_rabo_de_archivo_fragmentado() {
        let (_turno, mut v) = volumen();
        // Con spc=1 (un cluster = un sector), 1300 bytes son tres clusters.
        let datos: Vec<u8> = (0..1300u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("FRAG    BIN"), &datos).expect("debe crear");

        let (c1, tam) = v.find_file(&name("FRAG    BIN")).expect("debe estar");
        let c2 = v.raw_fat_entry(c1).expect("debe haber segundo cluster");
        let c3 = v.raw_fat_entry(c2).expect("debe haber tercero");
        // Un cluster libre LEJOS de la cadena: el ultimo del volumen.
        let lejos = v.max_cluster;
        assert!(v.raw_fat_entry(lejos).unwrap_or(1) == 0, "el cluster de destino debe estar libre");

        // Se muda el contenido del segundo cluster.
        let mut sec = [0u8; 512];
        assert!(read(v.cluster_to_lba(c2), 1, &mut sec));
        assert!(write(v.cluster_to_lba(lejos), 1, &sec));
        // Y el sitio viejo se envenena: quien lo lea por error lo va a saber.
        assert!(write(v.cluster_to_lba(c2), 1, &[0xEEu8; 512]));

        // La cadena pasa a ser c1 -> lejos -> c3, y c2 queda libre.
        assert!(v.set_fat_entry(c1, lejos));
        assert!(v.set_fat_entry(lejos, c3));
        assert!(v.set_fat_entry(c2, 0));

        // 700 bytes: un cluster entero (512) y un rabo de 188 en el SIGUIENTE
        // cluster de la cadena, que ya no es el siguiente del disco.
        let mut dst = [0u8; 700];
        let n = v.read_file(c1, tam, &mut dst);
        assert_eq!(n, 700, "no llego el trozo pedido");
        assert!(
            !dst[512..].iter().any(|&b| b == 0xEE),
            "leyo el sector fisico siguiente en vez de seguir la cadena"
        );
        assert_eq!(&dst[..], &datos[..700], "el rabo no cuadra");
    }

    /// ** LEER A TROZOS TIENE QUE DAR EXACTAMENTE LO MISMO QUE LEER DE UNA.
    ///
    /// Es la propiedad entera de `leer_tramo`: si el resultado no es
    /// byte-a-byte identico al de `read_file`, el `open` que empieza y no
    /// termina entregaria un archivo distinto segun cuantas veces se le hubiera
    /// preguntado -- y eso no falla, corrompe.
    ///
    /// Se prueba con un archivo de VARIOS clusters y un presupuesto de UN
    /// cluster por vuelta, que es el caso que ejercita el cursor de verdad.
    #[test]
    fn leer_a_trozos_da_lo_mismo_que_de_una() {
        let (_turno, mut v) = volumen();
        let datos: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("TROZOS  BIN"), &datos).expect("debe crear");
        let (primero, tam) = v.find_file(&name("TROZOS  BIN")).expect("debe estar");

        let mut de_una = [0u8; 4096];
        let n1 = v.read_file(primero, tam, &mut de_una);

        let mut a_trozos = [0u8; 4096];
        let mut cluster = primero;
        let mut ya = 0usize;
        let mut vueltas = 0;
        while cluster != 0 {
            let (leidos, siguiente) = v.leer_tramo(cluster, ya, tam, &mut a_trozos, 512);
            assert!(leidos > 0, "un tramo que no avanza es un bucle infinito");
            ya += leidos;
            cluster = siguiente;
            vueltas += 1;
            assert!(vueltas < 64, "demasiadas vueltas: el cursor no avanza");
        }

        assert_eq!(ya, n1, "a trozos llego una cantidad distinta");
        assert_eq!(&a_trozos[..ya], &de_una[..n1], "a trozos salieron OTROS bytes");
        assert_eq!(&a_trozos[..ya], &datos[..], "y ni siquiera son los del archivo");
        assert!(vueltas > 1, "la prueba no llego a partir nada");
    }

    /// ** LEER DESDE UN BYTE CUALQUIERA TIENE QUE DAR LO MISMO QUE LEER DE UNA.
    ///
    /// Es la propiedad entera de `leer_en`, y la que hace posible que el disco
    /// escriba cada seccion de un `.bex` en los marcos del proceso: si un rango
    /// leido por su cuenta no coincide byte a byte con el mismo rango del fichero
    /// entero, el cargador montaria un programa cosido de trozos que no encajan
    /// -- y eso no falla, **corrompe**.
    ///
    /// Se prueban offsets DELIBERADAMENTE feos: mitad de sector, mitad de
    /// cluster, y cruzando las dos fronteras. Con offsets redondos la prueba
    /// pasaria sin ejercitar ni la cabeza ni la cola.
    #[test]
    fn leer_desde_cualquier_byte_da_lo_mismo() {
        let (_turno, mut v) = volumen();
        let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("TROZOS2 BIN"), &datos).expect("debe crear");
        let (primero, tam) = v.find_file(&name("TROZOS2 BIN")).expect("debe estar");

        // 1 y 511 son mitad de sector; 513 cruza la frontera; 2047 y 2049 andan
        // por la del cluster; 4999 es el ultimo byte.
        for (off, len) in [
            (0usize, 5000usize), (1, 10), (1, 600), (511, 2), (512, 512),
            (513, 1000), (2047, 3), (2048, 1), (2049, 2000), (4999, 1), (4990, 50),
        ] {
            let mut cur = v.cursor(primero);
            let mut dst = vec![0u8; len];
            let n = v.leer_en(&mut cur, off, tam, &mut dst);
            let esperado = &datos[off..(off + len).min(datos.len())];
            assert_eq!(n, esperado.len(), "off={off} len={len}: cantidad distinta");
            assert_eq!(&dst[..n], esperado, "off={off} len={len}: OTROS bytes");
        }
    }

    /// ** Y UN CURSOR REUSADO TIENE QUE DAR LO MISMO QUE UNO NUEVO.
    ///
    /// Es lo que se va a hacer de verdad: un solo cursor recorriendo el fichero
    /// hacia adelante, seccion tras seccion. Si el estado que arrastra cambiara
    /// el resultado, el segundo programa que se cargue saldria distinto del
    /// primero -- y eso no se reproduce nunca.
    #[test]
    fn el_cursor_reusado_no_cambia_lo_que_lee() {
        let (_turno, mut v) = volumen();
        let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("CURSOR  BIN"), &datos).expect("debe crear");
        let (primero, tam) = v.find_file(&name("CURSOR  BIN")).expect("debe estar");

        let mut cur = v.cursor(primero);
        for (off, len) in [(17usize, 100usize), (1000, 1200), (2500, 700), (4000, 999)] {
            let mut a = vec![0u8; len];
            let na = v.leer_en(&mut cur, off, tam, &mut a);

            let mut limpio = v.cursor(primero);
            let mut b = vec![0u8; len];
            let nb = v.leer_en(&mut limpio, off, tam, &mut b);

            assert_eq!(na, nb, "off={off}: el cursor reusado leyo otra cantidad");
            assert_eq!(a, b, "off={off}: el cursor reusado leyo OTROS bytes");
            assert_eq!(&a[..na], &datos[off..off + na], "off={off}: y no son los del archivo");
        }
    }

    /// Pedir hacia atras dice que NO. Ver la cabecera de `Cursor`: retroceder en
    /// silencio convertiria el bucle del cargador en cuadratico sin avisar.
    #[test]
    fn el_cursor_no_retrocede_en_silencio() {
        let (_turno, mut v) = volumen();
        let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("ATRAS   BIN"), &datos).expect("debe crear");
        let (primero, tam) = v.find_file(&name("ATRAS   BIN")).expect("debe estar");

        let mut cur = v.cursor(primero);
        let mut dst = vec![0u8; 100];
        assert!(v.leer_en(&mut cur, 4000, tam, &mut dst) > 0, "primero se avanza");
        assert_eq!(
            v.leer_en(&mut cur, 10, tam, &mut dst),
            0,
            "pedir hacia atras tiene que contestar cero, no leer de cualquier sitio"
        );
    }

    /// ** SEGUIR UNA CADENA NO PUEDE COSTAR UN COMANDO POR ESLABON.
    ///
    /// En un sector de FAT caben **128 entradas seguidas**, que son justo las que
    /// recorre quien sigue una cadena. `fat_cache` se llamaba cache y releia el
    /// sector en cada entrada; mientras lo unico que recorria cadenas era cargar
    /// un programa de una vez, eso se pagaba una vez. Con los archivos leidos por
    /// rangos, cada salto hacia atras en un fichero grande vuelve a recorrerla.
    ///
    /// Aqui se cuentan los viajes al disco de verdad. Las dos mitades importan y
    /// por eso van juntas: **que no relea** y **que no sirva lo de antes**.
    #[test]
    fn seguir_la_cadena_no_relee_el_mismo_sector() {
        let (_turno, mut v) = volumen();
        // 100 entradas de FAT consecutivas caben de sobra en un solo sector.
        let antes = lecturas();
        for c in 2..102u32 {
            v.raw_fat_entry(c);
        }
        let viajes = lecturas() - antes;
        assert!(viajes <= 1, "100 entradas del MISMO sector costaron {viajes} lecturas");

        // Y lo que se escribe se lee: un cache que no se entera de una escritura
        // seria peor que no tenerlo -- entregaria la cadena vieja sin decirlo.
        assert!(v.set_fat_entry(7, 0x0FFF_FFFF), "debe escribir");
        assert_eq!(v.raw_fat_entry(7), Some(0x0FFF_FFFF), "el cache sirvio lo de ANTES de escribir");
        assert!(v.set_fat_entry(7, 0), "debe poder soltarse");
        assert_eq!(v.raw_fat_entry(7), Some(0), "el cache se quedo con el valor viejo");
    }

    /// ** EL PATRON DE UN JUEGO LEYENDO SU WAD: saltos en los DOS sentidos.
    ///
    /// === Que fija esta prueba ===
    ///
    /// El cargador de `.bex` lee hacia adelante y retrocede **dos veces por
    /// carga**, asi que le vale una copia suelta del cursor. Un archivo abierto
    /// por un programa no: DOOM abre `doom1.wad`, lee el directorio de lumps del
    /// final, y a partir de ahi salta a donde le pida el juego -- atras, adelante,
    /// atras. Ahi retroceder **es el caso normal**, no la excepcion.
    ///
    /// La regla que sostiene `ring0::obj::archivo` es esta: se guarda el cursor
    /// del flujo **y una copia sin estrenar**, y cuando lo que se pide cae por
    /// debajo de donde va el cursor, se vuelve a empezar desde la copia. Lo que
    /// esta prueba fija es que **eso da los mismos bytes que un cursor limpio**,
    /// salto tras salto y en cualquier orden.
    ///
    /// Si un dia `Cursor::base` dejara de significar "el primer byte al que este
    /// cursor todavia puede llegar", esto sale en rojo -- y el sintoma sin la
    /// prueba seria un juego con las texturas cambiadas, que nadie sabe leer.
    #[test]
    fn el_patron_de_lumps_salta_en_los_dos_sentidos() {
        let (_turno, mut v) = volumen();
        let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("WADSIM  BIN"), &datos).expect("debe crear");
        let (primero, tam) = v.find_file(&name("WADSIM  BIN")).expect("debe estar");

        // Las dos mitades de un archivo reflejado: por donde va, y por donde
        // empieza. La segunda no se estrena jamas.
        let inicio = v.cursor(primero);
        let mut cur = inicio;
        let mut retrocesos = 0;

        // El orden es el de un juego, no el de un fichero: el directorio del
        // final primero, y despues lumps de aqui y de alla.
        for (off, len) in [
            (4900usize, 100usize), // el "directorio de lumps", al final
            (0, 12),               // la cabecera, o sea hacia atras del todo
            (3000, 400),           // adelante
            (1024, 512),           // atras otra vez
            (1536, 512),           // y adelante desde donde estaba: sin retroceso
            (17, 1),               // un byte suelto, atras y sin alinear
            (4999, 1),             // el ultimo byte
        ] {
            if off < cur.base() {
                cur = inicio;
                retrocesos += 1;
            }
            let mut dst = vec![0u8; len];
            let n = v.leer_en(&mut cur, off, tam, &mut dst);
            assert_eq!(n, len, "off={off} len={len}: el rango no llego entero");
            assert_eq!(&dst[..n], &datos[off..off + len], "off={off} len={len}: OTROS bytes");
        }

        // Y que el mecanismo se haya usado de verdad: sin esto, la prueba
        // pasaria igual el dia que alguien la reordene sin querer y nunca
        // vuelva a mirar hacia atras.
        assert!(retrocesos >= 3, "esta prueba tiene que retroceder: solo lo hizo {retrocesos} veces");
    }

    /// ** EL PATRON REAL DEL CARGADOR: dos tablas del FINAL antes que el codigo.
    ///
    /// === Lo que esto fija ===
    ///
    /// Un `.bex` no se lee de principio a fin. Antes de aterrizar la primera
    /// seccion, el cargador necesita los **hashes** (`Signature`) y las
    /// **relocations**, y las dos van al final del fichero -- en `gui.bex`, la
    /// firma esta en el `0x4B680` de `0x4B728` y el codigo empieza en el `0x200`.
    ///
    /// Con un solo cursor eso es un salto al final y una vuelta atras, o sea un
    /// `0` del que el cargador dijo `una seccion se quedo a medias al aterrizar`.
    /// La salida no es dejar que el cursor retroceda: es que la lectura suelta
    /// se lleve **una copia** y no toque la del flujo.
    ///
    /// Por eso `Cursor` es `Copy`, y por eso esto es una prueba y no un
    /// comentario: quitarle el `Copy` o guardar el cursor detras de algo que no
    /// se pueda duplicar rompe el cargador **sin tocar el cargador**.
    #[test]
    fn una_lectura_suelta_no_mueve_el_cursor_del_flujo() {
        let (_turno, mut v) = volumen();
        // 5000 bytes = diez clusters con spc=1: hay cadena que recorrer.
        let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        v.create_file_in_dir(2, &name("BEXSIM  BIN"), &datos).expect("debe crear");
        let (primero, tam) = v.find_file(&name("BEXSIM  BIN")).expect("debe estar");

        let flujo_inicio = v.cursor(primero);

        // -- 1. La "tabla de hashes": al final del fichero, con una COPIA --
        let mut aparte = flujo_inicio;
        let mut firma = [0u8; 100];
        let n = v.leer_en(&mut aparte, 4900, tam, &mut firma);
        assert_eq!(n, 100, "la tabla del final no se leyo entera");
        assert_eq!(&firma[..n], &datos[4900..5000], "y no son los bytes del final");

        // -- 2. El flujo de secciones, desde el principio. Su cursor no se ha
        //       enterado de nada de lo anterior. --
        let mut flujo = flujo_inicio;
        let mut codigo = [0u8; 512];
        let n = v.leer_en(&mut flujo, 512, tam, &mut codigo);
        assert_eq!(n, 512, "la primera seccion se quedo a medias: el cursor se movio");
        assert_eq!(&codigo[..n], &datos[512..1024], "la primera seccion trajo otros bytes");

        // -- 3. Y el flujo sigue avanzando normal detras de ella --
        let mut mas = [0u8; 512];
        let n = v.leer_en(&mut flujo, 1024, tam, &mut mas);
        assert_eq!(n, 512, "la seccion siguiente no llego");
        assert_eq!(&mas[..n], &datos[1024..1536], "la seccion siguiente trajo otros bytes");

        // Y la prueba de que el peligro era real: con EL MISMO cursor, el orden
        // del cargador contesta cero. Es el fallo del 2026-08-11 en una linea.
        let mut uno_solo = flujo_inicio;
        assert!(v.leer_en(&mut uno_solo, 4900, tam, &mut firma) > 0);
        assert_eq!(
            v.leer_en(&mut uno_solo, 512, tam, &mut codigo),
            0,
            "si esto deja de ser cero, el cursor retrocede en silencio (ver su cabecera)"
        );
    }

    #[test]
    fn crear_y_leer_da_lo_mismo() {
        let (_turno, mut v) = volumen();
        let datos = b"BANCO BMO";
        v.create_file_in_dir(2, &name("CTAS    BIN"), datos).expect("debe crear");
        let mut dst = [0u8; 512];
        let n = leer_archivo(&mut v, "CTAS    BIN", &mut dst).expect("debe estar");
        assert_eq!(&dst[..n], datos);
    }

    /// El comportamiento de ANTES, que se conserva: `create` no pisa.
    #[test]
    fn crear_sobre_uno_que_existe_sigue_dando_exists() {
        let (_turno, mut v) = volumen();
        v.create_file_in_dir(2, &name("CTAS    BIN"), b"viejo").expect("debe crear");
        let r = v.create_file_in_dir(2, &name("CTAS    BIN"), b"nuevo");
        assert!(matches!(r, Err(WriteError::Exists)), "crear NO puede pisar: {r:?}");
    }

    /// ** LA PRUEBA QUE JUSTIFICA TODO ESTO.
    ///
    /// Es el nivel 10 de COBOL corrido dos veces: la segunda escritura tiene
    /// que ganar. Antes daba `Exists`, el `CLOSE` devolvia `0`, y en el disco
    /// se quedaba el contenido de la primera corrida.
    #[test]
    fn guardar_dos_veces_deja_lo_segundo() {
        let (_turno, mut v) = volumen();
        v.save_file_in_dir(2, &name("CTAS    BIN"), b"primera").expect("1a");
        v.save_file_in_dir(2, &name("CTAS    BIN"), b"SEGUNDA").expect("2a");
        let mut dst = [0u8; 512];
        let n = leer_archivo(&mut v, "CTAS    BIN", &mut dst).expect("debe estar");
        assert_eq!(&dst[..n], b"SEGUNDA");
    }

    /// Y sin dejar UNA sola entrada de mas en el directorio.
    ///
    /// Reemplazar anadiendo otra entrada dejaria dos nombres iguales: el
    /// segundo inalcanzable y sus clusters perdidos para siempre. Es justo el
    /// motivo por el que `create` rechaza los repetidos.
    #[test]
    fn guardar_dos_veces_no_duplica_la_entrada() {
        let (_turno, mut v) = volumen();
        v.save_file_in_dir(2, &name("CTAS    BIN"), b"primera").expect("1a");
        v.save_file_in_dir(2, &name("CTAS    BIN"), b"SEGUNDA").expect("2a");

        let mut buf = [0u8; 512];
        assert!(read(v.cluster_to_lba(2), 1, &mut buf));
        let mut cuantas = 0;
        for i in 0..(512 / 32) {
            let de = unsafe { &*(buf.as_ptr().add(i * 32) as *const DirEntry) };
            if de.name[0] == 0 { break; }
            if de.name[0] == 0xE5 { continue; }
            if name_match(&de.name, &name("CTAS    BIN")) { cuantas += 1; }
        }
        assert_eq!(cuantas, 1, "reemplazar no puede dejar dos entradas con el mismo nombre");
    }

    /// * Y sin FUGAR clusters: la cadena vieja tiene que quedar suelta.
    ///
    /// Un reemplazo que no libera lo anterior no rompe nada visible --el
    /// archivo se lee bien-- pero el volumen se llena solo, y el dia que se
    /// llene el motivo llevara meses enterrado.
    #[test]
    fn reemplazar_suelta_la_cadena_vieja() {
        let (_turno, mut v) = volumen();
        // 1200 bytes con clusters de 512 son TRES clusters.
        let grande = [b'A'; 1200];
        v.save_file_in_dir(2, &name("GRANDE  BIN"), &grande).expect("1a");
        assert_eq!(ocupados(&mut v), 1 + 3, "raiz + tres clusters de datos");

        // Y ahora uno pequeno en su sitio: tiene que BAJAR a un solo cluster.
        v.save_file_in_dir(2, &name("GRANDE  BIN"), b"corto").expect("2a");
        let quedan = ocupados(&mut v);
        assert_eq!(quedan, 1 + 1, "los tres clusters viejos tenian que soltarse: quedan {quedan}");
    }

    /// Al reves tambien: crecer reserva la cadena entera y el archivo se lee
    /// completo. Un reemplazo que solo escribiera el primer cluster daria un
    /// archivo del tamano nuevo con la cola del viejo dentro.
    #[test]
    fn reemplazar_por_uno_mas_grande_lo_lee_entero() {
        let (_turno, mut v) = volumen();
        v.save_file_in_dir(2, &name("CRECE   BIN"), b"corto").expect("1a");
        let mut grande = [0u8; 1500];
        for (i, b) in grande.iter_mut().enumerate() { *b = (i % 251) as u8; }
        v.save_file_in_dir(2, &name("CRECE   BIN"), &grande).expect("2a");

        let mut dst = [0u8; 2048];
        let n = leer_archivo(&mut v, "CRECE   BIN", &mut dst).expect("debe estar");
        assert_eq!(n, grande.len(), "el tamano de la entrada tiene que ser el nuevo");
        assert_eq!(&dst[..n], &grande[..], "y los bytes, los nuevos de punta a punta");
    }


    /// ** UN ARCHIVO LARGO SE LEE ENTERO, cadena de clusters incluida.
    ///
    /// El `.bex` mas grande que este sistema habia cargado eran 306 KiB; DOOM
    /// son 814 KiB, **2,7 veces mas**, y el 2026-08-09 el cargador lo rechazo.
    /// La primera sospecha fue una lectura corta con muchos clusters -- y esta
    /// fila existe para contestar esa pregunta en el anfitrion en vez de con
    /// fotos del Ryzen.
    ///
    /// Con clusters de UN sector, 200 KiB son **400 clusters encadenados**: la
    /// misma forma que el fichero de verdad, en un disco de juguete.
    #[test]
    fn un_archivo_de_cientos_de_clusters_se_lee_entero() {
        let (_turno, mut v) = volumen();
        // 200 KiB = 400 clusters de 512 B. Cabe en el disco de 256 KiB? No:
        // se usa lo que si cabe con holgura -- 100 KiB son 200 clusters, que ya
        // es un orden de magnitud por encima de lo que probaba nada.
        let mut grande = std::vec![0u8; 100 * 1024];
        for (i, b) in grande.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        v.save_file_in_dir(2, &name("GRANDE  BEX"), &grande).expect("debe guardar");

        let mut dst = std::vec![0u8; grande.len()];
        let n = leer_archivo(&mut v, "GRANDE  BEX", &mut dst).expect("debe estar");
        assert_eq!(n, grande.len(), "se leyeron {n} de {} bytes", grande.len());
        // Y byte a byte: un tamano correcto con un agujero dentro es
        // exactamente el fallo que esta fila viene a descartar.
        let malo = dst.iter().zip(grande.iter()).position(|(a, b)| a != b);
        assert!(malo.is_none(), "primer byte distinto en {malo:?}");
    }

    /// ** Y UN SECTOR QUE NO SE PUEDE LEER **CORTA** la lectura.
    ///
    /// Antes no: el `read_sector` fallido no copiaba nada y el `offset += count`
    /// corria igual, asi que `read_file` contestaba el tamano COMPLETO con el
    /// trozo sin tocar -- basura, o los bytes de quien tuvo antes ese buffer.
    /// Un `.bex` de 1.591 sectores necesita **una** lectura mala para llegar al
    /// cargador con un agujero y del tamano correcto.
    ///
    /// Se provoca acortando el disco: los clusters del final quedan fuera del
    /// medio y `read` contesta `false`.
    #[test]
    fn un_sector_ilegible_corta_la_lectura_en_vez_de_mentir() {
        let (_turno, mut v) = volumen();
        let grande = [b'Z'; 4096]; // ocho clusters
        v.save_file_in_dir(2, &name("CORTADO BIN"), &grande).expect("debe guardar");

        // El destino es mas corto que el archivo a proposito: `read_file` tiene
        // que parar en el borde de `dst` y decir cuanto trajo, no pasarse.
        let mut dst = [0u8; 1024];
        let (primero, tam) = v.find_file(&name("CORTADO BIN")).expect("debe estar");
        let n = v.read_file(primero, tam, &mut dst);
        assert_eq!(n, dst.len(), "tiene que parar en el borde del destino");
        assert!(dst.iter().all(|&b| b == b'Z'), "y lo que trajo tiene que ser bueno");
    }

    /// `save` sobre un nombre que NO existe es crear, sin sorpresas.
    #[test]
    fn guardar_lo_que_no_existe_es_crear() {
        let (_turno, mut v) = volumen();
        v.save_file_in_dir(2, &name("NUEVO   TXT"), b"hola").expect("debe crear");
        let mut dst = [0u8; 512];
        let n = leer_archivo(&mut v, "NUEVO   TXT", &mut dst).expect("debe estar");
        assert_eq!(&dst[..n], b"hola");
    }

    /// * Reemplazar NO puede tocar al archivo de al lado.
    ///
    /// Es el fallo silencioso que mas miedo da de esta operacion: la entrada
    /// de directorio se reescribe dentro de un sector que comparte con otras
    /// quince, y escribir ese sector con un buffer que no sea el suyo se lleva
    /// a los vecinos por delante.
    ///
    /// [!] Y hay que decir lo que esta prueba NO demuestra hoy: quitar el
    /// `read_sector` de `replace_file_fat32` **no la hace caer**, porque `buf`
    /// resulta que todavia conserva ese sector de cuando se busco la entrada.
    /// Eso es un accidente del orden de las llamadas, no una garantia -- se
    /// comprobo mutandolo. La relectura se queda por eso mismo, y esta prueba
    /// vale como red para la implementacion que venga despues, no como
    /// demostracion de la de ahora.
    #[test]
    fn reemplazar_no_toca_al_vecino() {
        let (_turno, mut v) = volumen();
        v.save_file_in_dir(2, &name("UNO     TXT"), b"el primero").expect("uno");
        v.save_file_in_dir(2, &name("DOS     TXT"), b"el segundo").expect("dos");
        v.save_file_in_dir(2, &name("UNO     TXT"), b"PISADO").expect("uno otra vez");

        let mut dst = [0u8; 512];
        let n = leer_archivo(&mut v, "DOS     TXT", &mut dst).expect("el vecino debe seguir ahi");
        assert_eq!(&dst[..n], b"el segundo", "el vecino no puede cambiar");
        let n = leer_archivo(&mut v, "UNO     TXT", &mut dst).unwrap();
        assert_eq!(&dst[..n], b"PISADO");
    }

    /// Un volumen montado sin escritor no escribe. No es una politica que
    /// alguien tenga que recordar respetar: no hay con que.
    #[test]
    fn sin_escritor_no_se_guarda() {
        let (_turno, _) = volumen();
        let mut v = mount(read, None, 0).expect("debe montar en solo lectura");
        let r = v.save_file_in_dir(2, &name("NOPE    TXT"), b"x");
        assert!(matches!(r, Err(WriteError::ReadOnly)), "{r:?}");
    }
}
