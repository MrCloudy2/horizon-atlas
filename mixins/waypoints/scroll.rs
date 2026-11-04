// The wheel over the mod's screen scrolls the list.
fn waypoints_scroll(game: &crate::game::Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    use crate::game::waypoints_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::with(|s| s.scroll_by(scroll[1]));
}
