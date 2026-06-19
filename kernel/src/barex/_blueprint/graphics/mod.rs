//! BareX Graphics — Device + Queue + CmdList + Swapchain
//!
//! GOP software rendering backend. All "GPU" operations are CPU-side
//! framebuffer blits via the UEFI GOP framebuffer.

#![allow(dead_code)]

use crate::drivers::serial;

// v1.2.0: blueprint está dos niveles abajo de `barex`, así que el
// import pasa por `super::super::BxError`.
use super::super::BxError;

/// GOP framebuffer state (set during init).
static mut FB_BASE: u64 = 0;
static mut FB_WIDTH: u32 = 0;
static mut FB_HEIGHT: u32 = 0;
static mut FB_PITCH: u32 = 0; // bytes per row
static mut FB_INITIALIZED: bool = false;

/// Initialize the graphics subsystem with GOP framebuffer info.
pub fn init_gop(fb_base: u64, width: u32, height: u32, pitch: u32) {
    unsafe {
        FB_BASE = fb_base;
        FB_WIDTH = width;
        FB_HEIGHT = height;
        FB_PITCH = pitch;
        FB_INITIALIZED = true;
    }
    serial::serial_write("[barex::graphics] GOP backend initialized\n");
}

pub fn is_initialized() -> bool {
    unsafe { FB_INITIALIZED }
}

pub fn screen_width() -> u32 { unsafe { FB_WIDTH } }
pub fn screen_height() -> u32 { unsafe { FB_HEIGHT } }

// ── BxDevice ──────────────────────────────────────────────────────────

pub struct BxDevice {
    id: u32,
}

impl BxDevice {
    pub fn new(id: u32) -> Self { Self { id } }
    pub fn id(&self) -> u32 { self.id }
}

/// Create the primary graphics device.
pub fn create_device() -> Result<BxDevice, BxError> {
    if !is_initialized() {
        return Err(BxError::NotInitialized);
    }
    Ok(BxDevice::new(0))
}

// ── BxQueue ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueueKind {
    Graphics,
    Compute,
    Copy,
    VideoDecode,
    VideoEncode,
}

pub struct BxQueue {
    kind: QueueKind,
}

impl BxQueue {
    pub fn kind(&self) -> QueueKind { self.kind }
}

pub fn create_queue(_device: &BxDevice, kind: QueueKind) -> Result<BxQueue, BxError> {
    Ok(BxQueue { kind })
}

// ── BxCmdList ─────────────────────────────────────────────────────────

/// Command types that can be recorded into a command list.
#[derive(Debug, Clone, Copy)]
enum Cmd {
    ClearRect { x: u32, y: u32, w: u32, h: u32, color: u32 },
    CopyBuffer { src: u32, dst: u32, size: u64 },
    DrawRect { x: u32, y: u32, w: u32, h: u32, color: u32 },
    DrawLine { x0: u32, y0: u32, x1: u32, y1: u32, color: u32, width: u32 },
    BlitTexture { tex_id: u32, dst_x: u32, dst_y: u32, dst_w: u32, dst_h: u32 },
    Present,
}

const MAX_CMDS: usize = 512;

pub struct BxCmdList {
    cmds: [Cmd; MAX_CMDS],
    count: usize,
}

impl BxCmdList {
    pub fn new() -> Self {
        Self {
            cmds: [Cmd::Present; MAX_CMDS], // zero-init with a safe default
            count: 0,
        }
    }

    pub fn is_empty(&self) -> bool { self.count == 0 }
    pub fn len(&self) -> usize { self.count }
}

// ── Recording commands ────────────────────────────────────────────────

pub fn cmd_clear(cmds: &mut BxCmdList, color: u32) {
    if cmds.count < MAX_CMDS {
        cmds.cmds[cmds.count] = Cmd::ClearRect {
            x: 0, y: 0,
            w: unsafe { FB_WIDTH },
            h: unsafe { FB_HEIGHT },
            color,
        };
        cmds.count += 1;
    }
}

