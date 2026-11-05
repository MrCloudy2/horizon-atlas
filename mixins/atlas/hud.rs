// The name and distance of each visible waypoint of this dimension, where it
// is in the world, seen from the camera the world was drawn from.
fn waypoints_hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    use crate::game::waypoints_mod as m;
    if !game.native_on(m::ID) {
        return;
    }
    let (Some(p), Some((pos, yaw, pitch, fov))) = (game.native_player(), game.locator_camera) else {
        return;
    };
    let game = &*game;
    let camera = m::Camera { pos, yaw, pitch, fov };
    let size = [batch.width, batch.height];
    let measure = |t: &str| game.native_text_width(t);
    let draws = m::with(|s| s.labels(camera, [p.x, p.y, p.z], p.dimension, size, &measure));
    for draw in draws {
        match draw {
            m::Draw::Fill(rect, argb) => game.native_fill(batch, rect, argb),
            m::Draw::Text(text, at, rgb, shadow) => game.native_text(batch, &text, at, rgb, shadow),
        }
    }
}
