#!/usr/bin/env python3
"""Generate examples/fighter/assets/fighters.png.

A 256x256 sprite sheet of two fighters, eight poses each, in 32x48 cells:

    cell (pose p, fighter f) is at x = (p % 6) * 40, y = (f * 2 + p / 6) * 60
    fighter 0 is red, fighter 1 is blue; both are drawn facing right, and the
    cart flips fighter 1 with sprite::draw_region's flip_x

The poses are drawn from a joint skeleton rather than by hand, so the sheet is
reproducible and the poses can be tweaked by moving a number. The palette is
read from docs/SPEC.md so the colours match the normative default palette the
cart will use: SPEC §4.7's table wins over its HSV recipe.

    python3 examples/fighter/tools/make_sprites.py
"""

import re
import struct
import zlib
from pathlib import Path

CELL_W, CELL_H = 40, 60
# The skeleton below is authored in a 32x48 cell and scaled up on the way in.
SCALE = 1.25
POSES = ["idle", "walk0", "walk1", "crouch", "punch", "kick0", "kick1", "hit"]

# --- the skeleton, in cell coordinates, fighter facing right ---------------

BASE = {
    "hip": (16, 30),
    "shoulder": (16, 17),
    "head": (16, 9),
    "elbow_back": (12, 23),
    "hand_back": (10, 28),
    "elbow_front": (20, 23),
    "hand_front": (23, 27),
    "knee_back": (13, 38),
    "foot_back": (11, 46),
    "knee_front": (19, 38),
    "foot_front": (22, 46),
}


def pose(**overrides):
    joints = dict(BASE)
    joints.update(overrides)
    return joints


POSES_JOINTS = [
    # idle: guard up, weight centred
    pose(elbow_back=(11, 22), hand_back=(12, 19), elbow_front=(21, 22), hand_front=(21, 19)),
    # walk0: legs apart, arms swinging
    pose(knee_back=(11, 38), foot_back=(7, 46), knee_front=(21, 38), foot_front=(26, 46),
         elbow_back=(11, 24), hand_back=(9, 27), elbow_front=(21, 24), hand_front=(24, 21)),
    # walk1: legs together
    pose(knee_back=(15, 38), foot_back=(14, 46), knee_front=(18, 38), foot_front=(19, 46),
         elbow_back=(12, 24), hand_back=(11, 28), elbow_front=(21, 24), hand_front=(22, 27)),
    # crouch
    pose(hip=(16, 35), shoulder=(16, 24), head=(16, 16),
         knee_back=(11, 41), foot_back=(11, 46), knee_front=(21, 41), foot_front=(21, 46),
         elbow_back=(11, 29), hand_back=(11, 33), elbow_front=(21, 29), hand_front=(21, 33)),
    # punch: front arm straight out
    pose(elbow_front=(25, 20), hand_front=(31, 19), elbow_back=(12, 24), hand_back=(15, 27)),
    # kick0: front knee up
    pose(knee_front=(22, 31), foot_front=(26, 34), knee_back=(13, 38), foot_back=(11, 46),
         elbow_back=(11, 23), hand_back=(13, 20), elbow_front=(21, 24), hand_front=(23, 21)),
    # kick1: front leg extended
    pose(knee_front=(25, 29), foot_front=(31, 26), knee_back=(14, 38), foot_back=(12, 46),
         shoulder=(15, 18), head=(15, 10), elbow_back=(11, 22), hand_back=(12, 19),
         elbow_front=(20, 24), hand_front=(22, 21)),
    # hit: recoiling backwards
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


# --- drawing --------------------------------------------------------------


class Cell:
    def __init__(self):
        self.px = [[0] * CELL_W for _ in range(CELL_H)]

    def dot(self, x, y, colour):
        if 0 <= x < CELL_W and 0 <= y < CELL_H:
            self.px[y][x] = colour

    def line(self, a, b, colour, radius=1):
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
        for y in range(cy - radius, cy + radius + 1):
            for x in range(cx - radius, cx + radius + 1):
                if (x - cx) ** 2 + (y - cy) ** 2 <= radius * radius:
                    self.dot(x, y, colour)


def scaled(joints):
    return {k: (round(x * SCALE), round(y * SCALE)) for k, (x, y) in joints.items()}


def draw_fighter(pose, gi, gi_dark, skin, outline):
    """Back limbs, torso, front limbs, head — in that order, so they overlap."""
    cell = Cell()
    j = scaled(POSES_JOINTS[pose])

    cell.line(j["hip"], j["knee_back"], outline, 3)
    cell.line(j["knee_back"], j["foot_back"], outline, 3)
    cell.line(j["shoulder"], j["elbow_back"], gi_dark, 1)
    cell.line(j["elbow_back"], j["hand_back"], gi_dark, 1)

    cell.line(j["shoulder"], j["hip"], gi, 3)
    cell.line(j["hip"], j["knee_front"], outline, 3)
    cell.line(j["knee_front"], j["foot_front"], outline, 3)

    cell.line(j["shoulder"], j["elbow_front"], gi, 1)
    cell.line(j["elbow_front"], j["hand_front"], gi, 1)
    cell.disc(j["hand_front"], 1, skin)
    cell.disc(j["hand_back"], 1, skin)

    cell.disc(j["head"], round(4 * SCALE), skin)
    return cell


def main():
    palette = load_palette()
    sheet = [[0] * 256 for _ in range(256)]

    fighters = [
        # red gi, blue gi — both with a tan skin that does not read as the floor
        (23, 19, 60, 17),
        (167, 163, 60, 161),
    ]

    # Eight 40x60 cells do not fit across 256 pixels, so each fighter gets two
    # rows: pose p of fighter f is at column p % 6, row f * 2 + p / 6.
    for f, (gi, gi_dark, skin, outline) in enumerate(fighters):
        for p in range(len(POSES)):
            cell = draw_fighter(p, gi, gi_dark, skin, outline)
            ox = (p % 6) * CELL_W
            oy = (f * 2 + p // 6) * CELL_H
            for y in range(CELL_H):
                for x in range(CELL_W):
                    sheet[oy + y][ox + x] = cell.px[y][x]

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
    print("cell (pose, fighter): x = (pose % 6) * 40, y = (fighter * 2 + pose / 6) * 60")
    for i, name in enumerate(POSES):
        print(f"  pose {i}: {name}")


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
