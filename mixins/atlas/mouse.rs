// A button down on the mod's screen: a button's action, a drag of the map,
// or (right button) a waypoint where the map is clicked.
fn atlas_mouse(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::atlas_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    let here = crate::game::atlas_bridge::here(game);
    // A mod's screen has no container panel: its rect is the whole window,
    // the size its drawing was laid out for.
    let size = [screen.rect[2], screen.rect[3]];
    let measure = |t: &str| game.native_text_width(t);
    m::with(|s| {
        let (_, widgets) = s.screen(size, screen.mouse, here.as_ref(), &measure);
        s.press(button, screen.mouse, &widgets, here.as_ref());
    });
}
