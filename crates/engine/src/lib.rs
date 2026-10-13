//! Headset session engine: GAIA client, state model, discovery-driven connection management.

pub mod client;
mod engine;
pub mod session;
pub mod sim;
pub mod state;
pub mod transport;

pub use client::{Client, ClientError, Unsolicited};
pub use engine::{Engine, EngineOptions};
pub use state::{
    Action, AppState, DeviceEntry, EqState, HeadsetState, LinkState, NoiseMode, PairedHost,
};
pub use transport::{Transport, connect_rfcomm};
