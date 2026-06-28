use crate::fb::FrameBuffer;
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, _s: &SystemSnapshot) {
    H::header(fb, "GPU", 0xFF00FFFF);
    let mut y = 80u32;
    y = H::line(fb, y, "GPU driver en v1.9+ (RDNA4).", 0xFFFFFF00);
    y = H::line(fb, y, "Estado actual: skeleton.", 0xFFCCCCCC);
    y = H::line(fb, y, "BMO_ABI::GPU se valida con el BEF header", 0xFFCCCCCC);
    H::line(fb, y, "(backend GPU completo no es objetivo v1.8.8).", 0xFF888888);
}
