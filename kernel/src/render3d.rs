//! Software 3D renderer — rotating cube with flat shading.
//!
//! Pure scalar f32 math, no_std, no dependencies.
//! Double-buffered: renders to a static RAM backbuffer, then blits
//! to the center of the 1920×1080 VBE framebuffer via volatile writes.

use core::ptr;

// ── Viewport / display constants ────────────────────────────────────────────

const VP_W: usize = 640;
const VP_H: usize = 480;
const DISPLAY_W: usize = 1920;
const DISPLAY_H: usize = 1080;
const FOCAL: f32 = 300.0;
const CUBE_Z: f32 = 4.0;
const BG_COLOR: u32 = 0xFF0D1117;
const BORDER_COLOR: u32 = 0xFF76B900; // NVIDIA green
const BORDER_PX: usize = 2;

// ── Backbuffer ──────────────────────────────────────────────────────────────

static mut BACKBUF: [u32; VP_W * VP_H] = [0u32; VP_W * VP_H];
static mut BACKBUF_READY: bool = false;

/// Initialise the backbuffer. Returns `true` on success.
pub fn init_backbuffer() -> bool {
    unsafe {
        let buf = &raw mut BACKBUF;
        ptr::write_bytes((*buf).as_mut_ptr(), 0, VP_W * VP_H);
        let ready = &raw mut BACKBUF_READY;
        *ready = true;
    }
    true
}

// ── Sine / cosine quarter-wave LUT (256 entries, 0 … π/2) ──────────────────

static SIN_TABLE: [f32; 256] = {
    // Generated with: sin(i * (π/2) / 255) for i in 0..256
    // We compute at compile time using a const fn below.
    const fn build() -> [f32; 256] {
        let mut t = [0.0f32; 256];
        let mut i = 0usize;
        while i < 256 {
            // Taylor series: sin(x) ≈ x − x³/6 + x⁵/120 − x⁷/5040 + x⁹/362880
            // x = i * (π/2) / 255
            let x = (i as f64) * 1.5707963267948966 / 255.0;
            let x2 = x * x;
            let x3 = x2 * x;
            let x5 = x3 * x2;
            let x7 = x5 * x2;
            let x9 = x7 * x2;
            let x11 = x9 * x2;
            let s = x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0 + x9 / 362880.0
                  - x11 / 39916800.0;
            t[i] = s as f32;
            i += 1;
        }
        t
    }
    build()
};

/// Sine via quarter-wave LUT reconstruction. Angle in radians.
fn sin_lut(angle: f32) -> f32 {
    const TWO_PI: f32 = 6.2831855;
    const HALF_PI: f32 = 1.5707964;
    const PI: f32 = 3.1415927;

    // Normalise to [0, 2π)
    let mut a = angle % TWO_PI;
    if a < 0.0 {
        a += TWO_PI;
    }

    let (sign, a) = if a < PI { (1.0f32, a) } else { (-1.0f32, a - PI) };
    let a = if a > HALF_PI { PI - a } else { a };

    // Map [0, π/2] → index [0, 255]
    let idx_f = a * (255.0 / HALF_PI);
    let idx = idx_f as usize;
    if idx >= 255 {
        return sign * SIN_TABLE[255];
    }
    // Linear interpolation between two LUT entries
    let frac = idx_f - idx as f32;
    sign * (SIN_TABLE[idx] * (1.0 - frac) + SIN_TABLE[idx + 1] * frac)
}

#[inline(always)]
fn cos_lut(angle: f32) -> f32 {
    sin_lut(angle + 1.5707964)
}

// ── Vec3 ────────────────────────────────────────────────────────────────────

#[derive(Copy, Clone)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    #[inline]
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    #[inline]
    fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    #[inline]
    fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[inline]
    fn normalize(self) -> Vec3 {
        let len2 = self.dot(self);
        if len2 < 1e-12 {
            return self;
        }
        let inv = inv_sqrt(len2);
        Vec3::new(self.x * inv, self.y * inv, self.z * inv)
    }
}

