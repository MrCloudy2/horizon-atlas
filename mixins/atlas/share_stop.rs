// The server stops: the map share writes everything and closes.
fn atlas_share_stop() {
    crate::game::atlas_bridge::share_server_stop();
}
