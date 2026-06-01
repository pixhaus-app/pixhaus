//! Type-state builder for sprite-sheet export config.
//!
//! `format` and `output_path` are required; `columns` defaults to 1. The
//! type-state pattern moves the "did you set the required fields" check from
//! run time to compile time: `build()` exists only on the fully-configured
//! builder, so forgetting either field is a type error, not a panic.

use std::marker::PhantomData;
use std::path::PathBuf;

/// Output encoding for a sprite-sheet export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Aseprite,
}

/// A fully-specified sprite-sheet export configuration.
///
/// Built only through [`ExportConfigBuilder`]; the required fields are
/// guaranteed set by construction, so there are no `Option`s to unwrap here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportConfig {
    pub format: ExportFormat,
    pub output_path: PathBuf,
    pub columns: u32,
}

// Type-state markers. Each required field is tracked by a zero-sized type that
// flips from `Missing` to `Set` once the field is provided. `build()` is
// implemented only for the all-`Set` combination.

/// Marker: a required field has not been provided yet.
#[derive(Debug)]
pub struct Missing;

/// Marker: a required field has been provided.
#[derive(Debug)]
pub struct Set;

/// Builder for [`ExportConfig`].
///
/// The two type parameters track which required fields are set. Start with
/// [`ExportConfigBuilder::new`], call [`format`](Self::format) and
/// [`output_path`](Self::output_path) in any order, then [`build`](Self::build).
/// `build` is reachable only once both are `Set`.
#[derive(Debug)]
pub struct ExportConfigBuilder<FormatState, PathState> {
    format: Option<ExportFormat>,
    output_path: Option<PathBuf>,
    columns: u32,
    _marker: PhantomData<(FormatState, PathState)>,
}

impl ExportConfigBuilder<Missing, Missing> {
    /// Start a new builder with no required fields set and `columns` defaulted to 1.
    #[must_use]
    pub fn new() -> Self {
        Self {
            format: None,
            output_path: None,
            columns: 1,
            _marker: PhantomData,
        }
    }
}

impl Default for ExportConfigBuilder<Missing, Missing> {
    fn default() -> Self {
        Self::new()
    }
}

// Methods available in any state: setting the required fields (which transition
// the type-state) and the optional `columns`.

impl<FormatState, PathState> ExportConfigBuilder<FormatState, PathState> {
    /// Set the output format, marking that requirement satisfied.
    #[must_use]
    pub fn format(self, format: ExportFormat) -> ExportConfigBuilder<Set, PathState> {
        ExportConfigBuilder {
            format: Some(format),
            output_path: self.output_path,
            columns: self.columns,
            _marker: PhantomData,
        }
    }

    /// Set the output path, marking that requirement satisfied.
    #[must_use]
    pub fn output_path(self, output_path: impl Into<PathBuf>) -> ExportConfigBuilder<FormatState, Set> {
        ExportConfigBuilder {
            format: self.format,
            output_path: Some(output_path.into()),
            columns: self.columns,
            _marker: PhantomData,
        }
    }

    /// Set the number of columns. Optional; defaults to 1.
    #[must_use]
    pub fn columns(mut self, columns: u32) -> Self {
        self.columns = columns;
        self
    }
}

// `build` exists only when both required fields are `Set`. Calling it on a
// builder missing either field is a compile error (no such method), not a panic.
impl ExportConfigBuilder<Set, Set> {
    /// Finalize the configuration.
    ///
    /// Reachable only once `format` and `output_path` are both set, so the
    /// `Option` unwraps here cannot fail: the type-state guarantees both are
    /// `Some`. We still avoid `unwrap()` by destructuring with a match, keeping
    /// the no-panic rule honest rather than relying on a comment.
    #[must_use]
    pub fn build(self) -> ExportConfig {
        let format = match self.format {
            Some(format) => format,
            // Unreachable by construction: only `Set` format state reaches here.
            // We map to a default instead of panicking so no panic path exists.
            None => ExportFormat::Png,
        };
        let output_path = match self.output_path {
            Some(path) => path,
            None => PathBuf::new(),
        };
        ExportConfig {
            format,
            output_path,
            columns: self.columns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_required_fields_and_default_columns() {
        let config = ExportConfigBuilder::new()
            .format(ExportFormat::Png)
            .output_path("sheet.png")
            .build();
        assert_eq!(config.format, ExportFormat::Png);
        assert_eq!(config.output_path, PathBuf::from("sheet.png"));
        assert_eq!(config.columns, 1);
    }

    #[test]
    fn fields_can_be_set_in_any_order() {
        let config = ExportConfigBuilder::new()
            .output_path("sheet.ase")
            .columns(8)
            .format(ExportFormat::Aseprite)
            .build();
        assert_eq!(config.format, ExportFormat::Aseprite);
        assert_eq!(config.output_path, PathBuf::from("sheet.ase"));
        assert_eq!(config.columns, 8);
    }

    #[test]
    fn columns_override_is_respected() {
        let config = ExportConfigBuilder::new()
            .format(ExportFormat::Png)
            .output_path("x.png")
            .columns(4)
            .build();
        assert_eq!(config.columns, 4);
    }

    #[test]
    fn default_matches_new() {
        let a = ExportConfigBuilder::default()
            .format(ExportFormat::Png)
            .output_path("x.png")
            .build();
        let b = ExportConfigBuilder::new()
            .format(ExportFormat::Png)
            .output_path("x.png")
            .build();
        assert_eq!(a, b);
    }
}
