"""Line up two block grids of the same place: the offset (in blocks) at which
the most blocks agree."""
import numpy as np


def align(a, a_ok, b, b_ok, search=40, around=(0, 0)):
    """(dx, dz, score): block (i, j) of `a` is block (i - dx, j - dz) of `b`.
    The score is the median colour distance over the blocks both saw."""
    best = None
    ha, wa = a_ok.shape
    hb, wb = b_ok.shape
    for dz in range(around[1] - search, around[1] + search + 1):
        for dx in range(around[0] - search, around[0] + search + 1):
            # Overlap in a's indices.
            i0, i1 = max(0, dx), min(wa, wb + dx)
            j0, j1 = max(0, dz), min(ha, hb + dz)
            if i1 - i0 < 8 or j1 - j0 < 8:
                continue
            pa = a[j0:j1, i0:i1]
            pb = b[j0 - dz:j1 - dz, i0 - dx:i1 - dx]
            ok = a_ok[j0:j1, i0:i1] & b_ok[j0 - dz:j1 - dz, i0 - dx:i1 - dx]
            n = ok.sum()
            if n < 64:
                continue
            d = np.abs(pa - pb).sum(axis=2)[ok]
            # Structure, not grade: the share of blocks that look alike.
            score = (d < 60).mean()
            if best is None or score > best[2]:
                best = (dx, dz, float(score), int(n))
    return best
