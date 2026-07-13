//! UEFI boot chain — 5 layer modules.

pub mod layer0_enter;
pub mod layer1_getmem;
pub mod layer2_getgop;
pub mod layer3_load;
pub mod layer4_exit;
