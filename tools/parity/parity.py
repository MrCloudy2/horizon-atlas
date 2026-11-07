#!/usr/bin/env python3
"""Pixel parity between Xaero's maps and Atlas, over a scene of known blocks.

  parity.py scene    --centre X Z --datapack DIR
      The scene as a Java datapack: copy DIR into the Java world's
      datapacks/, then /reload and /function parity:build (it builds the
      scene and puts you in the middle, looking down).
  parity.py capture  --seed S --centre X Z --view map|mini --out PNG
      Builds the scene in a Horizon test world of that seed, stands in the
      middle, and screenshots Atlas's world map or minimap.
  parity.py compare  XAERO.png ATLAS.png --centre X Z [--world SAVE] [--out DIR]
      Finds the block lattice in both shots, lines them up, and reports each
      patch: what the world holds there, Xaero's colour, Atlas's, and how far
      apart (and where inside the patch they differ).

Both shots must be of the same scene; zoom and window size may differ.
"""
import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import numpy as np  # noqa: E402
import align  # noqa: E402
import grid  # noqa: E402
import scene  # noqa: E402
import world  # noqa: E402

ROOT = HERE.parents[1]
CTL = ROOT / 'tools' / 'ctl.sh'
SAVES = ROOT / 'target' / 'home' / 'saves'


def ctl(*args, check=True):
    r = subprocess.run([str(CTL), *map(str, args)], capture_output=True, text=True)
    if check and r.returncode != 0:
        raise SystemExit(f'ctl {" ".join(map(str, args))}: {r.stdout}{r.stderr}')
    return r.stdout.strip()


def cmd_scene(a):
    root = scene.datapack(a.datapack, a.centre[0], a.centre[1])
    print(f'Wrote {root}. In Java: copy it into <world>/datapacks/, then /reload and '
          f'/function parity:build')


def cmd_java(a):
    """Shots of Xaero's minimap and world map from a running Java game in
    the parity world (the datapack holds the player in the middle)."""
    import shutil
    import javactl
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    time.sleep(a.settle)
    shot = javactl.screenshot(a.instance)
    shutil.copy(shot, out / 'xaero-mini.png')
    javactl.key('zcaron')
    time.sleep(2)
    if a.wheel:
        javactl.wheel(a.wheel)
        time.sleep(1.5)
    shot = javactl.screenshot(a.instance)
    shutil.copy(shot, out / 'xaero-map.png')
    javactl.key('esc')
    print(json.dumps({'mini': str(out / 'xaero-mini.png'), 'map': str(out / 'xaero-map.png')}))


def cmd_flat(a):
    import flat
    out = flat.make(a.template, a.out, a.seed, a.centre[0], a.centre[1])
    print(f'Made {out}')


def cmd_capture(a):
    cx, cz = a.centre
    name = a.world or f'parity-{a.seed}'
    if not a.attach:
        env = dict(**__import__('os').environ, ATLAS_WORLD=name, ATLAS_SEED=str(a.seed))
        subprocess.run([str(ROOT / 'tools' / 'play.sh'), '--width', str(a.size[0]), '--height',
                        str(a.size[1])], env=env, check=True, capture_output=True)
    ctl('wait-ready')
    marker = SAVES / name / 'atlas-parity-built.json'
    import hashlib
    want = {'centre': [cx, cz], 'scene': hashlib.sha256('\n'.join(scene.build(cx, cz)).encode()).hexdigest()}
    ctl('tp', cx + 0.5, scene.FLOOR + 30, cz + 0.5)
    ctl('wait-ready')
    if not marker.exists() or json.loads(marker.read_text()) != want:
        failed = []
        for line in scene.build(cx, cz):
            r = ctl('command', line, check=False)
            if 'error' in r:
                failed.append((line, r))
        for line, r in failed:
            print(f'command failed: {line}: {r}', file=sys.stderr)
        marker.write_text(json.dumps(want))
    ctl('tp', cx + 0.5, scene.FLOOR + 1, cz + 0.5)
    ctl('look', 180, 90)
    ctl('time', 6000)
    for setting in a.atlas:
        ctl('chat', f'/atlas {setting}')
    ctl('wait-ready')
    # Let Atlas scan the rebuilt chunks.
    ctl('wait', 60)
    if a.view == 'map':
        ctl('key', 'm', 'tap')
        ctl('wait', 10)
        # Wheel notches over the middle: each doubles the zoom.
        gui = json.loads(ctl('screen')).get('gui_size') or [a.size[0] / 3, a.size[1] / 3]
        for _ in range(a.zoom):
            ctl('scroll-at', gui[0] / 2, gui[1] / 2, 1)
        ctl('wait', 20)
    ctl('screenshot', Path(a.out).resolve())
    if a.view == 'map':
        ctl('key', 'esc', 'tap')
    ctl('save')
    print(json.dumps({'shot': str(Path(a.out).resolve()), 'world': str(SAVES / name)}))


