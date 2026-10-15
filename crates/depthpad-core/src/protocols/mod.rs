//! Keyboard protocols: pure parsing and building, no I/O, one module per protocol family. The
//! matching drivers in `platform::drivers` do the I/O.

pub mod analogsense;
pub mod analogsense_tables;
pub mod sonix_ajazz;

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
