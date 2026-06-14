use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use tui_kit::image::PlaceOptions;
use tui_kit::layout::{CellOffset, PixelRect};
use tui_kit::renderer::{ImageSurfaceRenderer, RenderEffect, RenderFrame, RendererBackend};
use tui_kit::testkit::{MockImageCall, MockImageSurface, MockRendererBackend};

fn place_effect(image_id: u32, placement_id: u32) -> RenderEffect {
    RenderEffect::PlaceImage {
        origin: CellOffset { col: 3, row: 4 },
        options: PlaceOptions {
            image_id,
            placement_id,
            source: PixelRect {
                x: 1,
                y: 2,
                width: 16,
                height: 9,
            },
            cell_cols: 8,
            cell_rows: 4,
        },
    }
}

fn buffer_with_text(text: &str) -> Buffer {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 1));
    buffer.set_string(0, 0, text, Style::default());
    buffer
}

#[test]
fn render_frame_preserves_buffer_and_effect_parts() {
    let buffer = buffer_with_text("frame");
    let effects = vec![place_effect(1, 10)];
    let frame = RenderFrame::new(buffer.clone(), effects.clone());

    assert_eq!(frame.area(), Rect::new(0, 0, 12, 1));
    assert_eq!(frame.buffer(), &buffer);
    assert_eq!(frame.effects(), effects.as_slice());

    let (actual_buffer, actual_effects) = frame.into_parts();
    assert_eq!(actual_buffer, buffer);
    assert_eq!(actual_effects, effects);
}

#[test]
fn mock_renderer_records_complete_frames_and_flattened_effects() {
    let buffer = buffer_with_text("mock");
    let effects = vec![
        place_effect(2, 20),
        RenderEffect::DeletePlacement { placement_id: 20 },
    ];
    let frame = RenderFrame::new(buffer, effects.clone());

    let mut renderer = MockRendererBackend::new();
    renderer.render_frame(&frame).unwrap();

    assert_eq!(renderer.frames(), &[frame]);
    assert_eq!(renderer.effects(), effects.as_slice());
}

#[test]
fn image_surface_renderer_applies_effects_to_wrapped_surface() {
    let effects = vec![
        RenderEffect::EnsureImageLoaded {
            image_id: 7,
            png: Arc::from([1u8, 2, 3, 4].as_slice()),
        },
        place_effect(7, 70),
        RenderEffect::DeleteImagePlacement {
            image_id: 7,
            placement_id: 70,
        },
    ];
    let frame = RenderFrame::new(buffer_with_text("surface"), effects);

    let mut renderer = ImageSurfaceRenderer::new(MockImageSurface::default());
    renderer.render_frame(&frame).unwrap();

    assert_eq!(renderer.last_buffer(), Some(frame.buffer()));
    assert_eq!(
        renderer.surface().calls(),
        &[
            MockImageCall::EnsureLoaded {
                image_id: 7,
                bytes: 4,
            },
            MockImageCall::Place(PlaceOptions {
                image_id: 7,
                placement_id: 70,
                source: PixelRect {
                    x: 1,
                    y: 2,
                    width: 16,
                    height: 9,
                },
                cell_cols: 8,
                cell_rows: 4,
            }),
            MockImageCall::DeleteImagePlacement {
                image_id: 7,
                placement_id: 70,
            },
        ]
    );
}
