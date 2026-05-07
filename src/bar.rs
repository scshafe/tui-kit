//! Slot-aligned, priority-truncated text bars.
//!
//! A [`SegmentBar<Ctx>`] is a registry of [`Segment<Ctx>`] producers. At
//! render time, each segment may emit a [`StatusFragment`] given a
//! user-defined context `Ctx`. The bar composes left- and right-aligned
//! fragments separated by a configurable string, padding the middle with
//! spaces. When fragments don't fit the available width, the lowest-
//! priority fragment is dropped first (across both sides), repeatedly,
//! until everything fits — or until the bar truncates with an ellipsis.
//!
//! `Ctx` is application-specific. tui-kit ships no built-in segments;
//! apps register their own.

use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentSlot {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct StatusFragment {
    pub text: String,
    pub priority: u8,
}

impl StatusFragment {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            priority: 100,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

pub trait Segment<Ctx>: std::fmt::Debug {
    fn id(&self) -> &'static str;
    fn render(&self, ctx: &Ctx) -> Option<StatusFragment>;
}

pub struct SegmentBar<Ctx> {
    segments: Vec<(SegmentSlot, Box<dyn Segment<Ctx> + Send + Sync>)>,
    separator: &'static str,
    elide: &'static str,
    _marker: PhantomData<Ctx>,
}

impl<Ctx> std::fmt::Debug for SegmentBar<Ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegmentBar")
            .field("segments", &self.segments.len())
            .field("separator", &self.separator)
            .field("elide", &self.elide)
            .finish()
    }
}

impl<Ctx> SegmentBar<Ctx> {
    pub fn builder() -> SegmentBarBuilder<Ctx> {
        SegmentBarBuilder::default()
    }

    pub fn render(&self, ctx: &Ctx, width: u16) -> String {
        let collect = |slot: SegmentSlot| -> Vec<StatusFragment> {
            self.segments
                .iter()
                .filter(|(s, _)| *s == slot)
                .filter_map(|(_, seg)| seg.render(ctx))
                .collect()
        };
        let left = collect(SegmentSlot::Left);
        let right = collect(SegmentSlot::Right);

        let width = usize::from(width).max(1);
        layout_status_line(left, right, width, self.separator, self.elide)
    }
}

pub struct SegmentBarBuilder<Ctx> {
    segments: Vec<(SegmentSlot, Box<dyn Segment<Ctx> + Send + Sync>)>,
    separator: Option<&'static str>,
    elide: Option<&'static str>,
    _marker: PhantomData<Ctx>,
}

impl<Ctx> std::fmt::Debug for SegmentBarBuilder<Ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegmentBarBuilder")
            .field("segments", &self.segments.len())
            .finish()
    }
}

impl<Ctx> Default for SegmentBarBuilder<Ctx> {
    fn default() -> Self {
        Self {
            segments: Vec::new(),
            separator: None,
            elide: None,
            _marker: PhantomData,
        }
    }
}

impl<Ctx> SegmentBarBuilder<Ctx> {
    pub fn add(
        mut self,
        slot: SegmentSlot,
        segment: impl Segment<Ctx> + Send + Sync + 'static,
    ) -> Self {
        self.segments.push((slot, Box::new(segment)));
        self
    }

    pub fn separator(mut self, sep: &'static str) -> Self {
        self.separator = Some(sep);
        self
    }

    pub fn elide(mut self, elide: &'static str) -> Self {
        self.elide = Some(elide);
        self
    }

    pub fn build(self) -> SegmentBar<Ctx> {
        SegmentBar {
            segments: self.segments,
            separator: self.separator.unwrap_or(" | "),
            elide: self.elide.unwrap_or("…"),
            _marker: PhantomData,
        }
    }
}

