//! Descriptores USB estándar (USB 2.0 §9 / USB 3.2 §9).
//!
//! Sin asumir endianness del host: USB es siempre little-endian en el bus.

#![allow(dead_code)]

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorType {
    Device              = 0x01,
    Configuration       = 0x02,
    String              = 0x03,
    Interface           = 0x04,
    Endpoint            = 0x05,
    DeviceQualifier     = 0x06,
    OtherSpeedConfig    = 0x07,
    InterfacePower      = 0x08,
    Otg                 = 0x09,
    Debug               = 0x0A,
    InterfaceAssociation = 0x0B,
    Bos                 = 0x0F,
    DeviceCapability    = 0x10,
    SuperspeedEp        = 0x30,
    HidReport           = 0x22,
    HidPhysical         = 0x23,
    CsInterface         = 0x24,
    CsEndpoint          = 0x25,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DeviceDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_usb: u16,
    pub b_device_class: u8,
    pub b_device_subclass: u8,
    pub b_device_protocol: u8,
    pub b_max_packet_size_0: u8,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub i_manufacturer: u8,
    pub i_product: u8,
    pub i_serial_number: u8,
    pub b_num_configurations: u8,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ConfigDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub w_total_length: u16,
    pub b_num_interfaces: u8,
    pub b_configuration_value: u8,
    pub i_configuration: u8,
    pub bm_attributes: u8,
    pub b_max_power: u8, // x2 mA
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct InterfaceDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_interface_number: u8,
    pub b_alternate_setting: u8,
    pub b_num_endpoints: u8,
    pub b_interface_class: u8,
    pub b_interface_subclass: u8,
    pub b_interface_protocol: u8,
    pub i_interface: u8,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct EndpointDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_endpoint_address: u8,
    pub bm_attributes: u8,
    pub w_max_packet_size: u16,
    pub b_interval: u8,
}

impl EndpointDescriptor {
    #[inline] pub const fn ep_number(&self) -> u8 { self.b_endpoint_address & 0x0F }
    #[inline] pub const fn is_in(&self) -> bool { (self.b_endpoint_address & 0x80) != 0 }
    #[inline] pub const fn transfer_type(&self) -> EpTransferType {
        match self.bm_attributes & 0x03 {
            0 => EpTransferType::Control,
            1 => EpTransferType::Isochronous,
            2 => EpTransferType::Bulk,
            _ => EpTransferType::Interrupt,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpTransferType { Control, Isochronous, Bulk, Interrupt }

/// Setup packet para Control Transfers (USB 2.0 §9.3).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SetupPacket {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}

impl SetupPacket {
    pub const REQ_GET_DESCRIPTOR: u8 = 0x06;
    pub const REQ_SET_CONFIGURATION: u8 = 0x09;
    pub const REQ_SET_INTERFACE: u8 = 0x0B;
    pub const REQ_HID_GET_REPORT: u8 = 0x01;
    pub const REQ_HID_SET_REPORT: u8 = 0x09;
    pub const REQ_HID_SET_IDLE: u8 = 0x0A;
    pub const REQ_HID_SET_PROTOCOL: u8 = 0x0B;
}
