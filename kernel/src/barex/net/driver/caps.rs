use crate::barex::abi::primitives::{bx_u32, bx_u64};

bitflags::bitflags! {
    /// Capacidades de offload del NIC. Permite al stack saltarse trabajo.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct NicOffloads: bx_u32 {
        const TX_CHECKSUM_IP    = 1 << 0;
        const TX_CHECKSUM_TCP   = 1 << 1;
        const TX_CHECKSUM_UDP   = 1 << 2;
        const RX_CHECKSUM       = 1 << 3;
        const TSO_V4            = 1 << 4; // TCP Segmentation Offload v4
        const TSO_V6            = 1 << 5;
        const LRO               = 1 << 6; // Large Receive Offload
        const RSS               = 1 << 7; // Receive Side Scaling (multi-queue)
        const VLAN_TAGGING      = 1 << 8;
        const SR_IOV            = 1 << 9; // virtualización
        const ZERO_COPY         = 1 << 10;
        const QUIC_OFFLOAD      = 1 << 11; // raro, NICs modernas
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NicCapabilities {
    pub offloads: NicOffloads,
    pub mtu: bx_u32,
    pub line_speed_bps: bx_u64,
    pub n_rx_queues: bx_u32,
    pub n_tx_queues: bx_u32,
}

impl NicCapabilities {
    pub const NONE: Self = Self {
        offloads: NicOffloads::empty(),
        mtu: 1500,
        line_speed_bps: 0,
        n_rx_queues: 1,
        n_tx_queues: 1,
    };
}
