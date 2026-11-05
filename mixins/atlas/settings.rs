// The mod's settings page: labels in the world on or off.
fn waypoints_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::waypoints_mod as m;
    if mod_id != m::ID {
        return;
    }
    let stored = game.native_setting(m::ID, "labels");
    let on = m::with(|s| *s.labels.get_or_insert_with(|| m::read_labels(stored)));
    buttons.push(m::labels_label(on));
}
