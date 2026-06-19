#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Rgba8UnormSrgb,
    Bgra8UnormSrgb,
    Rgba16Float,
    Rgba32Float,
    D32Float,
    D24UnormS8Uint,
    Bc7UnormSrgb,
    Astc4x4UnormSrgb,
}
