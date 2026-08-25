//! FAT32 and exFAT filesystem reader/writer -- minimal implementation.
//!
//! Supports both FAT32 (S: FASTOS-EFI) and exFAT (T: FastOS-Data, X: Commit-Real).
//! Reads BPB, locates root directory, finds files by 8.3 name,
//! and reads clusters via the FAT chain. El almacenamiento entra por el
//! contrato `bmo_block::BlockDevice`: no sabe si debajo hay SATA o NVMe.

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

/// **El dispositivo de bloques, por el CONTRATO.**
///
/// # Por que ya no son dos punteros a funcion (paso 0, 2026-08-14)
///
/// Aqui vivian `BlockReader` y `BlockWriter`, dos `fn(...)  -> bool`, con un
/// motivo escrito que era correcto: en Ring 0 no hay `alloc` y un trait
/// parecia pedir un `Box`. **No lo pide**: `bmo_block::BlockDevice` se pasa
/// como `&'static dyn`, que es un puntero gordo y ninguna reserva.
///
/// Lo que costaba tenerlos era mas caro que lo que ahorraban: `bmo-block`
/// declara el contrato de bloques de BMO --leer, escribir, capacidad,
/// identidad, `flush`-- y FAT32 lo esquivaba con una puerta propia. Un
/// contrato con puertas traseras no es un contrato; medido en
/// `docs/plan/PLAN_ALMACENAMIENTO.md`, seccion 0.1.
///
/// Lo que se gana al entrar por la puerta:
///
/// * **Identidad y capacidad**: antes el sistema de ficheros no podia saber
///   sobre que disco vivia ni cuantos bloques tenia.
/// * **Errores con nombre** en vez de `bool`: `OutOfRange` es un bug del que
///   llama y `Device` es hardware roto, y con un booleano son la misma cosa.
/// * **`flush` de verdad**: la barrera que un diseno transaccional necesita.
/// * **`writable()`**: se puede preguntar ANTES de empezar, no a mitad.
///
/// [!] Y no se pierde nada de lo que el motivo viejo protegia: el sistema de
/// ficheros sigue sin saber si debajo hay SATA, NVMe o un disco en RAM. Las
/// pruebas siguen inyectando un disco de mentira, ahora como un `static` que
/// implementa el trait.
pub use bmo_block::{BlockDevice, BlockError};

/// Cual de los dos buffers internos usa una operacion. Existe para que el
/// prestamo del buffer y el del dispositivo no se pisen: se copia el puntero
/// a funcion primero y el buffer se toma despues.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
enum Buf { buf, fat_cache }

