// A click on the settings page steps that setting, which is kept at once.
fn atlas_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::atlas_mod as m;
    if mod_id != m::ID {
        return;
    }
    // Report an Issue: the game's own "open this link?" screen, back to
    // this page after.
    if m::Settings::is_report(button) {
        let back = game.screen;
        game.confirm_trusted_link(m::ISSUES_URL, back);
        return;
    }
    let g = &*game;
    let mut st = m::with(|s| s.settings_with(|k| g.native_setting(m::ID, k)));
    st.click(button);
    m::with(|s| {
        s.change_settings(st);
        s.take_changed_settings();
    });
    game.native_set_setting(m::ID, m::SETTINGS_FILE, &st.to_text());
}
