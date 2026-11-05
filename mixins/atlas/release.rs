// A button up on the mod's screen ends a drag of the map.
fn atlas_release(game: &crate::game::Game<'_>, button: i32, canceled: &mut bool) {
    use crate::game::atlas_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::with(|s| s.release(button));
}
