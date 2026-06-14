use std::collections::BTreeSet;
use std::fmt;

use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Widget};
use serde::{Deserialize, Serialize};

use crate::component::{
    BufferComponent, ComponentChildren, ComponentId, ComponentOutcome, DirtyReason, DirtyState,
};
use crate::focus::{FocusId, FocusNode, FocusScopeKind};
use crate::input::KeyEvent;
use crate::keymap::KeyMap;

use super::core::{ContainerElement, Element, ElementOutcome, Padding};
use super::effect::{EffectElement, RenderEffect};

/// Window repaint strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowRepaintPolicy {
    Whole,
    ChildCached,
}

/// Inspectable window render counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowRenderStats {
    pub whole_repaints: u64,
    pub child_cache_hits: u64,
    pub child_cache_misses: u64,
    pub last_repaint_area: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowLifecycleEvent {
    Prefetch,
    Enter,
    Exit,
    Focus,
    Blur,
    Resize { old: Option<Rect>, new: Rect },
}

type WindowHook = Box<dyn FnMut(WindowLifecycleEvent)>;

#[derive(Default)]
struct WindowHooks {
    prefetch: Vec<WindowHook>,
    enter: Vec<WindowHook>,
    exit: Vec<WindowHook>,
    focus: Vec<WindowHook>,
    blur: Vec<WindowHook>,
    resize: Vec<WindowHook>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFocusScope {
    pub id: FocusId,
    pub kind: FocusScopeKind,
    pub nodes: Vec<FocusNode>,
}

/// Optional panel-like chrome for a behavioral window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowChrome {
    pub title: Option<String>,
    pub footer: Option<String>,
    pub border: bool,
    pub padding: Padding,
    pub border_style: Style,
}

impl WindowChrome {
    pub fn new() -> Self {
        Self {
            title: None,
            footer: None,
            border: false,
            padding: Padding::ZERO,
            border_style: Style::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn border(mut self, enabled: bool) -> Self {
        self.border = enabled;
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    fn block(&self) -> Block<'_> {
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style);
        if let Some(title) = &self.title {
            block = block.title(format!(" {} ", title.trim()));
        }
        if let Some(footer) = &self.footer {
            block = block.title_bottom(footer.clone());
        }
        block
    }

    fn render(&self, area: Rect, buffer: &mut Buffer) {
        if self.border {
            self.block().render(area, buffer);
        }
    }

    fn child_area(&self, area: Rect) -> Rect {
        let area = if self.border {
            self.block().inner(area)
        } else {
            area
        };
        self.padding.inner(area)
    }
}

impl Default for WindowChrome {
    fn default() -> Self {
        Self::new()
    }
}

/// Behavioral lifecycle/key/effect boundary around a child element.
///
/// `active` means the window participates in its local key scope. `focused`
/// means key input is offered to the child before the window keymap. `entered`
/// means the enter lifecycle hook has fired and the matching exit hook has not.
///
/// `activate` fires prefetch, enter, then focus when needed. `deactivate`
/// fires blur before exit. The lower-level `focus`, `blur`, `enter`, and
/// `exit` methods are public so callers can model unusual focus transitions;
/// as a result, `focused && !active` is valid and routes child-first key input.
pub struct Window<E>
where
    E: Element,
    E::Message: Clone,
{
    id: ComponentId,
    child: E,
    child_ids: Vec<ComponentId>,
    dirty: DirtyState,
    repaint_policy: WindowRepaintPolicy,
    keymap: KeyMap<E::Message>,
    active: bool,
    focused: bool,
    entered: bool,
    focus_scope_kind: FocusScopeKind,
    focus_nodes: Vec<FocusNode>,
    chrome: WindowChrome,
    last_area: Option<Rect>,
    child_cache: Option<Buffer>,
    child_cache_area: Option<Rect>,
    stats: WindowRenderStats,
    effect_placements: BTreeSet<(u32, u32)>,
    hooks: WindowHooks,
}

impl<E> fmt::Debug for Window<E>
where
    E: Element,
    E::Message: Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Window")
            .field("id", &self.id)
            .field("child", self.child.id())
            .field("dirty", &self.dirty)
            .field("repaint_policy", &self.repaint_policy)
            .field("active", &self.active)
            .field("focused", &self.focused)
            .field("entered", &self.entered)
            .field("focus_scope_kind", &self.focus_scope_kind)
            .field("focus_nodes", &self.focus_nodes)
            .field("chrome", &self.chrome)
            .field("last_area", &self.last_area)
            .field("stats", &self.stats)
            .field("effect_placements", &self.effect_placements)
            .finish()
    }
}

