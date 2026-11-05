// Once a frame: settings, world, chunk scans, map images, screen state.
fn atlas_frame(game: &mut crate::game::Game<'_>) {
    crate::game::atlas_bridge::frame(game);
}
