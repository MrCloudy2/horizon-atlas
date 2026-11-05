// A click on the mod's screen: the button under the pointer does its thing.
fn waypoints_mouse(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::waypoints_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    if button != 0 {
        return;
    }
    let here = game.native_player().map(|p| m::Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    });
    // A mod's screen has no container panel: its rect is the whole window,
    // the size its drawing was laid out for.
    let size = [screen.rect[2], screen.rect[3]];
    let measure = |t: &str| game.native_text_width(t);
    m::with(|s| {
        let (_, widgets) = s.screen(size, screen.mouse, here.as_ref(), &measure);
        if let Some(action) = m::hit(&widgets, screen.mouse) {
            s.act(action, here.as_ref());
        }
    });
}
