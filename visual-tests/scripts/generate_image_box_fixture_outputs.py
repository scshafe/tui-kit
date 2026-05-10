#!/usr/bin/env python3
"""Generate scripted crop and negative-space fixtures for ImageBox tests."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from image_box_png import image_box_source_fixture, widget_crop, negative_space, write_png_rgba


ROOT = Path(__file__).resolve().parents[1]
ARCHIVE_DIR = ROOT / "archived" / "pngs"
SOURCE = ARCHIVE_DIR / "image_box_source_fixture.png"
CROP_DIR = ARCHIVE_DIR / "generated" / "crops"
NEGATIVE_DIR = ARCHIVE_DIR / "generated" / "negative_space"


@dataclass(frozen=True)
class Case:
    index: int
    widget_width: int
    widget_height: int
    offset_x: int
    offset_y: int
    scale: float
    scale_label: str

    @property
    def stem(self) -> str:
        return (
            f"case_{self.index:02d}_widget_{self.widget_width}x{self.widget_height}"
            f"_offset_{self.offset_x}_{self.offset_y}_scale_{self.scale_label}"
        )


CASES = [
    Case(1, 200, 150, 0, 0, 1.0, "1"),
    Case(2, 100, 75, 0, 0, 1.0, "1"),
    Case(3, 100, 75, 0, 0, 0.5, "0_5"),
    Case(4, 100, 75, 460, 300, 1.0, "1"),
    Case(5, 200, 150, 920, 600, 2.0, "2"),
    Case(6, 200, 150, 0, 0, 0.25, "0_25"),
    Case(7, 1600, 1200, 0, 0, 2.0, "2"),
    Case(8, 1600, 1200, 920, 600, 2.0, "2"),
    # Largest uniform scale where the full purple rectangle still fits in an
    # 800x600 aperture with offset 0. It is constrained by x: 800 / (460 + 100).
    Case(9, 800, 600, 0, 0, 10.0 / 7.0, "10_over_7"),
]


def main() -> None:
    source = image_box_source_fixture()
    write_png_rgba(SOURCE, source)
    for case in CASES:
        crop = widget_crop(
            source,
            case.widget_width,
            case.widget_height,
            case.offset_x,
            case.offset_y,
            case.scale,
        )
        negative = negative_space(
            source,
            case.widget_width,
            case.widget_height,
            case.offset_x,
            case.offset_y,
            case.scale,
        )

        crop_path = CROP_DIR / f"crop_{case.stem}.png"
        negative_path = NEGATIVE_DIR / f"negative_{case.stem}.png"
        write_png_rgba(crop_path, crop)
        write_png_rgba(negative_path, negative)
        print(
            f"{case.index:02d}: crop={crop.width}x{crop.height} "
            f"negative={negative.width}x{negative.height} "
            f"offset=({case.offset_x},{case.offset_y}) scale={case.scale}"
        )


if __name__ == "__main__":
    main()
