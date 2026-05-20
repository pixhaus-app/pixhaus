//! Local error enum for `pixhaus-vectorize`.

use thiserror::Error;

/// Errors returned by `centerline_vectorize` and its stages.
#[derive(Debug, Error)]
pub enum Error {
    /// Input raster had zero width or zero height.
    #[error("empty input raster")]
    EmptyRaster,
    /// Input palette had no color entries.
    #[error("palette has no entries")]
    EmptyPalette,
    /// Skeleton extraction failed for the stated reason.
    #[error("skeleton extraction failed: {0}")]
    Skeleton(String),
    /// Stroke fitting failed for the stated reason.
    #[error("stroke fitting failed: {0}")]
    StrokeFit(String),
}

/// Crate-local `Result` alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_displays_message() {
        let e = Error::EmptyRaster;
        assert_eq!(format!("{e}"), "empty input raster");
    }

    #[test]
    fn skeleton_error_carries_payload() {
        let e = Error::Skeleton("boom".into());
        assert!(format!("{e}").contains("boom"));
    }
}