pub struct FatVolume {
    /// El dispositivo. Uno solo, y de el salen lectura Y escritura.
    dev: &'static dyn BlockDevice,
    /// Se monto para escribir? Es una decision del que MONTA y no del
    /// dispositivo: un disco escribible se puede montar en solo lectura a
    /// proposito, y entonces la imposibilidad de escribir queda ESTRUCTURAL --
    /// no hay writer al que llamar, igual que antes con el `Option`.
    escribible: bool,
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
    /// El volumen se monto en solo lectura (`escribible = false`).
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
pub fn mount(dev: &'static dyn BlockDevice, escribible: bool, part_lba: u64) -> Option<FatVolume> {
    let mut buf = [0u8; 512];
    if dev.read(part_lba, 1, &mut buf).is_err() { return None; }

    // Check for exFAT signature ("EXFAT   ") at offset 3
    let fs_name = &buf[3..11];
    if fs_name == b"EXFAT   " {
        return mount_exfat(dev, escribible, part_lba, &buf);
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
    // ** Y EL RAIZ TAMBIEN VIENE DEL DISCO. Es el quinto productor y el mas
    // facil de olvidar porque se lee una sola vez, al montar -- pero un
    // `root_cluster` de 0 o 1 en el BPB manda a leer un LBA cualquiera en la
    // PRIMERA operacion que se haga con el volumen.
    if bpb.root_cluster < 2 || bpb.root_cluster > max_cluster { return None; }
    Some(FatVolume { dev, escribible, part_lba, fs_type: FsType::Fat32, bytes_per_sector: bpb.bytes_per_sector, sectors_per_cluster: spc,
        num_fats, fat_start, fat_size_sectors, data_start, root_cluster: bpb.root_cluster, max_cluster, fallos_mudos: 0, buf: [0; 512], fat_cache: [0; 512], fat_cache_lba: SIN_CACHE })
}

fn mount_exfat(dev: &'static dyn BlockDevice, escribible: bool, part_lba: u64, buf: &[u8; 512]) -> Option<FatVolume> {
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
    Some(FatVolume { dev, escribible, part_lba, fs_type: FsType::ExFat, bytes_per_sector, sectors_per_cluster,
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
        self.dev.read(self.abs(lba), count, dst).is_ok()
    }

    /// Lee un sector del VOLUMEN a uno de los buffers internos.
    ///
    /// El puntero a funcion se copia ANTES de tomar el buffer: si no, seria un
    /// doble prestamo de `self` y no compilaria.
    fn read_sector(&mut self, lba: u64, which: Buf) -> bool {
        let rd = self.dev;
        let abs = self.abs(lba);
        match which {
            Buf::buf => rd.read(abs, 1, &mut self.buf).is_ok(),
            Buf::fat_cache => {
                // ** Y AQUI SI SE RECUERDA. Ver el campo `fat_cache_lba`: en un
                // sector de FAT caben 128 entradas seguidas, que son justo las
                // que recorre quien sigue una cadena. Sin esta linea, seguir
                // una cadena de mil clusters son mil comandos al disco.
                if self.fat_cache_lba == lba {
                    return true;
                }
                let ok = rd.read(abs, 1, &mut self.fat_cache).is_ok();
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
        if !self.escribible {
            return false;
        }
        let wr = self.dev;
        let abs = self.abs(lba);
        match which {
            Buf::buf => wr.write(abs, 1, &self.buf).is_ok(),
            Buf::fat_cache => {
                let ok = wr.write(abs, 1, &self.fat_cache).is_ok();
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
        if !self.escribible {
            return false;
        }
        self.dev.write(self.abs(lba), 1, data).is_ok()
    }

    /// Primer LBA de la particion montada, por si alguien de arriba lo
    /// necesita para diagnostico.
    pub fn partition_lba(&self) -> u64 { self.part_lba }

    /// **Existe este cluster en este volumen?**
    ///
    /// # *** LO QUE ESTO IMPIDE, Y ESTABA VIVO (auditoria 2026-08-24)
    ///
    /// Los numeros de cluster **vienen del disco**, y el disco puede ser de
    /// otro. La numeracion de FAT empieza en 2 --el 0 y el 1 estan reservados--
    /// asi que [`cluster_to_lba`](Self::cluster_to_lba) hace `cluster - 2`:
    ///
    /// ```text
    ///    una entrada de directorio con first_cluster = 0
    ///      -> 0 - 2 da la vuelta al contador: 0xFFFF_FFFF_FFFF_FFFE
    ///      -> por sectores_por_cluster, vuelve a dar la vuelta
    ///      -> y sale un LBA CUALQUIERA de ese disco
    /// ```
    ///
    /// ** El camino de ESCRITURA ya lo comprobaba (`escribir.rs`, `cluster < 2
    /// || cluster > max_cluster`). El de LECTURA no -- y leer un sector
    /// arbitrario no rompe nada visible: **devuelve datos de otra particion**
    /// como si fueran del fichero que se pidio.
    ///
    /// *** Se comprueba en los CINCO SITIOS DONDE NACE un cluster --la cadena
    /// de la FAT, las dos clases de entrada de directorio y el `root_cluster`
    /// del BPB-- y no en `cluster_to_lba`, que tiene dieciocho llamantes y
    /// devuelve un `u64` que no puede decir que no. Cerrar en el origen es
    /// cuatro comprobaciones; cerrar en el destino serian dieciocho.
    #[inline]
    fn cluster_valido(&self, cluster: u32) -> bool {
        cluster >= 2 && cluster <= self.max_cluster
    }

    /// [!] **PRECONDICION: `cluster_valido(cluster)`.** Ver ahi por que.
    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        debug_assert!(self.cluster_valido(cluster), "cluster fuera de rango en cluster_to_lba");
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
            // ** EL TOPE, que faltaba. Un `1` o un numero mayor que los clusters
            // que este volumen tiene son los dos casos que `cluster_to_lba`
            // convierte en un LBA de cualquier sitio. Una FAT corrupta --o
            // fabricada-- los trae, y hasta hoy pasaban.
            n if !self.cluster_valido(n) => None,
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
                            first_cluster: {
                                // Los dos medios vienen del disco. Un cero aqui
                                // es un fichero vacio legitimo; cualquier otro
                                // valor fuera de rango es basura, y se corta.
                                let c = (de.first_cluster_hi as u32) << 16
                                    | de.first_cluster_lo as u32;
                                if c != 0 && !self.cluster_valido(c) {
                                    return None;
                                }
                                c
                            },
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
                                if first_cluster != 0 && !self.cluster_valido(first_cluster) {
                                    return None;
                                }
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
    /// --`BlockDevice::read` recibe `count`-- y nadie lo usaba. Otra vez el
    /// mecanismo escrito y sin lector.
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

/// **La FORMA en el disco**: BPB, entradas de directorio, stream de exFAT.
/// Aparte porque son tipos sin decisiones -- el mismo corte que `ir/forma.rs`
/// en INTI. Y los escribio otro sistema: cambiarlos no es refactorizar.
mod forma;
pub use forma::*;

/// **Encontrar** un fichero o un subdirectorio por su nombre, en los dos
/// formatos. Aparte porque es otra pregunta (L6b).
mod buscar;
/// **Lo unico que MODIFICA el disco.** Junto, para poder leerlo entero antes de
/// tocarlo -- en esta maquina el volumen de al lado es el Windows del dueno.
mod escribir;

#[cfg(test)]
mod pruebas;
