// `/atlas ...`: exact settings and the world's seed; the answer goes to chat.
fn atlas_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::atlas_mod as m;
    let Some(args) = command.strip_prefix("atlas").filter(|a| a.is_empty() || a.starts_with(' ')) else {
        return;
    };
    *ran = true;
    let said = m::with(|s| s.command(args));
    game.native_chat(&said);
}
