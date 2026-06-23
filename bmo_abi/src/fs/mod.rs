//! `bmo_abi::fs` — Tipos del filesystem.
//!
//! Define los **datos** que las funciones `fs_*` (declaradas en
//! `crate::bmo_abi::syscalls`) reciben o devuelven.
//!
//! ## Modelo
//!
//! - Paths son **UTF-8** válido, separado por `/`.
//! - File handles son procesos-locales.
//! - Directorios se leen con `bmo_fs_readdir` que devuelve un buffer
//!   de `BmoDirEntry` contiguos.
//! - `bmo_fs_seek` usa `BmoSeekWhence`.

#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;

// ─── Handle ─────────────────────────────────────────────────────────

/// Handle a un archivo abierto. Proceso-local.
pub type BmoFileHandle = BmoHandle;

/// Handle a un directorio abierto (para `readdir`).
pub type BmoDirHandle = BmoHandle;

// ─── Open flags ─────────────────────────────────────────────────────

/// Flags para `bmo_fs_open`. Se combinan con `|`.
///
/// Layout: bits 0..7 = access mode, bits 8..15 = creation, bits 16.. = misc.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BmoOpenFlags(pub u32);

impl BmoOpenFlags {
    /// Modo de acceso.
    pub const RDONLY: Self = Self(0x0000_0000);
    pub const WRONLY: Self = Self(0x0000_0001);
    pub const RDWR:   Self = Self(0x0000_0002);

    /// Modo de creación (uno de los tres).
    pub const CREATE:    Self = Self(0x0000_0040);
    pub const EXCLUSIVE: Self = Self(0x0000_0080);
    pub const TRUNCATE:  Self = Self(0x0000_0200);
    pub const APPEND:    Self = Self(0x0000_0400);

    /// Misc.
    pub const NOCTTY:  Self = Self(0x0000_0100);
    pub const NONBLOCK: Self = Self(0x0000_0800);
    pub const DIRECTORY: Self = Self(0x0001_0000);
    pub const NOFOLLOW:  Self = Self(0x0002_0000);
    pub const SYMLINK:   Self = Self(0x0004_0000);
    pub const CLOSE_ON_EXEC: Self = Self(0x0008_0000);

    /// Bits de modo de acceso (máscara).
    pub const ACCESS_MASK: u32 = 0x0000_0003;

    #[inline]
    pub fn access_mode(self) -> u32 { self.0 & Self::ACCESS_MASK }

    #[inline]
    pub fn is_readable(self) -> bool {
        let m = self.access_mode();
        m == Self::RDONLY.0 || m == Self::RDWR.0
    }
    #[inline]
    pub fn is_writable(self) -> bool {
        let m = self.access_mode();
        m == Self::WRONLY.0 || m == Self::RDWR.0
    }

    /// Combina dos sets de flags.
    #[inline]
    pub fn union(self, other: Self) -> Self { Self(self.0 | other.0) }
    /// Intersección.
    #[inline]
    pub fn intersect(self, other: Self) -> Self { Self(self.0 & other.0) }
    /// `true` si todos los bits de `other` están en `self`.
    #[inline]
    pub fn contains(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
}

// ─── Seek whence ────────────────────────────────────────────────────

/// Referencia para `bmo_fs_seek`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoSeekWhence {
    /// Desde el inicio del archivo.
    Set = 0,
    /// Desde la posición actual.
    Cur = 1,
    /// Desde el final (offset debe ser ≤ 0).
    End = 2,
    /// Devuelve el tamaño sin mover el cursor.
    Size = 3,
}

// ─── Stat ───────────────────────────────────────────────────────────

/// Tipo de archivo (campo `kind` de `BmoStat`).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BmoFileType {
    #[default]
    Unknown  = 0,
    Regular  = 1,
    Directory = 2,
    Symlink  = 3,
    Character = 4,
    Block    = 5,
    Pipe     = 6,
    Socket   = 7,
}