/// Fast inverse square root (good enough for lighting normals).
fn inv_sqrt(x: f32) -> f32 {
    let half = 0.5 * x;
    let i = x.to_bits();
    let i = 0x5F37_59DF - (i >> 1); // magic constant
    let y = f32::from_bits(i);
    y * (1.5 - half * y * y) // one Newton iteration
}

// ── Cube definition ─────────────────────────────────────────────────────────

const VERTS: [Vec3; 8] = [
    Vec3::new(-1.0, -1.0, -1.0), // 0
    Vec3::new( 1.0, -1.0, -1.0), // 1
    Vec3::new( 1.0,  1.0, -1.0), // 2
    Vec3::new(-1.0,  1.0, -1.0), // 3
    Vec3::new(-1.0, -1.0,  1.0), // 4
    Vec3::new( 1.0, -1.0,  1.0), // 5
    Vec3::new( 1.0,  1.0,  1.0), // 6
    Vec3::new(-1.0,  1.0,  1.0), // 7
];

// Each face: 4 vertex indices (two triangles: [0,1,2] and [0,2,3])
const FACES: [[usize; 4]; 6] = [
    [0, 1, 2, 3], // front  (z = -1)
    [5, 4, 7, 6], // back   (z = +1)
    [4, 0, 3, 7], // left   (x = -1)
    [1, 5, 6, 2], // right  (x = +1)
    [3, 2, 6, 7], // top    (y = +1)
    [4, 5, 1, 0], // bottom (y = -1)
];

const FACE_COLORS: [u32; 6] = [
    0xFF76B900, // NVIDIA green
    0xFF56D4DD, // cyan
    0xFF58A6FF, // blue
    0xFFBC8CFF, // purple
    0xFFD29922, // orange
    0xFFFF7B72, // red
];

// Light direction (normalised, pointing upper-right-forward)
const LIGHT_DIR: Vec3 = Vec3::new(0.36, 0.48, -0.80);

const AMBIENT: f32 = 0.25;

// ── Rotation helpers ────────────────────────────────────────────────────────

fn rotate_y(v: Vec3, sin: f32, cos: f32) -> Vec3 {
    Vec3::new(
        v.x * cos + v.z * sin,
        v.y,
       -v.x * sin + v.z * cos,
    )
}

fn rotate_x(v: Vec3, sin: f32, cos: f32) -> Vec3 {
    Vec3::new(
        v.x,
        v.y * cos - v.z * sin,
        v.y * sin + v.z * cos,
    )
}

// ── Perspective projection ──────────────────────────────────────────────────

#[derive(Copy, Clone)]
struct ScreenPt {
    x: f32,
    y: f32,
}

fn project(v: Vec3) -> ScreenPt {
    let iz = FOCAL / v.z;
    ScreenPt {
        x: (VP_W as f32) * 0.5 + v.x * iz,
        y: (VP_H as f32) * 0.5 - v.y * iz,
    }
}

// ── Triangle rasteriser (edge-function / barycentric) ───────────────────────

fn edge_fn(a: ScreenPt, b: ScreenPt, cx: f32, cy: f32) -> f32 {
    (cx - a.x) * (b.y - a.y) - (cy - a.y) * (b.x - a.x)
}

fn raster_triangle(buf: &mut [u32; VP_W * VP_H], p0: ScreenPt, p1: ScreenPt, p2: ScreenPt, color: u32) {
    // Bounding box (clamped to viewport)
    let min_x = min3f(p0.x, p1.x, p2.x) as i32;
    let max_x = max3f(p0.x, p1.x, p2.x) as i32;
    let min_y = min3f(p0.y, p1.y, p2.y) as i32;
    let max_y = max3f(p0.y, p1.y, p2.y) as i32;

    let min_x = if min_x < 0 { 0 } else { min_x as usize };
    let max_x = if max_x >= VP_W as i32 { VP_W - 1 } else { max_x as usize };
    let min_y = if min_y < 0 { 0 } else { min_y as usize };
    let max_y = if max_y >= VP_H as i32 { VP_H - 1 } else { max_y as usize };

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = edge_fn(p1, p2, px, py);
            let w1 = edge_fn(p2, p0, px, py);
            let w2 = edge_fn(p0, p1, px, py);
            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                buf[y * VP_W + x] = color;
            }
        }
    }
}

