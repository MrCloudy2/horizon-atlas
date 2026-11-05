// Keys over the mod's screen are all the mod's: typing, Tab, Enter, Escape,
// arrows, and the mod's own keys.
fn atlas_keys(game: &crate::game::Game<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    use crate::game::atlas_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    let here = crate::game::atlas_bridge::here(game);
    let map = game.native_mod_key(m::ID, m::KEY_MAP, m::DEFAULT_MAP);
    let create = game.native_mod_key(m::ID, m::KEY_CREATE, m::DEFAULT_CREATE);
    let list = game.native_mod_key(m::ID, m::KEY_LIST, m::DEFAULT_LIST);
    let settings = game.native_mod_key(m::ID, m::KEY_SETTINGS, m::DEFAULT_SETTINGS);
    let shift = modifiers & 1 != 0;
    m::with(|s| s.key(key, shift, here.as_ref(), [&map, &create, &list, &settings]));
}
