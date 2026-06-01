//! Pixhaus animation module: the Animate workspace and the timeline.
//!
//! Registers the Animate workspace, the Clip Properties and AI Animation
//! Assistant dock panels, the Timeline tray panel, and the Frame menu group
//! (architecture bible section 7.3). Animate reuses sprite-edit's shared panels
//! (Layers, Sprites, Frames, Console) by id - it is editing in space over time
//! atop the same sprite-editing core (bible rule 2), so it never re-registers
//! those shared panels, only references them by id.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod animate;

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The animation module. Registers the Animate workspace, its own panels, the
/// AI-animation actions, and the Frame menu group.
pub struct AnimationModule;

impl Module for AnimationModule {
    fn id(&self) -> &'static str {
        "animation"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        animate::register(host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_is_animation() {
        assert_eq!(AnimationModule.id(), "animation");
    }
}
