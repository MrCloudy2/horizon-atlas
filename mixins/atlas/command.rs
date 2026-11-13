// `/atlas ...`: exact settings and the world's seed; the answer goes to chat.
fn atlas_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::atlas_mod as m;
    let Some(args) = command.strip_prefix("atlas").filter(|a| a.is_empty() || a.starts_with(' ')) else {
        return;
    };
    *ran = true;
    // The maps show the next dimension's map (the dimension key's job).
    if args.trim() == "dimension" {
        if let Some(here) = crate::game::atlas_bridge::here(game) {
            let said = m::with(|s| s.switch_dimension(&here));
            game.native_chat(&said);
        }
        return;
    }
    let said = m::with(|s| s.command(args));
    for line in said.lines() {
        game.native_chat(line);
    }
}
