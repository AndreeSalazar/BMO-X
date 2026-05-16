#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryHint {
    /// VRAM exclusivo (heap default DX12).
    DeviceLocal,
    /// CPU-write, GPU-read (heap upload).
    Upload,
    /// CPU-read, GPU-write (heap readback).
    Readback,
    /// VRAM con mapping CPU vía ReBAR (Agility GPU Upload Heap).
    DeviceLocalUploadable,
}
