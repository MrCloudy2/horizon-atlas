"""Patch colours from a minimap shot, drawn at `scale` px a block (too small
for grid.py's lattice search): finds where block (0, 0) is, near `guess`, by
matching the patches to a reference's colours whatever their brightness (a
`compare` patches.json, whose `xaero` colours are used), or else by making
them even inside and unlike each other.

    python3 -I tools/parity/mini.py SHOT GX GY [SCALE [REF_PATCHES_JSON]]
"""
import json
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import numpy as np  # noqa: E402
import scene  # noqa: E402
from PIL import Image

def patches(path, guess, scale=1, search=12, ref=None):
    img = np.asarray(Image.open(path).convert('RGB')).astype(float)
    cells = scene.cells()
    best = None
    for oy in range(-search, search + 1):
        for ox in range(-search, search + 1):
            cx, cz = guess[0] + ox, guess[1] + oy
            tot = 0.0
            meds = []
            for row, col, label, what in cells:
                px, pz = scene.origin(row, col, 0, 0)
                x0, y0 = int(cx + (px + 1) * scale), int(cz + (pz + 1) * scale)
                a = img[y0:y0 + 4 * scale, x0:x0 + 4 * scale].reshape(-1, 3)
                if len(a) == 0:
                    tot += 1e9
                    continue
                tot += a.std(axis=0).sum()
                meds.append(np.median(a, axis=0))
            if ref:
                # Like the reference's colours, whatever the brightness.
                lm = np.log(np.maximum(np.array(meds), 1))
                lr = np.log(np.maximum(np.array([ref.get(c[2], [1, 1, 1]) for c in cells], float), 1))
                dlt = lm - lr
                tot = np.abs(dlt - np.median(dlt)).sum() * 100
            else:
                tot -= 2 * np.array(meds).std(axis=0).sum() * len(cells) ** 0.5
            if best is None or tot < best[0]:
                best = (tot, cx, cz)
    _, cx, cz = best
    out = {}
    for row, col, label, what in cells:
        px, pz = scene.origin(row, col, 0, 0)
        x0, y0 = int(cx + (px + 1) * scale), int(cz + (pz + 1) * scale)
        a = img[y0:y0 + 4 * scale, x0:x0 + 4 * scale].reshape(-1, 3)
        out[label] = np.median(a, axis=0).round().astype(int).tolist()
    return (cx, cz), out

if __name__ == '__main__':
    path, gx, gy = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
    scale = int(sys.argv[4]) if len(sys.argv) > 4 else 1
    ref = None
    if len(sys.argv) > 5:
        ref = {r['patch']: r['xaero'] for r in json.load(open(sys.argv[5]))}
    centre, out = patches(path, (gx, gy), scale, ref=ref)
    print(json.dumps({'centre': centre, 'patches': out}))