impl<E> Window<E>
where
    E: Element,
    E::Message: Clone,
{
    pub fn new(id: impl Into<ComponentId>, child: E) -> Self {
        let child_ids = vec![child.id().clone()];
        Self {
            id: id.into(),
            child,
            child_ids,
            dirty: DirtyState::layout(DirtyReason::Explicit),
            repaint_policy: WindowRepaintPolicy::Whole,
            keymap: KeyMap::new(),
            active: false,
            focused: false,
            entered: false,
            focus_scope_kind: FocusScopeKind::Normal,
            focus_nodes: Vec::new(),
            chrome: WindowChrome::new(),
            last_area: None,
            child_cache: None,
            child_cache_area: None,
            stats: WindowRenderStats::default(),
            effect_placements: BTreeSet::new(),
            hooks: WindowHooks::default(),
        }
    }

    pub fn child(&self) -> &E {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut E {
        self.invalidate_child_cache();
        self.mark_dirty(DirtyReason::Explicit);
        &mut self.child
    }

    pub fn repaint_policy(&self) -> WindowRepaintPolicy {
        self.repaint_policy
    }

    pub fn with_repaint_policy(mut self, policy: WindowRepaintPolicy) -> Self {
        self.repaint_policy = policy;
        self
    }

    pub fn keymap(&self) -> &KeyMap<E::Message> {
        &self.keymap
    }

    pub fn keymap_mut(&mut self) -> &mut KeyMap<E::Message> {
        &mut self.keymap
    }

    /// Set the window-local keymap.
    ///
    /// This is the inherent `Window` method, not the
    /// [`ElementExt`](crate::elements::ElementExt) decorator.
    /// Bindings participate only while the window is active or focused.
    pub fn with_keymap(mut self, keymap: KeyMap<E::Message>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn with_focus_scope(
        mut self,
        kind: FocusScopeKind,
        nodes: impl IntoIterator<Item = FocusNode>,
    ) -> Self {
        self.focus_scope_kind = kind;
        self.focus_nodes = nodes.into_iter().collect();
        self
    }

    pub fn modal(mut self) -> Self {
        self.focus_scope_kind = FocusScopeKind::Modal;
        self
    }

    pub fn focus_scope(&self) -> WindowFocusScope {
        WindowFocusScope {
            id: FocusId::new(self.id.as_str().to_string()),
            kind: self.focus_scope_kind,
            nodes: self.focus_nodes.clone(),
        }
    }

    pub fn chrome(&self) -> &WindowChrome {
        &self.chrome
    }

    pub fn with_chrome(mut self, chrome: WindowChrome) -> Self {
        self.chrome = chrome;
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.chrome.title = Some(title.into());
        self.chrome.border = true;
        self
    }

    pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
        self.chrome.footer = Some(footer.into());
        self.chrome.border = true;
        self
    }

    pub fn with_border(mut self, enabled: bool) -> Self {
        self.chrome.border = enabled;
        self
    }

    pub fn with_padding(mut self, padding: impl Into<Padding>) -> Self {
        self.chrome.padding = padding.into();
        self
    }

    pub fn child_area(&self, area: Rect) -> Rect {
        self.chrome.child_area(area)
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn has_entered(&self) -> bool {
        self.entered
    }

    pub fn activate(&mut self) {
        if !self.active {
            self.prefetch();
            self.active = true;
            self.enter();
        }
        self.focus();
    }

    pub fn deactivate(&mut self) {
        self.blur();
        if self.active {
            self.active = false;
            self.exit();
        }
    }

    pub fn enter(&mut self) {
        if !self.entered {
            self.entered = true;
            self.fire(WindowLifecycleEvent::Enter);
        }
    }

    pub fn exit(&mut self) {
        if self.entered {
            self.entered = false;
            self.fire(WindowLifecycleEvent::Exit);
        }
    }

    pub fn focus(&mut self) {
        if !self.focused {
            self.focused = true;
            self.fire(WindowLifecycleEvent::Focus);
        }
    }

    pub fn blur(&mut self) {
        if self.focused {
            self.focused = false;
            self.fire(WindowLifecycleEvent::Blur);
        }
    }

    pub fn prefetch(&mut self) {
        self.fire(WindowLifecycleEvent::Prefetch);
    }

    pub fn on_prefetch<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.prefetch.push(Box::new(hook));
        self
    }

    pub fn on_enter<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.enter.push(Box::new(hook));
        self
    }

    pub fn on_exit<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.exit.push(Box::new(hook));
        self
    }

    pub fn on_focus<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.focus.push(Box::new(hook));
        self
    }

    pub fn on_blur<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.blur.push(Box::new(hook));
        self
    }

    pub fn on_resize<F>(mut self, hook: F) -> Self
    where
        F: FnMut(WindowLifecycleEvent) + 'static,
    {
        self.hooks.resize.push(Box::new(hook));
        self
    }

    pub fn stats(&self) -> WindowRenderStats {
        self.stats
    }

    pub fn key_scope(&self, role: KeyScopeRole) -> KeyScope<E::Message> {
        KeyScope {
            id: self.id.clone(),
            role,
            keymap: self.keymap.clone(),
            active: self.active,
            focused: self.focused,
        }
    }

    /// Image placements currently owned by this window as `(image_id, placement_id)`.
    pub fn registered_effect_placements(&self) -> &BTreeSet<(u32, u32)> {
        &self.effect_placements
    }

    fn fire(&mut self, event: WindowLifecycleEvent) {
        let hooks = match event {
            WindowLifecycleEvent::Prefetch => &mut self.hooks.prefetch,
            WindowLifecycleEvent::Enter => &mut self.hooks.enter,
            WindowLifecycleEvent::Exit => &mut self.hooks.exit,
            WindowLifecycleEvent::Focus => &mut self.hooks.focus,
            WindowLifecycleEvent::Blur => &mut self.hooks.blur,
            WindowLifecycleEvent::Resize { .. } => &mut self.hooks.resize,
        };
        for hook in hooks {
            hook(event);
        }
    }

    fn record_resize(&mut self, area: Rect) {
        if self.last_area != Some(area) {
            let old = self.last_area;
            self.last_area = Some(area);
            self.invalidate_child_cache();
            self.dirty.mark_layout(DirtyReason::Resize);
            self.fire(WindowLifecycleEvent::Resize { old, new: area });
        }
    }

    fn invalidate_child_cache(&mut self) {
        self.child_cache = None;
        self.child_cache_area = None;
    }

    fn render_child_cached(&mut self, child_area: Rect, buffer: &mut Buffer) -> Result<()> {
        let needs_render = self.child_cache.is_none()
            || self.child_cache_area != Some(child_area)
            || !self.child.dirty().is_clean();
        if needs_render {
            let mut child_buffer = Buffer::empty(child_area);
            self.child.render_buffer(child_area, &mut child_buffer)?;
            self.child.clear_dirty();
            self.child_cache = Some(child_buffer);
            self.child_cache_area = Some(child_area);
            self.stats.child_cache_misses += 1;
        } else {
            self.stats.child_cache_hits += 1;
        }
        if let Some(child_buffer) = &self.child_cache {
            blit(child_buffer, buffer);
        }
        Ok(())
    }
}

