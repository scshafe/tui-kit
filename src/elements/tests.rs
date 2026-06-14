use super::*;
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::component::{BufferComponent, ComponentId, ComponentOutcome, DirtyReason, DirtyState};
use crate::focus::{FocusNode, FocusScopeKind};
use crate::image::PlaceOptions;
use crate::input::KeyEvent;
use crate::keymap::KeyMap;
use crate::keymap::{KeyTrigger, SpecialKey};
use crate::layout::{CanvasMetrics, CellOffset, CellPixel, CellSize, PixelRect, PixelSize};
use crate::testkit::{assert_teardown_covers, find_place_with_placement_id, render_to_buffer};
use crate::widgets::image_viewport::{ImageViewport, ImageViewportWidget, ViewportImage};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestCommand {
    Window,
    Child,
    Modal,
    Base,
    Top,
}

type SharedAreas = std::rc::Rc<std::cell::RefCell<Vec<Rect>>>;
type SharedDirtyEvents = std::rc::Rc<std::cell::RefCell<Vec<DirtyReason>>>;
type SharedTeardowns = std::rc::Rc<std::cell::RefCell<u32>>;

#[derive(Debug, Clone)]
struct ProbeElement {
    id: ComponentId,
    marker: &'static str,
    keymap: KeyMap<TestCommand>,
    dirty: DirtyState,
}

impl ProbeElement {
    fn new(id: &'static str, marker: &'static str) -> Self {
        Self {
            id: ComponentId::new(id),
            marker,
            keymap: KeyMap::new(),
            dirty: DirtyState::paint(DirtyReason::Explicit),
        }
    }

    fn with_binding(mut self, trigger: char, command: TestCommand) -> Self {
        self.keymap.bind(KeyTrigger::Char(trigger), command);
        self
    }
}

impl BufferComponent for ProbeElement {
    type Event = KeyEvent;
    type Message = TestCommand;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        buffer.set_stringn(
            area.x,
            area.y,
            self.marker,
            usize::from(area.width),
            Style::default(),
        );
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        Ok(self
            .keymap
            .lookup(*event)
            .map(ComponentOutcome::Message)
            .unwrap_or(ComponentOutcome::Ignored))
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark_paint(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
    }
}

#[derive(Debug, Clone)]
struct AreaProbeElement {
    id: ComponentId,
    marker: &'static str,
    areas: SharedAreas,
    dirty_events: SharedDirtyEvents,
    keymap: KeyMap<TestCommand>,
    dirty: DirtyState,
}

impl AreaProbeElement {
    fn new(id: &'static str, marker: &'static str) -> (Self, SharedAreas, SharedDirtyEvents) {
        let areas = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let dirty_events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        (
            Self {
                id: ComponentId::new(id),
                marker,
                areas: std::rc::Rc::clone(&areas),
                dirty_events: std::rc::Rc::clone(&dirty_events),
                keymap: KeyMap::new(),
                dirty: DirtyState::paint(DirtyReason::Explicit),
            },
            areas,
            dirty_events,
        )
    }
}

impl BufferComponent for AreaProbeElement {
    type Event = KeyEvent;
    type Message = TestCommand;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.areas.borrow_mut().push(area);
        if area.width > 0 && area.height > 0 {
            buffer.set_stringn(
                area.x,
                area.y,
                self.marker,
                usize::from(area.width),
                Style::default(),
            );
        }
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        Ok(self
            .keymap
            .lookup(*event)
            .map(ComponentOutcome::Message)
            .unwrap_or(ComponentOutcome::Ignored))
    }

    fn dirty(&self) -> &DirtyState {
        &self.dirty
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty_events.borrow_mut().push(reason.clone());
        self.dirty.mark_paint(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
    }
}

#[derive(Debug, Clone)]
struct EffectProbeElement {
    id: ComponentId,
    placement_id: u32,
    areas: SharedAreas,
    teardowns: SharedTeardowns,
    dirty: DirtyState,
}

