//! Platform-independent headset logic shared by the desktop app and the web app (via `crates/web`): the state
//! model, what to read after connecting, how UI actions become GAIA writes, and a simulated MOMENTUM 4.

pub mod plan;
pub mod sim;
pub mod state;

pub use plan::{
    Priority, coalesce_key, follow_ups, is_user_change, noise_reads, plan, refresh_requests,
};
pub use state::{Action, EqState, HeadsetState, NoiseMode, PairedHost};
