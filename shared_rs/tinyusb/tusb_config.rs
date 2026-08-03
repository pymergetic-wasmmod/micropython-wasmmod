//! rewrite of shared/tinyusb/tusb_config.h
// symmetry: done

pub const HW_ENABLE_USBDEV: bool = false;
pub const HW_ENABLE_USB_RUNTIME_DEVICE: bool = false;
pub const HW_USB_CDC: bool = false;
pub const HW_USB_MSC: bool = false;

pub const MANUFACTURER_STRING: &str = "MicroPython";
pub const PRODUCT_FS_STRING: &str = "Board in FS mode";
pub const CDC_INTERFACE_STRING: &str = "Board CDC";
pub const MSC_INQUIRY_VENDOR_STRING: &str = "MicroPy";
pub const MSC_INQUIRY_PRODUCT_STRING: &str = "Mass Storage";
pub const MSC_INQUIRY_REVISION_STRING: &str = "1.00";

pub const CFG_TUD_CDC: u8 = 0;
pub const CFG_TUD_MSC: u8 = 0;
pub const CFG_TUD_ENDPOINT0_SIZE: u8 = 64;
pub const CFG_TUD_MAX_SPEED: u8 = 0;

pub const CDC_RX_BUFSIZE: usize = 256;
pub const CDC_TX_BUFSIZE: usize = 256;
