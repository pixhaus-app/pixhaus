//! Type-state builder for sprite-sheet export config.
//!
//! `format` and `output_path` are required: `build()` exists only once both
//! have been set, so forgetting either is a compile error, not a runtime panic.
//! `columns` is optional and defaults to 1.
//!
//! The required fields are tracked with two `Set`/`Unset` type parameters. The
//! setters are generic over the *other* parameter, so they compose in any order
//! and we don't have to write out every `impl Builder<A, B>` combination. The
//! field values are plain (defaulted) values, not `Option`s — there is nothing
//! to unwrap in `build()`, which keeps us clear of the workspace no-unwrap rule.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// Sprite-sheet output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Aseprite,
}

/// A finished, validated export configuration.
///
/// The only way to get one is through [`ExportConfigBuilder`], which guarantees
/// `format` and `output_path` were both set before `build()` was reachable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportConfig {
    format: Format,
    output_path: PathBuf,
    columns: u32,
}

impl ExportConfig {
    /// Start building. Both required fields are still unset.
    pub fn builder() -> ExportConfigBuilder<Unset, Unset> {
        ExportConfigBuilder::new()
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub fn columns(&self) -> u32 {
        self.columns
    }
}

/// Marker: a required field has been set.
#[derive(Debug)]
pub struct Set;

/// Marker: a required field has not been set yet.
#[derive(Debug)]
pub struct Unset;

/// Builder for [`ExportConfig`].
///
/// The two type parameters track whether `format` and `output_path` have been
/// set. `build()` is implemented only for `ExportConfigBuilder<Set, Set>`, so
/// the compiler rejects any attempt to build before both are supplied.
pub struct ExportConfigBuilder<HasFormat, HasPath> {
    format: Format,
    output_path: PathBuf,
    columns: u32,
    _format: PhantomData<HasFormat>,
    _path: PhantomData<HasPath>,
}

impl ExportConfigBuilder<Unset, Unset> {
    /// A fresh builder with neither required field set.
    ///
    /// The required fields hold harmless placeholders until the setters replace
    /// them — that's what lets `build()` read plain values instead of unwrapping
    /// `Option`s. `columns` defaults to 1, its real default.
    pub fn new() -> Self {
        ExportConfigBuilder {
            format: Format::Png,
            output_path: PathBuf::new(),
            columns: 1,
            _format: PhantomData,
            _path: PhantomData,
        }
    }
}

impl Default for ExportConfigBuilder<Unset, Unset> {
    fn default() -> Self {
        Self::new()
    }
}

// `format` flips only the HasFormat parameter; HasPath is carried through
// unchanged, so the setters compose in any order.
impl<HasPath> ExportConfigBuilder<Unset, HasPath> {
    pub fn format(self, format: Format) -> ExportConfigBuilder<Set, HasPath> {
        ExportConfigBuilder {
            format,
            output_path: self.output_path,
            columns: self.columns,
            _format: PhantomData,
            _path: PhantomData,
        }
    }
}

// `output_path` flips only HasPath.
impl<HasFormat> ExportConfigBuilder<HasFormat, Unset> {
    pub fn output_path(
        self,
        output_path: impl Into<PathBuf>,
    ) -> ExportConfigBuilder<HasFormat, Set> {
        ExportConfigBuilder {
            format: self.format,
            output_path: output_path.into(),
            columns: self.columns,
            _format: PhantomData,
            _path: PhantomData,
        }
    }
}

// `columns` is optional: it moves no type parameter, so it's available in every
// state and required in none.
impl<HasFormat, HasPath> ExportConfigBuilder<HasFormat, HasPath> {
    pub fn columns(mut self, columns: u32) -> Self {
        self.columns = columns;
        self
    }
}

// `build()` exists only once both required fields are `Set`. No unwrap needed:
// the fields are plain values, set by the typed setters above.
impl ExportConfigBuilder<Set, Set> {
    pub fn build(self) -> ExportConfig {
        ExportConfig {
            format: self.format,
            output_path: self.output_path,
            columns: self.columns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_both_required_fields() {
        let cfg = ExportConfig::builder()
            .format(Format::Png)
            .output_path("sheet.png")
            .build();

        assert_eq!(cfg.format(), Format::Png);
        assert_eq!(cfg.output_path(), Path::new("sheet.png"));
        assert_eq!(cfg.columns(), 1); // default
    }

    #[test]
    fn setters_compose_in_any_order() {
        let cfg = ExportConfig::builder()
            .output_path("out.aseprite")
            .format(Format::Aseprite)
            .build();

        assert_eq!(cfg.format(), Format::Aseprite);
        assert_eq!(cfg.output_path(), Path::new("out.aseprite"));
    }

    #[test]
    fn columns_overrides_the_default() {
        let cfg = ExportConfig::builder()
            .format(Format::Png)
            .columns(8)
            .output_path("grid.png")
            .build();

        assert_eq!(cfg.columns(), 8);
    }

    #[test]
    fn columns_is_optional_and_position_independent() {
        let cfg = ExportConfig::builder()
            .columns(4)
            .output_path("a.png")
            .format(Format::Png)
            .build();

        assert_eq!(cfg.columns(), 4);
    }

    // The following must NOT compile — they're the whole point of the type-state
    // design. Kept as documentation; uncomment to confirm the compiler rejects
    // them.
    //
    // fn missing_path() {
    //     // error: no method `build` on ExportConfigBuilder<Set, Unset>
    //     let _ = ExportConfig::builder().format(Format::Png).build();
    // }
    //
    // fn missing_format() {
    //     // error: no method `build` on ExportConfigBuilder<Unset, Set>
    //     let _ = ExportConfig::builder().output_path("x.png").build();
    // }
    //
    // fn nothing_set() {
    //     // error: no method `build` on ExportConfigBuilder<Unset, Unset>
    //     let _ = ExportConfig::builder().build();
    // }
}
