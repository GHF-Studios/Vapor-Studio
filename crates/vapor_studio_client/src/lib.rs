//! Bounded child-process client for the private Vapor Studio service protocol.

mod process;
mod transport;

pub use process::{ClientError, ServiceBootstrap, StudioClient, service_sibling_path};
