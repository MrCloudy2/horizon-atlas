// Keys over the mod's screen are all the mod's: typing, Tab, Enter, Escape,
// and N / M in the list.
fn waypoints_keys(game: &crate::game::Game<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    use crate::game::waypoints_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    let here = game.native_player().map(|p| m::Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    });
    let list = game.native_mod_key(m::ID, m::KEY_LIST, m::DEFAULT_LIST);
    let create = game.native_mod_key(m::ID, m::KEY_CREATE, m::DEFAULT_CREATE);
    let shift = modifiers & 1 != 0;
    m::with(|s| s.key(key, shift, here.as_ref(), &list, &create));
}
