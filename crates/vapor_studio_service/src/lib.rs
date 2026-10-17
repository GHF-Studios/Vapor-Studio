//! Process-owned application state and the private stdio protocol endpoint.

mod scenario;
mod server;
mod session;
mod simulator;

pub use server::run_stdio_service;
pub use session::{ServiceBootstrap, StudioSession};
pub use simulator::run_simulator;
