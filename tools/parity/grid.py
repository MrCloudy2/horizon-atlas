"""Turn a map screenshot into one colour per block.

Both Xaero's maps and Atlas draw every block as a flat square, so a screenshot
is a lattice of squares at some pitch (screen pixels per block) and phase.
`find_lattice` measures both from where the colour changes; `sample` then
reads each square's middle, and marks squares that aren't flat (text, icons,
the arrow, a waypoint) as unknown so they never count in a comparison.
"""
import numpy as np


def load(path):
    from PIL import Image
    return np.asarray(Image.open(path).convert('RGB'), dtype=np.int16)


def edges(img, axis):
    """How much the colour changes between neighbouring pixels along `axis`,
    summed over the other axis: peaks at block borders."""
    d = np.abs(np.diff(img, axis=axis)).sum(axis=2)
    # A change is a border or not: a few strong lines (text) mustn't drown
    # the lattice, so count changes instead of summing them.
    return (d > 12).sum(axis=1 - axis).astype(float)


def find_pitch(profile, lo=1.5, hi=64.0):
    """The lattice pitch of a border profile, to a hundredth of a pixel: the
    period whose comb of borders collects the most change."""
    x = np.arange(len(profile))
    pitches = np.arange(lo, hi, 0.01)
    # Phasor: borders at a steady pitch add up, others cancel.
    z = np.exp(2j * np.pi * np.outer(1.0 / pitches, x)) @ (profile - profile.mean())
    score = np.abs(z)
    # Borders every p pixels also line up every p/2, p/3...: of the pitches
    # that score nearly the best, the lattice is the largest.
    good = np.nonzero(score >= score.max() * 0.8)[0]
    k = good[-1]
    # Refine to the local peak there.
    while k + 1 < len(score) and score[k + 1] > score[k]:
        k += 1
    while k > 0 and score[k - 1] > score[k]:
        k -= 1
    pitch, angle = pitches[k], np.angle(z[k])
    # Borders sit between pixel b and b+1 of the diff, at b + 0.5 + 1 = pixel
    # where the new block starts.
    phase = (angle / (2 * np.pi) * pitch) % pitch
    return pitch, (phase + 0.5) % pitch


def find_lattice(img, rect=None, pitch=None, cell=None):
    """(pitch, x0, y0): screen pixels per block and where a block starts.
    `cell`: the scene repeats every `cell` blocks, a stronger lattice to
    measure the pitch by."""
    x, y, w, h = rect or (0, 0, img.shape[1], img.shape[0])
    part = img[y:y + h, x:x + w]
    px = edges(part, 1)
    py = edges(part, 0)
    if cell and not pitch:
        big = (find_pitch(px, 12.0 * cell / 4, 40.0 * cell)[0] + find_pitch(py, 12.0 * cell / 4, 40.0 * cell)[0]) / 2
        pitch = big / cell
    if pitch:
        lo, hi = pitch * 0.97, pitch * 1.03
    else:
        lo, hi = 1.5, 64.0
    p1, ox = find_pitch(px, lo, hi)
    p2, oy = find_pitch(py, lo, hi)
    p = (p1 + p2) / 2
    return p, x + ox, y + oy


def sample(img, pitch, x0, y0, rect=None, flat=10):
    """Each whole block of `rect` as (colours HxWx3, ok HxW, origin): the
    colour at its middle, ok where its inner pixels agree within `flat`."""
    x, y, w, h = rect or (0, 0, img.shape[1], img.shape[0])
    i0 = int(np.ceil((x - x0) / pitch))
    j0 = int(np.ceil((y - y0) / pitch))
    i1 = int(np.floor((x + w - x0) / pitch)) - 1
    j1 = int(np.floor((y + h - y0) / pitch)) - 1
    cols = np.zeros((j1 - j0 + 1, i1 - i0 + 1, 3), dtype=np.int16)
    ok = np.zeros(cols.shape[:2], dtype=bool)
    inset = max(pitch * 0.2, 0.0)
    for j in range(j0, j1 + 1):
        for i in range(i0, i1 + 1):
            ax, ay = x0 + i * pitch, y0 + j * pitch
            l, r = int(np.ceil(ax + inset)), int(np.floor(ax + pitch - inset))
            t, b = int(np.ceil(ay + inset)), int(np.floor(ay + pitch - inset))
            if r <= l:
                l, r = int(ax + pitch / 2), int(ax + pitch / 2) + 1
            if b <= t:
                t, b = int(ay + pitch / 2), int(ay + pitch / 2) + 1
            block = img[t:b, l:r].reshape(-1, 3)
            mid = np.median(block, axis=0)
            cols[j - j0, i - i0] = mid
            ok[j - j0, i - i0] = np.abs(block - mid).max() <= flat
    return cols, ok, (i0, j0)
