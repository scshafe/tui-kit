//! Slot-aligned, priority-truncated text bars.
//!
//! tui-kit intentionally keeps this module at the data/algorithm layer for
//! now: c4tui's status contexts borrow per-render application state, so a
//! stored generic segment registry would force the wrong lifetime shape. Apps
//! own their segment traits and can share [`StatusFragment`], [`SegmentSlot`],
//! and [`layout_status_line`] for deterministic composition.
//!
//! **Stability:** consumed by c4tui's status bar. Public scope is deliberately
//! limited to the fragment/slot data and layout algorithm c4tui shares; the
//! removed segment registry should re-enter only with a consumer whose context
//! shape actually fits storage in tui-kit.

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

/// Compose a left/right status line, dropping the lowest-priority fragment
/// across both sides until the line fits `width`.
pub fn layout_status_line(
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

    #[test]
    fn renders_full_width() {
        let line = layout_status_line(
            vec![
                StatusFragment::new("kit").with_priority(255),
                StatusFragment::new("100%").with_priority(180),
            ],
            vec![StatusFragment::new("press q to quit").with_priority(40)],
            80,
            " | ",
            "…",
        );
        assert_eq!(line.chars().count(), 80);
        assert!(line.starts_with("kit"));
        assert!(line.trim_end().ends_with("press q to quit"));
    }

    #[test]
    fn drops_lowest_priority_when_narrow() {
        let line = layout_status_line(
            vec![
                StatusFragment::new("kit").with_priority(255),
                StatusFragment::new("100%").with_priority(180),
            ],
            vec![StatusFragment::new("press q to quit").with_priority(40)],
            12,
            " | ",
            "…",
        );
        assert!(line.contains("kit"));
        assert!(!line.contains("press q to quit"));
    }
}
