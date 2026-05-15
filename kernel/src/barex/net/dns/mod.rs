//! DNS — **DoH/DoT only**. UDP/53 plano deshabilitado por default
//! (filtrado por ISPs, censurable, sin integridad).

pub mod resolver;
pub mod answer;

pub use resolver::DnsResolver;
pub use answer::DnsAnswer;