impl<E> BufferComponent for Window<E>
where
    E: Element,
    E::Message: Clone,
{
    type Event = KeyEvent;
    type Message = E::Message;

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.record_resize(area);
        self.chrome.render(area, buffer);
        let child_area = self.child_area(area);
        match self.repaint_policy {
            WindowRepaintPolicy::Whole => {
                self.child.render_buffer(child_area, buffer)?;
                self.child.clear_dirty();
                self.stats.whole_repaints += 1;
                self.stats.last_repaint_area = Some(area);
            }
            WindowRepaintPolicy::ChildCached => {
                self.render_child_cached(child_area, buffer)?;
                self.stats.last_repaint_area = Some(child_area);
            }
        }
        self.clear_dirty();
        Ok(())
    }

    fn handle_event(&mut self, event: &KeyEvent) -> Result<ElementOutcome<Self::Message>> {
        if !(self.active || self.focused) {
            return Ok(ComponentOutcome::Ignored);
        }
        // Focus controls child-first routing; active alone enables the
        // window-local keymap without handing input to the child.
        if self.focused {
            let child = self.child.handle_event(event)?;
            if child.is_handled() {
                return Ok(child);
            }
        }
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
        self.dirty.mark_paint(reason.clone());
        if self.repaint_policy == WindowRepaintPolicy::ChildCached {
            self.invalidate_child_cache();
        }
        self.child.mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
        self.child.clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.child.focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        &self.child_ids
    }
}

impl<E> ContainerElement for Window<E>
where
    E: Element,
    E::Message: Clone,
{
}

