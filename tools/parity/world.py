"""What is really in a saved world: the top blocks of columns, read straight
from its Anvil region files (no game needed), to check what a map shows."""
import struct
import zlib
import gzip
from pathlib import Path


def nbt(b, i, t):
    """Tag of type `t` at `i` of `b`: (value, next index)."""
    if t == 1:
        return b[i], i + 1
    if t == 2:
        return struct.unpack_from('>h', b, i)[0], i + 2
    if t == 3:
        return struct.unpack_from('>i', b, i)[0], i + 4
    if t == 4:
        return struct.unpack_from('>q', b, i)[0], i + 8
    if t == 5:
        return struct.unpack_from('>f', b, i)[0], i + 4
    if t == 6:
        return struct.unpack_from('>d', b, i)[0], i + 8
    if t == 7:
        n = struct.unpack_from('>i', b, i)[0]
        return b[i + 4:i + 4 + n], i + 4 + n
    if t == 8:
        n = struct.unpack_from('>H', b, i)[0]
        return b[i + 2:i + 2 + n].decode('utf8', 'replace'), i + 2 + n
    if t == 9:
        et = b[i]
        n = struct.unpack_from('>i', b, i + 1)[0]
        i += 5
        out = []
        for _ in range(max(n, 0)):
            v, i = nbt(b, i, et)
            out.append(v)
        return out, i
    if t == 10:
        d = {}
        while True:
            tt = b[i]
            i += 1
            if tt == 0:
                return d, i
            n = struct.unpack_from('>H', b, i)[0]
            k = b[i + 2:i + 2 + n].decode('utf8', 'replace')
            d[k], i = nbt(b, i + 2 + n, tt)
    if t == 11:
        n = struct.unpack_from('>i', b, i)[0]
        return list(struct.unpack_from(f'>{n}i', b, i + 4)), i + 4 + 4 * n
    if t == 12:
        n = struct.unpack_from('>i', b, i)[0]
        return list(struct.unpack_from(f'>{n}q', b, i + 4)), i + 4 + 8 * n
    raise ValueError(f'tag {t}')


def root(b):
    n = struct.unpack_from('>H', b, 1)[0]
    return nbt(b, 3 + n, 10)[0]


def seed(world):
    """The seed of a saved world."""
    p = Path(world) / 'data/minecraft/world_gen_settings.dat'
    d = root(gzip.open(p).read())
    return d.get('data', d).get('seed')


# Blocks a map looks through, as Minecraft's map colour NONE does (and air).
SEE_THROUGH = {'air', 'cave_air', 'void_air', 'glass', 'glass_pane', 'barrier', 'light',
               'short_grass', 'tall_grass', 'fern', 'large_fern'}


def name(p):
    """A palette entry as `name[props]`: a compound, or already a string."""
    if isinstance(p, dict) and '' in p:
        # A mixed list wraps its odd ones out this way.
        p = p['']
    if isinstance(p, str):
        return p.removeprefix('minecraft:')
    # Minecraft writes Name/Properties; Horizon id/properties.
    props = p.get('Properties') or p.get('properties')
    tail = '[' + ','.join(f'{a}={v}' for a, v in sorted(props.items())) + ']' if props else ''
    return (p.get('Name') or p['id']).removeprefix('minecraft:') + tail


class World:
    def __init__(self, folder, dimension='minecraft/overworld'):
        self.dir = Path(folder) / 'dimensions' / dimension / 'region'
        if not self.dir.exists():
            self.dir = Path(folder) / 'region'
        self.chunks = {}
        self.biomes = {}

    def chunk(self, cx, cz):
        key = (cx, cz)
        if key in self.chunks:
            return self.chunks[key]
        self.chunks[key] = None
        path = self.dir / f'r.{cx >> 5}.{cz >> 5}.mca'
        if not path.exists():
            return None
        with open(path, 'rb') as f:
            data = f.read()
        k = 4 * ((cx & 31) + (cz & 31) * 32)
        loc = int.from_bytes(data[k:k + 3], 'big')
        if loc == 0:
            return None
        o = loc * 4096
        n = struct.unpack_from('>i', data, o)[0]
        kind = data[o + 4]
        raw = data[o + 5:o + 4 + n]
        raw = zlib.decompress(raw) if kind == 2 else gzip.decompress(raw) if kind == 1 else raw
        d = root(raw)
        sections = {}
        biomes = {}
        for s in d.get('sections', []):
            bi = s.get('biomes')
            if bi:
                biomes[s['Y']] = ([name(p) for p in bi['palette']], bi.get('data'))
            bs = s.get('block_states')
            if not bs:
                continue
            pal = [name(p) for p in bs['palette']]
            sections[s['Y']] = (pal, bs.get('data'))
        self.chunks[key] = sections
        self.biomes[key] = biomes
        return sections

    def raw_biomes(self, cx, cz):
        self.chunk(cx, cz)
        return self.biomes.get((cx, cz))

    def block(self, x, y, z):
        """The block at (x, y, z) as `name[props]`, or None if not saved."""
        sections = self.chunk(x >> 4, z >> 4)
        if sections is None:
            return None
        s = sections.get(y >> 4)
        if s is None:
            return 'air'
        pal, data = s
        if len(pal) == 1 or not data:
            return pal[0]
        bits = max(4, (len(pal) - 1).bit_length())
        per = 64 // bits
        idx = ((y & 15) * 16 + (z & 15)) * 16 + (x & 15)
        word = data[idx // per] & (2**64 - 1)
        return pal[(word >> ((idx % per) * bits)) & ((1 << bits) - 1)]

    def column(self, x, z, top=320, bottom=-64):
        """What a map sees at (x, z): (top block, its y, water depth, block
        under the water). None where the chunk was never saved."""
        if self.chunk(x >> 4, z >> 4) is None:
            return None
        y = top
        while y > bottom:
            b = self.block(x, y, z)
            name = b.split('[')[0]
            if name not in SEE_THROUGH:
                break
            y -= 1
        else:
            return ('void', bottom, 0, None)
        depth = 0
        if name in ('water', 'bubble_column') or 'waterlogged=true' in b and name in ('seagrass', 'kelp', 'kelp_plant'):
            yy = y
            while yy > bottom:
                n = self.block(x, yy, z).split('[')[0]
                if n not in ('water', 'bubble_column', 'seagrass', 'tall_seagrass', 'kelp', 'kelp_plant'):
                    break
                depth += 1
                yy -= 1
            return (b, y, depth, self.block(x, yy, z))
        return (b, y, 0, None)


def biomes_of(world, cx, cz, y=64):
    """{biome: count} of the 4x4x4 biome cells at height y in chunk (cx, cz)."""
    sections = world.raw_biomes(cx, cz)
    out = {}
    if not sections or (y >> 4) not in sections:
        return out
    pal, data = sections[y >> 4]
    bits = max(1, (len(pal) - 1).bit_length()) if len(pal) > 1 else 0
    for k in range(16):
        if bits == 0:
            b = pal[0]
        else:
            per = 64 // bits
            idx = ((y & 15) >> 2) * 16 + k
            word = data[idx // per] & (2**64 - 1)
            b = pal[(word >> ((idx % per) * bits)) & ((1 << bits) - 1)]
        out[b] = out.get(b, 0) + 1
    return out
