//! AutoCAD Color Index (ACI) → RGB.
//!
//! Indices 1–9 are the well-known fixed colors. Indices 10–249 form 24 hues
//! (15° apart) × 5 brightness levels, each in a "pure" and a "washed" (half
//! saturation) variant — the standard AutoCAD palette, generated here from HSV
//! rather than stored as a 256-entry table. Indices 250–255 are a gray ramp.

pub type Rgb = [u8; 3];

/// The five value (brightness) levels used for hues 10–249, in the order the
/// palette walks them.
const VALUES: [f64; 5] = [1.0, 0.65, 0.5, 0.3, 0.15];

/// Gray ramp for indices 250–255.
const GRAYS: [Rgb; 6] = [
    [51, 51, 51],
    [91, 91, 91],
    [132, 132, 132],
    [173, 173, 173],
    [214, 214, 214],
    [255, 255, 255],
];

/// Fixed colors for indices 1–9 (index 0 is unused / "by block").
const FIXED: [Rgb; 10] = [
    [0, 0, 0],       // 0 — unused
    [255, 0, 0],     // 1 red
    [255, 255, 0],   // 2 yellow
    [0, 255, 0],     // 3 green
    [0, 255, 255],   // 4 cyan
    [0, 0, 255],     // 5 blue
    [255, 0, 255],   // 6 magenta
    [255, 255, 255], // 7 white/black (usually treated as foreground)
    [128, 128, 128], // 8 dark gray
    [192, 192, 192], // 9 light gray
];

/// Resolve an AutoCAD Color Index (1–255) to an RGB triple.
pub fn aci_rgb(index: u8) -> Rgb {
    match index {
        1..=9 => FIXED[index as usize],
        10..=249 => {
            let i0 = index as usize - 10; // 0..=239
            let hue_index = i0 / 10; // 0..=23
            let shade = i0 % 10; // 0..=9
            let hue = hue_index as f64 * 15.0;
            let value = VALUES[shade / 2];
            let sat = if shade % 2 == 0 { 1.0 } else { 0.5 };
            hsv_to_rgb(hue, sat, value)
        }
        250..=255 => GRAYS[index as usize - 250],
        _ => FIXED[7],
    }
}

/// HSV → RGB. `h` in degrees [0,360), `s`/`v` in [0,1].
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Rgb {
    let c = v * s;
    let hp = (h / 60.0).rem_euclid(6.0);
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    // The canonical AutoCAD palette truncates (e.g. 0.5 → 0x7F = 127), so floor
    // rather than round to reproduce its exact byte values.
    [
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_primaries() {
        assert_eq!(aci_rgb(1), [255, 0, 0]);
        assert_eq!(aci_rgb(3), [0, 255, 0]);
        assert_eq!(aci_rgb(5), [0, 0, 255]);
    }

    #[test]
    fn generated_matches_known_palette_values() {
        // Spot-checks against the canonical AutoCAD palette.
        assert_eq!(aci_rgb(10), [255, 0, 0]); // hue 0, pure, full value
        assert_eq!(aci_rgb(11), [255, 127, 127]); // hue 0, washed, full value
        assert_eq!(aci_rgb(12), [165, 0, 0]); // hue 0, pure, 0.65 value
        assert_eq!(aci_rgb(20), [255, 63, 0]); // hue 15, pure, full value
        assert_eq!(aci_rgb(90), [0, 255, 0]); // hue 120 (green)
        assert_eq!(aci_rgb(91), [127, 255, 127]); // hue 120 washed
    }

    #[test]
    fn gray_ramp_ends_white() {
        assert_eq!(aci_rgb(255), [255, 255, 255]);
        assert_eq!(aci_rgb(250), [51, 51, 51]);
    }
}
