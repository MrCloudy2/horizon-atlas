"""A test scene of known blocks, built with commands around a centre, so a map
of it can be checked block by block against what is certainly there.

The same commands build it in Horizon (sent by the harness) and in Minecraft
Java (as a datapack function), on the same seed at the same place, so Xaero's
maps and Atlas can be shot over the same blocks.

An 80 x 80 floor of smooth stone at FLOOR (cleared of everything above it,
stone under it) holds 10 x 10 cells of 8 blocks: a 6 x 6 patch and a gap.
The player stands in the middle cell.
"""
FLOOR = 64
HALF = 40
CELL = 8
PATCH = 6
FLOOR_BLOCK = 'smooth_stone'

COLOURS = ['grass_block', 'dirt', 'sand', 'gravel', 'stone', 'snow_block', 'ice', 'packed_ice',
           'blue_ice', 'clay']
COLOURS2 = ['oak_leaves[persistent=true]', 'spruce_leaves[persistent=true]',
            'birch_leaves[persistent=true]', 'jungle_leaves[persistent=true]',
            'dark_oak_leaves[persistent=true]', 'podzol', 'coarse_dirt', 'red_sand', 'terracotta',
            'moss_block']
MISC = ['oak_log', 'spruce_log', 'glass', 'short_grass', 'dandelion', 'torch', 'smooth_stone_slab',
        'lava', 'cobblestone', 'oak_planks']


def cells():
    """Every patch: (row, col, label, what) where `what` says how it is built:
    ('top', block) the floor's top replaced; ('pool', depth, bottom) water of
    that depth over `bottom`; ('raise', n, top) a pillar n blocks up with
    `top` on it; ('on', block) a block standing on grass; ('ramp', dx, dz)
    one block up per block towards (dx, dz); ('pair', ...) special edges."""
    out = []
    for c, b in enumerate(COLOURS):
        out.append((0, c, b, ('top', b)))
    for c, b in enumerate(COLOURS2):
        out.append((1, c, b.split('[')[0], ('top', b)))
    for c in range(10):
        out.append((2, c, f'water {c + 1} over sand', ('pool', c + 1, 'sand')))
    for c in range(10):
        out.append((3, c, f'water {c + 1} over dirt', ('pool', c + 1, 'dirt')))
    for c, n in enumerate([1, 2, 3, 4, 5, 8, 12, 16, 24, 32]):
        out.append((4, c, f'grass up {n}', ('raise', n, 'grass_block')))
    # Row 5 is the player's: only its ends hold patches.
    for c, n in [(0, -1), (1, -2), (8, -3), (9, -4)]:
        out.append((5, c, f'grass down {-n}', ('raise', n, 'grass_block')))
    for c in range(8):
        out.append((6, c, f'snow {c + 1} layers', ('on', f'snow[layers={c + 1}]')))
    out.append((6, 8, 'powder_snow', ('top', 'powder_snow')))
    out.append((6, 9, 'snow on ice', ('pair', 'snow_ice')))
    # Edges seen on real maps: ice by higher snow, water by ice, steps.
    out.append((7, 0, 'ice under snow north', ('pair', 'ice_low_north')))
    out.append((7, 1, 'water under snow north', ('pair', 'water_low_north')))
    out.append((7, 2, 'ice next to water', ('pair', 'ice_water')))
    out.append((7, 3, 'grass step down north', ('pair', 'step_north')))
    out.append((7, 4, 'grass step down west', ('pair', 'step_west')))
    out.append((7, 5, 'grass step down south', ('pair', 'step_south')))
    out.append((7, 6, 'grass step down east', ('pair', 'step_east')))
    out.append((7, 7, 'leaves over grass', ('pair', 'leaf_grass')))
    out.append((7, 8, 'sand step 2 north', ('pair', 'step2_north')))
    out.append((7, 9, 'water over ice', ('pair', 'water_ice')))
    for c, (dx, dz) in enumerate([(1, 0), (-1, 0), (0, 1), (0, -1)]):
        name = {(1, 0): 'east', (-1, 0): 'west', (0, 1): 'south', (0, -1): 'north'}[(dx, dz)]
        out.append((8, c, f'grass ramp up {name}', ('ramp', dx, dz, 'grass_block')))
        out.append((8, c + 4, f'sand ramp up {name}', ('ramp', dx, dz, 'sand')))
    for c, b in enumerate(MISC):
        out.append((9, c, b, ('on', b) if b in ('short_grass', 'dandelion', 'torch', 'smooth_stone_slab')
                    else ('top', b)))
    return out


