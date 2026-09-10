pub mod animation;
pub mod app;
pub mod camera;
pub mod entity_bounds;
pub mod input;
pub mod math;
pub mod mesh;
pub mod model;
pub mod movement_interp;
pub mod props;
pub mod renderer;
pub mod ui;
pub mod world_style;

pub use app::{EngineApp, NetCommand};
pub use camera::Camera;
pub use input::InputState;

/// WASM browser target entry point (see docs/engine.md).
#[cfg(target_arch = "wasm32")]
pub fn wasm_main() {
    // WASM browser bootstrap placeholder — desktop client is the supported target today.
}
