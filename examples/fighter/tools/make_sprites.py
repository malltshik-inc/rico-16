#!/usr/bin/env python3
"""Generate examples/fighter/assets/fighters.png.

A 256x256 sprite sheet of two fighters, twelve poses each, in 40x60 cells:

    cell (pose p, fighter f) is at x = (p % 6) * 40, y = (f * 2 + p / 6) * 60
    fighter 0 is red, fighter 1 is blue; the art faces right, and the cart
    mirrors it with sprite::draw_region's flip_x when a fighter faces left

The budget is the whole point. SPEC §7 fixes the sheet at 65,536 bytes for *all*
art, so 40x60 cells with twelve poses each is 24 cells = 57,600 bytes, 88% of
it. Bigger characters means fewer frames: 64x96 leaves room for five poses each,
which is not a fighting game. Shading and silhouette cost nothing here — they
are the same pixels in better colours — so that is where the effort goes.

Poses are drawn from a joint skeleton with a shading pass: a dark outline, then
a back limb in shadow, a torso and front limbs with a light edge up-left and a
shadow edge down-right, and a lit head. The palette is read from docs/SPEC.md so
the colours match the normative default palette the cart uses.

    python3 examples/fighter/tools/make_sprites.py            # write the sheet
    python3 examples/fighter/tools/make_sprites.py --preview  # and show poses
"""

import math
import re
import struct
import sys
import zlib
from pathlib import Path

CELL_W, CELL_H = 40, 60
SCALE = 1.25  # the skeleton is authored in a 32x48 cell and scaled on the way in
RADIUS = 2  # limb half-thickness
TORSO_R = 4
HEAD_R = 5
NECK_R = 2

POSES = [
    "idle",
    "breathe",
    "walk0",
    "walk1",
    "crouch",
    "punch_wind",
    "punch",
    "punch_recover",
    "kick_wind",
    "kick",
    "kick_recover",
    "hit",
]

# --- the skeleton, in 32x48 cell coordinates, fighter facing right ---------

BASE = {
    "hip": (16, 29),
    "shoulder": (16, 16),
    "head": (16, 8),
    # The limbs have to sit *outside* the torso or they vanish into it, which is
    # what made the first version read as a blob.
    "elbow_back": (10, 22),
    "hand_back": (8, 27),
    "elbow_front": (22, 22),
    "hand_front": (24, 27),
    "knee_back": (12, 38),
    "foot_back": (9, 46),
    "knee_front": (20, 38),
    "foot_front": (23, 46),
}


def pose(**overrides):
    joints = dict(BASE)
    joints.update(overrides)
    return joints


GUARD = pose(elbow_back=(11, 22), hand_back=(12, 18), elbow_front=(21, 22), hand_front=(21, 18))

