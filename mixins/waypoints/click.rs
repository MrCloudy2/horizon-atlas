// A click on the settings page: turn the labels in the world on or off.
fn waypoints_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::waypoints_mod as m;
    if mod_id != m::ID || button != 0 {
        return;
    }
    let stored = game.native_setting(m::ID, "labels");
    let on = m::with(|s| {
        let on = !s.labels.unwrap_or_else(|| m::read_labels(stored));
        s.labels = Some(on);
        on
    });
    game.native_set_setting(m::ID, "labels", if on { "true" } else { "false" });
}
