//! Depthpad core: supported keyboard models and their analog protocols, input fusion, stick
//! logic, profiles and (on Windows) the drivers, key-event, key-blocking and virtual-controller
//! plumbing.
//!
//! The pure modules (`device`, `protocols`, `fusion`, `sticks`, `profile`, `mapper`, `routes`) have no
//! platform code and are covered by unit tests plus a replay of a real recorded session.

pub mod device;
pub mod fusion;
pub mod keymap;
pub mod mapper;
pub mod profile;
pub mod protocols;
pub mod recording;
pub mod routes;
pub mod sticks;

#[cfg(windows)]
pub mod platform;
#[cfg(windows)]
pub mod engine;
