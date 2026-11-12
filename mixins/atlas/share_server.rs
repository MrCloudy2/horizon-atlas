// A mod message from the server: the shared map.
fn atlas_share_server(channel: &str, data: &[u8]) {
    crate::game::atlas_bridge::share_from_server(channel, data);
}