impl EffectProbeElement {
    fn new(id: &'static str) -> (Self, SharedAreas, SharedTeardowns) {
        let areas = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let teardowns = std::rc::Rc::new(std::cell::RefCell::new(0));
        (
            Self {
                id: ComponentId::new(id),
                placement_id: 42,
                areas: std::rc::Rc::clone(&areas),
                teardowns: std::rc::Rc::clone(&teardowns),
                dirty: DirtyState::paint(DirtyReason::Explicit),
            },
            areas,
            teardowns,
        )
    }

    fn with_placement_id(mut self, placement_id: u32) -> Self {
        self.placement_id = placement_id;
        self
    }
}

impl BufferComponent for EffectProbeElement {
    type Event = KeyEvent;
    type Message = TestCommand;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, _area: Rect, _buffer: &mut Buffer) -> Result<()> {
        self.clear_dirty();
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

impl EffectElement for EffectProbeElement {
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        self.areas.borrow_mut().push(area);
        Ok(vec![RenderEffect::PlaceImage {
            origin: CellOffset {
                col: area.x,
                row: area.y,
            },
            options: PlaceOptions {
                image_id: 1,
                placement_id: self.placement_id,
                source: PixelRect {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
                cell_cols: area.width,
                cell_rows: area.height,
            },
        }])
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        *self.teardowns.borrow_mut() += 1;
        Ok(vec![RenderEffect::DeletePlacement {
            placement_id: self.placement_id,
        }])
    }
}

#[derive(Debug, Clone)]
struct ToggleEffectProbeElement {
    id: ComponentId,
    delete_next: std::rc::Rc<std::cell::Cell<bool>>,
    dirty: DirtyState,
}

impl ToggleEffectProbeElement {
    fn new(id: &'static str) -> (Self, std::rc::Rc<std::cell::Cell<bool>>) {
        let delete_next = std::rc::Rc::new(std::cell::Cell::new(false));
        (
            Self {
                id: ComponentId::new(id),
                delete_next: std::rc::Rc::clone(&delete_next),
                dirty: DirtyState::paint(DirtyReason::Explicit),
            },
            delete_next,
        )
    }
}

impl BufferComponent for ToggleEffectProbeElement {
    type Event = KeyEvent;
    type Message = TestCommand;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, _area: Rect, _buffer: &mut Buffer) -> Result<()> {
        self.clear_dirty();
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

impl EffectElement for ToggleEffectProbeElement {
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        if self.delete_next.get() {
            return Ok(vec![RenderEffect::DeleteImagePlacement {
                image_id: 3,
                placement_id: 4,
            }]);
        }
        Ok(vec![RenderEffect::PlaceImage {
            origin: CellOffset {
                col: area.x,
                row: area.y,
            },
            options: PlaceOptions {
                image_id: 3,
                placement_id: 4,
                source: PixelRect {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
                cell_cols: area.width,
                cell_rows: area.height,
            },
        }])
    }
}

fn command_map(trigger: char, command: TestCommand) -> KeyMap<TestCommand> {
    let mut keymap = KeyMap::new();
    keymap.bind(KeyTrigger::Char(trigger), command);
    keymap
}

#[test]
fn text_wraps_and_truncates() -> Result<()> {
    let mut text = Text::with_id("body", "abcdef").truncate(true);
    let buffer = render_to_buffer(&mut text, Rect::new(0, 0, 4, 1))?;

    assert!(format!("{buffer:?}").contains("a..."));
    assert!(text.dirty().is_clean());
    Ok(())
}

#[test]
fn panel_calculates_child_area_from_border_and_padding() {
    let panel = Panel::new("panel", Text::new("child")).padding((2, 1));

    assert_eq!(
        panel.child_area(Rect::new(0, 0, 20, 8)),
        Rect::new(3, 2, 14, 4)
    );
}

#[test]
fn stack_distributes_fill_space() {
    let stack: Stack<()> = Stack::vertical("stack")
        .with_child(Text::with_id("a", "a"), StackConstraint::Length(1))
        .with_child(Text::with_id("b", "b"), StackConstraint::Fill(1))
        .with_child(Text::with_id("c", "c"), StackConstraint::Fill(1));

    let areas = stack.layout_areas(Rect::new(0, 0, 10, 5));

    assert_eq!(areas[0].height, 1);
    assert_eq!(areas[1].height, 2);
    assert_eq!(areas[2].height, 2);
}

#[test]
fn stack_min_constraints_reserve_then_grow_with_remaining_space() {
    let stack: Stack<()> = Stack::vertical("stack")
        .with_child(Text::with_id("a", "a"), StackConstraint::Min(1))
        .with_child(Text::with_id("b", "b"), StackConstraint::Fill(1));

    let areas = stack.layout_areas(Rect::new(0, 0, 10, 5));

    assert_eq!(areas[0].height, 3);
    assert_eq!(areas[1].height, 2);
}

#[test]
fn stack_forwards_effect_children_through_layout_areas() -> Result<()> {
    let (effect, effect_areas, teardowns) = EffectProbeElement::new("effect");
    let mut stack = Stack::vertical("stack")
        .with_child(
            ProbeElement::new("buffer", "buffer"),
            StackConstraint::Length(1),
        )
        .with_effect_child(effect, StackConstraint::Length(2));

    let effects = stack.render_effects(Rect::new(3, 4, 10, 6))?;

    assert_eq!(effect_areas.borrow().as_slice(), &[Rect::new(3, 5, 10, 2)]);
    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        RenderEffect::PlaceImage { origin, options }
            if *origin == CellOffset { col: 3, row: 5 }
                && options.cell_cols == 10
                && options.cell_rows == 2
    ));
    assert_eq!(
        stack.teardown_effects()?,
        vec![RenderEffect::DeletePlacement { placement_id: 42 }]
    );
    assert_eq!(*teardowns.borrow(), 1);
    Ok(())
}

