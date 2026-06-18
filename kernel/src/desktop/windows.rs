//! Windows — window title catalog, dock labels, and content-per-title mapping.
//!
//! Separated from render.rs so the render module stays focused on drawing.

#![allow(dead_code)]

use super::render::palette;

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
            (b"$ bmo > help" as &[u8], palette::TEXT_OK),
            (b"  desktop   -- compositor Win/Mac/Linux" as &[u8], palette::TEXT_PRIMARY),
            (b"  ring0     -- estado GDT/IDT/MSR" as &[u8], palette::TEXT_PRIMARY),
            (b"  user      -- spawn 'hello' Ring 3" as &[u8], palette::TEXT_PRIMARY),
            (b"$ bmo > _" as &[u8], palette::TEXT_OK),
            (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"Arrastra la barra para mover." as &[u8], palette::TEXT_SECOND),
            (b"Click rojo para cerrar." as &[u8], palette::TEXT_SECOND),
        ],
        1 => {
            unsafe {
                if !BMOFS_README_READ {
                    BMOFS_README_READ = true;
                    if let Ok(readme) = crate::fs::bmofs_loop::read_readme_from_bmofs() {
                        let bytes = readme.as_bytes();
                        let len = bytes.len().min(BMOFS_README_CONTENT.len() - 1);
                        BMOFS_README_CONTENT[..len].copy_from_slice(&bytes[..len]);
                        BMOFS_README_LEN = len;
                    }
                }
            }
            let readme_slice = unsafe {
                core::slice::from_raw_parts(BMOFS_README_CONTENT.as_ptr(), BMOFS_README_LEN)
            };
            [
                (b"FastOS / BMO-FS Reader" as &[u8], palette::TEXT_INFO),
                (readme_slice as &[u8], palette::TEXT_PRIMARY),
                (b"" as &[u8], palette::TEXT_PRIMARY),
                (b"Montaje de Loop Device: OK" as &[u8], palette::TEXT_OK),
                (b"Firma de Superblock: OK" as &[u8], palette::TEXT_OK),
                (b"Particion FAT32: OK" as &[u8], palette::TEXT_OK),
                (b"Interoperabilidad UEFI: OK" as &[u8], palette::TEXT_OK),
                (b"Arrastra la barra para mover." as &[u8], palette::TEXT_SECOND),
            ]
        },
        2 => [
            (b"== Juegos ==" as &[u8], palette::TEXT_INFO),
            (b"Snake     (pendiente)" as &[u8], palette::TEXT_SECOND),
            (b"Tetris    (pendiente)" as &[u8], palette::TEXT_SECOND),
            (b"Pong      (pendiente)" as &[u8], palette::TEXT_SECOND),
            (b"DOOM      (4-6 sesiones)" as &[u8], palette::TEXT_SECOND),
            (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"Ver ROADMAP_GAMES.md" as &[u8], palette::TEXT_OK),
            (b"" as &[u8], palette::TEXT_PRIMARY),
        ],
        3 => [
            (b"== Web ==" as &[u8], palette::TEXT_INFO),
            (b"barex::net listo:" as &[u8], palette::TEXT_PRIMARY),
            (b"  TCP/UDP/QUIC/TLS13" as &[u8], palette::TEXT_SECOND),
            (b"  HTTP3 + DNS" as &[u8], palette::TEXT_SECOND),
            (b"  ring buffers io_uring-style" as &[u8], palette::TEXT_SECOND),
            (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"Falta: driver NIC real." as &[u8], palette::TEXT_SECOND),
            (b"" as &[u8], palette::TEXT_PRIMARY),
        ],
        4 => [
            (b"== Ajustes ==" as &[u8], palette::TEXT_INFO),
            (b"CPU: AMD Ryzen 5 5600X" as &[u8], palette::TEXT_PRIMARY),
            (b"GPU: UEFI GOP framebuffer" as &[u8], palette::TEXT_PRIMARY),
            (b"RAM: BootInfo memory map" as &[u8], palette::TEXT_PRIMARY),
            (b"USB: keyboard + mouse + Redragon" as &[u8], palette::TEXT_PRIMARY),
            (b"Boot: UEFI puro (sin legacy)" as &[u8], palette::TEXT_PRIMARY),
            (b"BMO ABI: 7-GPR, 64B align" as &[u8], palette::TEXT_OK),
            (b"" as &[u8], palette::TEXT_PRIMARY),
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
                (l1, palette::TEXT_INFO),
                (l2, palette::TEXT_INFO),
                (b"Renderer: Ring 0 / Rust" as &[u8], palette::TEXT_PRIMARY),
                (b"Wallpaper: gradiente azul -> purpura" as &[u8], palette::TEXT_SECOND),
                (b"Ventanas: rounded + shadow + traffic" as &[u8], palette::TEXT_SECOND),
                (b"Dock: macOS-style + click launch" as &[u8], palette::TEXT_SECOND),
                (b"Drag-and-drop sobre titlebar" as &[u8], palette::TEXT_OK),
                (b"ESC para salir." as &[u8], palette::TEXT_OK),
            ]
        }
        6 => [
            (b"Papelera vacia." as &[u8], palette::TEXT_SECOND),
            (b"" as &[u8], palette::TEXT_PRIMARY), (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"" as &[u8], palette::TEXT_PRIMARY), (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"" as &[u8], palette::TEXT_PRIMARY), (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"" as &[u8], palette::TEXT_PRIMARY),
        ],
        _ => [
            (b"(ventana sin contenido)" as &[u8], palette::TEXT_SECOND),
            (b"" as &[u8], palette::TEXT_PRIMARY), (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"" as &[u8], palette::TEXT_PRIMARY), (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"" as &[u8], palette::TEXT_PRIMARY), (b"" as &[u8], palette::TEXT_PRIMARY),
            (b"" as &[u8], palette::TEXT_PRIMARY),
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
