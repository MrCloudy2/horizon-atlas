// While playing: the map key (M) opens the world map, the create key (B) the
// new waypoint form where the player stands, the list key (U) the list, the
// settings key (Y) the settings screen.
fn atlas_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::atlas_mod as m;
    if !game.native_on(m::ID) {
        return;
    }
    if key == "key.keyboard.tab" {
        m::with(|s| s.tab_held = pressed);
        return;
    }
    // The enlarge key is heard going up too, wherever that happens.
    if key == game.native_mod_key(m::ID, m::KEY_ENLARGE, m::DEFAULT_ENLARGE) {
        if !pressed || game.native_playing() {
            m::with(|s| s.enlarge_key(pressed));
        }
        return;
    }
    if !pressed || !game.native_playing() {
        return;
    }
    if key == game.native_mod_key(m::ID, m::KEY_TEMP, m::DEFAULT_TEMP) {
        if let Some(here) = crate::game::atlas_bridge::here(game) {
            m::with(|s| s.add_temporary(&here));
        }
        return;
    }
    if key == game.native_mod_key(m::ID, m::KEY_ZOOM_IN, m::DEFAULT_ZOOM_IN) {
        m::with(|s| s.zoom_key(true));
        return;
    }
    if key == game.native_mod_key(m::ID, m::KEY_ZOOM_OUT, m::DEFAULT_ZOOM_OUT) {
        m::with(|s| s.zoom_key(false));
        return;
    }
    let map = game.native_mod_key(m::ID, m::KEY_MAP, m::DEFAULT_MAP);
    let create = game.native_mod_key(m::ID, m::KEY_CREATE, m::DEFAULT_CREATE);
    let list = game.native_mod_key(m::ID, m::KEY_LIST, m::DEFAULT_LIST);
    let settings = game.native_mod_key(m::ID, m::KEY_SETTINGS, m::DEFAULT_SETTINGS);
    if key != map && key != create && key != list && key != settings {
        return;
    }
    let Some(here) = crate::game::atlas_bridge::here(game) else {
        return;
    };
    m::with(|s| {
        if key == map {
            s.open_map(Some(&here), m::key_char(key));
        } else if key == settings {
            s.open_settings(m::Page::Main);
            s.skip_char = m::key_char(key);
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
