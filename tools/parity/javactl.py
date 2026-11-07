"""Drive a Minecraft Java window on KDE Wayland: focus it through a KWin
script, check it really is the active window, and only then press keys with
ydotool (which types into whatever has focus)."""
import subprocess
import tempfile
import time
from pathlib import Path

QDBUS = 'qdbus6'
# Linux input key codes.
KEYS = {'f2': 60, 'esc': 1, 'zcaron': 43, 'backslash': 43, 'm': 50, 'f1': 59, 't': 20,
        'enter': 28, 'tab': 15}

SCRIPT = """
const list = workspace.windowList ? workspace.windowList() : workspace.clientList();
let found = null;
for (const w of list) {
    if (w.caption && w.caption.indexOf("Minecraft") === 0) { found = w; }
}
if (found && %(activate)s) { workspace.activeWindow = found; }
const a = workspace.activeWindow;
print("parity-active %(tag)s " + (a ? a.caption : "none"));
"""


def _kwin(activate):
    tag = f'{time.time_ns()}'
    with tempfile.NamedTemporaryFile('w', suffix='.js', delete=False) as f:
        f.write(SCRIPT % {'activate': 'true' if activate else 'false', 'tag': tag})
        path = f.name
    name = f'parity{tag}'
    sid = subprocess.run([QDBUS, 'org.kde.KWin', '/Scripting', 'org.kde.kwin.Scripting.loadScript',
                          path, name], capture_output=True, text=True).stdout.strip()
    subprocess.run([QDBUS, 'org.kde.KWin', f'/Scripting/Script{sid}', 'org.kde.kwin.Script.run'],
                   capture_output=True)
    caption = None
    for _ in range(20):
        time.sleep(0.05)
        log = subprocess.run(['journalctl', '--user', '-n', '50', '--no-pager', '-o', 'cat'],
                             capture_output=True, text=True).stdout
        for line in log.splitlines():
            if f'parity-active {tag} ' in line:
                caption = line.split(f'parity-active {tag} ', 1)[1]
        if caption is not None:
            break
    subprocess.run([QDBUS, 'org.kde.KWin', '/Scripting', 'org.kde.kwin.Scripting.unloadScript', name],
                   capture_output=True)
    Path(path).unlink(missing_ok=True)
    return caption


def focused():
    """Minecraft is the active window."""
    c = _kwin(False)
    return bool(c and c.startswith('Minecraft'))


def key(name, hold=0.05):
    """Focus Minecraft, make sure it has focus, press `name`."""
    for _ in range(3):
        c = _kwin(True)
        time.sleep(0.15)
        if focused():
            code = KEYS[name]
            subprocess.run(['ydotool', 'key', f'{code}:1', f'{code}:0'], capture_output=True)
            time.sleep(hold)
            return
    raise SystemExit(f'Minecraft would not take focus (active: {c}); no key sent')


def screenshot(instance, wait=10.0):
    """F2 in the game; the new file in its screenshots folder."""
    shots = Path(instance) / 'screenshots'
    before = set(shots.glob('*.png'))
    key('f2')
    end = time.time() + wait
    while time.time() < end:
        new = set(shots.glob('*.png')) - before
        if new:
            shot = max(new, key=lambda p: p.stat().st_mtime)
            # Written on another thread: wait until it stops growing.
            size = -1
            while shot.stat().st_size != size:
                size = shot.stat().st_size
                time.sleep(0.5)
            return shot
        time.sleep(0.1)
    raise SystemExit('no screenshot appeared')


def wheel(notches):
    """Turn the wheel `notches` (positive: up, zooming in) over Minecraft."""
    for _ in range(abs(notches)):
        if not focused():
            key_focus_only()
        subprocess.run(['ydotool', 'mousemove', '-w', '--', '0', '1' if notches > 0 else '-1'],
                       capture_output=True)
        time.sleep(0.15)


def key_focus_only():
    _kwin(True)
    time.sleep(0.15)
    if not focused():
        raise SystemExit('Minecraft would not take focus; no wheel sent')