pub fn cmd_fill_rect(cmds: &mut BxCmdList, x: u32, y: u32, w: u32, h: u32, color: u32) {
    if cmds.count < MAX_CMDS {
        cmds.cmds[cmds.count] = Cmd::DrawRect { x, y, w, h, color };
        cmds.count += 1;
    }
}

pub fn cmd_draw_line(cmds: &mut BxCmdList, x0: u32, y0: u32, x1: u32, y1: u32, color: u32, width: u32) {
    if cmds.count < MAX_CMDS {
        cmds.cmds[cmds.count] = Cmd::DrawLine { x0, y0, x1, y1, color, width };
        cmds.count += 1;
    }
}

pub fn cmd_blit_texture(cmds: &mut BxCmdList, tex_id: u32, dx: u32, dy: u32, dw: u32, dh: u32) {
    if cmds.count < MAX_CMDS {
        cmds.cmds[cmds.count] = Cmd::BlitTexture { tex_id, dst_x: dx, dst_y: dy, dst_w: dw, dst_h: dh };
        cmds.count += 1;
    }
}

pub fn cmd_present(cmds: &mut BxCmdList) {
    if cmds.count < MAX_CMDS {
        cmds.cmds[cmds.count] = Cmd::Present;
        cmds.count += 1;
    }
}

// ── Executing commands (GOP software renderer) ────────────────────────

pub fn execute(cmds: &BxCmdList) -> Result<(), BxError> {
    if !is_initialized() {
        return Err(BxError::NotInitialized);
    }

    for i in 0..cmds.count {
        match cmds.cmds[i] {
            Cmd::ClearRect { x, y, w, h, color } => {
                fill_rect_raw(x, y, w, h, color);
            }
            Cmd::DrawRect { x, y, w, h, color } => {
                fill_rect_raw(x, y, w, h, color);
            }
            Cmd::DrawLine { x0, y0, x1, y1, color, width } => {
                draw_line_raw(x0, y0, x1, y1, color, width);
            }
            Cmd::BlitTexture { tex_id: _, dst_x, dst_y, dst_w, dst_h } => {
                // Texture blit — placeholder (needs texture registry)
                fill_rect_raw(dst_x, dst_y, dst_w, dst_h, 0xFF_FF00FF); // magenta = missing
            }
            Cmd::CopyBuffer { .. } => {} // No-op for software renderer
            Cmd::Present => {
                // In GOP mode, framebuffer is already visible — no-op
            }
        }
    }

    Ok(())
}

// ── BxSwapchain ───────────────────────────────────────────────────────

pub struct BxSwapchain {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    back_buffer: u64, // physical address of back buffer (0 = use front directly)
}

impl BxSwapchain {
    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn is_valid(&self) -> bool { self.width > 0 && self.height > 0 }
}

pub fn create_swapchain(width: u32, height: u32, format: u32) -> Result<BxSwapchain, BxError> {
    if width == 0 || height == 0 {
        return Err(BxError::InvalidArgument);
    }
    // Use GOP front buffer directly (double-buffering requires separate alloc)
    Ok(BxSwapchain {
        width,
        height,
        format,
        back_buffer: 0, // 0 = direct front buffer
    })
}

pub fn present_swapchain(_swap: &BxSwapchain) -> Result<(), BxError> {
    // In GOP mode, presenting is a no-op (front buffer is the display)
    Ok(())
}

// ── BxBuffer ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferHint {
    GpuLocal,
    GpuUpload,
    GpuReadback,
    CpuOnly,
}

pub struct BxBuffer {
    pub size_bytes: u64,
    pub hint: BufferHint,
    data: *mut u8,
}

impl BxBuffer {
    pub fn size(&self) -> u64 { self.size_bytes }
    pub fn hint(&self) -> BufferHint { self.hint }
    pub fn is_mapped(&self) -> bool { !self.data.is_null() }
}

