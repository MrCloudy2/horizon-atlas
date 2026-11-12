// A chunk loaded on the server: the map share looks at it soon.
fn atlas_share_chunk(dimension: &str, chunk: [i32; 2]) {
    crate::game::atlas_bridge::share_chunk_loaded(dimension, chunk);
}
