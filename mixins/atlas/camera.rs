// Keeps the camera this frame is drawn from (after any mod posed it), so the
// waypoint labels the HUD draws sit on the same frame and don't jitter.
fn atlas_camera(value: (glam::DVec3, f32, f32)) -> (glam::DVec3, f32, f32) {
    crate::game::atlas_mod::with(|s| s.frame_camera = Some((value.0.to_array(), value.1, value.2)));
    value
}
