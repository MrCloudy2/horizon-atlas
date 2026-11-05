// Characters typed on the mod's screen go into the focused field.
fn waypoints_chars(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    use crate::game::waypoints_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::with(|s| s.char_typed(c));
}
