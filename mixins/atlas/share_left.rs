// A player left the server: their map share stops.
fn atlas_share_left(player: u128) {
    crate::game::atlas_bridge::share_player_left(player);
}
