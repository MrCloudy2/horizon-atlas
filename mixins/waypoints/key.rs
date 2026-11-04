// While playing, the create key (N) opens the new waypoint form and the list
// key (M) the waypoint list, as the mod's own screen.
fn waypoints_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::waypoints_mod as m;
    if !pressed || !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let create = game.native_mod_key(m::ID, m::KEY_CREATE, m::DEFAULT_CREATE);
    let list = game.native_mod_key(m::ID, m::KEY_LIST, m::DEFAULT_LIST);
    if key != create && key != list {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let here = m::Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    };
    m::with(|s| {
        if key == create {
            s.open_create(&here, false, m::key_char(key));
        } else {
            s.open_list(m::key_char(key));
        }
    });
    if !game.native_mod_screen_open(m::ID) {
        m::with(|s| s.reset());
    }
}
