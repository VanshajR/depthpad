//! Keyboard protocols: pure parsing and building, no I/O, one module per protocol family. The
//! matching drivers in `platform::drivers` do the I/O.

pub mod analog;
pub mod analogsense;
pub mod analogsense_tables;
pub mod halljoy;
pub mod sonix_ajazz;

/// What the driver knows about the opened collection before talking to it.
#[derive(Debug, Clone, Default)]
pub struct UsbInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    /// The USB product string (some brands sell several boards under one USB ID).
    pub product: String,
}

/// The analog protocol a driver name calls for, given the opened collection (a driver can cover
/// several protocol versions).
pub fn analog_protocol(driver: &str, usb: &UsbInfo) -> Option<Box<dyn analog::AnalogProtocol>> {
    analogsense::for_driver(driver, usb.product_id, usb.usage_page).or_else(|| halljoy::for_driver(driver, usb))
}

/// One analog reading from a keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    /// The key, as the model's own key number (see `device::Model::keys`).
    pub position: u8,
    /// How far the key is pressed, in 0.01 mm.
    pub travel: u16,
    /// The key's full stroke, in 0.1 mm.
    pub stroke: u16,
}
