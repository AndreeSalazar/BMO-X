//! HID raw — parsing del Report Descriptor (USB HID 1.11) para dispositivos
//! no estándar. Reemplaza Win32 `HidP_*` y `HIDClass.sys`.

pub mod usage_page;
pub mod report_item;

pub use usage_page::HidUsagePage;
pub use report_item::HidReportItem;
