pub(super) fn coverage(pattern: i32, x: f32, y: f32, variation: f32) -> Result<f32, String> {
    let radius_squared = x * x + y * y;
    let radial = (1.0 - radius_squared).max(0.0);
    let value = match pattern - 1 {
        1 => 1.0,
        2 => {
            if radius_squared <= 1.0 {
                1.0
            } else {
                0.0
            }
        }
        3 => radial,
        4 => radial * radial,
        5 => (-4.0 * radius_squared).exp(),
        6 => (1.0 - radius_squared.sqrt()).max(0.0).powi(4),
        7 => (1.0 - x.abs().max(y.abs())).max(0.0),
        8 => {
            if x.abs() <= 1.0 - variation.clamp(0.0, 0.99) {
                1.0
            } else {
                0.0
            }
        }
        9 => (x + 1.0) * 0.5,
        10 => (x * std::f32::consts::PI).cos() * 0.5 + 0.5,
        11 => radial * radial * if x >= 0.0 { 1.0 } else { 0.0 },
        12 => (1.0 - x.abs()).max(0.0).powi(2),
        13 => {
            if radius_squared <= 1.0 && (x - variation.max(0.05)).powi(2) + y * y > 1.0 {
                1.0
            } else {
                0.0
            }
        }
        14 => {
            if (x.abs() - 0.5).max(0.0).powi(2) + y * y <= 0.25 {
                1.0
            } else {
                0.0
            }
        }
        15 => (1.0 - radius_squared.sqrt()).max(0.0),
        16 => radial.sqrt(),
        17 => ((x + 1.0) * 0.5 + variation).fract(),
        _ => return Err(format!("unknown built-in pattern {pattern}")),
    };
    Ok(value)
}