#[test]
fn stack_tears_down_effect_children_in_reverse_order() -> Result<()> {
    let (first, _first_areas, _first_teardowns) = EffectProbeElement::new("first");
    let (second, _second_areas, _second_teardowns) = EffectProbeElement::new("second");
    let mut stack: Stack<TestCommand> = Stack::horizontal("stack")
        .with_effect_child(first.with_placement_id(1), StackConstraint::Length(1))
        .with_effect_child(second.with_placement_id(2), StackConstraint::Length(1));

    assert_eq!(
        stack.teardown_effects()?,
        vec![
            RenderEffect::DeletePlacement { placement_id: 2 },
            RenderEffect::DeletePlacement { placement_id: 1 },
        ]
    );
    Ok(())
}

#[test]
fn stack_teardown_covers_every_rendered_placement() -> Result<()> {
    let (first, _, _) = EffectProbeElement::new("first");
    let (second, _, _) = EffectProbeElement::new("second");
    let mut stack: Stack<TestCommand> = Stack::horizontal("stack")
        .with_effect_child(first.with_placement_id(11), StackConstraint::Length(2))
        .with_effect_child(second.with_placement_id(22), StackConstraint::Length(2));

    let placed = stack.render_effects(Rect::new(0, 0, 4, 4))?;
    let teardown = stack.teardown_effects()?;

    assert_teardown_covers(&placed, &teardown);
    Ok(())
}

#[test]
fn scroll_y_offsets_buffer_output_and_marks_child_dirty() -> Result<()> {
    let mut element = Text::with_id("body", "one\ntwo\nthree").scroll_y();
    element.scroll_to(1);
    let buffer = render_to_buffer(&mut element, Rect::new(0, 0, 8, 2))?;

    let rendered = format!("{buffer:?}");
    assert!(rendered.contains("two"));
    assert!(rendered.contains("three"));
    assert!(!rendered.contains("one"));
    assert!(element.dirty().is_clean());
    Ok(())
}

#[test]
fn scroll_y_forwards_key_events_to_child() -> Result<()> {
    let mut element = ProbeElement::new("probe", "probe")
        .with_binding('x', TestCommand::Child)
        .scroll_y();

    assert_eq!(
        element.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Message(TestCommand::Child)
    );
    Ok(())
}

#[test]
fn scroll_y_dirty_propagates_to_child() {
    let (child, _areas, dirty_events) = AreaProbeElement::new("child", "child");
    let mut element = child.scroll_y();

    element.scroll_by(2);

    assert!(element.dirty().paint_dirty());
    assert_eq!(dirty_events.borrow().as_slice(), &[DirtyReason::Input]);
}

