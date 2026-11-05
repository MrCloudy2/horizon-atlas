// Draws the mod's own screen: the world map, the list or the form.
fn atlas_screen(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    use crate::game::atlas_mod as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    let here = crate::game::atlas_bridge::here(game);
    let size = [batch.width, batch.height];
    let measure = |t: &str| game.native_text_width(t);
    let (draws, _) = m::with(|s| s.screen(size, screen.mouse, here.as_ref(), &measure));
    crate::game::atlas_bridge::paint(game, batch, draws);
}
