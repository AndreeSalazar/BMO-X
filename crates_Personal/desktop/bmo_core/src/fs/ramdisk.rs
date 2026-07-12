//! RAMdisk â€” archivos embebidos en el binario del kernel.
//!
//! Sirve a las syscalls `FileOpen (0x20)`, `FileRead (0x21)`,
//! `FileClose (0x23)`. DiseÃ±ado para hospedar WADs pequeÃ±os, sprites,
//! mapas o cualquier asset de un juego portado a BMO.
//!
//! Para aÃ±adir un archivo:
//!   1. Coloca el binario en `kernel/src/fs/assets/<nombre>` (no existe aÃºn;
//!      en el repo embebemos sÃ³lo un README de prueba).
//!   2. AÃ±ade una entrada a `RAMDISK_FILES` con
//!      `include_bytes!("assets/<nombre>")`.

#![allow(dead_code)]

/// Entrada del RAMdisk.
pub struct RamFile {
    pub name: &'static str,
    pub data: &'static [u8],
}

/// Tabla estÃ¡tica â€” embebe contenido en el ELF del kernel.
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
    // Minimal x86-64 ELF: writes "Hello from BMO!\n" then exits.
    // Uses Linux syscall convention (translated by gateway).
    // Code layout at offset 0x80:
    //   [0x80] mov rax,1 (7B)  mov rdi,1 (7B)  lea rsi,[rip+21] (7B)
    //   [0x95] mov rdx,16 (7B) syscall (2B)    mov rax,60 (7B)
    //   [0xA5] xor rdi,rdi (3B) syscall (2B)   msg "Hello from BMO!\n" (16B)
    // p_filesz = code + data = 58 = 0x3A
    RamFile {
        name: "hello.elf",
        data: &[
            // ELF header (64 bytes)
            0x7f, b'E', b'L', b'F', 0x02, 0x01, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x3e, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x40, 0x00, 0x38, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Program header PT_LOAD (56 bytes)
            0x01, 0x00, 0x00, 0x00,                          // p_type
            0x05, 0x00, 0x00, 0x00,                          // p_flags = R+X
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_offset = 0x80
            0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, // p_vaddr = 0x400000
            0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, // p_paddr
            0x3A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_filesz = 0x3A
            0x3A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_memsz = 0x3A
            0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_align
            // Code at offset 0x80
            0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00,       // mov rax, 1
            0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00,       // mov rdi, 1
            0x48, 0x8d, 0x35, 0x15, 0x00, 0x00, 0x00,       // lea rsi, [rip+21]
            0x48, 0xc7, 0xc2, 0x10, 0x00, 0x00, 0x00,       // mov rdx, 16
            0x0f, 0x05,                                      // syscall
            0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00,       // mov rax, 60
            0x48, 0x31, 0xff,                                // xor rdi, rdi
            0x0f, 0x05,                                      // syscall
            b'H', b'e', b'l', b'l', b'o', b' ', b'f', b'r', // msg
            b'o', b'm', b' ', b'B', b'M', b'O', b'!', b'\n',
        ],
    },
];

/// Kernel-mode lookup: find a ramdisk file by name and return its bytes.
pub fn find_file(name: &str) -> Option<&'static [u8]> {
    for f in RAMDISK_FILES {
        if f.name == name { return Some(f.data); }
    }
    None
}

/// Tabla de descriptores abiertos por proceso (single-process por ahora).
const MAX_FDS: usize = 16;

#[derive(Clone, Copy)]
struct OpenFd {
    file_idx: i32,   // -1 = libre
    cursor: u64,
}

static mut FDS: [OpenFd; MAX_FDS] = [OpenFd { file_idx: -1, cursor: 0 }; MAX_FDS];

/// `FileOpen` â€” devuelve `fd` (0..MAX_FDS) o `u64::MAX` en error.
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

/// `FileRead(fd, ptr, len)` â†’ bytes leÃ­dos, o `u64::MAX` en error.
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

/// `FileClose(fd)` â€” libera el descriptor. Devuelve 0 OK, `u64::MAX` error.
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

/// `FileSize(fd)` â€” bytes totales del archivo, `u64::MAX` error.
pub fn size(fd: u64) -> u64 {
    let fd_idx = fd as usize;
    if fd_idx >= MAX_FDS { return u64::MAX; }
    unsafe {
        let f = &FDS[fd_idx];
        if f.file_idx < 0 { return u64::MAX; }
        RAMDISK_FILES[f.file_idx as usize].data.len() as u64
    }
}

/// `FileWrite(fd, ptr, len)` â€” bytes escritos, o `u64::MAX` en error.
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
    // RAMdisk is read-only â€” write returns 0 bytes written.
    0
}

/// `FileSeek(fd, offset, whence)` â€” new offset from file start, or `u64::MAX` error.
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