POSES_JOINTS = [
    # 0 idle
    GUARD,
    # 1 breathe: shoulders and head settle a pixel
    pose(shoulder=(16, 18), head=(16, 10),
         elbow_back=(11, 23), hand_back=(12, 19), elbow_front=(21, 23), hand_front=(21, 19)),
    # 2 walk0: stride
    pose(knee_back=(11, 38), foot_back=(7, 46), knee_front=(21, 38), foot_front=(26, 46),
         elbow_back=(11, 24), hand_back=(9, 27), elbow_front=(21, 24), hand_front=(24, 21)),
    # 3 walk1: together
    pose(knee_back=(15, 38), foot_back=(14, 46), knee_front=(18, 38), foot_front=(19, 46),
         elbow_back=(12, 24), hand_back=(11, 28), elbow_front=(21, 24), hand_front=(22, 27)),
    # 4 crouch
    pose(hip=(16, 35), shoulder=(16, 24), head=(16, 16),
         knee_back=(11, 41), foot_back=(11, 46), knee_front=(21, 41), foot_front=(21, 46),
         elbow_back=(11, 29), hand_back=(11, 33), elbow_front=(21, 29), hand_front=(21, 33)),
    # 5 punch wind-up: the arm is drawn back, weight shifts off the front foot
    pose(elbow_front=(14, 22), hand_front=(12, 20),
         elbow_back=(10, 24), hand_back=(12, 27),
         shoulder=(15, 17), head=(15, 9), knee_front=(20, 38), foot_front=(24, 46)),
    # 6 punch: the arm is out
    pose(elbow_front=(25, 20), hand_front=(31, 19), elbow_back=(12, 24), hand_back=(15, 27),
         shoulder=(17, 17), head=(18, 9), knee_front=(17, 38), foot_front=(20, 46)),
    # 7 punch recover: half back
    pose(elbow_front=(20, 21), hand_front=(26, 20), elbow_back=(12, 24), hand_back=(14, 27)),
    # 8 kick wind-up: knee high, leaning back
    pose(knee_front=(20, 30), foot_front=(22, 35), knee_back=(14, 38), foot_back=(11, 46),
         hip=(14, 30), shoulder=(14, 17), head=(13, 9),
         elbow_back=(9, 22), hand_back=(10, 18), elbow_front=(18, 22), hand_front=(18, 18)),
    # 9 kick: leg out
    pose(knee_front=(25, 29), foot_front=(31, 26), knee_back=(14, 38), foot_back=(12, 46),
         hip=(15, 30), shoulder=(15, 18), head=(14, 10),
         elbow_back=(11, 22), hand_back=(12, 19), elbow_front=(20, 24), hand_front=(22, 21)),
    # 10 kick recover: leg coming down
    pose(knee_front=(22, 34), foot_front=(26, 39), knee_back=(13, 38), foot_back=(11, 46),
         shoulder=(15, 18), head=(15, 10)),
    # 11 hit: recoiling
    pose(hip=(18, 30), shoulder=(15, 17), head=(13, 9),
         elbow_back=(9, 21), hand_back=(7, 18), elbow_front=(18, 22), hand_front=(19, 18),
         knee_back=(13, 38), foot_back=(10, 46), knee_front=(21, 38), foot_front=(24, 46)),
]

# --- palette --------------------------------------------------------------


def load_palette():
    """The 256 RGB555 words from docs/SPEC.md §4.7."""
    spec = Path(__file__).resolve().parents[3] / "docs" / "SPEC.md"
    text = spec.read_text()
    rows = re.findall(r"^\| (\d+)[–-](\d+) \| ((?:`[0-9A-Fa-f]{4}`\s*)+)\|", text, re.M)
    words = {}
    for low, high, blob in rows:
        low, high = int(low), int(high)
        hexes = re.findall(r"`([0-9A-Fa-f]{4})`", blob)
        for i, h in enumerate(hexes):
            words[low + i] = int(h, 16)
    if len(words) != 256:
        raise SystemExit(f"could not read the palette table from {spec}")
    return words


def expand5(v):
    return ((v & 0x1F) << 3) | ((v & 0x1F) >> 2)


def rgb8(word):
    return (expand5(word), expand5(word >> 5), expand5(word >> 10))


# --- the two fighters' tones ----------------------------------------------

# Ramp 0 is red (base 16), ramp 9 is blue (base 160), ramp 2 is yellow (base 48)
# for skin, and index 4 is light grey for the belt. SPEC §4.7.
FIGHTERS = [
    dict(
        name="A",
        outline=17, shadow=19, mid=23, light=27, bright=30,
        skin_shadow=54, skin=58, skin_light=61,
        belt=4,
    ),
    dict(
        name="B",
        outline=161, shadow=163, mid=167, light=171, bright=174,
        skin_shadow=54, skin=58, skin_light=61,
        belt=4,
    ),
]

# For the ASCII preview only.
TONE_LETTERS = {
    "outline": "#", "shadow": "+", "mid": "o", "light": "*", "bright": "@",
    "skin_shadow": "s", "skin": "S", "skin_light": "W", "belt": "=",
}


# --- drawing --------------------------------------------------------------