#[test]
fn focusable_exposes_stable_focus_node_and_forwards_child() -> Result<()> {
    let mut element = ProbeElement::new("probe", "probe")
        .with_binding('x', TestCommand::Child)
        .focusable()
        .with_focus_id("field")
        .enabled(false)
        .visible(true);

    let node = element.focus_node().expect("focus node");
    assert_eq!(node.id.as_str(), "field");
    assert!(!node.focusable());
    assert_eq!(
        element.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Message(TestCommand::Child)
    );
    Ok(())
}

#[test]
fn focusable_node_flows_through_window() {
    let window = Window::new(
        "window",
        Text::with_id("body", "body")
            .focusable()
            .with_focus_id("body-focus"),
    );

    let node = window.focus_node().expect("focus node");
    assert_eq!(node.id.as_str(), "body-focus");
}

#[test]
fn keymap_decorator_uses_child_first_then_local_binding() -> Result<()> {
    let mut local = KeyMap::new();
    local.bind(KeyTrigger::Char('x'), TestCommand::Window);
    local.bind(KeyTrigger::Char('y'), TestCommand::Window);
    let mut element = ProbeElement::new("probe", "probe")
        .with_binding('x', TestCommand::Child)
        .with_keymap(local);

    assert_eq!(
        element.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Message(TestCommand::Child)
    );
    assert_eq!(
        element.handle_event(&KeyEvent::Char('y'))?,
        ComponentOutcome::Message(TestCommand::Window)
    );
    assert_eq!(
        element.handle_event(&KeyEvent::Char('z'))?,
        ComponentOutcome::Ignored
    );
    Ok(())
}

#[test]
fn padded_decorator_calculates_child_area_and_marks_layout_dirty() -> Result<()> {
    let (child, areas, dirty_events) = AreaProbeElement::new("child", "child");
    let mut element = child.with_padding((2, 1));
    let area = Rect::new(0, 0, 10, 5);
    let mut buffer = Buffer::empty(area);

    assert_eq!(element.child_area(area), Rect::new(2, 1, 6, 3));
    element.mark_dirty(DirtyReason::Resize);
    element.render_buffer(area, &mut buffer)?;

    assert_eq!(areas.borrow().as_slice(), &[Rect::new(2, 1, 6, 3)]);
    assert_eq!(dirty_events.borrow().as_slice(), &[DirtyReason::Resize]);
    assert!(element.dirty().is_clean());
    Ok(())
}

#[test]
fn bordered_decorator_renders_border_and_calculates_child_area() -> Result<()> {
    let (child, areas, _dirty_events) = AreaProbeElement::new("child", "child");
    let mut element = child.with_border(ElementBorder::new().title("Title"));
    let area = Rect::new(0, 0, 12, 3);

    assert_eq!(element.child_area(area), Rect::new(1, 1, 10, 1));
    let buffer = render_to_buffer(&mut element, area)?;

    let rendered = format!("{buffer:?}");
    assert!(rendered.contains("Title"));
    assert_eq!(areas.borrow().as_slice(), &[Rect::new(1, 1, 10, 1)]);
    Ok(())
}

#[test]
fn nested_padding_and_border_apply_in_wrapper_order() -> Result<()> {
    let (child, areas, _dirty_events) = AreaProbeElement::new("child", "child");
    let mut element = child.with_padding(1).with_border(());
    let _ = render_to_buffer(&mut element, Rect::new(0, 0, 10, 5))?;

    assert_eq!(areas.borrow().as_slice(), &[Rect::new(2, 2, 6, 1)]);
    Ok(())
}

#[test]
fn padding_and_border_decorators_handle_zero_sized_area() -> Result<()> {
    let (child, areas, _dirty_events) = AreaProbeElement::new("child", "child");
    let mut element = child.with_padding(2).with_border(());
    let area = Rect::new(0, 0, 0, 0);
    let mut buffer = Buffer::empty(area);

    element.render_buffer(area, &mut buffer)?;

    assert_eq!(areas.borrow().as_slice(), &[Rect::new(0, 0, 0, 0)]);
    Ok(())
}

