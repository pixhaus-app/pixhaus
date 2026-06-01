//! WCAG 2.x relative-luminance contrast ratio. Pure; used by the theme test to
//! enforce the accessibility floor (text on its surface >= 4.5, large/structural
//! >= 3.0). Order-independent: returns the same ratio whichever color is lighter.

use egui::Color32;

/// WCAG 2.x contrast ratio between two colors, `(Llight + 0.05) / (Ldark + 0.05)`.
/// Both colors are treated as opaque (theme tokens always are). Range 1.0..=21.0.
pub fn wcag_contrast(fg: Color32, bg: Color32) -> f32 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// sRGB relative luminance per WCAG 2.x (0.0 black .. 1.0 white).
fn relative_luminance(c: Color32) -> f32 {
    let r = linearize(f32::from(c.r()) / 255.0);
    let g = linearize(f32::from(c.g()) / 255.0);
    let b = linearize(f32::from(c.b()) / 255.0);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Inverse sRGB companding for one channel.
fn linearize(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod wcag_contrast {
        use super::*;

        #[test]
        fn black_on_white_is_max_ratio() {
            let r = wcag_contrast(Color32::BLACK, Color32::WHITE);
            assert!((r - 21.0).abs() < 0.05, "expected ~21.0, got {r}");
        }

        #[test]
        fn identical_colors_are_one() {
            let r = wcag_contrast(Color32::WHITE, Color32::WHITE);
            assert!((r - 1.0).abs() < 0.001, "expected 1.0, got {r}");
        }

        #[test]
        fn is_order_independent() {
            let a = wcag_contrast(Color32::BLACK, Color32::WHITE);
            let b = wcag_contrast(Color32::WHITE, Color32::BLACK);
            assert!((a - b).abs() < 0.001, "ratio must not depend on argument order");
        }

        #[test]
        fn mid_gray_on_white_matches_reference() {
            // #777777 on #ffffff is a well-known WCAG reference at ~4.48:1.
            let gray = Color32::from_rgb(0x77, 0x77, 0x77);
            let r = wcag_contrast(gray, Color32::WHITE);
            assert!((r - 4.48).abs() < 0.1, "expected ~4.48, got {r}");
        }
    }
}