def blocks(path, rect, cell=None):
    """A shot's block grid: (colours, ok, origin, pitch, x0, y0)."""
    img = grid.load(path)
    rect = rect or (0, 0, img.shape[1], img.shape[0])
    pitch, x0, y0 = grid.find_lattice(img, rect, cell=cell)
    cols, ok, origin = grid.sample(img, pitch, x0, y0, rect)
    return img, cols, ok, origin, pitch, x0, y0


def world_of(index, centre_px, pitch, x0, y0, origin, cx, cz):
    """World (x, z) of block grid index (i, j): the block under the shot's
    centre pixel is the player's, (cx, cz)."""
    ci = int(np.floor((centre_px[0] - x0) / pitch)) - origin[0]
    cj = int(np.floor((centre_px[1] - y0) / pitch)) - origin[1]
    return cx + index[0] - ci, cz + index[1] - cj


def cmd_compare(a):
    cx, cz = a.centre
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    xi, xc, xok, xo, xp, xx0, xy0 = blocks(a.xaero, a.xaero_rect, scene.CELL)
    ai, ac, aok, ao, ap, ax0, ay0 = blocks(a.atlas, a.atlas_rect, scene.CELL)
    print(f'Xaero: {xp:.2f} px a block, grid {xc.shape[1]}x{xc.shape[0]}; '
          f'Atlas: {ap:.2f} px a block, grid {ac.shape[1]}x{ac.shape[0]}')
    # Both maps are centred on the player: where block (cx, cz) is in each
    # grid gives the offset; the search only refines it.
    def centre_index(img, x0, y0, pitch, origin, at):
        at = at or (img.shape[1] / 2, img.shape[0] / 2)
        return (int(np.floor((at[0] - x0) / pitch)) - origin[0], int(np.floor((at[1] - y0) / pitch)) - origin[1])
    xci = centre_index(xi, xx0, xy0, xp, xo, a.xaero_centre)
    aci = centre_index(ai, ax0, ay0, ap, ao, a.atlas_centre)
    guess = (xci[0] - aci[0], xci[1] - aci[1])
    dx, dz, score, n = align.align(xc, xok, ac, aok, search=a.search, around=guess)
    print(f'Aligned: Xaero block (i, j) = Atlas block (i{-dx:+d}, j{-dz:+d}); '
          f'{score:.0%} of {n} blocks alike')
    # World coordinates from Atlas's centre (the player).
    arect = a.atlas_rect or (0, 0, ai.shape[1], ai.shape[0])
    centre = (arect[0] + arect[2] / 2, arect[1] + arect[3] / 2)
    if a.atlas_centre:
        centre = tuple(a.atlas_centre)

    def at_atlas(x, z):
        i0, j0 = world_of((0, 0), centre, ap, ax0, ay0, ao, cx, cz)
        i, j = x - i0, z - j0
        if 0 <= j < ac.shape[0] and 0 <= i < ac.shape[1]:
            return ac[j, i], aok[j, i]
        return None, False

    def at_xaero(x, z):
        i0, j0 = world_of((0, 0), centre, ap, ax0, ay0, ao, cx, cz)
        i, j = x - i0 + dx, z - j0 + dz
        if 0 <= j < xc.shape[0] and 0 <= i < xc.shape[1]:
            return xc[j, i], xok[j, i]
        return None, False

    w = world.World(a.world) if a.world else None
    rows = []
    for row, col, label, what in scene.cells():
        px, pz = scene.origin(row, col, cx, cz)
        xs, as_, diffs, held = [], [], [], set()
        for z in range(pz, pz + scene.PATCH):
            for x in range(px, px + scene.PATCH):
                xv, xk = at_xaero(x, z)
                av, ak = at_atlas(x, z)
                if w:
                    c = w.column(x, z)
                    if c:
                        held.add(f'{c[0].split("[")[0]}@{c[1]}' + (f' d{c[2]}' if c[2] else ''))
                if xv is None or av is None or not (xk and ak):
                    continue
                xs.append(xv)
                as_.append(av)
                diffs.append(np.abs(xv.astype(int) - av).sum())
        if not xs:
            continue
        xm, am = np.median(xs, axis=0).astype(int), np.median(as_, axis=0).astype(int)
        rows.append(dict(patch=label, row=row, col=col, world=sorted(held)[:3],
                         xaero=xm.tolist(), atlas=am.tolist(),
                         ratio=[round(float(b) / max(float(x), 1), 2) for x, b in zip(xm, am)],
                         median_diff=float(np.median(diffs)), worst_diff=float(np.max(diffs))))
    rows.sort(key=lambda r: -r['median_diff'])
    (out / 'patches.json').write_text(json.dumps(rows, indent=1))
    with open(out / 'patches.txt', 'w') as f:
        f.write(f'{"patch":<28} {"xaero":>15} {"atlas":>15} {"atlas/xaero":>18} {"med":>5} {"worst":>5}  world\n')
        for r in rows:
            f.write(f'{r["patch"]:<28} {str(tuple(r["xaero"])):>15} {str(tuple(r["atlas"])):>15} '
                    f'{str(tuple(r["ratio"])):>18} {r["median_diff"]:5.0f} {r["worst_diff"]:5.0f}  '
                    f'{", ".join(r["world"])}\n')
    picture(out / 'blocks.png', cx, cz, at_xaero, at_atlas)
    print((out / 'patches.txt').read_text())
    print(f'Wrote {out}/patches.txt, patches.json and blocks.png')


