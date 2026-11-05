// While playing: the map key (M) opens the world map, the create key (B) the
// new waypoint form where the player stands, the list key (U) the list.
fn atlas_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::atlas_mod as m;
    if !pressed || !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let map = game.native_mod_key(m::ID, m::KEY_MAP, m::DEFAULT_MAP);
    let create = game.native_mod_key(m::ID, m::KEY_CREATE, m::DEFAULT_CREATE);
    let list = game.native_mod_key(m::ID, m::KEY_LIST, m::DEFAULT_LIST);
    if key != map && key != create && key != list {
        return;
    }
    let Some(here) = crate::game::atlas_bridge::here(game) else {
        return;
    };
    m::with(|s| {
        if key == map {
            s.open_map(Some(&here), m::key_char(key));
        } else if key == create {
            s.open_create(&here, m::Back::Game, m::key_char(key));
        } else {
            s.open_list(m::key_char(key));
        }
    });
    if !game.native_mod_screen_open(m::ID) {
        m::with(|s| s.reset());
    }
}
