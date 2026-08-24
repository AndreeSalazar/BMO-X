//! **LA MITAD QUE ESCRIBE** de FAT32: reservar clusters, encadenarlos y crear
//! entradas de directorio.
//!
//! ## Por que es un fichero (L6b), y por que este corte y no otro
//!
//! ** Contesta una pregunta distinta que `lib.rs`, y ademas es la pregunta
//! PELIGROSA. Leer un FAT32 mal devuelve bytes raros; escribirlo mal **rompe el
//! volumen de otro sistema**, y en esta maquina el volumen de al lado es el
//! Windows del dueno. Que todo lo que escribe este junto es lo que permite
//! leerlo entero de una sentada antes de tocarlo.
//!
//! ```text
//!    lib.rs         montar, leer sectores, seguir la cadena de la FAT
//!    buscar.rs      encontrar un fichero o un subdirectorio por su nombre
//!    escribir.rs    lo unico que MODIFICA el disco
//! ```
//!
//! ** Y el reparto es MOVER TEXTO: ni una linea cambia de contenido. Es lo que
//! L6d llama un reparto demostrable, y por eso las 21 pruebas siguen pasando
//! sin tocar ninguna.

use super::*;

impl FatVolume {
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
    // ** `pub(super)` y no privada: al mudarse de fichero, "privada" dejo de
    // significar "de este driver" y paso a significar "de este modulo". Son las
    // DOS unicas lineas que este reparto no pudo dejar iguales, y se dicen en
    // vez de cambiarlas callando -- L6d exige que un reparto sea texto movido,
    // y lo que no lo es tiene que verse.
    pub(super) fn raw_fat_entry(&mut self, cluster: u32) -> Option<u32> {
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
    // ** `pub(super)` y no privada: al mudarse de fichero, "privada" dejo de
    // significar "de este driver" y paso a significar "de este modulo". Son las
    // DOS unicas lineas que este reparto no pudo dejar iguales, y se dicen en
    // vez de cambiarlas callando -- L6d exige que un reparto sea texto movido,
    // y lo que no lo es tiene que verse.
    pub(super) fn set_fat_entry(&mut self, cluster: u32, value: u32) -> bool {
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
        if !self.escribible { return Err(WriteError::ReadOnly); }
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
        if !self.escribible { return Err(WriteError::ReadOnly); }
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