def picture(path, cx, cz, at_xaero, at_atlas, k=6):
    """Xaero | Atlas | difference over the scene, a block k pixels, with the
    patch grid drawn faintly."""
    from PIL import Image
    n = scene.HALF * 2
    x0, z0 = cx - scene.HALF, cz - scene.HALF
    img = np.zeros((n, n * 3 + 2, 3), dtype=np.uint8)
    for j in range(n):
        for i in range(n):
            xv, _ = at_xaero(x0 + i, z0 + j)
            av, _ = at_atlas(x0 + i, z0 + j)
            if xv is not None:
                img[j, i] = xv
            if av is not None:
                img[j, n + 1 + i] = av
            if xv is not None and av is not None:
                d = min(255, int(np.abs(xv.astype(int) - av).sum()) * 3)
                img[j, 2 * n + 2 + i] = (d, d // 4, 0)
    big = np.asarray(Image.fromarray(img).resize((img.shape[1] * k, n * k), Image.NEAREST)).copy()
    for c in range(0, n + 1, scene.CELL):
        for part in range(3):
            o = part * (n + 1) * k + c * k
            if o < big.shape[1]:
                big[:, o] = big[:, o] // 2 + 60
        big[min(c * k, big.shape[0] - 1), :] = big[min(c * k, big.shape[0] - 1), :] // 2 + 60
    Image.fromarray(big).save(path)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='cmd', required=True)
    s = sub.add_parser('scene')
    s.add_argument('--centre', type=int, nargs=2, default=[0, 0])
    s.add_argument('--datapack', required=True)
    c = sub.add_parser('capture')
    c.add_argument('--seed', type=int, required=True)
    c.add_argument('--centre', type=int, nargs=2, default=[0, 0])
    c.add_argument('--world', help='the test world (made with `flat` for a superflat one)')
    c.add_argument('--view', choices=['map', 'mini'], default='map')
    c.add_argument('--size', type=int, nargs=2, default=[1280, 720])
    c.add_argument('--atlas', action='append', default=[], help='an /atlas setting, e.g. "lock_north true"')
    c.add_argument('--zoom', type=int, default=1, help='wheel notches in on the world map (each doubles)')
    c.add_argument('--attach', action='store_true', help='use the game already running')
    c.add_argument('--out', required=True)
    j = sub.add_parser('java', help="shoot Xaero's maps in a running Java game")
    j.add_argument('--instance', default=str(Path.home() / '.local/share/atlauncher/instances/Main263'))
    j.add_argument('--wheel', type=int, default=0, help='wheel notches on the world map')
    j.add_argument('--settle', type=float, default=8.0)
    j.add_argument('--out', required=True)
    f = sub.add_parser('flat', help='a superflat plains world with the scene datapack')
    f.add_argument('--template', required=True, help="a world whose level.dat to start from")
    f.add_argument('--out', required=True)
    f.add_argument('--seed', type=int, default=1651784574633817667)
    f.add_argument('--centre', type=int, nargs=2, default=[0, 0])
    m = sub.add_parser('compare')
    m.add_argument('xaero')
    m.add_argument('atlas')
    m.add_argument('--centre', type=int, nargs=2, default=[0, 0])
    m.add_argument('--world')
    m.add_argument('--xaero-rect', type=int, nargs=4, help='x y w h of the map in the Xaero shot')
    m.add_argument('--atlas-rect', type=int, nargs=4, help='x y w h of the map in the Atlas shot')
    m.add_argument('--atlas-centre', type=float, nargs=2, help="the player's pixel in the Atlas shot")
    m.add_argument('--xaero-centre', type=float, nargs=2, help="the player's pixel in the Xaero shot")
    m.add_argument('--search', type=int, default=3)
    m.add_argument('--out', default='parity-out')
    a = p.parse_args()
    {'scene': cmd_scene, 'java': cmd_java, 'flat': cmd_flat, 'capture': cmd_capture, 'compare': cmd_compare}[a.cmd](a)


if __name__ == '__main__':
    main()
