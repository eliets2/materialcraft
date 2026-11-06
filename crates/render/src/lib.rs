//! # materialcraft-render
//!
//! The 3D view, L3: a CPU path from the material's maps to a shaded image of a built-in scene
//! (rounded cube, sphere, plane, …) under a procedural HDR environment. It needs no GPU, so the
//! desktop app, the CLI, the MCP server and the browser render the same pixels.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod ibl;
pub mod preview;

mod displacement;
mod relief;
pub use displacement::Displacement;
pub use relief::Relief;

pub use preview::{Lighting, Maps, Scene, Settings, VIEW_TRIANGLES, builtin_mesh, pan, render, render_cancellable};