/// Permisos (estilo Unix, pero simplificado).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BmoPerms(pub u16);

impl BmoPerms {
    pub const NONE:   Self = Self(0o000);
    pub const X:      Self = Self(0o111);
    pub const W:      Self = Self(0o222);
    pub const R:      Self = Self(0o444);
    pub const RWX:    Self = Self(0o777);
    pub const RW_R:   Self = Self(0o644); // usuario rw, grupo r, otros r
    pub const RWX_RXR: Self = Self(0o755);

    #[inline]
    pub fn is_readable(self) -> bool { (self.0 & 0o444) != 0 }
    #[inline]
    pub fn is_writable(self) -> bool { (self.0 & 0o222) != 0 }
    #[inline]
    pub fn is_executable(self) -> bool { (self.0 & 0o111) != 0 }

    /// Permisos `rw-r--r--` (0644).
    #[inline]
    pub const fn rw() -> Self {
        Self(0o644)
    }

    /// Permisos `rwxr-xr-x` (0755).
    #[inline]
    pub const fn rwx() -> Self {
        Self(0o755)
    }
}

/// Resultado de `bmo_fs_stat` / `bmo_fs_fstat`.
///
/// Tamaño: 96 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BmoStat {
    pub kind: BmoFileType,
    pub _pad0: u32,
    pub perms: BmoPerms,
    pub _pad1: u16,
    /// Tamaño en bytes.
    pub size: u64,
    /// Última modificación (ns desde epoch).
    pub mtime_ns: u64,
    /// Creación (ns desde epoch).
    pub ctime_ns: u64,
    /// Último acceso (ns desde epoch).
    pub atime_ns: u64,
    /// Número de links duros.
    pub nlinks: u32,
    /// ID del dispositivo.
    pub dev: u32,
    /// ID del inodo.
    pub ino: u64,
    /// UID del dueño.
    pub uid: u32,
    /// GID del grupo.
    pub gid: u32,
}

// ─── Dir entry ──────────────────────────────────────────────────────

/// Una entrada de directorio. Tamaño fijo: 320 bytes (256 name + 8 size + ...).
///
/// Los nombres son UTF-8 null-terminated, máximo 255 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BmoDirEntry {
    /// Nombre del archivo. UTF-8, null-terminated.
    pub name: [u8; 256],
    pub kind: BmoFileType,
    pub _pad0: u32,
    pub perms: BmoPerms,
    pub _pad1: u16,
    pub size: u64,
    pub inode: u64,
    pub _pad2: u32,
}

impl BmoDirEntry {
    pub const SIZE: usize = 320;

    /// Lee el nombre como `&str` (o `""` si no es UTF-8 válido).
    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

/// Capabilities de un proceso (qué puede hacer en el FS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities(pub u32);

impl Capabilities {
    pub const NONE: Self = Self(0);
    pub const READ_FS: Self = Self(1 << 0);
    pub const WRITE_FS: Self = Self(1 << 1);
    pub const EXEC: Self = Self(1 << 2);
    pub const NET: Self = Self(1 << 3);
    pub const GPU: Self = Self(1 << 4);
    pub const SYS_DEBUG: Self = Self(1 << 5);
    pub const FS_READ: Self = Self(1 << 0);
    pub const FS_WRITE: Self = Self(1 << 1);
    pub const SYS_TIME_HIRES: Self = Self(1 << 6);
    pub const SYS_GPU_SUBMIT: Self = Self(1 << 7);
    pub const SYS_INPUT: Self = Self(1 << 8);
    pub const NET_RAW: Self = Self(1 << 9);
    pub const ALL: Self = Self(0xFFFF_FFFF);

    pub fn has(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
    pub fn insert(&mut self, other: Self) { self.0 |= other.0; }
    pub fn remove(&mut self, other: Self) { self.0 &= !other.0; }
}