fn min3f(a: f32, b: f32, c: f32) -> f32 {
    let m = if a < b { a } else { b };
    if m < c { m } else { c }
}

fn max3f(a: f32, b: f32, c: f32) -> f32 {
    let m = if a > b { a } else { b };
    if m > c { m } else { c }
}

// ── Colour helpers ──────────────────────────────────────────────────────────

fn apply_brightness(color: u32, brightness: f32) -> u32 {
    let r = ((color >> 16) & 0xFF) as f32;
    let g = ((color >> 8) & 0xFF) as f32;
    let b = (color & 0xFF) as f32;
    let r = (r * brightness) as u32;
    let g = (g * brightness) as u32;
    let b = (b * brightness) as u32;
    let r = if r > 255 { 255 } else { r };
    let g = if g > 255 { 255 } else { g };
    let b = if b > 255 { 255 } else { b };
    0xFF000000 | (r << 16) | (g << 8) | b
}

// ── Blit backbuffer → framebuffer ───────────────────────────────────────────

fn blit_to_fb(fb_base: u64, fb_pitch: u32) {
    let offset_x = (DISPLAY_W - VP_W) / 2;
    let offset_y = (DISPLAY_H - VP_H) / 2;
    let pitch_px = fb_pitch as usize / 4;
    let fb = fb_base as *mut u32;

    for y in 0..VP_H {
        let src_row = unsafe { &(&(*(&raw const BACKBUF)))[y * VP_W..(y + 1) * VP_W] };
        let dst_start = (offset_y + y) * pitch_px + offset_x;
        for x in 0..VP_W {
            unsafe {
                fb.add(dst_start + x).write_volatile(src_row[x]);
            }
        }
    }
}

// ── Sort helper (painter's algorithm for ≤6 faces) ──────────────────────────

struct FaceEntry {
    face_idx: usize,
    avg_z: f32,
    shaded_color: u32,
}

// ── Main entry points ───────────────────────────────────────────────────────

