pub struct RamFile {
    pub name: &'static str,
    pub data: &'static [u8],
}

pub static RAMDISK_FILES: &[RamFile] = &[
    RamFile {
        name: "bmo:readme",
        data: b"BMO / BMO RAMdisk operativo.\n\
               Para cargar un WAD de DOOM coloca el binario y\n\
               declara la entrada en src/fs/ramdisk.rs::RAMDISK_FILES.\n\
               Las syscalls FileOpen(0x20)/FileRead(0x21)/FileClose(0x23)\n\
               sirven el contenido desde Ring 3 sin tocar el disco.\n",
    },
    RamFile {
        name: "datos:readme",
        data: b"BMO / BMO Datos\n\
               \n\
               Montaje de Loop Device: OK\n\
               Firma de Superblock: OK\n\
               Interoperabilidad UEFI: OK\n\
               \n\
               Archivos del sistema:\n\
               - /proc  : informacion de procesos\n\
               - /dev   : dispositivos\n\
               - /tmp   : archivos temporales\n\
               - /data  : particion exFAT (datos del usuario)\n",
    },
];

const MAX_FDS: usize = 16;

#[derive(Clone, Copy)]
struct OpenFd {
    file_idx: i32,
    cursor: u64,
}

static mut FDS: [OpenFd; MAX_FDS] = [OpenFd { file_idx: -1, cursor: 0 }; MAX_FDS];

pub fn open(name: &str) -> Option<u32> {
    let mut file_idx: i32 = -1;
    for (i, f) in RAMDISK_FILES.iter().enumerate() {
        if f.name == name { file_idx = i as i32; break; }
    }
    if file_idx < 0 { return None; }

    unsafe {
        for i in 0..MAX_FDS {
            if FDS[i].file_idx < 0 {
                FDS[i].file_idx = file_idx;
                FDS[i].cursor = 0;
                return Some(i as u32);
            }
        }
    }
    None
}

pub fn find(name: &str) -> Option<usize> {
    RAMDISK_FILES.iter().position(|f| f.name == name)
}

pub fn file_size(idx: usize) -> usize {
    if idx < RAMDISK_FILES.len() { RAMDISK_FILES[idx].data.len() } else { 0 }
}

pub fn read_at(idx: usize, offset: u64, buf: &mut [u8]) -> usize {
    if idx >= RAMDISK_FILES.len() { return 0; }
    let data = RAMDISK_FILES[idx].data;
    let start = offset as usize;
    if start >= data.len() { return 0; }
    let n = buf.len().min(data.len() - start);
    buf[..n].copy_from_slice(&data[start..start + n]);
    n
}

pub fn close(fd: u32) -> bool {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return false; }
    unsafe {
        if FDS[fd_idx].file_idx < 0 { return false; }
        FDS[fd_idx].file_idx = -1;
        FDS[fd_idx].cursor = 0;
    }
    true
}

pub fn read(fd: u32, buf: &mut [u8]) -> Option<usize> {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return None; }
    unsafe {
        let f = &mut FDS[fd_idx];
        if f.file_idx < 0 { return None; }
        let file = &RAMDISK_FILES[f.file_idx as usize];
        let remaining = (file.data.len() as u64).saturating_sub(f.cursor);
        let n = buf.len().min(remaining as usize);
        if n == 0 { return Some(0); }
        buf[..n].copy_from_slice(&file.data[f.cursor as usize..f.cursor as usize + n]);
        f.cursor += n as u64;
        Some(n)
    }
}

pub fn write(fd: u32, _data: &[u8]) -> Option<usize> {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return None; }
    unsafe {
        let f = &FDS[fd_idx];
        if f.file_idx < 0 { return None; }
    }
    Some(0)
}

pub fn seek(fd: u32, offset: u64, whence: u64) -> Option<u64> {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return None; }
    unsafe {
        let f = &mut FDS[fd_idx];
        if f.file_idx < 0 { return None; }
        let file_size = RAMDISK_FILES[f.file_idx as usize].data.len() as u64;
        let new_pos = match whence {
            0 => offset.min(file_size),
            1 => f.cursor.saturating_add(offset).min(file_size),
            2 => file_size.saturating_sub(offset).min(file_size),
            _ => return None,
        };
        f.cursor = new_pos;
        Some(new_pos)
    }
}

pub fn size(fd: u32) -> Option<u64> {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return None; }
    unsafe {
        let f = &FDS[fd_idx];
        if f.file_idx < 0 { return None; }
        Some(RAMDISK_FILES[f.file_idx as usize].data.len() as u64)
    }
}
