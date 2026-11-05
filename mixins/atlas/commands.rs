// `/atlas` is the mod's own client command: the chat box completes it.
fn atlas_commands(commands: &mut Vec<&'static str>) {
    if !commands.contains(&"atlas") {
        commands.push("atlas");
    }
}
