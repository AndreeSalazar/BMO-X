use super::button::HeadsetButton;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HeadsetButtonEvent {
    pub button: HeadsetButton,
    pub pressed: bool,
}
