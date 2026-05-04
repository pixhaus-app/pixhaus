//! Palette file format I/O and the Lospec community palette browser.
//!
//! | Module | Format | Read | Write |
//! |--------|--------|------|-------|
//! | [`gpl`] | GIMP Palette (`.gpl`) | ✓ | ✓ |
//! | [`pal`] | Microsoft RIFF `.pal` | ✓ | ✓ |
//! | [`pal`] | JASC `.pal` | ✓ | ✓ |
//! | [`aco`] | Photoshop Color Swatch (`.aco`) | ✓ | — |
//! | [`hex`] | Lospec `.hex` | ✓ | ✓ |
//! | [`lospec`] | Lospec HTTP API | ✓ | — |

pub mod aco;
pub mod gpl;
pub mod hex;
pub mod lospec;
pub mod pal;
