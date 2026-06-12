//! Bridge a drivers físicos. Cada backend concreto en su archivo.

pub mod backend;
pub mod usb_ac2;
pub mod realtek_hda;

pub use backend::Backend;
pub use usb_ac2::UsbAc2Backend;
pub use realtek_hda::RealtekHdaBackend;