class Cell:
    def __init__(self):
        self.px = [[0] * CELL_W for _ in range(CELL_H)]

    def dot(self, x, y, colour):
        if 0 <= x < CELL_W and 0 <= y < CELL_H:
            self.px[y][x] = colour

    def line(self, a, b, colour, radius=1):
        radius = max(0, radius)
        (x0, y0), (x1, y1) = a, b
        dx, dy = abs(x1 - x0), abs(y1 - y0)
        sx = 1 if x0 < x1 else -1
        sy = 1 if y0 < y1 else -1
        err = dx - dy
        while True:
            for oy in range(-radius, radius + 1):
                for ox in range(-radius, radius + 1):
                    self.dot(x0 + ox, y0 + oy, colour)
            if x0 == x1 and y0 == y1:
                return
            e2 = 2 * err
            if e2 > -dy:
                err -= dy
                x0 += sx
            if e2 < dx:
                err += dx
                y0 += sy

    def disc(self, centre, radius, colour):
        cx, cy = centre
        radius = max(0, radius)
        for y in range(cy - radius, cy + radius + 1):
            for x in range(cx - radius, cx + radius + 1):
                if (x - cx) ** 2 + (y - cy) ** 2 <= radius * radius:
                    self.dot(x, y, colour)


def scaled(joints):
    return {k: (round(x * SCALE), round(y * SCALE)) for k, (x, y) in joints.items()}


def shift(p, dx, dy):
    return (p[0] + dx, p[1] + dy)


def normal(a, b):
    """The perpendicular to a->b that points up and to the left.

    Shading has to follow the limb's own edge, not a fixed screen offset: a
    blind (-1,-1) shift slides the whole limb sideways instead of lighting it.
    """
    dx, dy = b[0] - a[0], b[1] - a[1]
    length = math.hypot(dx, dy)
    if length == 0:
        return (0.0, -1.0)
    nx, ny = -dy / length, dx / length
    if nx + ny > 0:
        nx, ny = -nx, -ny
    return (nx, ny)


def shaded(cell, a, b, radius, mid, light, shadow):
    """A limb: mid fill, one pixel of light along the up-left edge, one of
    shadow along the down-right edge. Light from the upper left is the
    convention pixel art has used forever, and it is what makes a figure read
    as a volume rather than a flat shape."""
    cell.line(a, b, mid, radius)
    if radius < 1:
        return
    nx, ny = normal(a, b)

    def offset(p, sign):
        return (round(p[0] + sign * nx * radius), round(p[1] + sign * ny * radius))

    cell.line(offset(a, 1), offset(b, 1), light, 0)
    cell.line(offset(a, -1), offset(b, -1), shadow, 0)


def draw_fighter(pose, t):
    cell = Cell()
    j = scaled(POSES_JOINTS[pose])
    head_r = round(HEAD_R * SCALE)

    back_leg = [("hip", "knee_back"), ("knee_back", "foot_back")]
    front_leg = [("hip", "knee_front"), ("knee_front", "foot_front")]
    back_arm = [("shoulder", "elbow_back"), ("elbow_back", "hand_back")]
    front_arm = [("shoulder", "elbow_front"), ("elbow_front", "hand_front")]

    # 1. outline pass: every limb a touch thicker, in the darkest tone. This is
    # what stops a fighter dissolving into whatever is behind it.
    for a, b in back_leg + front_leg:
        cell.line(j[a], j[b], t["outline"], RADIUS + 1)
    for a, b in back_arm + front_arm:
        cell.line(j[a], j[b], t["outline"], RADIUS + 1)
    cell.line(j["shoulder"], j["hip"], t["outline"], TORSO_R + 1)
    cell.line(j["head"], j["shoulder"], t["outline"], NECK_R + 1)
    cell.disc(j["head"], head_r + 1, t["outline"])

    # 2. the far side of the body, flat in shadow
    for a, b in back_leg:
        cell.line(j[a], j[b], t["shadow"], RADIUS)
    for a, b in back_arm:
        cell.line(j[a], j[b], t["shadow"], RADIUS)

    # 3. torso and neck, then the belt across the waist
    shaded(cell, j["shoulder"], j["hip"], TORSO_R, t["mid"], t["light"], t["shadow"])
    cell.line(j["head"], j["shoulder"], t["skin_shadow"], NECK_R)
    waist = j["hip"]
    cell.line(shift(waist, -4, -1), shift(waist, 4, -1), t["belt"], 1)

    # 4. the near side, a step brighter than the torso so it reads as nearer
    for a, b in front_leg + front_arm:
        shaded(cell, j[a], j[b], RADIUS, t["light"], t["bright"], t["mid"])
    cell.disc(j["hand_front"], 2, t["skin"])
    cell.disc(j["hand_back"], 2, t["skin_shadow"])

    # 5. head, lit from above, with a headband in the costume colour
    cell.disc(j["head"], head_r, t["skin"])
    cell.disc(shift(j["head"], -1, -2), head_r - 2, t["skin_light"])
    band = shift(j["head"], 0, -3)
    cell.line(shift(band, -head_r + 2, 0), shift(band, head_r - 2, 0), t["bright"], 0)

    return cell


