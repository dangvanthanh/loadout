//! WCAG 2.2 contrast math on sRGB colors (0–1 channels).

/// WCAG AA for body text (1.4.3).
pub const TEXT: f32 = 4.5;
/// WCAG AA for icons, borders and state indicators (1.4.11).
pub const NON_TEXT: f32 = 3.0;

/// Relative luminance of an sRGB color.
pub fn luminance([r, g, b]: [f32; 3]) -> f32 {
    let linear = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// Contrast ratio between two colors, 1–21.
pub fn ratio(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(v: u32) -> [f32; 3] {
        let c = |s: u32| ((v >> s) & 0xff) as f32 / 255.0;
        [c(16), c(8), c(0)]
    }

    #[test]
    fn known_ratios() {
        assert!((ratio(hex(0x000000), hex(0xffffff)) - 21.0).abs() < 0.01);
        assert!((ratio(hex(0xffffff), hex(0xffffff)) - 1.0).abs() < 0.001);
        // Tailwind neutral-500 on white: the light theme's muted text.
        assert!((ratio(hex(0x737373), hex(0xffffff)) - 4.74).abs() < 0.01);
        // #777 is the classic just-fails gray.
        assert!(ratio(hex(0x777777), hex(0xffffff)) < TEXT);
        assert_eq!(
            ratio(hex(0x123456), hex(0xabcdef)),
            ratio(hex(0xabcdef), hex(0x123456))
        );
    }
}