fn layout_status_line(
    left: Vec<StatusFragment>,
    right: Vec<StatusFragment>,
    width: usize,
    separator: &str,
    elide: &str,
) -> String {
    let mut tagged: Vec<(SegmentSlot, StatusFragment)> = left
        .into_iter()
        .map(|f| (SegmentSlot::Left, f))
        .chain(right.into_iter().map(|f| (SegmentSlot::Right, f)))
        .collect();
    let sep_pad_between_sides = 1;

    loop {
        let total = compose(&tagged, separator, sep_pad_between_sides).visible_width;
        if total <= width || tagged.is_empty() {
            break;
        }
        let lowest = tagged
            .iter()
            .map(|(_, f)| f.priority)
            .min()
            .unwrap_or(u8::MAX);
        let drop_idx = tagged
            .iter()
            .rposition(|(_, f)| f.priority == lowest)
            .unwrap_or(0);
        tagged.remove(drop_idx);
    }

    let composed = compose(&tagged, separator, sep_pad_between_sides);
    if composed.visible_width <= width {
        let pad = width - composed.visible_width + sep_pad_between_sides;
        let mut out = String::with_capacity(width + 8);
        out.push_str(&composed.left);
        out.push_str(&" ".repeat(pad));
        out.push_str(&composed.right);
        return out;
    }

    let combined = if composed.right.is_empty() {
        composed.left
    } else {
        format!("{} {}", composed.left, composed.right)
    };
    let max_visible = width.saturating_sub(elide.chars().count());
    let truncated: String = combined.chars().take(max_visible).collect();
    let mut out = truncated;
    if visible_width(&out) < width {
        out.push_str(elide);
    }
    out
}

struct ComposedLine {
    left: String,
    right: String,
    visible_width: usize,
}

fn compose(
    tagged: &[(SegmentSlot, StatusFragment)],
    separator: &str,
    sep_between_sides: usize,
) -> ComposedLine {
    let join = |slot: SegmentSlot| -> String {
        tagged
            .iter()
            .filter(|(s, _)| *s == slot)
            .map(|(_, f)| f.text.as_str())
            .collect::<Vec<_>>()
            .join(separator)
    };
    let left = join(SegmentSlot::Left);
    let right = join(SegmentSlot::Right);
    let mut visible_width = visible_width(&left) + visible_width(&right);
    if !left.is_empty() && !right.is_empty() {
        visible_width += sep_between_sides;
    }
    ComposedLine {
        left,
        right,
        visible_width,
    }
}

pub(crate) fn visible_width(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestCtx {
        zoom: f32,
    }

    #[derive(Debug)]
    struct AppName;
    impl Segment<TestCtx> for AppName {
        fn id(&self) -> &'static str {
            "app_name"
        }
        fn render(&self, _: &TestCtx) -> Option<StatusFragment> {
            Some(StatusFragment::new("kit").with_priority(255))
        }
    }

    #[derive(Debug)]
    struct Zoom;
    impl Segment<TestCtx> for Zoom {
        fn id(&self) -> &'static str {
            "zoom"
        }
        fn render(&self, ctx: &TestCtx) -> Option<StatusFragment> {
            Some(StatusFragment::new(format!("{:.0}%", ctx.zoom * 100.0)).with_priority(180))
        }
    }

    #[derive(Debug)]
    struct Hint;
    impl Segment<TestCtx> for Hint {
        fn id(&self) -> &'static str {
            "hint"
        }
        fn render(&self, _: &TestCtx) -> Option<StatusFragment> {
            Some(StatusFragment::new("press q to quit").with_priority(40))
        }
    }

    fn bar() -> SegmentBar<TestCtx> {
        SegmentBar::builder()
            .add(SegmentSlot::Left, AppName)
            .add(SegmentSlot::Left, Zoom)
            .add(SegmentSlot::Right, Hint)
            .build()
    }

    #[test]
    fn renders_full_width() {
        let line = bar().render(&TestCtx { zoom: 1.0 }, 80);
        assert_eq!(line.chars().count(), 80);
        assert!(line.starts_with("kit"));
        assert!(line.trim_end().ends_with("press q to quit"));
    }

    #[test]
    fn drops_lowest_priority_when_narrow() {
        let line = bar().render(&TestCtx { zoom: 1.0 }, 12);
        assert!(line.contains("kit"));
        assert!(!line.contains("press q to quit"));
    }
}
