//! Routing de endpoints. Quién toca qué dispositivo.

pub mod endpoint;
pub mod router;

pub use endpoint::{Endpoint, EndpointKind};
pub use router::Router;
