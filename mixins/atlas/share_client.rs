// A mod message from a player's client: a map share hello.
fn atlas_share_client(game: &crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::atlas_bridge::share_from_client(game, player, channel, data);
}
