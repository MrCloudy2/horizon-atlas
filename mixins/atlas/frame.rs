// Once a frame: load the waypoints of the world or server being played,
// close the screen when the mod asked to, and forget a screen the game closed.
fn waypoints_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::waypoints_mod as m;
    let key = if game.native_player().is_none() {
        None
    } else if let (None, Some(remote)) = (&game.integrated, &game.remote) {
        Some(format!("server-{}", remote.session.address.trim().to_lowercase()))
    } else if let Some(folder) = game
        .world_icon
        .as_ref()
        .and_then(|icon| icon.path.parent()?.file_name())
    {
        // The single player world's save folder: the window keeps it for the
        // world's icon from the moment its server starts.
        Some(format!("world-{}", folder.to_string_lossy()))
    } else {
        // The icon is taken and forgotten: the world stays the one loaded.
        let current = m::with(|s| s.world.clone()).filter(|w| w.starts_with("world-"));
        let named = game.integrated.as_ref().map(|i| format!("world-{}", i.world_name));
        current.or(named).or_else(|| Some("world".into()))
    };
    let dir = game.native_data_dir(m::ID);
    let stored = game.native_setting(m::ID, "labels");
    let close = m::with(|s| {
        s.load_world(key, dir);
        s.labels.get_or_insert_with(|| m::read_labels(stored));
        s.take_close()
    });
    if close {
        game.native_mod_screen_close(m::ID);
    }
    let ours = game.native_mod_screen() == Some(m::ID);
    m::with(|s| s.sync_open(ours));
}
