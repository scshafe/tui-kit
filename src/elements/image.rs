use std::sync::Arc;

use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::component::{BufferComponent, ComponentId, DirtyReason, DirtyState};
use crate::input::KeyEvent;
use crate::layout::{CanvasMetrics, CellOffset, CellPixel, CellSize};
use crate::widgets::image_viewport::ImageViewportWidget;

use super::effect::{EffectElement, RenderEffect};

/// Effect-aware wrapper for [`ImageViewportWidget`].
#[derive(Debug, Clone)]
pub struct ImageViewportElement {
    id: ComponentId,
    widget: ImageViewportWidget,
    image_id: u32,
    placement_id: u32,
    png: Option<Arc<[u8]>>,
    dirty: DirtyState,
}

impl ImageViewportElement {
    pub fn new(
        id: impl Into<ComponentId>,
        image_id: u32,
        placement_id: u32,
        widget: ImageViewportWidget,
    ) -> Self {
        Self {
            id: id.into(),
            widget,
            image_id,
            placement_id,
            png: None,
            dirty: DirtyState::image_placement(DirtyReason::ImagePlacement),
        }
    }

    pub fn with_png(mut self, png: impl Into<Arc<[u8]>>) -> Self {
        self.png = Some(png.into());
        self
    }

    pub fn widget(&self) -> &ImageViewportWidget {
        &self.widget
    }

    pub fn widget_mut(&mut self) -> &mut ImageViewportWidget {
        self.mark_dirty(DirtyReason::ImagePlacement);
        &mut self.widget
    }

    pub fn image_id(&self) -> u32 {
        self.image_id
    }

    pub fn placement_id(&self) -> u32 {
        self.placement_id
    }

    pub fn update_canvas_for_area(&mut self, area: Rect, cell_pixel: CellPixel) {
        self.widget.update_canvas(CanvasMetrics::new(
            CellSize::new(area.width, area.height),
            cell_pixel,
        ));
        self.mark_dirty(DirtyReason::Resize);
    }
}

impl BufferComponent for ImageViewportElement {
    type Event = KeyEvent;
    type Message = ();

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, _area: Rect, _buffer: &mut Buffer) -> Result<()> {
        Ok(())
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_image_placement(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
    }
}

impl EffectElement for ImageViewportElement {
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        let canvas = self.widget.canvas();
        let area_cells = CellSize::new(area.width, area.height);
        if canvas.cells != area_cells {
            self.widget
                .update_canvas(CanvasMetrics::new(area_cells, canvas.cell_pixel));
        }
        let Some(placement) = self.widget.placement()? else {
            return Ok(vec![RenderEffect::DeleteImagePlacement {
                image_id: self.image_id,
                placement_id: self.placement_id,
            }]);
        };
        let mut effects = Vec::new();
        if let Some(png) = &self.png {
            effects.push(RenderEffect::EnsureImageLoaded {
                image_id: self.image_id,
                png: png.clone(),
            });
        }
        effects.push(RenderEffect::PlaceImage {
            origin: CellOffset {
                col: area.x.saturating_add(placement.origin.col),
                row: area.y.saturating_add(placement.origin.row),
            },
            options: placement.place_options(self.image_id, self.placement_id),
        });
        self.clear_dirty();
        Ok(effects)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        Ok(vec![RenderEffect::DeleteImagePlacement {
            image_id: self.image_id,
            placement_id: self.placement_id,
        }])
    }
}
