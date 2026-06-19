//! Capabilities de red declaradas por la app en su `manifest.bef.toml`.
//!
//! Se chequean al cargar el BEF; el sandbox los aplica vía `barex::abi::handle`
//! en cada syscall de `bx_net`.

use crate::bmo_abi::primitives::bx_u32;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct NetCapabilities: bx_u32 {
        /// Conexiones salientes permitidas.
        const OUTBOUND          = 1 << 0;
        /// Conexiones entrantes (servidor).
        const INBOUND           = 1 << 1;
        /// Kernel bypass / acceso directo a la NIC (HFT, gaming).
        const RAW_KERNEL_BYPASS = 1 << 2;
        /// QUIC/HTTP3 explícito (separado de TCP por privilegio).
        const QUIC              = 1 << 3;
        /// Multicast IPv4/IPv6 (mDNS, gaming LAN).
        const MULTICAST         = 1 << 4;
        /// Sockets RAW (ping, traceroute).
        const RAW_SOCKETS       = 1 << 5;
        /// Bind a puertos privilegiados (<1024).
        const PRIVILEGED_PORTS  = 1 << 6;
        /// DNS-over-HTTPS / DNS-over-TLS personalizado (no usar resolver del sistema).
        const CUSTOM_DNS        = 1 << 7;
    }
}
