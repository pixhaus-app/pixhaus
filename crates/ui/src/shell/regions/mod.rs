//! The seven window regions. Each exposes `show(host, ui)`; the runtime calls them
//! outer-first, central last (egui panel-ordering contract).

pub mod bottom_tray;
pub mod canvas_stage;
pub mod left_rail;
pub mod right_dock;
pub mod status_bar;
pub mod tool_options;
pub mod top_bar;

pub(crate) mod scope_split;
