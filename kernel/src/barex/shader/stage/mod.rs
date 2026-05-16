//! `ShaderStage` — 12 stages soportados (graphics + RT + mesh + work-graph).

use crate::barex::abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex          = 0,
    Pixel           = 1,
    Compute         = 2,
    Mesh            = 3,
    Amplification   = 4,
    RayGen          = 5,
    RayMiss         = 6,
    RayClosestHit   = 7,
    RayAnyHit       = 8,
    RayIntersect    = 9,
    RayCallable     = 10,
    WorkGraphNode   = 11,
}

impl ShaderStage {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    #[inline(always)]
    pub const fn is_raytracing(self) -> bool {
        matches!(
            self,
            Self::RayGen | Self::RayMiss | Self::RayClosestHit
            | Self::RayAnyHit | Self::RayIntersect | Self::RayCallable
        )
    }
}