#[test]
fn padding_and_border_forward_effects_to_transformed_child_area() -> Result<()> {
    let (child, areas, teardowns) = EffectProbeElement::new("effect");
    let mut element = child.with_padding(1).with_border(());

    let effects = element.render_effects(Rect::new(10, 5, 10, 5))?;

    assert_eq!(areas.borrow().as_slice(), &[Rect::new(12, 7, 6, 1)]);
    assert!(matches!(
        &effects[0],
        RenderEffect::PlaceImage { origin, options }
            if *origin == CellOffset { col: 12, row: 7 }
                && options.cell_cols == 6
                && options.cell_rows == 1
    ));
    assert_eq!(
        element.teardown_effects()?,
        vec![RenderEffect::DeletePlacement { placement_id: 42 }]
    );
    assert_eq!(*teardowns.borrow(), 1);
    Ok(())
}

#[test]
fn focusable_and_keymapped_forward_effects_without_area_change() -> Result<()> {
    let (child, areas, _teardowns) = EffectProbeElement::new("effect");
    let mut element = child.focusable().with_keymap(KeyMap::new());

    let effects = element.render_effects(Rect::new(3, 4, 5, 6))?;

    assert_eq!(areas.borrow().as_slice(), &[Rect::new(3, 4, 5, 6)]);
    assert!(matches!(
        &effects[0],
        RenderEffect::PlaceImage { origin, options }
            if *origin == CellOffset { col: 3, row: 4 }
                && options.cell_cols == 5
                && options.cell_rows == 6
    ));
    Ok(())
}

#[test]
fn panel_forwards_effects_to_presentational_child_area() -> Result<()> {
    let (child, areas, _teardowns) = EffectProbeElement::new("effect");
    let mut panel = Panel::new("panel", child).padding(1);

    let effects = panel.render_effects(Rect::new(0, 0, 10, 5))?;

    assert_eq!(areas.borrow().as_slice(), &[Rect::new(2, 2, 6, 1)]);
    assert!(matches!(
        &effects[0],
        RenderEffect::PlaceImage { origin, options }
            if *origin == CellOffset { col: 2, row: 2 }
                && options.cell_cols == 6
                && options.cell_rows == 1
    ));
    Ok(())
}

#[test]
fn modal_forwards_window_effects_and_grouped_teardown() -> Result<()> {
    let (child, areas, teardowns) = EffectProbeElement::new("effect");
    let mut modal = Modal::new("modal", child);

    let effects = modal.render_effects(Rect::new(2, 3, 4, 5))?;

    assert_eq!(areas.borrow().as_slice(), &[Rect::new(2, 3, 4, 5)]);
    assert!(matches!(
        &effects[0],
        RenderEffect::PlaceImage { origin, options }
            if *origin == CellOffset { col: 2, row: 3 }
                && options.placement_id == 42
    ));
    assert!(modal
        .window()
        .registered_effect_placements()
        .contains(&(1, 42)));

    assert_eq!(
        modal.teardown_effects()?,
        vec![RenderEffect::DeletePlacement { placement_id: 42 }]
    );
    assert_eq!(*teardowns.borrow(), 1);
    assert!(modal.window().registered_effect_placements().is_empty());
    Ok(())
}

#[test]
fn overlay_forwards_effect_layers_using_layer_areas() -> Result<()> {
    let (effect, areas, _teardowns) = EffectProbeElement::new("effect");
    let mut overlay = Overlay::new("overlay")
        .with_layer(ProbeElement::new("buffer", "buffer"), None)
        .with_effect_layer(effect, Some(Rect::new(5, 6, 7, 8)));

    let effects = overlay.render_effects(Rect::new(0, 0, 20, 10))?;

    assert_eq!(areas.borrow().as_slice(), &[Rect::new(5, 6, 7, 8)]);
    assert_eq!(effects.len(), 1);
    assert!(matches!(
        &effects[0],
        RenderEffect::PlaceImage { origin, options }
            if *origin == CellOffset { col: 5, row: 6 }
                && options.cell_cols == 7
                && options.cell_rows == 8
    ));
    Ok(())
}