/// Render one frame of the rotating cube, driven by `tick`.
pub fn render_cube(fb_base: u64, fb_pitch: u32, tick: u64) {
    unsafe {
        let ready = &raw const BACKBUF_READY;
        if !*ready {
            return;
        }
    }

    let buf = unsafe { &mut *(&raw mut BACKBUF) };

    // ── 1. Clear backbuffer ─────────────────────────────────────────────
    for px in buf.iter_mut() {
        *px = BG_COLOR;
    }

    // ── 2. Compute rotation angles ──────────────────────────────────────
    let angle_y = tick as f32 * 0.03;
    let angle_x = tick as f32 * 0.02;
    let sin_y = sin_lut(angle_y);
    let cos_y = cos_lut(angle_y);
    let sin_x = sin_lut(angle_x);
    let cos_x = cos_lut(angle_x);

    // ── 3. Transform vertices ───────────────────────────────────────────
    let mut xv = [Vec3::new(0.0, 0.0, 0.0); 8]; // camera-space
    let mut sp = [ScreenPt { x: 0.0, y: 0.0 }; 8]; // screen-space

    for i in 0..8 {
        let v = rotate_y(VERTS[i], sin_y, cos_y);
        let v = rotate_x(v, sin_x, cos_x);
        let v = v.add(Vec3::new(0.0, 0.0, CUBE_Z));
        xv[i] = v;
        sp[i] = project(v);
    }

    // ── 4. Classify faces ───────────────────────────────────────────────
    let mut visible: [FaceEntry; 6] = [
        FaceEntry { face_idx: 0, avg_z: 0.0, shaded_color: 0 },
        FaceEntry { face_idx: 0, avg_z: 0.0, shaded_color: 0 },
        FaceEntry { face_idx: 0, avg_z: 0.0, shaded_color: 0 },
        FaceEntry { face_idx: 0, avg_z: 0.0, shaded_color: 0 },
        FaceEntry { face_idx: 0, avg_z: 0.0, shaded_color: 0 },
        FaceEntry { face_idx: 0, avg_z: 0.0, shaded_color: 0 },
    ];
    let mut vis_count = 0usize;

    for fi in 0..6 {
        let f = &FACES[fi];
        let v0 = xv[f[0]];
        let v1 = xv[f[1]];
        let v2 = xv[f[2]];
        let v3 = xv[f[3]];

        let normal = (v1.sub(v0)).cross(v2.sub(v0)).normalize();

        // Backface cull: camera at origin, so view dir ≈ face centre.
        // If normal points away from camera (z >= 0), skip.
        if normal.z >= 0.0 {
            continue;
        }

        // Lambert shading
        let ndl = normal.dot(LIGHT_DIR);
        let ndl = if ndl < 0.0 { -ndl } else { ndl }; // use abs so back-lit faces still get light
        let brightness = AMBIENT + (1.0 - AMBIENT) * ndl;
        let brightness = if brightness > 1.0 { 1.0 } else { brightness };

        let avg_z = (v0.z + v1.z + v2.z + v3.z) * 0.25;

        visible[vis_count] = FaceEntry {
            face_idx: fi,
            avg_z,
            shaded_color: apply_brightness(FACE_COLORS[fi], brightness),
        };
        vis_count += 1;
    }

    // ── 5. Sort back-to-front (bubble sort, at most 6 entries) ──────────
    if vis_count > 1 {
        for i in 0..vis_count - 1 {
            for j in 0..vis_count - 1 - i {
                if visible[j].avg_z < visible[j + 1].avg_z {
                    // Swap
                    let tmp_fi = visible[j].face_idx;
                    let tmp_z = visible[j].avg_z;
                    let tmp_c = visible[j].shaded_color;
                    visible[j].face_idx = visible[j + 1].face_idx;
                    visible[j].avg_z = visible[j + 1].avg_z;
                    visible[j].shaded_color = visible[j + 1].shaded_color;
                    visible[j + 1].face_idx = tmp_fi;
                    visible[j + 1].avg_z = tmp_z;
                    visible[j + 1].shaded_color = tmp_c;
                }
            }
        }
    }

    // ── 6. Rasterise visible faces ──────────────────────────────────────
    for i in 0..vis_count {
        let fi = visible[i].face_idx;
        let color = visible[i].shaded_color;
        let f = &FACES[fi];

        // Triangle 1: f[0], f[1], f[2]
        raster_triangle(buf, sp[f[0]], sp[f[1]], sp[f[2]], color);
        // Triangle 2: f[0], f[2], f[3]
        raster_triangle(buf, sp[f[0]], sp[f[2]], sp[f[3]], color);
    }

    // ── 7. Draw NVIDIA green border (2 px) ──────────────────────────────
    for t in 0..BORDER_PX {
        // Top & bottom rows
        for x in 0..VP_W {
            buf[t * VP_W + x] = BORDER_COLOR;
            buf[(VP_H - 1 - t) * VP_W + x] = BORDER_COLOR;
        }
        // Left & right columns
        for y in 0..VP_H {
            buf[y * VP_W + t] = BORDER_COLOR;
            buf[y * VP_W + (VP_W - 1 - t)] = BORDER_COLOR;
        }
    }

    // ── 8. Blit to display ──────────────────────────────────────────────
    blit_to_fb(fb_base, fb_pitch);
}
