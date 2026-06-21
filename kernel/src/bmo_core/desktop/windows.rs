//! Windows — window title catalog, dock labels, and content-per-title mapping.
//!
//! Separated from render.rs so the render module stays focused on drawing.

#![allow(dead_code)]

use super::theme as palette;

pub const TITLES: [&[u8]; 7] = [
    b"BMO Terminal",
    b"Datos.md viewer",
    b"Juegos",
    b"Web",
    b"Ajustes",
    b"Compositor Info",
    b"Papelera",
];

pub const DOCK_LABELS: [&[u8]; 7] = [
    b"BMO Terminal", b"Datos.md", b"Juegos", b"Web", b"Ajustes", b"Buscar", b"Papelera",
];

pub const DOCK_TO_TITLE: [u8; 7] = [0, 1, 2, 3, 4, 5, 6];

static mut BMOFS_README_CONTENT: [u8; 256] = [0; 256];
static mut BMOFS_README_LEN: usize = 0;
static mut BMOFS_README_READ: bool = false;

#[allow(static_mut_refs)]
pub fn content_for<'a>(
    title_id: u8,
    fps: u32,
    frame: u64,
    buf1: &'a mut [u8; 48],
    buf2: &'a mut [u8; 48],
) -> [(&'a [u8], u32); 8] {
    match title_id {
        0 => [
            (b"$ bmo > help" as &[u8], palette::OK_FG),
            (b"  desktop   -- compositor Win/Mac/Linux" as &[u8], palette::TITLE),
            (b"  ring0     -- estado GDT/IDT/MSR" as &[u8], palette::TITLE),
            (b"  user      -- spawn 'hello' Ring 3" as &[u8], palette::TITLE),
            (b"$ bmo > _" as &[u8], palette::OK_FG),
            (b"" as &[u8], palette::TITLE),
            (b"Arrastra la barra para mover." as &[u8], palette::SUBTITLE),
            (b"Click rojo para cerrar." as &[u8], palette::SUBTITLE),
        ],
        1 => {
            unsafe {
                if !BMOFS_README_READ {
                    BMOFS_README_READ = true;
                    // Leer contenido de datos:readme desde el ramdisk
                    let name = b"datos:readme";
                    let fd = crate::bmo_core::fs::ramdisk::open(
                        name.as_ptr() as u64, name.len() as u64,
                    );
                    if fd != u64::MAX {
                        let n = crate::bmo_core::fs::ramdisk::read(
                            fd,
                            BMOFS_README_CONTENT.as_mut_ptr() as u64,
                            (BMOFS_README_CONTENT.len() - 1) as u64,
                        );
                        if n != u64::MAX {
                            BMOFS_README_LEN = n as usize;
                        }
                        crate::bmo_core::fs::ramdisk::close(fd);
                    }
                }
            }
            let readme_slice = unsafe {
                core::slice::from_raw_parts(BMOFS_README_CONTENT.as_ptr(), BMOFS_README_LEN)
            };
            [
                (b"FastOS / Datos Viewer" as &[u8], palette::CYAN_INFO),
                (readme_slice as &[u8], palette::TITLE),
                (b"" as &[u8], palette::TITLE),
                (b"RamFs: montado en /" as &[u8], palette::OK_FG),
                (b"FAT32: particion boot (UEFI)" as &[u8], palette::OK_FG),
                (b"exFAT: particion datos (RW)" as &[u8], palette::OK_FG),
                (b"Arrastra la barra para mover." as &[u8], palette::SUBTITLE),
                (b"" as &[u8], palette::TITLE),
            ]
        },
        2 => [
            (b"== Juegos ==" as &[u8], palette::CYAN_INFO),
            (b"Snake     (pendiente)" as &[u8], palette::SUBTITLE),
            (b"Tetris    (pendiente)" as &[u8], palette::SUBTITLE),
            (b"Pong      (pendiente)" as &[u8], palette::SUBTITLE),
            (b"DOOM      (4-6 sesiones)" as &[u8], palette::SUBTITLE),
            (b"" as &[u8], palette::TITLE),
            (b"Ver ROADMAP_GAMES.md" as &[u8], palette::OK_FG),
            (b"" as &[u8], palette::TITLE),
        ],
        3 => [
            (b"== Web ==" as &[u8], palette::CYAN_INFO),
            (b"bmo_gpu::net listo:" as &[u8], palette::TITLE),
            (b"  TCP/UDP/QUIC/TLS13" as &[u8], palette::SUBTITLE),
            (b"  HTTP3 + DNS" as &[u8], palette::SUBTITLE),
            (b"  ring buffers io_uring-style" as &[u8], palette::SUBTITLE),
            (b"" as &[u8], palette::TITLE),
            (b"Falta: driver NIC real." as &[u8], palette::SUBTITLE),
            (b"" as &[u8], palette::TITLE),
        ],
        4 => [
            (b"== Ajustes ==" as &[u8], palette::CYAN_INFO),
            (b"CPU: AMD Ryzen 5 5600X" as &[u8], palette::TITLE),
            (b"GPU: UEFI GOP framebuffer" as &[u8], palette::TITLE),
            (b"RAM: BootInfo memory map" as &[u8], palette::TITLE),
            (b"USB: keyboard + mouse + Redragon" as &[u8], palette::TITLE),
            (b"Boot: UEFI puro (sin legacy)" as &[u8], palette::TITLE),
            (b"BMO ABI: 7-GPR, 64B align" as &[u8], palette::OK_FG),
            (b"" as &[u8], palette::TITLE),
        ],
        5 => {
            let mut p = 0;
            buf1[p..p+5].copy_from_slice(b"FPS: "); p += 5;
            let s = fmt_u64_into(&mut buf1[p..], fps as u64); p += s;
            let p1 = p;
            let mut q = 0;
            buf2[q..q+8].copy_from_slice(b"Frame:  "); q += 8;
            let s2 = fmt_u64_into(&mut buf2[q..], frame); q += s2;
            let p2 = q;
            let l1: &[u8] = &buf1[..p1];
            let l2: &[u8] = &buf2[..p2];
            [
                (l1, palette::CYAN_INFO),
                (l2, palette::CYAN_INFO),
                (b"Renderer: Ring 0 / Rust" as &[u8], palette::TITLE),
                (b"Wallpaper: gradiente azul -> purpura" as &[u8], palette::SUBTITLE),
                (b"Ventanas: rounded + shadow + traffic" as &[u8], palette::SUBTITLE),
                (b"Dock: macOS-style + click launch" as &[u8], palette::SUBTITLE),
                (b"Drag-and-drop sobre titlebar" as &[u8], palette::OK_FG),
                (b"ESC para salir." as &[u8], palette::OK_FG),
            ]
        }
        6 => [
            (b"Papelera vacia." as &[u8], palette::SUBTITLE),
            (b"" as &[u8], palette::TITLE), (b"" as &[u8], palette::TITLE),
            (b"" as &[u8], palette::TITLE), (b"" as &[u8], palette::TITLE),
            (b"" as &[u8], palette::TITLE), (b"" as &[u8], palette::TITLE),
            (b"" as &[u8], palette::TITLE),
        ],
        _ => [
            (b"(ventana sin contenido)" as &[u8], palette::SUBTITLE),
            (b"" as &[u8], palette::TITLE), (b"" as &[u8], palette::TITLE),
            (b"" as &[u8], palette::TITLE), (b"" as &[u8], palette::TITLE),
            (b"" as &[u8], palette::TITLE), (b"" as &[u8], palette::TITLE),
            (b"" as &[u8], palette::TITLE),
        ],
    }
}

pub fn fmt_u64_into(buf: &mut [u8], mut v: u64) -> usize {
    if v == 0 { buf[0] = b'0'; return 1; }
    let mut tmp = [0u8; 20]; let mut i = 0;
    while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
    for k in 0..i { buf[k] = tmp[i - 1 - k]; }
    i
}
