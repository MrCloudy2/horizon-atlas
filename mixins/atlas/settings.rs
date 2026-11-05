// The mod's settings page: one button per setting (`/atlas` sets exact values).
fn atlas_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::atlas_mod as m;
    if mod_id != m::ID {
        return;
    }
    let g = &*game;
    let st = m::with(|s| s.settings_with(|k| g.native_setting(m::ID, k)));
    buttons.extend(st.labels());
}
