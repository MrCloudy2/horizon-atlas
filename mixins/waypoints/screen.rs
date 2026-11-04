// Draws the mod's own screen: the list or the create / edit form.
fn waypoints_screen(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    use crate::game::waypoints_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    let here = game.native_player().map(|p| m::Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    });
    let size = [batch.width, batch.height];
    let measure = |t: &str| game.native_text_width(t);
    let (draws, _) = m::with(|s| s.screen(size, screen.mouse, here.as_ref(), &measure));
    for draw in draws {
        match draw {
            m::Draw::Fill(rect, argb) => game.native_fill(batch, rect, argb),
            m::Draw::Text(text, at, rgb, shadow) => game.native_text(batch, &text, at, rgb, shadow),
        }
    }
}
