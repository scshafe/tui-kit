#!/usr/bin/env python3
"""Deterministic PNG helpers for ImageBox visual fixtures.

The model here is intentionally small:

1. Scale the full source image by one uniform scalar.
2. Treat offset as a coordinate in scaled-image pixels.
3. Crop a widget-sized aperture from that scaled image.
"""

from __future__ import annotations

import argparse
import math
import struct
import zlib
from dataclasses import dataclass
from pathlib import Path


RGBA_TRANSPARENT = (0, 0, 0, 0)


@dataclass(frozen=True)
class RgbaImage:
    width: int
    height: int
    pixels: bytearray


@dataclass(frozen=True)
class PngComparison:
    equal: bool
    message: str


def image_box_source_fixture() -> RgbaImage:
    width, height = 800, 600
    border = 16
    widget_width, widget_height = 200, 150
    float_x, float_y, float_width, float_height = 460, 300, 100, 75

    pixels = bytearray([236, 238, 240, 255] * width * height)

    def blend_px(x: int, y: int, rgba: tuple[int, int, int, int]) -> None:
        if not (0 <= x < width and 0 <= y < height):
            return
        r, g, b, alpha_u8 = rgba
        i = (y * width + x) * 4
        alpha = alpha_u8 / 255.0
        inv = 1.0 - alpha
        pixels[i] = round(r * alpha + pixels[i] * inv)
        pixels[i + 1] = round(g * alpha + pixels[i + 1] * inv)
        pixels[i + 2] = round(b * alpha + pixels[i + 2] * inv)
        pixels[i + 3] = 255

    def fill_rect(x: int, y: int, rect_width: int, rect_height: int, rgba: tuple[int, int, int, int]) -> None:
        x1 = max(0, x)
        y1 = max(0, y)
        x2 = min(width, x + rect_width)
        y2 = min(height, y + rect_height)
        for yy in range(y1, y2):
            for xx in range(x1, x2):
                blend_px(xx, yy, rgba)

    def stroke_rect(
        x: int,
        y: int,
        rect_width: int,
        rect_height: int,
        depth: int,
        rgba: tuple[int, int, int, int],
    ) -> None:
        fill_rect(x, y, rect_width, depth, rgba)
        fill_rect(x, y + rect_height - depth, rect_width, depth, rgba)
        fill_rect(x, y, depth, rect_height, rgba)
        fill_rect(x + rect_width - depth, y, depth, rect_height, rgba)

    def line_x(x: int, rgba: tuple[int, int, int, int]) -> None:
        for y in range(height):
            blend_px(x, y, rgba)

    def line_y(y: int, rgba: tuple[int, int, int, int]) -> None:
        for x in range(width):
            blend_px(x, y, rgba)

    for x in range(0, width, 50):
        line_x(x, (96, 104, 116, 62))
    for y in range(0, height, 50):
        line_y(y, (96, 104, 116, 62))
    for x in range(0, width, 100):
        line_x(x, (32, 38, 46, 120))
    for y in range(0, height, 100):
        line_y(y, (32, 38, 46, 120))

    fill_rect(0, 0, width, border, (246, 64, 64, 255))
    fill_rect(0, height - border, width, border, (55, 105, 245, 255))
    fill_rect(0, 0, border, height, (35, 190, 100, 255))
    fill_rect(width - border, 0, border, height, (245, 212, 55, 255))

    fill_rect(0, 0, widget_width, widget_height, (0, 190, 225, 78))
    stroke_rect(0, 0, widget_width, widget_height, 6, (0, 220, 255, 255))
    stroke_rect(10, 10, widget_width - 20, widget_height - 20, 2, (5, 95, 120, 210))

    fill_rect(float_x, float_y, float_width, float_height, (220, 40, 225, 88))
    stroke_rect(float_x, float_y, float_width, float_height, 5, (238, 35, 238, 255))
    stroke_rect(float_x + 8, float_y + 8, float_width - 16, float_height - 16, 2, (112, 24, 116, 220))

    fill_rect(widget_width // 2 - 2, widget_height // 2 - 10, 4, 20, (0, 70, 90, 230))
    fill_rect(widget_width // 2 - 10, widget_height // 2 - 2, 20, 4, (0, 70, 90, 230))
    fill_rect(float_x + float_width // 2 - 2, float_y + float_height // 2 - 8, 4, 16, (92, 12, 96, 230))
    fill_rect(float_x + float_width // 2 - 8, float_y + float_height // 2 - 2, 16, 4, (92, 12, 96, 230))

    return RgbaImage(width, height, pixels)


def compare_pngs(left: Path, right: Path) -> PngComparison:
    left_image = read_png_rgba(left)
    right_image = read_png_rgba(right)
    return compare_rgba_images(left_image, right_image, str(left), str(right))


def compare_rgba_images(
    left: RgbaImage,
    right: RgbaImage,
    left_label: str = "left",
    right_label: str = "right",
) -> PngComparison:
    if left.width != right.width or left.height != right.height:
        return PngComparison(
            False,
            f"dimension mismatch: {left_label}={left.width}x{left.height}, "
            f"{right_label}={right.width}x{right.height}",
        )
    if left.pixels == right.pixels:
        return PngComparison(True, "images match")

    for idx, (left_byte, right_byte) in enumerate(zip(left.pixels, right.pixels)):
        if left_byte == right_byte:
            continue
        pixel_index = idx // 4
        channel = "rgba"[idx % 4]
        x = pixel_index % left.width
        y = pixel_index // left.width
        left_rgba = tuple(left.pixels[pixel_index * 4 : pixel_index * 4 + 4])
        right_rgba = tuple(right.pixels[pixel_index * 4 : pixel_index * 4 + 4])
        return PngComparison(
            False,
            f"pixel mismatch at ({x},{y}) channel {channel}: "
            f"{left_label}={left_rgba}, {right_label}={right_rgba}; "
            f"{channel} {left_byte} != {right_byte}",
        )

    return PngComparison(False, "pixel data length mismatch")


def read_png_rgba(path: Path) -> RgbaImage:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path} is not a PNG")

    pos = 8
    width = height = bit_depth = color_type = interlace = None
    idat = bytearray()
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        pos += 4
        name = data[pos : pos + 4]
        pos += 4
        chunk = data[pos : pos + length]
        pos += length
        pos += 4

        if name == b"IHDR":
            width, height, bit_depth, color_type, _compression, _filter, interlace = (
                struct.unpack(">IIBBBBB", chunk)
            )
        elif name == b"IDAT":
            idat.extend(chunk)
        elif name == b"IEND":
            break

    if (bit_depth, color_type, interlace) != (8, 6, 0):
        raise ValueError(
            f"{path} must be 8-bit non-interlaced RGBA PNG; "
            f"got bit_depth={bit_depth}, color_type={color_type}, interlace={interlace}"
        )

    raw = zlib.decompress(bytes(idat))
    stride = width * 4
    pixels = bytearray(width * height * 4)
    raw_pos = 0
    prev = bytearray(stride)

    for y in range(height):
        filter_type = raw[raw_pos]
        raw_pos += 1
        row = bytearray(raw[raw_pos : raw_pos + stride])
        raw_pos += stride
        recon = _unfilter_row(filter_type, row, prev, 4)
        start = y * stride
        pixels[start : start + stride] = recon
        prev = recon

    return RgbaImage(width, height, pixels)


def write_png_rgba(path: Path, image: RgbaImage) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    stride = image.width * 4
    raw = bytearray()
    for y in range(image.height):
        raw.append(0)
        start = y * stride
        raw.extend(image.pixels[start : start + stride])

    png = bytearray(b"\x89PNG\r\n\x1a\n")
    png.extend(_chunk(b"IHDR", struct.pack(">IIBBBBB", image.width, image.height, 8, 6, 0, 0, 0)))
    png.extend(_chunk(b"IDAT", zlib.compress(bytes(raw), 9)))
    png.extend(_chunk(b"IEND", b""))
    path.write_bytes(png)


def render_widget_crop(
    original_png: Path,
    output_png: Path,
    widget_width: int,
    widget_height: int,
    offset_x: int,
    offset_y: int,
    scale: float,
) -> None:
    source = read_png_rgba(original_png)
    rendered = widget_crop(source, widget_width, widget_height, offset_x, offset_y, scale)
    write_png_rgba(output_png, rendered)


def render_negative_space(
    original_png: Path,
    output_png: Path,
    widget_width: int,
    widget_height: int,
    offset_x: int,
    offset_y: int,
    scale: float,
) -> None:
    source = read_png_rgba(original_png)
    rendered = negative_space(source, widget_width, widget_height, offset_x, offset_y, scale)
    write_png_rgba(output_png, rendered)


def widget_crop(
    source: RgbaImage,
    widget_width: int,
    widget_height: int,
    offset_x: int,
    offset_y: int,
    scale: float,
) -> RgbaImage:
    _validate_geometry(widget_width, widget_height, scale)
    theoretical_width, theoretical_height = scaled_size(source, scale)
    out = bytearray(RGBA_TRANSPARENT * (widget_width * widget_height))

    for y in range(widget_height):
        scaled_y = offset_y + y
        if scaled_y < 0 or scaled_y >= theoretical_height:
            continue
        src_y = _scaled_to_source(scaled_y, source.height, scale)
        for x in range(widget_width):
            scaled_x = offset_x + x
            if scaled_x < 0 or scaled_x >= theoretical_width:
                continue
            src_x = _scaled_to_source(scaled_x, source.width, scale)
            _copy_pixel(source.pixels, source.width, out, widget_width, src_x, src_y, x, y)

    return RgbaImage(widget_width, widget_height, out)


def negative_space(
    source: RgbaImage,
    widget_width: int,
    widget_height: int,
    offset_x: int,
    offset_y: int,
    scale: float,
) -> RgbaImage:
    _validate_geometry(widget_width, widget_height, scale)
    theoretical_width, theoretical_height = scaled_size(source, scale)
    out = bytearray(RGBA_TRANSPARENT * (theoretical_width * theoretical_height))

    for y in range(theoretical_height):
        src_y = _scaled_to_source(y, source.height, scale)
        for x in range(theoretical_width):
            if offset_x <= x < offset_x + widget_width and offset_y <= y < offset_y + widget_height:
                continue
            src_x = _scaled_to_source(x, source.width, scale)
            _copy_pixel(source.pixels, source.width, out, theoretical_width, src_x, src_y, x, y)

    return RgbaImage(theoretical_width, theoretical_height, out)


def scaled_size(source: RgbaImage, scale: float) -> tuple[int, int]:
    return (_round_positive(source.width * scale), _round_positive(source.height * scale))


def _validate_geometry(widget_width: int, widget_height: int, scale: float) -> None:
    if widget_width <= 0 or widget_height <= 0:
        raise ValueError("widget dimensions must be positive")
    if not math.isfinite(scale) or scale <= 0:
        raise ValueError("scale must be positive and finite")


def _scaled_to_source(scaled_coordinate: int, source_extent: int, scale: float) -> int:
    return min(source_extent - 1, max(0, math.floor(scaled_coordinate / scale)))


def _round_positive(value: float) -> int:
    return max(1, math.floor(value + 0.5))


def _copy_pixel(
    src: bytearray,
    src_width: int,
    dst: bytearray,
    dst_width: int,
    src_x: int,
    src_y: int,
    dst_x: int,
    dst_y: int,
) -> None:
    src_i = (src_y * src_width + src_x) * 4
    dst_i = (dst_y * dst_width + dst_x) * 4
    dst[dst_i : dst_i + 4] = src[src_i : src_i + 4]


def _unfilter_row(filter_type: int, row: bytearray, prev: bytearray, bytes_per_pixel: int) -> bytearray:
    if filter_type == 0:
        return row
    if filter_type == 1:
        recon = bytearray(len(row))
        for i, val in enumerate(row):
            left = recon[i - bytes_per_pixel] if i >= bytes_per_pixel else 0
            recon[i] = (val + left) & 0xFF
        return recon
    if filter_type == 2:
        return bytearray((val + prev[i]) & 0xFF for i, val in enumerate(row))
    if filter_type == 3:
        recon = bytearray(len(row))
        for i, val in enumerate(row):
            left = recon[i - bytes_per_pixel] if i >= bytes_per_pixel else 0
            up = prev[i]
            recon[i] = (val + ((left + up) // 2)) & 0xFF
        return recon
    if filter_type == 4:
        recon = bytearray(len(row))
        for i, val in enumerate(row):
            left = recon[i - bytes_per_pixel] if i >= bytes_per_pixel else 0
            up = prev[i]
            up_left = prev[i - bytes_per_pixel] if i >= bytes_per_pixel else 0
            recon[i] = (val + _paeth(left, up, up_left)) & 0xFF
        return recon
    raise ValueError(f"unsupported PNG filter type {filter_type}")


def _paeth(left: int, up: int, up_left: int) -> int:
    p = left + up - up_left
    pa = abs(p - left)
    pb = abs(p - up)
    pc = abs(p - up_left)
    if pa <= pb and pa <= pc:
        return left
    if pb <= pc:
        return up
    return up_left


def _chunk(name: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + name + data + struct.pack(
        ">I", zlib.crc32(name + data) & 0xFFFFFFFF
    )


def _parse_size(raw: str) -> tuple[int, int]:
    width, height = raw.lower().split("x", 1)
    return int(width), int(height)


def _parse_offset(raw: str) -> tuple[int, int]:
    x, y = raw.split(",", 1)
    return int(x), int(y)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    for command in ("crop", "negative-space"):
        sub = subparsers.add_parser(command)
        sub.add_argument("original_png", type=Path)
        sub.add_argument("output_png", type=Path)
        sub.add_argument("--widget", required=True, type=_parse_size, metavar="WIDTHxHEIGHT")
        sub.add_argument("--offset", required=True, type=_parse_offset, metavar="X,Y")
        sub.add_argument("--scale", required=True, type=float)

    args = parser.parse_args()
    widget_width, widget_height = args.widget
    offset_x, offset_y = args.offset
    if args.command == "crop":
        render_widget_crop(
            args.original_png,
            args.output_png,
            widget_width,
            widget_height,
            offset_x,
            offset_y,
            args.scale,
        )
    else:
        render_negative_space(
            args.original_png,
            args.output_png,
            widget_width,
            widget_height,
            offset_x,
            offset_y,
            args.scale,
        )


if __name__ == "__main__":
    main()
