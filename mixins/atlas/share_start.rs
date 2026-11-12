// The server starts: whether it is a dedicated one (sharing's Auto).
fn atlas_share_start(dedicated: bool) {
    crate::game::atlas_bridge::share_server_start(dedicated);
}
