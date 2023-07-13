#[inline(always)]
pub fn calc_slope(a: f32, b: f32) -> f32 {
    if b.abs() < 0.1 {
        return 1.0;
    }
    if a.abs() < 0.1 {
        return  0.0;
    }
    return a/(b.abs());
}