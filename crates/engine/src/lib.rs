pub mod app;
pub mod camera;
pub mod input;
pub mod math;
pub mod renderer;
pub mod ui;

pub use app::{EngineApp, NetCommand};
pub use camera::Camera;
pub use input::InputState;

/// WASM browser target entry point (see docs/engine.md).
#[cfg(target_arch = "wasm32")]
pub fn wasm_main() {
    // Browser client bootstrap will mount the canvas and call EngineApp::run.
}
