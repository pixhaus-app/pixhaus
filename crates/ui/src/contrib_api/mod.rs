//! The permanent contribution trait surface.
//!
//! These traits and descriptors are the stable contract every module registers
//! through and the shell consumes. All four registry traits ([`Panel`],
//! [`Tool`], [`Workspace`], [`Module`]) are dyn-compatible and stored as
//! `Box<dyn _>` - registries are the textbook heterogeneous-collection case and
//! none sits on the per-pixel hot path, so the vtable hop is free. The
//! `_assert_boxable` guard below fails the build if any of them regresses.

pub mod context;
pub mod ids;
pub mod module;
pub mod panel;
pub mod tool;
pub mod workspace;

pub use context::{ContribCtx, PanelScope};
pub use ids::{ActionId, PanelId, ToolId, WorkspaceId};
pub use module::{ActionDesc, HostRegistrar, MenuGroup, MenuItem, Module};
pub use panel::{Panel, PanelMeta};
pub use tool::{Tool, ToolMeta};
pub use workspace::{StatusItem, Workspace, WorkspaceLayout, WorkspaceMeta};

/// Compile-time dyn-compatibility guard on the actual storage form.
///
/// If any registry trait gains a generic method, a `-> Self`, or a by-value
/// receiver, it stops being dyn-compatible and this block fails to compile -
/// the crate stops building immediately (test plan item 5). Free and permanent.
const _: () = {
    fn _assert_boxable(_: Box<dyn Panel>, _: Box<dyn Tool>, _: Box<dyn Workspace>, _: Box<dyn Module>) {}
};