#[test]
fn overlay_tears_down_effect_layers_top_down() -> Result<()> {
    let (base, _base_areas, _base_teardowns) = EffectProbeElement::new("base");
    let (top, _top_areas, _top_teardowns) = EffectProbeElement::new("top");
    let mut overlay: Overlay<TestCommand> = Overlay::new("overlay")
        .with_effect_layer(base.with_placement_id(1), None)
        .with_modal_effect_layer(top.with_placement_id(2), None);

    assert_eq!(
        overlay.teardown_effects()?,
        vec![
            RenderEffect::DeletePlacement { placement_id: 2 },
            RenderEffect::DeletePlacement { placement_id: 1 },
        ]
    );
    Ok(())
}

#[test]
fn window_hooks_fire_in_activation_order() {
    use std::cell::RefCell;
    use std::rc::Rc;

    let events = Rc::new(RefCell::new(Vec::new()));
    let mut window = Window::new("window", Text::new("body"))
        .on_prefetch({
            let events = Rc::clone(&events);
            move |event| events.borrow_mut().push(event)
        })
        .on_enter({
            let events = Rc::clone(&events);
            move |event| events.borrow_mut().push(event)
        })
        .on_focus({
            let events = Rc::clone(&events);
            move |event| events.borrow_mut().push(event)
        });

    window.activate();

    assert_eq!(
        *events.borrow(),
        vec![
            WindowLifecycleEvent::Prefetch,
            WindowLifecycleEvent::Enter,
            WindowLifecycleEvent::Focus,
        ]
    );
}

#[test]
fn window_deactivation_blurs_before_exit() {
    use std::cell::RefCell;
    use std::rc::Rc;

    let events = Rc::new(RefCell::new(Vec::new()));
    let mut window = Window::new("window", Text::new("body"))
        .on_blur({
            let events = Rc::clone(&events);
            move |event| events.borrow_mut().push(event)
        })
        .on_exit({
            let events = Rc::clone(&events);
            move |event| events.borrow_mut().push(event)
        });

    window.activate();
    window.deactivate();

    assert_eq!(
        *events.borrow(),
        vec![WindowLifecycleEvent::Blur, WindowLifecycleEvent::Exit]
    );
    assert!(!window.is_active());
    assert!(!window.is_focused());
    assert!(!window.has_entered());
}

#[test]
fn window_resize_hook_fires_only_when_area_changes() -> Result<()> {
    use std::cell::RefCell;
    use std::rc::Rc;

    let first = Rect::new(0, 0, 10, 1);
    let second = Rect::new(0, 0, 12, 1);
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut window = Window::new("window", Text::new("body")).on_resize({
        let events = Rc::clone(&events);
        move |event| events.borrow_mut().push(event)
    });
    let mut buffer = Buffer::empty(second);

    window.render_buffer(first, &mut buffer)?;
    window.render_buffer(first, &mut buffer)?;
    window.render_buffer(second, &mut buffer)?;

    assert_eq!(
        *events.borrow(),
        vec![
            WindowLifecycleEvent::Resize {
                old: None,
                new: first,
            },
            WindowLifecycleEvent::Resize {
                old: Some(first),
                new: second,
            },
        ]
    );
    Ok(())
}

#[test]
fn window_key_scope_participates_only_while_active_or_focused() -> Result<()> {
    let child = ProbeElement::new("child", "child").with_binding('x', TestCommand::Child);
    let mut window =
        Window::new("window", child).with_keymap(command_map('x', TestCommand::Window));

    assert_eq!(
        window.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Ignored
    );

    window.activate();
    assert_eq!(
        window.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Message(TestCommand::Child)
    );

    window.blur();
    assert_eq!(
        window.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Message(TestCommand::Window)
    );

    window.deactivate();
    assert_eq!(
        window.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Ignored
    );
    Ok(())
}

#[test]
fn window_focus_scope_metadata_is_declarative() {
    let window = Window::new("window", Text::new("body")).with_focus_scope(
        FocusScopeKind::Capturing,
        [FocusNode::new("field"), FocusNode::new("button")],
    );

    let scope = window.focus_scope();
    assert_eq!(scope.id.as_str(), "window");
    assert_eq!(scope.kind, FocusScopeKind::Capturing);
    assert_eq!(scope.nodes.len(), 2);
    assert_eq!(scope.nodes[0].id.as_str(), "field");
}

