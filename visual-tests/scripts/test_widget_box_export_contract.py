#!/usr/bin/env python3
"""Contract test for widget-box PNG export.

This test intentionally stays in pixel space. Terminal cell width/height should
affect how a terminal widget is planned, but exported widget-box pixels should
not be normalized by the terminal cell aspect ratio.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))

from generate_image_box_fixture_outputs import CASES, Case
from image_box_png import RgbaImage, compare_rgba_images, image_box_source_fixture, widget_crop


WIDGET_BOX_EXPORT_AVAILABLE = False


def export_widget_box(source: RgbaImage, case: Case) -> RgbaImage:
    """Return the tui-kit widget-box export for this source image and case.

    Replace this stub with the real tui-kit export call once that API exists.
    The export should return exactly `case.widget_width x case.widget_height`
    RGBA pixels for the visible widget aperture.
    """

    _ = (source, case)
    raise NotImplementedError("tui-kit widget-box PNG export is not implemented yet")


class WidgetBoxExportContractTest(unittest.TestCase):
    def assert_rgba_pixels_equal(
        self,
        actual: RgbaImage,
        expected: RgbaImage,
        actual_label: str,
        expected_label: str,
    ) -> None:
        comparison = compare_rgba_images(actual, expected, actual_label, expected_label)
        self.assertTrue(comparison.equal, comparison.message)

    def test_widget_box_export_matches_generated_crops(self) -> None:
        if not WIDGET_BOX_EXPORT_AVAILABLE:
            self.skipTest("waiting for tui-kit widget-box PNG export")

        source = image_box_source_fixture()
        for case in CASES:
            with self.subTest(case=case.stem):
                expected = widget_crop(
                    source,
                    case.widget_width,
                    case.widget_height,
                    case.offset_x,
                    case.offset_y,
                    case.scale,
                )
                actual = export_widget_box(source, case)

                self.assert_rgba_pixels_equal(
                    actual,
                    expected,
                    f"tui-kit widget-box export {case.stem}",
                    f"generated expected crop {case.stem}",
                )


if __name__ == "__main__":
    unittest.main()
