pub(super) fn direction(turns: f32) -> (f32, f32) {
    let a = turns * std::f32::consts::TAU;
    (a.cos(), -a.sin())
}