#[test]
fn window_groups_effect_teardown_without_duplicate_child_teardown() -> Result<()> {
    let image = ViewportImage::new(PixelSize::new(2, 2), vec![255; 16])?;
    let widget = ImageViewportWidget::new(
        ImageViewport::new(image),
        CanvasMetrics::new(CellSize::new(4, 2), CellPixel::new(1, 1)),
    );
    let image = ImageViewportElement::new("image", 7, 9, widget).with_png(b"png".to_vec());
    let mut window = Window::new("window", image);

    let effects = window.render_effects(Rect::new(0, 0, 4, 2))?;
    assert!(find_place_with_placement_id(&effects, 9).is_some());
    assert!(window.registered_effect_placements().contains(&(7, 9)));

    let teardown = window.teardown_effects()?;
    assert_eq!(
        teardown,
        vec![RenderEffect::DeleteImagePlacement {
            image_id: 7,
            placement_id: 9,
        }]
    );
    assert!(window.registered_effect_placements().is_empty());
    Ok(())
}

#[test]
fn window_prunes_effect_tracking_when_child_deletes_during_render() -> Result<()> {
    let (child, delete_next) = ToggleEffectProbeElement::new("effect");
    let mut window = Window::new("window", child);

    window.render_effects(Rect::new(0, 0, 4, 2))?;
    assert!(window.registered_effect_placements().contains(&(3, 4)));

    delete_next.set(true);
    let effects = window.render_effects(Rect::new(0, 0, 4, 2))?;

    assert_eq!(
        effects,
        vec![RenderEffect::DeleteImagePlacement {
            image_id: 3,
            placement_id: 4,
        }]
    );
    assert!(window.registered_effect_placements().is_empty());
    assert!(window.teardown_effects()?.is_empty());
    Ok(())
}

#[test]
fn window_fallback_teardown_uses_image_placement_pair() -> Result<()> {
    let (child, _delete_next) = ToggleEffectProbeElement::new("effect");
    let mut window = Window::new("window", child);

    window.render_effects(Rect::new(0, 0, 4, 2))?;

    assert_eq!(
        window.teardown_effects()?,
        vec![RenderEffect::DeleteImagePlacement {
            image_id: 3,
            placement_id: 4,
        }]
    );
    assert!(window.registered_effect_placements().is_empty());
    Ok(())
}

#[test]
fn child_cached_window_reuses_clean_child_buffer() -> Result<()> {
    let mut window = Window::new("window", Text::new("cached"))
        .with_repaint_policy(WindowRepaintPolicy::ChildCached);
    let area = Rect::new(0, 0, 12, 1);
    let mut first = Buffer::empty(area);
    let mut second = Buffer::empty(area);

    window.render_buffer(area, &mut first)?;
    window.render_buffer(area, &mut second)?;

    assert_eq!(window.stats().child_cache_misses, 1);
    assert_eq!(window.stats().child_cache_hits, 1);
    Ok(())
}

#[test]
fn key_scope_resolver_uses_modal_child_window_root_precedence() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Cmd {
        Root,
        Window,
        Child,
        Modal,
    }

    fn map(trigger: char, cmd: Cmd) -> KeyMap<Cmd> {
        let mut map = KeyMap::new();
        map.bind(KeyTrigger::Char(trigger), cmd);
        map
    }

    let resolver = KeyScopeResolver::new(map('x', Cmd::Root))
        .with_scope(
            KeyScope::new("window", KeyScopeRole::Window, map('x', Cmd::Window)).active(true),
        )
        .with_scope(
            KeyScope::new("child", KeyScopeRole::FocusedChild, map('x', Cmd::Child))
                .active(true)
                .focused(true),
        )
        .with_scope(
            KeyScope::new("modal", KeyScopeRole::ModalWindow, map('x', Cmd::Modal))
                .active(true)
                .focused(true),
        );

    assert_eq!(
        resolver.resolve(KeyEvent::Char('x')).unwrap().command,
        Cmd::Modal
    );
}

