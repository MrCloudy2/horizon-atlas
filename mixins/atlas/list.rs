// The mod's row in Options > Mods, with its three keys.
fn atlas_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::atlas_mod as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: vec![
            [m::KEY_MAP.into(), "World map".into(), m::DEFAULT_MAP.into()],
            [m::KEY_CREATE.into(), "New waypoint".into(), m::DEFAULT_CREATE.into()],
            [m::KEY_LIST.into(), "Waypoint list".into(), m::DEFAULT_LIST.into()],
            [m::KEY_SETTINGS.into(), "Settings".into(), m::DEFAULT_SETTINGS.into()],
            [m::KEY_TEMP.into(), "Temporary waypoint".into(), m::DEFAULT_TEMP.into()],
            [m::KEY_ENLARGE.into(), "Enlarge minimap".into(), m::DEFAULT_ENLARGE.into()],
            [m::KEY_ZOOM_IN.into(), "Zoom in".into(), m::DEFAULT_ZOOM_IN.into()],
            [m::KEY_ZOOM_OUT.into(), "Zoom out".into(), m::DEFAULT_ZOOM_OUT.into()],
        ],
    });
}
