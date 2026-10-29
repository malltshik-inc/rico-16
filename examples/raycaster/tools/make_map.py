#!/usr/bin/env python3
"""Generate examples/raycaster/assets/map.png.

The level is a 128x128 tilemap. SPEC §10.2.3 reads a grayscale map PNG by
taking the sample value as the tile id, so this writes 8-bit grayscale: 0 is
open floor, 1-4 are wall types (the cart colours walls by id).

Deterministic: the seed is fixed, so re-running reproduces the committed PNG.

    python3 examples/raycaster/tools/make_map.py
"""

import random
import struct
import zlib
from pathlib import Path

WIDTH = HEIGHT = 128
SEED = 0xD00D
START_ROOM = (18, 18, 26, 26)  # x0, y0, x1, y1 — the cart starts at (21.5, 21.5)

random.seed(SEED)


def carve(grid, x0, y0, x1, y1):
    """Turn a rectangle into open floor, clipped to the map."""
    for y in range(max(0, y0), min(HEIGHT, y1)):
        for x in range(max(0, x0), min(WIDTH, x1)):
            grid[y][x] = 0


def main():
    grid = [[1] * WIDTH for _ in range(HEIGHT)]

    # A guaranteed open start room, then rooms scattered over the level. Rooms
    # stay small: a raycaster draws a wall `240 / distance` pixels tall, so an
    # open hall looks like a distant strip while tight rooms look like rooms.
    rooms = [START_ROOM]
    carve(grid, *START_ROOM)
    for _ in range(34):
        w = random.randint(4, 9)
        h = random.randint(4, 8)
        x = random.randint(2, WIDTH - w - 2)
        y = random.randint(2, HEIGHT - h - 2)
        carve(grid, x, y, x + w, y + h)
        rooms.append((x, y, x + w, y + h))

    # Connect consecutive rooms with two-cell-wide L corridors. The player is
    # centred in them, so the walls are one cell away and fill the view.
    def centre(room):
        return ((room[0] + room[2]) // 2, (room[1] + room[3]) // 2)

    for previous, room in zip(rooms, rooms[1:]):
        ax, ay = centre(previous)
        bx, by = centre(room)
        if random.random() < 0.5:
            carve(grid, min(ax, bx), ay - 1, max(ax, bx) + 1, ay + 1)
            carve(grid, bx - 1, min(ay, by), bx + 1, max(ay, by) + 1)
        else:
            carve(grid, ax - 1, min(ay, by), ax + 1, max(ay, by) + 1)
            carve(grid, min(ax, bx), by - 1, max(ax, bx) + 1, by + 1)

    # Pillars, for something to look at. Never inside the start room.
    for _ in range(140):
        x = random.randint(3, WIDTH - 4)
        y = random.randint(3, HEIGHT - 4)
        inside_start = START_ROOM[0] - 2 <= x <= START_ROOM[2] + 1 and \
            START_ROOM[1] - 2 <= y <= START_ROOM[3] + 1
        if grid[y][x] == 0 and not inside_start:
            grid[y][x] = 1

    # Wall types by region, so the level is not one flat colour.
    pixels = bytearray()
    for y in range(HEIGHT):
        for x in range(WIDTH):
            if grid[y][x] > 0:
                pixels.append(1 + ((x // 20 + y // 20) % 4))
            else:
                pixels.append(0)

    assert pixels[21 * WIDTH + 21] == 0, "the cart's start cell must be open"

    report_best_view(grid)

    out = Path(__file__).resolve().parent.parent / "assets" / "map.png"
    out.write_bytes(gray_png(WIDTH, HEIGHT, pixels))
    print(f"wrote {out} ({out.stat().st_size} bytes)")


def report_best_view(grid):
    """Print the longest straight run of open cells, for the cart's start.

    A raycaster is at its best looking down a corridor: walls at varying
    distances give the perspective, and a flat wall filling the screen gives
    none. The cart hardcodes this start, so the level is stable.
    """
    directions = ((1, 0, 0.0), (0, 1, 1.5708), (-1, 0, 3.1416), (0, -1, 4.7124))
    margin = 6
    best = (None, 0, 0, 0.0)
    best_score = -1000
    for y in range(margin, HEIGHT - margin):
        for x in range(margin, WIDTH - margin):
            if grid[y][x] != 0:
                continue
            for dx, dy, angle in directions:
                run = 0
                while run < 48 and grid[y + dy * run][x + dx * run] == 0:
                    run += 1
                if run < 5:
                    continue
                # A view down a corridor of roughly a dozen cells puts walls at
                # a readable size; a 48-cell run is a sliver, a 2-cell run is a
                # wall in the face.
                score = -abs(run - 12)
                if score > best_score:
                    best_score = score
                    best = (run, x, y, angle)
    run, x, y, angle = best

    # Centre the player across the corridor, so the view is not half wall.
    if dx_of(angle) != 0:
        lo = hi = y
        while grid[lo - 1][x] == 0:
            lo -= 1
        while grid[hi + 1][x] == 0:
            hi += 1
        px, py = x + 0.5, (lo + hi) / 2.0 + 0.5
    else:
        lo = hi = x
        while grid[y][lo - 1] == 0:
            lo -= 1
        while grid[y][hi + 1] == 0:
            hi += 1
        px, py = (lo + hi) / 2.0 + 0.5, y + 0.5

    print(f"best view: {run} cells clear at ({px}, {py}) angle {angle:.4f}")


def dx_of(angle):
    import math

    return round(math.cos(angle))


def gray_png(width, height, pixels):
    """Minimal 8-bit grayscale PNG encoder (no third-party dependency)."""
    raw = bytearray()
    for y in range(height):
        raw.append(0)  # filter type 0: none
        raw += pixels[y * width:(y + 1) * width]

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 0, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


if __name__ == "__main__":
    main()