#[test]
fn image_viewport_element_separates_buffer_render_and_effects() -> Result<()> {
    let image = ViewportImage::new(PixelSize::new(2, 2), vec![255; 16])?;
    let widget = ImageViewportWidget::new(
        ImageViewport::new(image),
        CanvasMetrics::new(CellSize::new(4, 2), CellPixel::new(1, 1)),
    );
    let mut element = ImageViewportElement::new("image", 7, 9, widget).with_png(b"png".to_vec());

    let effects = element.render_effects(Rect::new(10, 3, 4, 2))?;

    assert!(matches!(
        &effects[0],
        RenderEffect::EnsureImageLoaded { image_id: 7, .. }
    ));
    assert!(matches!(
        &effects[1],
        RenderEffect::PlaceImage { origin, options }
            if *origin == CellOffset { col: 10, row: 3 }
                && options.image_id == 7
                && options.placement_id == 9
    ));
    Ok(())
}

#[test]
fn modal_activates_as_modal_focus_boundary() {
    let modal = Modal::new("modal", Text::new("body"));
    let scope = modal.window().focus_scope();

    assert!(modal.window().is_active());
    assert!(modal.window().is_focused());
    assert!(modal.window().has_entered());
    assert_eq!(scope.kind, FocusScopeKind::Modal);
    assert_eq!(scope.id.as_str(), "modal");
}

#[test]
fn modal_from_window_allows_hooks_before_activation() {
    use std::cell::RefCell;
    use std::rc::Rc;

    let events = Rc::new(RefCell::new(Vec::new()));
    let window = Window::new("modal", Text::new("body")).on_enter({
        let events = Rc::clone(&events);
        move |event| events.borrow_mut().push(event)
    });

    let modal = Modal::from_window(window);

    assert!(modal.window().is_active());
    assert_eq!(*events.borrow(), vec![WindowLifecycleEvent::Enter]);
}

#[test]
fn modal_routes_keys_to_focused_child() -> Result<()> {
    let child = ProbeElement::new("child", "child").with_binding('m', TestCommand::Modal);
    let mut modal = Modal::new("modal", child);

    assert_eq!(
        modal.handle_event(&KeyEvent::Char('m'))?,
        ComponentOutcome::Message(TestCommand::Modal)
    );
    Ok(())
}

#[test]
fn overlay_renders_layers_in_order() -> Result<()> {
    let mut overlay = Overlay::new("overlay")
        .with_layer(ProbeElement::new("base", "base"), None)
        .with_layer(ProbeElement::new("top", "top!"), None);
    let area = Rect::new(0, 0, 4, 1);
    let mut buffer = Buffer::empty(area);

    overlay.render_buffer(area, &mut buffer)?;

    assert!(format!("{buffer:?}").contains("top!"));
    Ok(())
}

#[test]
fn overlay_routes_to_topmost_non_modal_layer() -> Result<()> {
    let mut overlay = Overlay::new("overlay")
        .with_layer(
            ProbeElement::new("base", "base").with_binding('x', TestCommand::Base),
            None,
        )
        .with_layer(
            ProbeElement::new("top", "top").with_binding('x', TestCommand::Top),
            None,
        );

    assert_eq!(
        overlay.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Message(TestCommand::Top)
    );
    Ok(())
}

#[test]
fn overlay_modal_layer_captures_key_routing() -> Result<()> {
    let mut overlay = Overlay::new("overlay")
        .with_layer(
            ProbeElement::new("base", "base").with_binding('x', TestCommand::Base),
            None,
        )
        .with_modal_layer(
            ProbeElement::new("modal", "modal").with_binding('m', TestCommand::Modal),
            None,
        );

    assert_eq!(
        overlay.handle_event(&KeyEvent::Char('x'))?,
        ComponentOutcome::Ignored
    );
    assert_eq!(
        overlay.handle_event(&KeyEvent::Char('m'))?,
        ComponentOutcome::Message(TestCommand::Modal)
    );
    Ok(())
}

#[test]
fn key_mapped_decorator_emits_message() -> Result<()> {
    let mut map = KeyMap::new();
    map.bind(KeyTrigger::Special(SpecialKey::Enter), ());
    let mut element = Text::new("ok").with_keymap(map);

    assert!(element.handle_event(&KeyEvent::Enter)?.is_handled());
    Ok(())
}
