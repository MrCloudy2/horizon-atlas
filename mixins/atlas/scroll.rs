// The wheel over the mod's screen: the map zooms round the pointer, the
// list scrolls.
fn atlas_scroll(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    use crate::game::atlas_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::with(|s| s.scroll_by(scroll[1], screen.mouse));
}
