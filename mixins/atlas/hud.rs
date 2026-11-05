// The minimap in its corner and the waypoints' labels in the world.
fn atlas_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::atlas_mod as m;
    // Skins only when faces show.
    let icons = m::with(|s| s.settings().icons == 2 || (s.settings().icons == 1 && s.tab_held));
    let skins = if icons {
        game.native_client_profiles()
            .into_iter()
            .map(|p| (p.uuid, p.skin))
            .collect()
    } else {
        Default::default()
    };
    crate::game::atlas_bridge::hud(&*game, batch, corners, &skins);
}
