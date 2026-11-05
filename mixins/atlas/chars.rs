// Characters typed on the mod's screen go into the focused field.
fn atlas_chars(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    use crate::game::atlas_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::with(|s| s.char_typed(c));
}