pub fn create_buffer(size: u64, hint: BufferHint) -> Result<BxBuffer, BxError> {
    if size == 0 {
        return Err(BxError::InvalidArgument);
    }

    // Allocate CPU-accessible memory
    let layout = core::alloc::Layout::from_size_align(size as usize, 16)
        .map_err(|_| BxError::InvalidArgument)?;
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() {
        return Err(BxError::OutOfMemory);
    }

    // Zero the buffer
    unsafe { core::ptr::write_bytes(ptr, 0, size as usize); }

    Ok(BxBuffer {
        size_bytes: size,
        hint,
        data: ptr,
    })
}

pub fn destroy_buffer(buf: &mut BxBuffer) {
    if !buf.data.is_null() {
        let layout = core::alloc::Layout::from_size_align(buf.size_bytes as usize, 16).unwrap();
        unsafe { alloc::alloc::dealloc(buf.data, layout); }
        buf.data = core::ptr::null_mut();
    }
}

pub fn map_buffer(buf: &mut BxBuffer) -> Result<*mut u8, BxError> {
    if buf.data.is_null() {
        return Err(BxError::BadHandle);
    }
    Ok(buf.data)
}

// ── BxTexture ─────────────────────────────────────────────────────────

pub struct BxTexture {
    pub width: u32,
    pub height: u32,
    pub depth_or_array: u32,
    pub format: u32,
    pixels: *mut u8, // BGRA pixel data
    pitch: u32,
}

impl BxTexture {
    pub fn pixel_count(&self) -> u64 { self.width as u64 * self.height as u64 * self.depth_or_array as u64 }
}

pub fn create_texture(width: u32, height: u32, format: u32) -> Result<BxTexture, BxError> {
    if width == 0 || height == 0 {
        return Err(BxError::InvalidArgument);
    }
    let pitch = width * 4; // BGRA = 4 bytes per pixel
    let size = pitch * height;
    let layout = core::alloc::Layout::from_size_align(size as usize, 16)
        .map_err(|_| BxError::InvalidArgument)?;
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() {
        return Err(BxError::OutOfMemory);
    }
    unsafe { core::ptr::write_bytes(ptr, 0, size as usize); }

    Ok(BxTexture {
        width, height,
        depth_or_array: 1,
        format,
        pixels: ptr,
        pitch,
    })
}

pub fn destroy_texture(tex: &mut BxTexture) {
    if !tex.pixels.is_null() {
        let size = tex.pitch * tex.height;
        let layout = core::alloc::Layout::from_size_align(size as usize, 16).unwrap();
        unsafe { alloc::alloc::dealloc(tex.pixels, layout); }
        tex.pixels = core::ptr::null_mut();
    }
}

pub fn texture_pixels(tex: &BxTexture) -> Option<&[u8]> {
    if tex.pixels.is_null() { return None; }
    let len = (tex.pitch * tex.height) as usize;
    unsafe { Some(core::slice::from_raw_parts(tex.pixels, len)) }
}

pub fn texture_pixels_mut(tex: &mut BxTexture) -> Option<&mut [u8]> {
    if tex.pixels.is_null() { return None; }
    let len = (tex.pitch * tex.height) as usize;
    unsafe { Some(core::slice::from_raw_parts_mut(tex.pixels, len)) }
}

// ── BxPSO (Pipeline State Object) ────────────────────────────────────

pub struct BxPso {
    pub blend_enabled: bool,
    pub depth_test: bool,
    pub cull_mode: u8, // 0=none, 1=back, 2=front
}

impl BxPso {
    pub fn new() -> Self {
        Self { blend_enabled: false, depth_test: false, cull_mode: 0 }
    }
}

pub fn create_pso(_desc: u64) -> Result<BxPso, BxError> {
    Ok(BxPso::new())
}

// ── BxRootSig (Root Signature) ────────────────────────────────────────

pub struct BxRootSig {
    pub param_count: u32,
}

impl BxRootSig {
    pub fn empty() -> Self { Self { param_count: 0 } }
}

pub fn create_root_sig(param_count: u32) -> Result<BxRootSig, BxError> {
    Ok(BxRootSig { param_count })
}

// ── BxFence ───────────────────────────────────────────────────────────

pub struct BxFence {
    pub value: u64,
    signaled: bool,
}