def main():
    palette = load_palette()
    sheet = [[0] * 256 for _ in range(256)]

    # Eight 40x60 cells do not fit across 256 pixels, so each fighter gets two
    # rows: pose p of fighter f is at column p % 6, row f * 2 + p / 6.
    for f, tones in enumerate(FIGHTERS):
        for p in range(len(POSES)):
            cell = draw_fighter(p, tones)
            ox = (p % 6) * CELL_W
            oy = (f * 2 + p // 6) * CELL_H
            for y in range(CELL_H):
                for x in range(CELL_W):
                    sheet[oy + y][ox + x] = cell.px[y][x]

    used = sum(1 for row in sheet for v in row if v)
    total = sum(1 for row in sheet for _ in row)

    # RGBA: alpha 0 is the blit key, so it becomes palette index 0 on load.
    pixels = bytearray()
    for y in range(256):
        for x in range(256):
            index = sheet[y][x]
            r, g, b = rgb8(palette[index])
            pixels += bytes((r, g, b, 0 if index == 0 else 255))

    out = Path(__file__).resolve().parent.parent / "assets" / "fighters.png"
    out.write_bytes(rgba_png(256, 256, pixels))
    print(f"wrote {out} ({out.stat().st_size} bytes)")
    print(f"cells: {len(POSES) * 2} at {CELL_W}x{CELL_H} = {len(POSES) * 2 * CELL_W * CELL_H:,} of {total:,} sheet pixels ({len(POSES) * 2 * CELL_W * CELL_H / total:.0%})")
    print(f"non-empty sheet pixels: {used:,} ({used / total:.0%})")
    for i, name in enumerate(POSES):
        print(f"  pose {i}: {name}")

    if "--preview" in sys.argv:
        preview(palette)


def preview(palette):
    """Print a pose as ASCII, one letter per tone, so shading can be judged."""
    wanted = [int(a) for a in sys.argv[sys.argv.index("--preview") + 1:] if a.isdigit()]
    if not wanted:
        wanted = [0, 6, 9]
    for p in wanted:
        tones = FIGHTERS[0]
        letters = {v: TONE_LETTERS[k] for k, v in tones.items() if k in TONE_LETTERS}
        cell = draw_fighter(p, tones)
        print(f"\n--- pose {p}: {POSES[p]} ---")
        for row in cell.px:
            print("  " + "".join(letters.get(v, ".") if v else "." for v in row))


def rgba_png(width, height, pixels):
    """Minimal 8-bit RGBA PNG encoder (no third-party dependency)."""
    raw = bytearray()
    for y in range(height):
        raw.append(0)  # filter type 0: none
        raw += pixels[y * width * 4:(y + 1) * width * 4]

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


if __name__ == "__main__":
    main()