impl<E> EffectElement for Window<E>
where
    E: EffectElement,
    E::Message: Clone,
{
    fn render_effects(&mut self, area: Rect) -> Result<Vec<RenderEffect>> {
        let child_area = self.child_area(area);
        let effects = self.child.render_effects(child_area)?;
        for effect in &effects {
            match effect {
                RenderEffect::PlaceImage { options, .. } => {
                    self.effect_placements
                        .insert((options.image_id, options.placement_id));
                }
                RenderEffect::DeleteImagePlacement {
                    image_id,
                    placement_id,
                } => {
                    self.effect_placements.remove(&(*image_id, *placement_id));
                }
                RenderEffect::DeletePlacement { placement_id } => {
                    self.effect_placements
                        .retain(|(_, registered)| registered != placement_id);
                }
                RenderEffect::DeleteAllPlacements | RenderEffect::ForgetAllImages => {
                    self.effect_placements.clear();
                }
                RenderEffect::EnsureImageLoaded { .. } | RenderEffect::FlushImages => {}
            }
        }
        Ok(effects)
    }

    fn teardown_effects(&mut self) -> Result<Vec<RenderEffect>> {
        let mut effects = self.child.teardown_effects()?;
        let already_deleted_images: BTreeSet<(u32, u32)> = effects
            .iter()
            .filter_map(|effect| match effect {
                RenderEffect::DeleteImagePlacement {
                    image_id,
                    placement_id,
                } => Some((*image_id, *placement_id)),
                _ => None,
            })
            .collect();
        let already_deleted_placements: BTreeSet<u32> = effects
            .iter()
            .filter_map(|effect| match effect {
                RenderEffect::DeletePlacement { placement_id } => Some(*placement_id),
                _ => None,
            })
            .collect();
        let all_deleted = effects.iter().any(|effect| {
            matches!(
                effect,
                RenderEffect::DeleteAllPlacements | RenderEffect::ForgetAllImages
            )
        });
        for (image_id, placement_id) in std::mem::take(&mut self.effect_placements) {
            if !all_deleted
                && !already_deleted_images.contains(&(image_id, placement_id))
                && !already_deleted_placements.contains(&placement_id)
            {
                effects.push(RenderEffect::DeleteImagePlacement {
                    image_id,
                    placement_id,
                });
            }
        }
        Ok(effects)
    }
}

fn blit(source: &Buffer, target: &mut Buffer) {
    let area = *source.area();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let (Some(src), Some(dst)) = (source.cell((x, y)), target.cell_mut((x, y))) {
                *dst = src.clone();
            }
        }
    }
}

/// Declarative key scope role used by [`KeyScopeResolver`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyScopeRole {
    Root,
    ModalWindow,
    FocusedChild,
    Window,
}

#[derive(Debug, Clone)]
pub struct KeyScope<C: Clone> {
    pub id: ComponentId,
    pub role: KeyScopeRole,
    pub keymap: KeyMap<C>,
    pub active: bool,
    pub focused: bool,
}

impl<C: Clone> KeyScope<C> {
    pub fn new(id: impl Into<ComponentId>, role: KeyScopeRole, keymap: KeyMap<C>) -> Self {
        Self {
            id: id.into(),
            role,
            keymap,
            active: true,
            focused: false,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyResolution<C> {
    pub scope_id: ComponentId,
    pub role: KeyScopeRole,
    pub command: C,
}

#[derive(Debug, Clone)]
pub struct KeyScopeResolver<C: Clone> {
    root: KeyMap<C>,
    scopes: Vec<KeyScope<C>>,
}

impl<C: Clone> KeyScopeResolver<C> {
    pub fn new(root: KeyMap<C>) -> Self {
        Self {
            root,
            scopes: Vec::new(),
        }
    }

    pub fn push_scope(&mut self, scope: KeyScope<C>) -> &mut Self {
        self.scopes.push(scope);
        self
    }

    pub fn with_scope(mut self, scope: KeyScope<C>) -> Self {
        self.push_scope(scope);
        self
    }

    pub fn resolve(&self, key: KeyEvent) -> Option<KeyResolution<C>> {
        self.resolve_role(key, KeyScopeRole::ModalWindow, |scope| scope.active)
            .or_else(|| {
                self.resolve_role(key, KeyScopeRole::FocusedChild, |scope| {
                    scope.active && scope.focused
                })
            })
            .or_else(|| self.resolve_role(key, KeyScopeRole::Window, |scope| scope.active))
            .or_else(|| {
                self.root.lookup(key).map(|command| KeyResolution {
                    scope_id: ComponentId::new("root"),
                    role: KeyScopeRole::Root,
                    command,
                })
            })
    }

    fn resolve_role(
        &self,
        key: KeyEvent,
        role: KeyScopeRole,
        eligible: impl Fn(&KeyScope<C>) -> bool,
    ) -> Option<KeyResolution<C>> {
        self.scopes
            .iter()
            .rev()
            .find(|scope| scope.role == role && eligible(scope))
            .and_then(|scope| {
                scope.keymap.lookup(key).map(|command| KeyResolution {
                    scope_id: scope.id.clone(),
                    role: scope.role,
                    command,
                })
            })
    }
}
