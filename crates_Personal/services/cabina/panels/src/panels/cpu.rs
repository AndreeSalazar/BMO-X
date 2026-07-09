use crate::fb::FrameBuffer;
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "CPU", 0xFF00FFAA);
    let mut y = 40u32;
    let c = &s.telemetry.cpu;

    y = H::section(fb, y, "Interrupts", 0xFF00FFAA);
    y = H::kv_u64(fb, y, "Total",  c.interrupts, 0xFFFFFFFF);
    y = H::kv_u64(fb, y, "Timer",  c.timer_ticks, 0xFFCCCCCC);

    y = H::section(fb, y, "Faults", 0xFF00FFAA);
    y = H::kv_u64(fb, y, "Page (#PF)",    c.page_faults, 0xFFFFFF00);
    y = H::kv_u64(fb, y, "General (#GP)", c.general_protection, 0xFFFF8800);
    y = H::kv_u64(fb, y, "NMI",           c.nmi, 0xFFFF4400);
    y = H::kv_u64(fb, y, "Double (#DF)",  c.double_fault, 0xFFFF0000);
    y = H::kv_u64(fb, y, "Invalid (#UD)", c.undefined_opcode, 0xFFFF8800);
    H::kv_u64(fb, y, "Machine (#MC)", c.machine_check, 0xFFFF0000);
}