def origin(row, col, cx, cz):
    """The north west block of patch (row, col)."""
    return cx - HALF + col * CELL + 1, cz - HALF + row * CELL + 1


def fills(x0, y0, z0, x1, y1, z1, block, limit=32768):
    """`fill` commands for a box, split under the block limit."""
    xs, zs = sorted((x0, x1)), sorted((z0, z1))
    ys = sorted((y0, y1))
    h = ys[1] - ys[0] + 1
    rows = max(1, limit // (h * (xs[1] - xs[0] + 1)))
    out = []
    z = zs[0]
    while z <= zs[1]:
        z2 = min(zs[1], z + rows - 1)
        out.append(f'fill {xs[0]} {ys[0]} {z} {xs[1]} {ys[1]} {z2} {block}')
        z = z2 + 1
    return out


def build(cx, cz):
    """The commands that build the scene centred on (cx, cz)."""
    x0, z0, x1, z1 = cx - HALF, cz - HALF, cx + HALF - 1, cz + HALF - 1
    cmd = ['gamerule doDaylightCycle false', 'gamerule doWeatherCycle false', 'gamerule doFireTick false',
           'gamerule doMobSpawning false', 'time set 6000', 'weather clear', 'kill @e[type=!player]']
    # Clear the sky over it (high trees and hills too), then the floor.
    cmd += fills(x0, FLOOR + 1, z0, x1, 200, z1, 'air')
    cmd += fills(x0, FLOOR - 14, z0, x1, FLOOR - 1, z1, 'stone')
    cmd += fills(x0, FLOOR, z0, x1, FLOOR, z1, FLOOR_BLOCK)
    for row, col, _, what in cells():
        px, pz = origin(row, col, cx, cz)
        qx, qz = px + PATCH - 1, pz + PATCH - 1
        kind = what[0]
        if kind == 'top':
            cmd += fills(px, FLOOR, pz, qx, FLOOR, qz, what[1])
        elif kind == 'on':
            cmd += fills(px, FLOOR, pz, qx, FLOOR, qz, 'grass_block')
            cmd += fills(px, FLOOR + 1, pz, qx, FLOOR + 1, qz, what[1])
        elif kind == 'pool':
            depth, bottom = what[1], what[2]
            cmd += fills(px, FLOOR - depth, pz, qx, FLOOR - depth, qz, bottom)
            cmd += fills(px, FLOOR - depth + 1, pz, qx, FLOOR, qz, 'water')
        elif kind == 'raise':
            n, top = what[1], what[2]
            if n > 0:
                cmd += fills(px, FLOOR, pz, qx, FLOOR + n - 1, qz, 'stone')
                cmd += fills(px, FLOOR + n, pz, qx, FLOOR + n, qz, top)
            else:
                cmd += fills(px, FLOOR + n + 1, pz, qx, FLOOR, qz, 'air')
                cmd += fills(px, FLOOR + n, pz, qx, FLOOR + n, qz, top)
        elif kind == 'ramp':
            dx, dz, top = what[1], what[2], what[3]
            for k in range(PATCH):
                # Strip k (counted towards the rise) stands k + 1 up.
                if dx:
                    x = px + k if dx > 0 else qx - k
                    box = (x, pz, x, qz)
                else:
                    z = pz + k if dz > 0 else qz - k
                    box = (px, z, qx, z)
                cmd += fills(box[0], FLOOR, box[1], box[2], FLOOR + k, box[3], 'stone')
                cmd += fills(box[0], FLOOR + k + 1, box[1], box[2], FLOOR + k + 1, box[3], top)
        elif kind == 'pair':
            cmd += pair(what[1], px, pz, qx, qz)
    return cmd


def pair(name, px, pz, qx, qz):
    """Patches of two halves: the north (or west) half and the rest."""
    mz, mx = pz + PATCH // 2, px + PATCH // 2
    f = FLOOR
    c = []
    if name == 'ice_low_north':
        # Snow ground one up to the north, ice at the floor south of it.
        c += fills(px, f, pz, qx, f, mz - 1, 'stone') + fills(px, f + 1, pz, qx, f + 1, mz - 1, 'snow_block')
        c += fills(px, f, mz, qx, f, qz, 'ice')
    elif name == 'water_low_north':
        c += fills(px, f, pz, qx, f, mz - 1, 'stone') + fills(px, f + 1, pz, qx, f + 1, mz - 1, 'snow_block')
        c += fills(px, f - 3, mz, qx, f - 3, qz, 'sand') + fills(px, f - 2, mz, qx, f, qz, 'water')
    elif name == 'ice_water':
        c += fills(px, f - 3, pz, qx, f - 3, qz, 'sand') + fills(px, f - 2, pz, qx, f, qz, 'water')
        c += fills(px, f, pz, mx - 1, f, qz, 'ice')
    elif name == 'water_ice':
        c += fills(px, f - 3, pz, qx, f - 3, qz, 'sand') + fills(px, f - 2, pz, qx, f, qz, 'water')
        c += fills(px, f, pz, qx, f, mz - 1, 'ice')
    elif name == 'snow_ice':
        c += fills(px, f, pz, qx, f, qz, 'ice') + fills(px, f + 1, pz, qx, f + 1, qz, 'snow[layers=1]')
    elif name.startswith('step'):
        top = 'sand' if name.startswith('step2') else 'grass_block'
        n = 2 if name.startswith('step2') else 1
        c += fills(px, f, pz, qx, f, qz, top)
        side = name.split('_')[1]
        # The raised half is on `side`'s opposite: the ground steps down
        # going towards `side`.
        half = {'north': (px, mz, qx, qz), 'south': (px, pz, qx, mz - 1),
                'west': (mx, pz, qx, qz), 'east': (px, pz, mx - 1, qz)}[side]
        c += fills(half[0], f, half[1], half[2], f + n - 1, half[3], 'stone')
        c += fills(half[0], f + n, half[1], half[2], f + n, half[3], top)
    elif name == 'leaf_grass':
        c += fills(px, f, pz, qx, f, qz, 'grass_block')
        c += fills(px, f + 1, pz, mx - 1, f + 1, qz, 'oak_leaves[persistent=true]')
    return c


def truth(cx, cz):
    """What should be at the top of each column, by patch: {(x, z): label}."""
    out = {}
    for row, col, label, _ in cells():
        px, pz = origin(row, col, cx, cz)
        for z in range(pz, pz + PATCH):
            for x in range(px, px + PATCH):
                out[(x, z)] = (row, col, label)
    return out


def datapack(folder, cx, cz):
    """A Java datapack holding the scene: /function parity:build."""
    from pathlib import Path
    import json
    root = Path(folder)
    fn = root / 'data/parity/function'
    fn.mkdir(parents=True, exist_ok=True)
    (root / 'pack.mcmeta').write_text(json.dumps(
        {'pack': {'description': 'Atlas parity scene', 'pack_format': 81,
                  'supported_formats': [48, 200]}}, indent=2))
    lines = build(cx, cz) + [f'tp @s {cx + 0.5} {FLOOR + 1} {cz + 0.5} 180 90']
    (fn / 'build.mcfunction').write_text('\n'.join(lines) + '\n')
    return root
