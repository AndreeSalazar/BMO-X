//! RAMdisk — archivos embebidos en el binario del kernel.
//!
//! Sirve a las syscalls `FileOpen (0x20)`, `FileRead (0x21)`,
//! `FileClose (0x23)`. Diseñado para hospedar WADs pequeños, sprites,
//! mapas o cualquier asset de un juego portado a FastOS.
//!
//! Para añadir un archivo:
//!   1. Coloca el binario en `kernel/src/fs/assets/<nombre>` (no existe aún;
//!      en el repo embebemos sólo un README de prueba).
//!   2. Añade una entrada a `RAMDISK_FILES` con
//!      `include_bytes!("assets/<nombre>")`.

#![allow(dead_code)]

/// Entrada del RAMdisk.
pub struct RamFile {
    pub name: &'static str,
    pub data: &'static [u8],
}

/// Tabla estática — embebe contenido en el ELF del kernel.
///
/// La primera entrada (`bmo:readme`) sirve como autotest del FileRead.
pub static RAMDISK_FILES: &[RamFile] = &[
    RamFile {
        name: "bmo:readme",
        data: b"FastOS / BMO RAMdisk operativo.\n\
Para cargar un WAD de DOOM coloca el binario y\n\
declara la entrada en src/fs/ramdisk.rs::RAMDISK_FILES.\n\
Las syscalls FileOpen(0x20)/FileRead(0x21)/FileClose(0x23)\n\
sirven el contenido desde Ring 3 sin tocar el disco.\n",
    },
];

/// Tabla de descriptores abiertos por proceso (single-process por ahora).
const MAX_FDS: usize = 16;

#[derive(Clone, Copy)]
struct OpenFd {
    file_idx: i32,   // -1 = libre
    cursor: u64,
}

static mut FDS: [OpenFd; MAX_FDS] = [OpenFd { file_idx: -1, cursor: 0 }; MAX_FDS];

/// `FileOpen` — devuelve `fd` (0..MAX_FDS) o `u64::MAX` en error.
pub fn open(name_ptr: u64, name_len: u64) -> u64 {
    if name_ptr == 0 || name_len == 0 || name_len > 256 {
        return u64::MAX;
    }
    let bytes = unsafe { core::slice::from_raw_parts(name_ptr as *const u8, name_len as usize) };
    let Ok(name) = core::str::from_utf8(bytes) else { return u64::MAX };

    let mut file_idx: i32 = -1;
    for (i, f) in RAMDISK_FILES.iter().enumerate() {
        if f.name == name { file_idx = i as i32; break; }
    }
    if file_idx < 0 { return u64::MAX; }

    unsafe {
        for i in 0..MAX_FDS {
            if FDS[i].file_idx < 0 {
                FDS[i].file_idx = file_idx;
                FDS[i].cursor = 0;
                return i as u64;
            }
        }
    }
    u64::MAX
}

/// `FileRead(fd, ptr, len)` → bytes leídos, o `u64::MAX` en error.
pub fn read(fd: u64, ptr: u64, len: u64) -> u64 {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return u64::MAX; }
    if ptr == 0 || len == 0 || len > (1 << 24) { return u64::MAX; }

    unsafe {
        let f = &mut FDS[fd_idx];
        if f.file_idx < 0 { return u64::MAX; }
        let file = &RAMDISK_FILES[f.file_idx as usize];
        let remaining = (file.data.len() as u64).saturating_sub(f.cursor);
        let n = len.min(remaining) as usize;
        if n == 0 { return 0; }
        let src = file.data.as_ptr().add(f.cursor as usize);
        let dst = ptr as *mut u8;
        core::ptr::copy_nonoverlapping(src, dst, n);
        f.cursor += n as u64;
        n as u64
    }
}

/// `FileClose(fd)` — libera el descriptor. Devuelve 0 OK, `u64::MAX` error.
pub fn close(fd: u64) -> u64 {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return u64::MAX; }
    unsafe {
        if FDS[fd_idx].file_idx < 0 { return u64::MAX; }
        FDS[fd_idx].file_idx = -1;
        FDS[fd_idx].cursor = 0;
    }
    0
}

/// `FileSize(fd)` — bytes totales del archivo, `u64::MAX` error.
pub fn size(fd: u64) -> u64 {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return u64::MAX; }
    unsafe {
        let f = &FDS[fd_idx];
        if f.file_idx < 0 { return u64::MAX; }
        RAMDISK_FILES[f.file_idx as usize].data.len() as u64
    }
}

/// `FileWrite(fd, ptr, len)` — bytes escritos, o `u64::MAX` en error.
///
/// RAMdisk is read-only by default. This function supports a small
/// writable overlay: the first RAMDISK_FILES entry can be marked
/// writable, or we allocate a write buffer on first write.
/// For now, returns 0 (read-only filesystem) to prevent corruption.
pub fn write(fd: u64, _ptr: u64, _len: u64) -> u64 {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return u64::MAX; }
    unsafe {
        let f = &FDS[fd_idx];
        if f.file_idx < 0 { return u64::MAX; }
    }
    // RAMdisk is read-only — write returns 0 bytes written.
    0
}

/// `FileSeek(fd, offset, whence)` — new offset from file start, or `u64::MAX` error.
///
/// whence:
///   0 = SEEK_SET (offset from start)
///   1 = SEEK_CUR (offset from current position)
///   2 = SEEK_END (offset from end)
pub fn seek(fd: u64, offset: u64, whence: u64) -> u64 {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return u64::MAX; }
    unsafe {
        let f = &mut FDS[fd_idx];
        if f.file_idx < 0 { return u64::MAX; }
        let file_size = RAMDISK_FILES[f.file_idx as usize].data.len() as u64;
        let new_pos = match whence {
            0 => offset.min(file_size),                                    // SEEK_SET
            1 => f.cursor.saturating_add(offset).min(file_size),           // SEEK_CUR
            2 => file_size.saturating_sub(offset).min(file_size),          // SEEK_END
            _ => return u64::MAX,
        };
        f.cursor = new_pos;
        new_pos
    }
}
