# BFS — FastOS Filesystem

> BFS es el filesystem nativo de FastOS. Diseñado para ser
> simple, robusto, y rápido en operaciones comunes.

## Layout en disco

```
┌──────────────┐ Block 0
│  Superblock  │   4096 bytes
├──────────────┤ Block 1..B0
│ Block Bitmap │   1 bit por bloque
├──────────────┤ B0..B1
│ Inode Bitmap │   1 bit por inode
├──────────────┤ B1..B1+I
│ Inode Table  │   128 bytes por inode
├──────────────┤
│              │
│  Data Blocks │
│              │
└──────────────┘
```

Magic: `0xBFBF_BFBF`. Versión actual: `0x0003_0000` (BFS 3.0).

## Superblock (4096 bytes)

```rust
pub struct Superblock {
    pub magic: u32,        // 0xBFBF_BFBF
    pub version: u32,      // 0x0003_0000
    pub block_size: u32,   // 4096
    pub total_blocks: u64,
    pub free_blocks: u64,
    pub root_ino: u32,     // 1
    pub inode_count: u32,
    pub mount_count: u32,
    pub max_mounts: u32,
    pub flags: u32,
    pub label: [u8; 64],
    pub uuid: [u8; 16],
    pub created_at: u64,
    pub last_mounted: u64,
    pub last_check: u64,
    pub reserved: [u8; 4032],
}
```

## Inode (128 bytes)

```rust
pub struct Inode {
    pub magic: u32,        // 0xB1B1_B1B1
    pub mode: u32,         // regular/dir/symlink
    pub uid: u16,
    pub gid: u16,
    pub size: u64,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
    pub block_count: u32,
    pub direct: [u64; 8],  // 32 KB
    pub indirect: u64,     // 1 ptr (4 MB indirect)
    pub double_indirect: u64,
    pub reserved: [u8; 30],
}
```

Modos:
- 0x8000: regular file
- 0x4000: directory
- 0xA000: symlink
- 0x1000: FIFO (no implementado en v1.7.4)

Permisos (3 bits rwx para owner/group/other).

## Directory entry (256 bytes)

```rust
pub struct DirEntry {
    pub ino: u32,
    pub name_len: u16,    // max 200
    pub file_type: u8,
    pub reserved: u8,
    pub name: [u8; 200],  // null-terminated
    pub padding: [u8; 38],
}
```

16 entries por bloque de 4 KB.

## Path resolution

`/home/user/file.txt` se resuelve así:

1. Comienza en inodo 1 (root).
2. Para cada componente (después de /), busca en el directorio actual.
3. Devuelve el inodo del último componente.

Cada lookup es O(n) en el directorio. v1.7.4 no usa dir index
(no es necesario para el scope de la release).

## API pública (`bmo_core::fs`)

### `mount(dev_path) -> Result<(), FsError>`
Monta un dispositivo BFS. Llena el superblock desde el disco.

### `unmount() -> Result<(), FsError>`
Desmonta. Hace sync.

### `open(path, flags) -> Result<File, FsError>`
Abre un archivo. `flags`:
- 0: O_RDONLY
- 1: O_WRONLY
- 2: O_RDWR
- 0x40: O_CREAT
- 0x200: O_TRUNC
- 0x400: O_APPEND

### `read(file, buf, len) -> Result<usize, FsError>`
Lee hasta `len` bytes. Avanza el cursor.

### `write(file, buf, len) -> Result<usize, FsError>`
Escribe. Avanza el cursor.

### `close(file) -> Result<(), FsError>`
Cierra. Hace flush si dirty.

### `seek(file, offset, whence) -> Result<u64, FsError>`
Mueve el cursor. `whence`: 0 (set), 1 (cur), 2 (end).

### `stat(path) -> Result<Inode, FsError>`
Devuelve el inodo de un path.

### `mkdir(path) -> Result<(), FsError>`
Crea un directorio.

### `rmdir(path) -> Result<(), FsError>`
Borra un directorio (debe estar vacío).

### `unlink(path) -> Result<(), FsError>`
Borra un archivo.

### `rename(old, new) -> Result<(), FsError>`
Renombra.

### `readdir(path) -> Result<DirEntry, FsError>`
Lee la siguiente entry de un directorio. Llamar en loop hasta EOF.

### `sync() -> Result<(), FsError>`
Flush de todos los buffers al disco.

## Block allocator

- `alloc_block() -> Option<u64>`: encuentra el primer bit libre en el block bitmap.
- `free_block(u64)`: pone el bit a 0.
- Implementación: linear scan del bitmap. v1.7.4 no usa Buddy allocator.

## Inode allocator

Igual al block allocator, pero con el inode bitmap.

## Journaling

v1.7.4 **no tiene journal**. La consistencia se hace con `sync()`
explícito y en `unmount`. Una caída puede corromper el FS.

En v1.8.0: añadir journal write-ahead en el block 1, con
commit cada 5 segundos.

## Tamaño máximo

- Archivo: 8 bloques directos (32 KB) + 1024 indirect (4 MB) +
  1024*1024 double-indirect (4 GB) = ~4 GB.
- Filesystem: limitado por block count. Con bloques de 4 KB,
  max 16 TB.

## Limitaciones v1.7.4

- Sin permisos POSIX (sólo un usuario).
- Sin hard links.
- Sin extended attributes.
- Sin ACL.
- Sin quota.
- Sin encryption.
