// The mod's row in Options > Mods, with its two keys.
fn waypoints_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::waypoints_mod as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: vec![
            [m::KEY_CREATE.into(), "Create waypoint".into(), m::DEFAULT_CREATE.into()],
            [m::KEY_LIST.into(), "Waypoint list".into(), m::DEFAULT_LIST.into()],
        ],
    });
}
