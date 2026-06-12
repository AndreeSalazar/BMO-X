//! Hardware drivers.

pub mod pci;
pub mod serial;
pub mod nvme;
pub mod ahci;
pub mod gop;
// GPU acelerada experimental queda fuera del kernel funcional.
// FastOS usa UEFI GOP/framebuffer como backend estable; los prototipos en
// `drivers/gpu/fastgpu/` permanecen como investigación, pero no se compilan
// ni forman parte del boot path.
// Stack USB para teclado, ratón y headset Redragon (xHCI + HID + UAC2).
pub mod usb;
