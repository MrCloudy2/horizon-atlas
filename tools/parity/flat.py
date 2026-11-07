"""A superflat plains test world, made the same for Minecraft Java and Horizon
(both read the same save format): one biome, nothing to clear, and the scene
built by a datapack as soon as a player is in it.

The world is a copy of a template world's level.dat with its terrain, players
and maps left behind, and a flat generator written over its settings.
"""
import gzip
import json
import shutil
import struct
from pathlib import Path

import scene

DATA_FORMAT = 121  # 26.3's data pack format (pack_version.data_major).


# ---- NBT writing, with explicit types -------------------------------------------


class Int(int):
    pass


class Byte(int):
    pass


class Long(int):
    pass


def _name(s):
    b = s.encode()
    return struct.pack('>H', len(b)) + b


def _tag(v):
    """(type id, payload) of a value."""
    if isinstance(v, Byte) or isinstance(v, bool):
        return 1, struct.pack('>b', int(v))
    if isinstance(v, Long):
        return 4, struct.pack('>q', v)
    if isinstance(v, int):
        return 3, struct.pack('>i', v)
    if isinstance(v, str):
        return 8, _name(v)
    if isinstance(v, dict):
        out = b''
        for k, x in v.items():
            t, p = _tag(x)
            out += bytes([t]) + _name(k) + p
        return 10, out + b'\0'
    if isinstance(v, list):
        if not v:
            return 9, b'\0' + struct.pack('>i', 0)
        tags = [_tag(x) for x in v]
        return 9, bytes([tags[0][0]]) + struct.pack('>i', len(v)) + b''.join(p for _, p in tags)
    raise TypeError(type(v))


def write_nbt(path, value):
    _, payload = _tag(value)
    with gzip.open(path, 'wb') as f:
        f.write(b'\x0a' + _name('') + payload)


# ---- The world ------------------------------------------------------------------------


def generator(seed):
    """world_gen_settings.dat: the overworld flat plains, grass on top at
    y 63 (so the scene's floor at 64 stands one block above it)."""
    flat = {
        'type': 'minecraft:flat',
        'settings': {
            'biome': 'minecraft:plains',
            'features': Byte(0),
            'lakes': Byte(0),
            'structure_overrides': [],
            'layers': [
                {'block': 'minecraft:bedrock', 'height': Int(1)},
                {'block': 'minecraft:stone', 'height': Int(123)},
                {'block': 'minecraft:dirt', 'height': Int(3)},
                {'block': 'minecraft:grass_block', 'height': Int(1)},
            ],
        },
    }
    noise = lambda preset, settings: {  # noqa: E731
        'type': 'minecraft:noise',
        'biome_source': {'type': 'minecraft:multi_noise', 'preset': preset},
        'settings': settings,
    }
    return {
        'DataVersion': Int(5023),
        'data': {
            'seed': Long(seed),
            'generate_structures': Byte(0),
            'bonus_chest': Byte(0),
            'dimensions': {
                'minecraft:overworld': {'type': 'minecraft:overworld', 'generator': flat},
                'minecraft:the_nether': {'type': 'minecraft:the_nether',
                                         'generator': noise('minecraft:nether', 'minecraft:nether')},
                'minecraft:the_end': {'type': 'minecraft:the_end',
                                      'generator': {'type': 'minecraft:noise',
                                                    'biome_source': {'type': 'minecraft:the_end'},
                                                    'settings': 'minecraft:end'}},
            },
        },
    }


# 26.3's names (the older camelCase ones no longer parse).
RULES = ['spawn_mobs', 'spawn_monsters', 'advance_time', 'advance_weather', 'spawn_patrols',
         'spawn_wandering_traders', 'spawn_phantoms']


def datapack(folder, cx, cz, daytime=6000):
    """The scene as a datapack that builds itself: once a player is in the
    world it force-loads the area, waits for it, builds, and puts every
    player in the middle looking down, at `daytime` (noon unless told)."""
    root = Path(folder)
    fn = root / 'data' / 'parity' / 'function'
    tags = root / 'data' / 'minecraft' / 'tags' / 'function'
    fn.mkdir(parents=True, exist_ok=True)
    tags.mkdir(parents=True, exist_ok=True)
    (root / 'pack.mcmeta').write_text(json.dumps(
        {'pack': {'description': 'Atlas parity scene', 'min_format': DATA_FORMAT,
                  'max_format': DATA_FORMAT}}, indent=2))
    (tags / 'tick.json').write_text(json.dumps({'values': ['parity:tick']}))
    # No spawning, no daylight or weather cycle. A bad line stops a whole
    # function from loading, so each rule is a function of its own that may
    # fail alone in a version that names it otherwise.
    rules = []
    for i, rule in enumerate(RULES):
        (fn / f'rule{i}.mcfunction').write_text(f'gamerule {rule} false\n')
        rules.append({'id': f'parity:rule{i}', 'required': False})
    (tags / 'load.json').write_text(json.dumps({'values': rules}))
    x0, z0 = cx - scene.HALF, cz - scene.HALF
    x1, z1 = cx + scene.HALF - 1, cz + scene.HALF - 1
    # Every tick: the player held in the middle looking north (nothing can
    # move the view between shots), no mobs but the test's own, one time.
    (fn / 'tick.mcfunction').write_text(
        'execute as @a[tag=!parity] run function parity:start\n'
        f'tp @a[tag=parity] {cx + 0.5} {scene.FLOOR + 1} {cz + 0.5} 180 90\n'
        'kill @e[type=!player,tag=!parity_keep]\n'
        f'time set {daytime}\n')
    (fn / 'start.mcfunction').write_text(
        'tag @s add parity\n'
        'gamemode creative @s\n'
        f'tp @s {cx + 0.5} {scene.FLOOR + 1} {cz + 0.5} 180 90\n'
        f'forceload add {x0} {z0} {x1} {z1}\n'
        'schedule function parity:build 60t\n')
    lines = [c for c in scene.build(cx, cz) if not c.startswith('gamerule')]
    lines += ['weather clear 1000000', 'kill @e[type=!player]',
              f'tp @a {cx + 0.5} {scene.FLOOR + 1} {cz + 0.5} 180 90',
              'say parity scene built']
    (fn / 'build.mcfunction').write_text('\n'.join(lines) + '\n')
    return root


def make(template, out, seed, cx, cz, daytime=6000):
    """World folder `out` from `template`'s level.dat, flat, with the scene."""
    out = Path(out)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    shutil.copy(Path(template) / 'level.dat', out / 'level.dat')
    (out / 'data' / 'minecraft').mkdir(parents=True)
    write_nbt(out / 'data' / 'minecraft' / 'world_gen_settings.dat', generator(seed))
    datapack(out / 'datapacks' / 'parity', cx, cz, daytime)
    return out