impl BxFence {
    pub fn new(initial: u64) -> Self { Self { value: initial, signaled: true } }
    pub fn is_signaled(&self) -> bool { self.signaled }
}

pub fn create_fence(initial: u64) -> Result<BxFence, BxError> {
    Ok(BxFence::new(initial))
}

pub fn signal_fence(fence: &mut BxFence, value: u64) {
    fence.value = value;
    fence.signaled = true;
}

pub fn wait_fence(fence: &BxFence) {
    // In software renderer, fences are immediately signaled
    let _ = fence;
}

// ── BxHeap ────────────────────────────────────────────────────────────

pub struct BxHeap {
    pub capacity: u64,
    used: u64,
}

impl BxHeap {
    pub fn available(&self) -> u64 { self.capacity - self.used }
}

pub fn create_heap(capacity: u64) -> Result<BxHeap, BxError> {
    Ok(BxHeap { capacity, used: 0 })
}

pub fn heap_alloc(heap: &mut BxHeap, size: u64, _align: u64) -> Result<u64, BxError> {
    if size > heap.available() {
        return Err(BxError::OutOfMemory);
    }
    let offset = heap.used;
    heap.used += size;
    Ok(offset)
}

// ── BxSampler ─────────────────────────────────────────────────────────

pub struct BxSampler {
    pub filter: u32,  // 0=point, 1=linear, 2=anisotropic
    pub wrap: u32,    // 0=clamp, 1=repeat, 2=mirror
}

pub fn create_sampler(filter: u32, wrap: u32) -> Result<BxSampler, BxError> {
    Ok(BxSampler { filter, wrap })
}

// ── BxQueryHeap ───────────────────────────────────────────────────────

pub struct BxQueryHeap {
    pub count: u32,
    results: [u64; 256], // Max 256 queries, no alloc
}

pub fn create_query_heap(count: u32) -> Result<BxQueryHeap, BxError> {
    Ok(BxQueryHeap {
        count,
        results: [0; 256],
    })
}

pub fn resolve_query(_heap: &mut BxQueryHeap, _index: u32) -> Result<u64, BxError> {
    Ok(0) // Software renderer has no GPU timing
}

// ── Raw framebuffer primitives ────────────────────────────────────────

fn fill_rect_raw(x: u32, y: u32, w: u32, h: u32, color: u32) {
    unsafe {
        let fb = FB_BASE as *mut u32;
        let pitch = FB_PITCH / 4; // pixels per row
        let sw = FB_WIDTH;
        let sh = FB_HEIGHT;

        let x0 = x.min(sw);
        let y0 = y.min(sh);
        let x1 = (x + w).min(sw);
        let y1 = (y + h).min(sh);

        for row in y0..y1 {
            for col in x0..x1 {
                let offset = row * pitch + col;
                core::ptr::write_volatile(fb.add(offset as usize), color);
            }
        }
    }
}

fn draw_line_raw(x0: u32, y0: u32, x1: u32, y1: u32, color: u32, line_width: u32) {
    // Bresenham's line algorithm
    let dx = if x1 > x0 { x1 - x0 } else { x0 - x1 };
    let dy = if y1 > y0 { y1 - y0 } else { y0 - y1 };
    let sx = if x0 < x1 { 1u32 } else { u32::MAX }; // -1 as u32
    let sy = if y0 < y1 { 1u32 } else { u32::MAX };
    let mut err = if dx > dy { dx } else { dy };

    let mut cx = x0;
    let mut cy = y0;

    let hw = line_width / 2;

    loop {
        // Draw thick point
        fill_rect_raw(cx.saturating_sub(hw), cy.saturating_sub(hw), line_width, line_width, color);

        if cx == x1 && cy == y1 { break; }

        let e2 = 2 * err;
        if e2 > dy {
            err = err.wrapping_sub(dy);
            cx = cx.wrapping_add(sx);
        }
        if e2 < dx {
            err = err.wrapping_add(dx);
            cy = cy.wrapping_add(sy);
        }
    }
}
