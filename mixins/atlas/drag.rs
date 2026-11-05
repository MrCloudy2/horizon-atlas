// The pointer moved with a button held: the world map follows it.
fn atlas_drag(game: &crate::game::Game<'_>, button: i32, delta: [f32; 2], canceled: &mut bool) {
    use crate::game::atlas_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    if m::with(|s| s.drag(button, delta)) {
        *canceled = true;
    }
}
