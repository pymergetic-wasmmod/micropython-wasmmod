//! rewrite of shared/tinyusb/mp_usbd_descriptor.c
// symmetry: done

use super::tusb_config::{
    CDC_INTERFACE_STRING, CFG_TUD_CDC, CFG_TUD_ENDPOINT0_SIZE, CFG_TUD_MSC, HW_ENABLE_USBDEV,
    MANUFACTURER_STRING, MSC_INQUIRY_PRODUCT_STRING, MSC_INQUIRY_REVISION_STRING,
    MSC_INQUIRY_VENDOR_STRING, PRODUCT_FS_STRING,
};

pub const BUILTIN_DESC_CFG_LEN: usize = 64;

pub const STR_MANUF: u8 = 1;
pub const STR_PRODUCT: u8 = 2;
pub const STR_SERIAL: u8 = 3;
pub const STR_CDC: u8 = 4;
pub const STR_MSC: u8 = 5;
pub const STR_0: u8 = 0;

#[repr(C, packed)]
pub struct DescDevice {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_usb: u16,
    pub b_device_class: u8,
    pub b_device_sub_class: u8,
    pub b_device_protocol: u8,
    pub b_max_packet_size0: u8,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub i_manufacturer: u8,
    pub i_product: u8,
    pub i_serial_number: u8,
    pub b_num_configurations: u8,
}

pub static BUILTIN_DESC_DEV: DescDevice = DescDevice {
    b_length: core::mem::size_of::<DescDevice>() as u8,
    b_descriptor_type: 1,
    bcd_usb: 0x0200,
    b_device_class: 0xef,
    b_device_sub_class: 0x02,
    b_device_protocol: 0x01,
    b_max_packet_size0: CFG_TUD_ENDPOINT0_SIZE,
    id_vendor: 0,
    id_product: 0,
    bcd_device: 0x0100,
    i_manufacturer: STR_MANUF,
    i_product: STR_PRODUCT,
    i_serial_number: STR_SERIAL,
    b_num_configurations: 1,
};

pub static BUILTIN_DESC_CFG: [u8; BUILTIN_DESC_CFG_LEN] = [0; BUILTIN_DESC_CFG_LEN];

pub fn string_descriptor(index: u8) -> Option<&'static str> {
    if !HW_ENABLE_USBDEV {
        return None;
    }
    match index {
        STR_MANUF => Some(MANUFACTURER_STRING),
        STR_PRODUCT => Some(PRODUCT_FS_STRING),
        STR_CDC if CFG_TUD_CDC != 0 => Some(CDC_INTERFACE_STRING),
        STR_MSC if CFG_TUD_MSC != 0 => Some(MSC_INQUIRY_PRODUCT_STRING),
        _ => None,
    }
}

pub fn msc_inquiry_strings() -> (&'static str, &'static str, &'static str) {
    (
        MSC_INQUIRY_VENDOR_STRING,
        MSC_INQUIRY_PRODUCT_STRING,
        MSC_INQUIRY_REVISION_STRING,
    )
}
