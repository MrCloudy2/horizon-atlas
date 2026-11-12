// After each level's tick on the server: the map share's work, in a budget.
fn atlas_share_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::atlas_bridge::share_level_tick(game, dimension);
}
