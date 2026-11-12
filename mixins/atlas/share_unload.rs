// A level closed on the server: the map share writes what changed.
fn atlas_share_unload() {
    crate::game::atlas_bridge::share_level_unload();
}
