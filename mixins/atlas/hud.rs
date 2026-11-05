// The minimap in its corner and the waypoints' labels in the world.
fn atlas_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    crate::game::atlas_bridge::hud(&*game, batch, corners);
}
