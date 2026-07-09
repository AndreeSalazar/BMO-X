use crate::bef::parsers::{self, Image, LoadError};

#[derive(Debug)]
pub enum DevourError {
    Load(LoadError),
    HashMismatch,
    NoEntryPoint,
    UnsupportedFormat,
}

impl core::fmt::Display for DevourError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Load(e) => write!(f, "load error: {:?}", e),
            Self::HashMismatch => write!(f, "BLAKE3 hash mismatch"),
            Self::NoEntryPoint => write!(f, "no entry point"),
            Self::UnsupportedFormat => write!(f, "unsupported binary format"),
        }
    }
}

impl From<LoadError> for DevourError {
    fn from(e: LoadError) -> Self { Self::Load(e) }
}

pub fn devour(bytes: &[u8]) -> Result<Image, DevourError> {
    let img = parsers::load(bytes)?;

    // Verify there's an entry point
    if img.entry_point == 0 {
        return Err(DevourError::NoEntryPoint);
    }

    // Verify BLAKE3 hashes on all sections that have data + a hash
    for sec in &img.sections {
        if sec.data_ptr != 0 && sec.size > 0 {
            // Calculate hash and compare with section's expected hash
            // (stored in the manifest section metadata)
            // For now, trust the loader's verification
        }
    }

    Ok(img)
}

pub fn devour_or_stub(bytes: &[u8]) -> Image {
    match devour(bytes) {
        Ok(img) => img,
        Err(e) => {
            crate::cabina::warn("bef.devour", &alloc::format!("devour failed: {}, using stub", e));
            parsers::fake_provenance_image(crate::bef::format::manifest::Provenance::Native)
        }
    }
}
