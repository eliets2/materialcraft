#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod bridge;
pub mod budgets;
pub mod headless;
pub mod rpc;
pub mod security;
pub mod server;
pub mod workspace;

pub use bridge::BridgeClient;
mod filesystem;
mod render_options;
mod rendered;
pub use filesystem::Filesystem;
pub use headless::Headless;
pub use render_options::RenderOptions;
pub use rendered::Rendered;
pub use server::{Backend, MaterialCraftMcp};
pub use workspace::AuthorizedWorkspace;

mod automation_error;
pub use automation_error::AutomationError;
pub(crate) use automation_error::bad;
