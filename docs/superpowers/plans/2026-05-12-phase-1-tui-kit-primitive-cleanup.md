# Phase 1 — tui-kit Primitive Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse the three overlapping rendering traits in tui-kit into one (`BufferComponent`), shrink the prelude to constructors and traits, and drop the dead Sixel/iTerm2 `ImageProtocol` variants — atomically across `tui-kit` and `c4tui` with no compatibility shims.

**Architecture:** `BufferComponent` becomes the single rendering trait. `Element<Message=M>` becomes a marker subtrait `pub trait Element: BufferComponent<Event = Key>` with a blanket impl, so `Box<dyn Element<Message=M>>` and `impl Element<Message=M>` continue to work syntactically while only one trait carries the methods. The Frame-based `Component` trait is deleted entirely; `Cached<C>` keeps its `render_to_buffer` method, and callers that wrote `cached.render(frame, area)` now write `cached.render_to_buffer(area, frame.buffer_mut())`. `ImageProtocol` is reduced to `{ Kitty, Noop }` so `ImageBackendPreference::Explicit(Sixel)` becomes a compile error. The prelude keeps only the surface a typical app glob-imports (`Terminal`, `Element`, `BufferComponent`, `Cached`, `ImageBox`, `Key`, key/focus/event handles, status bar fragments) and pushes config/error/internal-state types behind `tui_kit::widgets::image_box::*` / `tui_kit::widgets::image_viewport::*` module paths.

**Tech Stack:** Rust 2021, `ratatui` 0.x buffers, `cargo test`, `cargo build`.

**Repos under simultaneous edit:**
- `tui-kit` at `/Users/coleshaffer/Projects/tui-kit`
- `c4tui` at `/Users/coleshaffer/Projects/c4tui`

**Hard constraints from user:**
- No backwards-compatibility shims. No deprecated re-exports. No `// kept for migration` comments. Rip and replace.
- Both repos are edited atomically. If a tui-kit change breaks c4tui imports, fix the c4tui imports in the same task.
- Goal is end state, not graceful transition.

**Execution order:**
- Tasks 1–6 land Item #3 (trait collapse). #3 is sequenced first because #9 (prelude slim) depends on knowing which trait names survive.
- Tasks 7–8 land Item #10 (ImageProtocol reduction). Independent of #9; runs before #9 so that the dead `ImageBackendPreference` arms are gone before we re-inventory the prelude.
- Task 9 lands Item #9 (prelude slim).
- Task 10 is the final cross-repo verification.

**Conventions used in this plan:**
- All file paths absolute.
- Code blocks show the literal text after edit. When a step says "replace block at lines A-B", apply the new code as the full replacement.
- Commit after every task. Commit messages match the codebase style observed in `git log` (lowercase summary, no trailing period, no emoji).

---

## File map

**tui-kit (modified):**
- `src/component.rs` — delete `Component` trait + its blanket impl on `Cached`; keep `BufferComponent`, `Cached`, ID/dirty/outcome types.
- `src/elements.rs` — collapse all 14 production `impl Element for ...` blocks (plus 4 test impls) into `impl BufferComponent for ...` with `type Event = Key`; rename `render → render_buffer`; change `handle_key(key: Key)` to `handle_event(event: &Key)`; redeclare `Element` as a marker subtrait; remove the manual `Box<dyn Element<Message=M>>` forwarding impl (the blanket impl + supertrait give it for free).
- `src/image.rs` — delete `ImageProtocol::Sixel` and `ImageProtocol::ITerm2` variants and every match arm / test / helper that names them; collapse the now-trivial `image_protocol_is_implemented` helper.
- `src/terminal.rs` — drop the one `ImageProtocol::Sixel` test fixture.
- `src/prelude.rs` — rewrite to a minimal surface.
- `src/widgets/grid.rs` — already only imports `ComponentOutcome`, no change required.

**c4tui (modified — small follow-ups only):**
- `src/app.rs` — drop the now-stale `Component` import (the trait is gone). `Cached<ViewPicker>::render_to_buffer` continues to compile unchanged; no other call-site change needed because c4tui never used the Frame-based `Component::render`.

**Untouched (verified during ground-truth):**
- `c4tui/src/picker.rs`, `c4tui/src/connection_picker.rs` — already implement `BufferComponent` directly. The Element subtrait is irrelevant to them.
- `c4tui/src/backend.rs`, `c4tui/src/terminal.rs`, `c4tui/src/view.rs` — import from `tui_kit::widgets::image_viewport::*` directly, not from the prelude. Prelude slim does not affect them.
- `tui-kit/visual-tests/src/main.rs` — uses `ImageBox`, `ImageBoxPlan`, `ImageBoxState`, `Terminal`, `TerminalConfig` from prelude. All four survive the slim.
- `tui-kit/examples/terminal_dialog.rs` — uses only ratatui `Dialog` widgets, no trait imports.

---

## Task 1: Snapshot the green baseline

**Files:** none (verification only)

- [ ] **Step 1: Run tui-kit tests from a clean tree**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet`
Expected: all tests pass, exit code 0. Record the test count from the last `test result:` line for later comparison.

- [ ] **Step 2: Run c4tui tests from a clean tree**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo test --quiet`
Expected: all tests pass, exit code 0.

- [ ] **Step 3: Confirm git status is clean enough to commit per-task**

Run: `cd /Users/coleshaffer/Projects/tui-kit && git status --short`
Expected output (the pre-existing dirty files are tracked separately and are not in this plan's scope):
```
 M README.md
?? architecture.md
?? specification.md
```
If anything else is dirty, stop and ask before continuing — this plan assumes the rest of the tree is clean.

- [ ] **Step 4: Confirm c4tui git status**

Run: `cd /Users/coleshaffer/Projects/c4tui && git status --short`
Expected: no files modified (or document what's dirty before starting).

---

## Task 2: Add `Box<dyn BufferComponent>` forwarding impl

**Files:**
- Modify: `/Users/coleshaffer/Projects/tui-kit/src/component.rs`

**Why this comes first:** `elements.rs` currently has a manual `impl<M> Element for Box<dyn Element<Message = M>>` (lines 60-94). After the trait collapse, that impl needs a `BufferComponent` equivalent that lives next to the trait. We add it now, before touching elements.rs, so the symbol exists when the rewrite happens. Test-first: the new impl is verifiable in isolation.

- [ ] **Step 1: Write the failing test**

Append to `/Users/coleshaffer/Projects/tui-kit/src/component.rs` inside the `#[cfg(test)] mod tests` block (after the existing `cached_buffer_component_invalidates_on_dirty_or_area_change` test, before the closing `}` of the module):

```rust
    #[test]
    fn boxed_buffer_component_forwards_trait_methods() -> anyhow::Result<()> {
        let area = Rect::new(0, 0, 1, 1);
        let mut boxed: Box<dyn BufferComponent<Event = (), Message = ()>> =
            Box::new(CountingBufferComponent::new());

        assert_eq!(boxed.id().as_str(), "counter");
        assert!(!boxed.dirty().is_clean());

        let mut buffer = Buffer::empty(area);
        boxed.render_buffer(area, &mut buffer)?;
        boxed.clear_dirty();
        assert!(boxed.dirty().is_clean());

        boxed.mark_dirty(DirtyReason::Input);
        assert!(!boxed.dirty().is_clean());

        let outcome = boxed.handle_event(&())?;
        assert!(matches!(outcome, ComponentOutcome::Ignored));

        Ok(())
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet component::tests::boxed_buffer_component_forwards_trait_methods 2>&1 | tail -20`
Expected: compile error of the form
```
the trait `BufferComponent` is not implemented for `Box<dyn BufferComponent<Event = (), Message = ()>>`
```
(or `cannot ... on a Box<dyn ...>`). The test fails to compile because the forwarding impl does not exist yet.

- [ ] **Step 3: Add the forwarding impl**

In `/Users/coleshaffer/Projects/tui-kit/src/component.rs`, after the `pub trait BufferComponent { ... }` block (which currently ends at line 239) and before the `CachedRenderStats` struct, insert:

```rust
impl<E, M> BufferComponent for Box<dyn BufferComponent<Event = E, Message = M>> {
    type Event = E;
    type Message = M;

    fn id(&self) -> &ComponentId {
        self.as_ref().id()
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> anyhow::Result<()> {
        self.as_mut().render_buffer(area, buffer)
    }

    fn handle_event(
        &mut self,
        event: &Self::Event,
    ) -> anyhow::Result<ComponentOutcome<Self::Message>> {
        self.as_mut().handle_event(event)
    }

    fn dirty(&self) -> &DirtyState {
        self.as_ref().dirty()
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.as_mut().mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.as_mut().clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.as_ref().focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        self.as_ref().children()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet component::tests::boxed_buffer_component_forwards_trait_methods 2>&1 | tail -10`
Expected: `test result: ok. 1 passed`.

- [ ] **Step 5: Run the full tui-kit suite to confirm no regression**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet 2>&1 | tail -5`
Expected: same test count as Task 1 Step 1, plus one (the new test). All passing.

- [ ] **Step 6: Commit**

```bash
cd /Users/coleshaffer/Projects/tui-kit
git add src/component.rs
git commit -m "add forwarding BufferComponent impl for boxed trait objects"
```

---

## Task 3: Delete the Frame-based `Component` trait

**Files:**
- Modify: `/Users/coleshaffer/Projects/tui-kit/src/component.rs`
- Modify: `/Users/coleshaffer/Projects/c4tui/src/app.rs:19`

**Why now:** `Component` has no production `impl` anywhere — confirmed via `grep -rn "impl Component for" /Users/coleshaffer/Projects/{tui-kit,c4tui}/src/` returning zero hits. The only consumer is the blanket `impl<C> Component for Cached<C> where C: BufferComponent`, which only exists to satisfy the trait. Both can be deleted. After deletion, `Cached<C>::render_to_buffer(area, buffer)` remains the supported call path.

- [ ] **Step 1: Delete the `Component` trait definition**

In `/Users/coleshaffer/Projects/tui-kit/src/component.rs`, delete lines 174-204 (the `/// Optional trait for reusable UI mechanics.` doc block, the `pub trait Component { ... }` definition, and its trailing blank line). After deletion, the section that previously ran from `/// Optional trait for reusable UI mechanics.` through the close brace of `Component` is gone, and the file goes directly from the `pub type ComponentChildren<'a> = &'a [ComponentId];` line to the `/// Buffer-native component shape used by [Cached].` doc on `BufferComponent`.

- [ ] **Step 2: Delete the `impl Component for Cached<C>` block**

In the same file, delete the block at lines 329-371 (the entire `impl<C> Component for Cached<C> where C: BufferComponent { ... }` definition, including its surrounding blank lines as needed to leave one blank line between the preceding `impl<C> Cached<C> where C: BufferComponent` block and the following `fn blit(...)` function).

After this deletion, the only impl on `Cached<C>` is the inherent block (`render_to_buffer`, `inner`, `inner_mut`, `into_inner`, `invalidate`, `stats`, etc.) plus `impl<C> Cached<C> where C: BufferComponent { fn render_to_buffer ... }`.

- [ ] **Step 3: Drop the `Component` import in c4tui**

In `/Users/coleshaffer/Projects/c4tui/src/app.rs` line 19, change:

```rust
use tui_kit::component::{Cached, Component, ComponentOutcome};
```

to:

```rust
use tui_kit::component::{Cached, ComponentOutcome};
```

- [ ] **Step 4: Build tui-kit**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet 2>&1 | tail -10`
Expected: clean build, no errors. (`Cached::render` was only ever provided by the trait impl; nothing in tui-kit calls it.)

- [ ] **Step 5: Build c4tui against the modified tui-kit**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo build --quiet 2>&1 | tail -10`
Expected: clean build. The only use of `Cached` in c4tui is `Cached<ViewPicker>::render_to_buffer` (the inherent method) and `Cached<ViewPicker>::inner()/inner_mut()` (also inherent) — none of which depend on the deleted trait.

- [ ] **Step 6: Run tui-kit tests**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 7: Run c4tui tests**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo test --quiet 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 8: Commit (tui-kit first, then c4tui)**

```bash
cd /Users/coleshaffer/Projects/tui-kit
git add src/component.rs
git commit -m "drop frame-based Component trait in favor of BufferComponent"
```

```bash
cd /Users/coleshaffer/Projects/c4tui
git add src/app.rs
git commit -m "drop dead Component import after tui-kit trait collapse"
```

---

## Task 4: Redeclare `Element` as a marker subtrait of `BufferComponent`

**Files:**
- Modify: `/Users/coleshaffer/Projects/tui-kit/src/elements.rs:34-94` (the `Element` trait definition + its hand-written `Box<dyn Element>` impl)

**Why this approach:** Trait aliases (`trait Element = BufferComponent<Event=Key>`) are not stable Rust. The closest stable equivalent that keeps the `Element<Message=M>` syntax working is an empty subtrait with a blanket impl. Concretely:

```rust
pub trait Element: BufferComponent<Event = Key> {}
impl<T> Element for T where T: BufferComponent<Event = Key> {}
```

Any `T: BufferComponent<Event = Key>` is automatically also `T: Element`, and `dyn Element<Message = M>` resolves through the supertrait to `dyn BufferComponent<Event = Key, Message = M>`. Subsequent tasks convert each `impl Element for Foo { type Message = M; fn render(...); fn handle_key(...); ... }` to `impl BufferComponent for Foo { type Event = Key; type Message = M; fn render_buffer(...); fn handle_event(&self, event: &Key); ... }`, and the blanket impl makes `Foo: Element` follow.

This task only redefines `Element`. The 14 `impl Element for Foo` rewrites happen in Task 5. Until then, the redeclared `Element` has zero implementors and the crate will not compile. **That is expected** — Task 4 ends without a successful build; Task 5 is what brings it back to green. We commit at the end of Task 5, not Task 4. Step 4 below explicitly verifies the expected build failure.

- [ ] **Step 1: Replace the `Element` trait definition and the manual boxed forwarding impl**

In `/Users/coleshaffer/Projects/tui-kit/src/elements.rs`, replace lines 26-94 — which currently contain:

```rust
/// Alias retained so element event handlers read independently from components.
pub type ElementOutcome<Message> = ComponentOutcome<Message>;

/// Buffer-rendered UI object.
///
/// Elements render into a caller-owned [`Buffer`]. Terminal side effects such
/// as image placement stay outside this trait and are exposed through
/// [`EffectElement`].
pub trait Element {
    type Message;

    fn id(&self) -> &ComponentId;

    fn render(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()>;

    fn handle_key(&mut self, _key: Key) -> Result<ElementOutcome<Self::Message>> {
        Ok(ComponentOutcome::Ignored)
    }

    fn dirty(&self) -> &DirtyState;

    fn mark_dirty(&mut self, reason: DirtyReason);

    fn clear_dirty(&mut self);

    fn focus_node(&self) -> Option<FocusNode> {
        None
    }

    fn children(&self) -> ComponentChildren<'_> {
        &[]
    }
}

impl<M> Element for Box<dyn Element<Message = M>> {
    type Message = M;

    fn id(&self) -> &ComponentId {
        self.as_ref().id()
    }

    fn render(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        self.as_mut().render(area, buffer)
    }

    fn handle_key(&mut self, key: Key) -> Result<ElementOutcome<Self::Message>> {
        self.as_mut().handle_key(key)
    }

    fn dirty(&self) -> &DirtyState {
        self.as_ref().dirty()
    }

    fn mark_dirty(&mut self, reason: DirtyReason) {
        self.as_mut().mark_dirty(reason);
    }

    fn clear_dirty(&mut self) {
        self.as_mut().clear_dirty();
    }

    fn focus_node(&self) -> Option<FocusNode> {
        self.as_ref().focus_node()
    }

    fn children(&self) -> ComponentChildren<'_> {
        self.as_ref().children()
    }
}
```

with:

```rust
/// Alias retained so element event handlers read independently from components.
pub type ElementOutcome<Message> = ComponentOutcome<Message>;

/// Buffer-rendered UI object: a [`BufferComponent`] whose event type is keyboard input.
///
/// `Element` is a marker subtrait. Every implementation is written as
/// `impl BufferComponent for Foo` (with `type Event = Key;`), and the blanket
/// impl below makes any such type automatically `Element`. The `Element` name
/// remains useful in `dyn Element<Message = M>` and `impl Element<Message = M>`
/// positions to express "buffer-rendered keyboard-driven UI object" without
/// repeating the `BufferComponent<Event = Key>` bound at every call site.
pub trait Element: BufferComponent<Event = Key> {}

impl<T> Element for T where T: BufferComponent<Event = Key> {}
```

Note the manual `impl<M> Element for Box<dyn Element<Message = M>>` is *gone* — it is no longer needed. The `impl<E, M> BufferComponent for Box<dyn BufferComponent<Event = E, Message = M>>` added in Task 2 satisfies the supertrait, and the blanket `impl<T> Element for T` then lifts it to `Element`.

- [ ] **Step 2: Update the existing import line**

In `/Users/coleshaffer/Projects/tui-kit/src/elements.rs` line 18, change:

```rust
use crate::component::{ComponentChildren, ComponentId, ComponentOutcome, DirtyReason, DirtyState};
```

to:

```rust
use crate::component::{
    BufferComponent, ComponentChildren, ComponentId, ComponentOutcome, DirtyReason, DirtyState,
};
```

(adding `BufferComponent` to the import — `Element` needs to name its supertrait).

- [ ] **Step 3: Run the build (expected to fail loudly)**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet 2>&1 | tail -30`
Expected: many errors, all variants of "the trait bound `Foo: BufferComponent` is not satisfied" — because every `impl Element for Foo` still uses the old method names and the trait `Element` now requires `BufferComponent` as a supertrait that is not yet implemented for `Foo`. **This is the expected state** at the end of Task 4 — Task 5 fixes it.

Do not commit yet. Move directly to Task 5.

---

## Task 5: Convert every `impl Element for Foo` to `impl BufferComponent for Foo`

**Files:**
- Modify: `/Users/coleshaffer/Projects/tui-kit/src/elements.rs` (14 production impls + 4 test impls)

**The transformation:** For each block, apply mechanically:

| Before | After |
|---|---|
| `impl Element for Foo {` | `impl BufferComponent for Foo {` |
| `impl<...> Element for Foo {` | `impl<...> BufferComponent for Foo {` |
| (start of body) `type Message = M;` | `type Event = Key;`<br>`type Message = M;` |
| `fn render(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {` | `fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {` |
| `fn handle_key(&mut self, key: Key) -> Result<ElementOutcome<Self::Message>> {`<br>(or `_key: Key` for empty bodies) | `fn handle_event(&mut self, event: &Key) -> Result<ElementOutcome<Self::Message>> {`<br>(or `_event: &Key`) |
| Inside the body: any forwarding call like<br>`self.child.handle_key(key)` | `self.child.handle_event(event)` |
| Inside the body: any forwarding call like<br>`self.child.render(area, buffer)` | `self.child.render_buffer(area, buffer)` |

Bounds on generic params remain `where E: Element` — that still resolves correctly because `Element: BufferComponent<Event=Key>` and every method body now calls the renamed methods.

**Discovered impl line numbers** (production impls, current file state — re-check these locally before editing each one because line numbers shift as earlier edits land):
- Line 420 — `impl Element for Text`
- Line 606 — `impl<E: Element> Element for Panel<E>`
- Line 838 (approx) — `impl<M> Element for Stack<M>`
- Line 1030 (approx) — `impl<E> Element for ScrollY<E>`
- Line 1139 (approx) — `impl<E> Element for Focusable<E>`
- Line 1235 (approx) — `impl<E> Element for Padded<E>`
- Line 1333 (approx) — `impl<E> Element for Bordered<E>`
- Line 1413 (approx) — `impl<E, M> Element for KeyMapped<E>` (or similar)
- Line 1961 (approx) — `impl<E> Element for Window<E>`
- Line 2288 — `impl Element for ImageViewportElement`
- Line 2411 (approx) — `impl<E> Element for Modal<E>`
- Line 2601 (approx) — `impl<M> Element for Overlay<M>`
- Plus the impl block inside `ChildElement<M>` (lines 695-730 area, which uses `handle_key`/`render` on boxed children).

**Discovered test impls** (inside the `#[cfg(test)] mod tests`):
- Line 2710 — `impl Element for ProbeElement`
- Line 2779 — `impl Element for AreaProbeElement`
- Line 2855 — `impl Element for EffectProbeElement`
- Line 2932 — `impl Element for ToggleEffectProbeElement`

This task is intentionally many small steps. Each step converts exactly one impl block, then the next step converts the next. Build and commit at the end.

- [ ] **Step 1: Convert `impl Element for Text` (line 420 region)**

Locate the block opening `impl Element for Text {` and replace its header + signatures so the block becomes:

```rust
impl BufferComponent for Text {
    type Event = Key;
    type Message = ();

    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        // (preserve the existing body verbatim; only the fn name on the surrounding line changed)
```

Concretely:
- Change `impl Element for Text {` → `impl BufferComponent for Text {`.
- Inside the body, immediately after `type Message = ();`, insert a new line `type Event = Key;` above it (so the order reads `type Event = Key;` then `type Message = ();`).
- Change `fn render(&mut self, area: Rect, buffer: &mut Buffer)` → `fn render_buffer(&mut self, area: Rect, buffer: &mut Buffer)`.
- `Text` has no `handle_key` override (it uses the default), so after the merge it has no `handle_event` override either. Verify there is no `fn handle_key` inside the block; if there is, rename it as in the rules table.

Do not remove any other lines or change any body content.

- [ ] **Step 2: Convert `impl<E: Element> Element for Panel<E>`**

Locate the block opening `impl<E: Element> Element for Panel<E> {`. Apply the rules table:
- Header: `impl<E: Element> BufferComponent for Panel<E> {`
- Add `type Event = Key;` line above `type Message = E::Message;`.
- `fn render(&mut self, area, buffer)` → `fn render_buffer(&mut self, area, buffer)`.
- The block contains a call `self.child.render(child_area, buffer)?;` — change to `self.child.render_buffer(child_area, buffer)?;`.
- `fn handle_key(&mut self, key: Key)` → `fn handle_event(&mut self, event: &Key)`. The body is `self.child.handle_key(key)` — change to `self.child.handle_event(event)`.

- [ ] **Step 3: Convert `ChildElement<M>` inherent impl (lines ~695-730)**

This is the internal forwarder for `Stack`'s mixed-typed children. It is an *inherent* `impl<M> ChildElement<M> { ... }` block, not a trait impl, so the rules table applies only to the names it calls on its inner `Box<dyn Element<Message = M>>` field:

```rust
impl<M> ChildElement<M> {
    fn render(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        match self {
            Self::Buffer(element) => element.render(area, buffer),
            Self::Effect(element) => element.render(area, buffer),
        }
    }

    fn handle_key(&mut self, key: Key) -> Result<ElementOutcome<M>> {
        match self {
            Self::Buffer(element) => element.handle_key(key),
            Self::Effect(element) => element.handle_key(key),
        }
    }
```

becomes:

```rust
impl<M> ChildElement<M> {
    fn render(&mut self, area: Rect, buffer: &mut Buffer) -> Result<()> {
        match self {
            Self::Buffer(element) => element.render_buffer(area, buffer),
            Self::Effect(element) => element.render_buffer(area, buffer),
        }
    }

    fn handle_key(&mut self, key: Key) -> Result<ElementOutcome<M>> {
        match self {
            Self::Buffer(element) => element.handle_event(&key),
            Self::Effect(element) => element.handle_event(&key),
        }
    }
```

The *inherent method* names `ChildElement::render` and `ChildElement::handle_key` stay as they are — they are private helpers, not the trait. Only the inner calls on the trait-object fields change. `handle_key` still takes `Key` by value because the higher-level call site in `Stack::handle_event` (which we update in the next step) owns the `Key` and passes it through.

The `EffectElement` field is a `Box<dyn EffectElement<Message = M>>`. After the trait collapse, `EffectElement` keeps its current definition (it is *not* part of #3 — only `Element` collapses). `EffectElement` itself becomes a subtrait of `Element` via the same blanket pattern but it already supertrait-extends `Element`, so it transitively inherits `BufferComponent`'s methods. Step 4 covers `EffectElement`.

- [ ] **Step 4: Verify `EffectElement` continues to compile as-is**

Locate the `pub trait EffectElement: Element { ... }` definition (around line 167 in the original file). It currently reads:

```rust
pub trait EffectElement: Element {
    fn terminal_effects(&mut self, area: Rect) -> Result<Vec<TerminalEffect>>;

    fn teardown_effects(&mut self) -> Result<Vec<TerminalEffect>> {
        Ok(Vec::new())
    }
}
```

No change is required to the trait definition. `Element` is now a subtrait of `BufferComponent<Event=Key>`, so `EffectElement: Element` still names a valid supertrait chain. All `impl EffectElement for Foo` blocks continue to compile *once their corresponding `impl BufferComponent for Foo` block exists*. Move on.

- [ ] **Step 5: Convert `impl<M> Element for Stack<M>`**

Locate the block (look for `impl<M> Element for Stack<M> {` — current line approx 838). Apply the rules table:
- Header: `impl<M> BufferComponent for Stack<M> {`
- Add `type Event = Key;` above `type Message = M;`.
- Rename `fn render` → `fn render_buffer`, and its body's inner call `child.element.render(child_area, buffer)?;` → `child.element.render_buffer(child_area, buffer)?;`.
- Rename `fn handle_key(&mut self, key: Key)` → `fn handle_event(&mut self, event: &Key)`. The body contains `let outcome = child.element.handle_key(key)?;`. Because the inherent `ChildElement::handle_key` still takes `Key` by value (Step 3), and `event` is now `&Key`, change that call to `let outcome = child.element.handle_key(*event)?;`. `Key` derives `Copy` (verify by checking `tui-kit/src/input.rs` — see Step 5b below if it does not).

- [ ] **Step 5b: Verify `Key: Copy`**

Run: `grep -n "pub enum Key\b\|derive.*Copy.*Key\|impl Copy for Key" /Users/coleshaffer/Projects/tui-kit/src/input.rs`

Expected: a line of the form `#[derive(... Copy ...)]` directly above `pub enum Key`. If `Key` is *not* `Copy`, replace the `*event` deref pattern in Step 5 (and every other step in this task that does the same) with `event.clone()` instead, and verify `Key: Clone` instead.

- [ ] **Step 6: Convert `impl<E> Element for ScrollY<E>` (where `E: Element`)**

Locate the block (approximate line 1030). Rules:
- Header: `impl<E> BufferComponent for ScrollY<E> where E: Element {` — preserve any existing where-clause; just rename the trait.
- Add `type Event = Key;` above `type Message = E::Message;`.
- `fn render` → `fn render_buffer`. Inner call on `self.child.render(...)` → `self.child.render_buffer(...)`.
- `fn handle_key(&mut self, key: Key)` → `fn handle_event(&mut self, event: &Key)`. Inner `self.child.handle_key(key)` → `self.child.handle_event(event)`.

- [ ] **Step 7: Convert `impl<E> Element for Focusable<E>`**

(approximate line 1139). Same recipe as Step 6.

- [ ] **Step 8: Convert `impl<E> Element for Padded<E>`**

(approximate line 1235). Same recipe.

- [ ] **Step 9: Convert `impl<E> Element for Bordered<E>`**

(approximate line 1333). Same recipe.

- [ ] **Step 10: Convert `impl<E, ...> Element for KeyMapped<E>`**

(approximate line 1413). The block at line 1413 corresponds to `KeyMapped` (the `with_keymap`-wrapped element). Apply the recipe:
- Header: `impl<...> BufferComponent for KeyMapped<E> { ... }` — preserve the existing generic parameter list and where-bounds.
- Add `type Event = Key;` above `type Message = ...;`.
- Rename `render` → `render_buffer`.
- Rename `handle_key(key: Key)` → `handle_event(event: &Key)`. Any inner `handle_key(key)` calls on the wrapped child become `handle_event(event)`. If the body matches against `key` as a value (e.g. inside a `match key { ... }`), dereference at the match site: `match *event { ... }` (relying on `Key: Copy` from Step 5b) or `match event { ... }` and adjust patterns to references.

- [ ] **Step 11: Convert `impl<E> Element for Window<E>`**

(approximate line 1961). Same recipe as Step 6. Window's `handle_key` body contains:

```rust
let child = self.child.handle_key(key)?;
```

— change to:

```rust
let child = self.child.handle_event(event)?;
```

- [ ] **Step 12: Convert `impl Element for ImageViewportElement`**

(line 2288). Recipe:
- Header: `impl BufferComponent for ImageViewportElement {`
- Add `type Event = Key;` above `type Message = ();`.
- `fn render(&mut self, _area, _buffer)` → `fn render_buffer(&mut self, _area, _buffer)`. The body is `Ok(())` — no inner calls to rename.
- `ImageViewportElement` does not override `handle_key`, so no `handle_event` override either.

- [ ] **Step 13: Convert `impl<E> Element for Modal<E>`**

(approximate line 2411). Modal forwards everything to its inner `Window`. Recipe as Step 6, with the inner method calls being `self.window.render_buffer(...)`, `self.window.handle_event(event)`, etc.

- [ ] **Step 14: Convert `impl<M> Element for Overlay<M>`**

(approximate line 2601). Recipe as Step 6. Overlay's `handle_key` body iterates layers; replace `layer.element.handle_key(key)` with `layer.element.handle_key(*event)` (Note: `layer.element` is a `ChildElement<M>` whose inherent `handle_key` still takes `Key` by value per Step 3. Deref the `event: &Key` at the call boundary.)

- [ ] **Step 15: Convert the 4 test `impl Element for ...` blocks**

Inside `#[cfg(test)] mod tests` in elements.rs:
- Line 2710 area: `impl Element for ProbeElement`
- Line 2779 area: `impl Element for AreaProbeElement`
- Line 2855 area: `impl Element for EffectProbeElement`
- Line 2932 area: `impl Element for ToggleEffectProbeElement`

Apply the same recipe to each: `Element` → `BufferComponent`, add `type Event = Key;`, rename `render`/`handle_key`.

- [ ] **Step 16: Update test bodies that invoke `handle_key` directly**

Lines 3112, 3142, 3171, 3175, 3179, 3471, 3477, 3483, 3489, 3692, 3725 (from the original grep) contain test calls like:

```rust
element.handle_key(Key::Char('x'))?
```

After the trait merge, the relevant inherited method on a trait object/impl block is `handle_event(&Key)`. Replace each call site:

```rust
element.handle_event(&Key::Char('x'))?
```

— and similarly for the `window.handle_key(...)`, `modal.handle_key(...)`, `overlay.handle_key(...)` test sites.

Do this in bulk:
```
cd /Users/coleshaffer/Projects/tui-kit
# Edit src/elements.rs — find every `.handle_key(Key::` in test code and replace with `.handle_event(&Key::`
```
Use the Edit tool's `replace_all` flag scoped to the test region if it is unambiguous; otherwise convert each occurrence individually. After the edits, `grep -n "\.handle_key(" /Users/coleshaffer/Projects/tui-kit/src/elements.rs` should return only the `ChildElement::handle_key` inherent method definition and its call sites in `Stack`/`Overlay` (lines around 855, 2618).

- [ ] **Step 17: Build tui-kit**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet 2>&1 | tail -20`
Expected: clean build. If errors remain, they will name a specific impl block — apply the rules table to it.

- [ ] **Step 18: Run tui-kit tests**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet 2>&1 | tail -5`
Expected: all green, same count as Task 2 Step 5 (one new test added in Task 2; no tests were deleted).

- [ ] **Step 19: Run c4tui tests (still implements `BufferComponent` directly — should be unaffected)**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo test --quiet 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 20: Commit**

```bash
cd /Users/coleshaffer/Projects/tui-kit
git add src/elements.rs
git commit -m "collapse Element trait into BufferComponent<Event=Key> subtrait"
```

---

## Task 6: Clean up obsolete `handle_key` references in the documentation strings of elements.rs

**Files:**
- Modify: `/Users/coleshaffer/Projects/tui-kit/src/elements.rs` (doc comments only)

**Why this task exists separately:** Task 5 changed the method names but did not necessarily touch every `/// ...` doc comment that referred to `handle_key` or `render`. Doc comments don't break the build but they're misleading after the rename. This task is a quick pass.

- [ ] **Step 1: Find remaining doc-comment references**

Run:
```
cd /Users/coleshaffer/Projects/tui-kit
grep -n "handle_key\|render(\&mut" src/elements.rs | grep -v "^.*://\|fn handle_key\|fn render"
```
Plus a generic doc-comment scan:
```
grep -n "/// .*handle_key\|/// .*\.render(" src/elements.rs
```

- [ ] **Step 2: Rewrite each match**

For each line returned, replace `handle_key` with `handle_event` and `render(` with `render_buffer(` in the doc-comment context only. If the doc comment refers to the *intent* of receiving keyboard input (e.g. "called when a key is pressed"), keep the semantic prose but update the method name.

- [ ] **Step 3: Build (sanity check — doc edits should not change behavior)**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet 2>&1 | tail -3`
Expected: clean.

- [ ] **Step 4: Run doc tests**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --doc --quiet 2>&1 | tail -5`
Expected: any doctests still compile (most are `ignore`-marked at the elements layer).

- [ ] **Step 5: Commit**

```bash
cd /Users/coleshaffer/Projects/tui-kit
git add src/elements.rs
git commit -m "refresh elements.rs doc comments after trait collapse"
```

---

## Task 7: Drop `ImageProtocol::Sixel` and `ImageProtocol::ITerm2` (Item #10)

**Files:**
- Modify: `/Users/coleshaffer/Projects/tui-kit/src/image.rs`

**Why now:** No production code names Sixel or iTerm2 — the only references are inside `image.rs` itself (the enum, the helper, four test cases) and one test fixture in `terminal.rs` (Task 8). With both variants gone, the `ImageBackendPreference::Explicit(Sixel)` validation arm becomes a compile error, the `unsupported_protocol_error` helper has no remaining caller, and `image_protocol_is_implemented` becomes `true` for the only remaining `Kitty` variant — i.e. dead.

- [ ] **Step 1: Delete the `Sixel` and `ITerm2` variants**

In `/Users/coleshaffer/Projects/tui-kit/src/image.rs` lines 55-62, replace:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageProtocol {
    Kitty,
    Sixel,
    ITerm2,
    Noop,
}
```

with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageProtocol {
    Kitty,
    Noop,
}
```

`#[non_exhaustive]` is kept so adding Sixel/iTerm2 back later remains non-breaking on the consumer side.

- [ ] **Step 2: Delete the `unsupported_protocol_error` arm in `ImageSurfaceRegistry::from_preference`**

In `image.rs` around lines 102-121, the current `from_preference` reads:

```rust
pub fn from_preference(preference: ImageBackendPreference) -> Result<Self, ConfigError> {
    preference.validate()?;
    let surface = match &preference {
        ImageBackendPreference::KittyOnly
        | ImageBackendPreference::Explicit(ImageProtocol::Kitty) => {
            SelectedImageSurface::Kitty(KittyImageRegistry::default())
        }
        ImageBackendPreference::Disabled => SelectedImageSurface::Noop(NoopImageSurface),
        ImageBackendPreference::Explicit(protocol) => {
            return Err(unsupported_protocol_error(
                *protocol,
                "image.backend.protocol",
            ));
        }
        ImageBackendPreference::AutoDetect { order } => select_auto_detect_surface(order)?,
    };
    Ok(Self {
        preference,
        surface,
    })
}
```

After the variant deletions, the only `ImageProtocol` values are `Kitty` and `Noop`. `Explicit(Kitty)` is handled by the first arm. `Explicit(Noop)` is rejected by `Validate::validate` (the `explicit_noop_error` arm) before this match is reached. So the `Explicit(protocol) => return Err(unsupported_protocol_error(...))` arm becomes unreachable. Delete it:

```rust
pub fn from_preference(preference: ImageBackendPreference) -> Result<Self, ConfigError> {
    preference.validate()?;
    let surface = match &preference {
        ImageBackendPreference::KittyOnly
        | ImageBackendPreference::Explicit(ImageProtocol::Kitty) => {
            SelectedImageSurface::Kitty(KittyImageRegistry::default())
        }
        ImageBackendPreference::Disabled => SelectedImageSurface::Noop(NoopImageSurface),
        ImageBackendPreference::AutoDetect { order } => select_auto_detect_surface(order)?,
    };
    Ok(Self {
        preference,
        surface,
    })
}
```

The match is now exhaustive over the live variants: `KittyOnly`, `Explicit(Kitty)`, `Explicit(Noop)` (unreachable via early validate-return), `Disabled`, `AutoDetect`. The compiler will require this because `ImageBackendPreference::Explicit(ImageProtocol::Noop)` is technically a constructible value. To keep the match exhaustive without re-introducing dead error construction, add a final arm:

```rust
        ImageBackendPreference::Explicit(ImageProtocol::Noop) => {
            unreachable!("Explicit(Noop) is rejected by Validate::validate above")
        }
```

— or, equivalently, restructure to call `validate()` last. The `unreachable!` form is preferred because it documents the invariant inline.

Final block:

```rust
pub fn from_preference(preference: ImageBackendPreference) -> Result<Self, ConfigError> {
    preference.validate()?;
    let surface = match &preference {
        ImageBackendPreference::KittyOnly
        | ImageBackendPreference::Explicit(ImageProtocol::Kitty) => {
            SelectedImageSurface::Kitty(KittyImageRegistry::default())
        }
        ImageBackendPreference::Disabled => SelectedImageSurface::Noop(NoopImageSurface),
        ImageBackendPreference::Explicit(ImageProtocol::Noop) => {
            unreachable!("Explicit(Noop) is rejected by Validate::validate above")
        }
        ImageBackendPreference::AutoDetect { order } => select_auto_detect_surface(order)?,
    };
    Ok(Self {
        preference,
        surface,
    })
}
```

- [ ] **Step 3: Delete the now-orphaned `unsupported_protocol_error` and `image_protocol_is_implemented` helpers**

In `image.rs` lines 258-274, delete:

```rust
fn unsupported_protocol_error(protocol: ImageProtocol, path: &'static str) -> ConfigError {
    ConfigError::new(
        path,
        format!("image protocol {protocol:?} is not implemented yet"),
    )
}

fn explicit_noop_error(path: &'static str) -> ConfigError {
    ConfigError::new(
        path,
        "Noop is a degraded fallback, not a terminal image protocol; use Disabled instead",
    )
}

fn image_protocol_is_implemented(protocol: ImageProtocol) -> bool {
    matches!(protocol, ImageProtocol::Kitty)
}
```

`explicit_noop_error` is still used inside `Validate for ImageBackendPreference`, so **keep `explicit_noop_error`**. Only delete `unsupported_protocol_error` and `image_protocol_is_implemented`. The corrected delete is *just* those two functions:

```rust
fn unsupported_protocol_error(protocol: ImageProtocol, path: &'static str) -> ConfigError {
    ConfigError::new(
        path,
        format!("image protocol {protocol:?} is not implemented yet"),
    )
}

fn image_protocol_is_implemented(protocol: ImageProtocol) -> bool {
    matches!(protocol, ImageProtocol::Kitty)
}
```

- [ ] **Step 4: Simplify `Validate for ImageBackendPreference`**

In `image.rs` lines 276-303, replace:

```rust
impl Validate for ImageBackendPreference {
    fn validate(&self) -> Result<(), ConfigError> {
        match self {
            Self::Explicit(ImageProtocol::Noop) => {
                Err(explicit_noop_error("image.backend.protocol"))
            }
            Self::Explicit(protocol) if !image_protocol_is_implemented(*protocol) => Err(
                unsupported_protocol_error(*protocol, "image.backend.protocol"),
            ),
            Self::AutoDetect { order } if order.is_empty() => Err(ConfigError::new(
                "image.backend.order",
                "auto-detect backend preference requires at least one protocol",
            )),
            Self::AutoDetect { order } if order.contains(&ImageProtocol::Noop) => {
                Err(explicit_noop_error("image.backend.order"))
            }
            Self::AutoDetect { order }
                if !order.iter().copied().any(image_protocol_is_implemented) =>
            {
                Err(ConfigError::new(
                    "image.backend.order",
                    "auto-detect order contains no implemented terminal image protocol",
                ))
            }
            _ => Ok(()),
        }
    }
}
```

with the slimmer version. After the variant deletions, every `ImageProtocol` is "implemented", so the "not implemented" arms become trivially unreachable. `Validate` shrinks to:

```rust
impl Validate for ImageBackendPreference {
    fn validate(&self) -> Result<(), ConfigError> {
        match self {
            Self::Explicit(ImageProtocol::Noop) => {
                Err(explicit_noop_error("image.backend.protocol"))
            }
            Self::AutoDetect { order } if order.is_empty() => Err(ConfigError::new(
                "image.backend.order",
                "auto-detect backend preference requires at least one protocol",
            )),
            Self::AutoDetect { order } if order.contains(&ImageProtocol::Noop) => {
                Err(explicit_noop_error("image.backend.order"))
            }
            _ => Ok(()),
        }
    }
}
```

The "auto-detect order contains no implemented protocol" arm is also gone — the only allowed protocol in `order` is now `Kitty` (Noop is rejected upstream), so any non-empty validated order necessarily contains an implemented protocol. The `select_auto_detect_surface` helper still iterates safely.

- [ ] **Step 5: Delete the four Sixel/iTerm2 tests in `image.rs`**

In `image.rs`, locate and delete the entire test functions (with their `#[test]` attribute):

- `fn backend_preference_rejects_unimplemented_explicit_protocol` (line 585-593 region — references `ImageProtocol::Sixel`)
- `fn backend_preference_rejects_auto_detect_without_implemented_protocol` (line 595-605 region — references `Sixel`, `ITerm2`)
- `fn surface_registry_rejects_unimplemented_explicit_protocol` (line 639-648 region — references `Sixel`)
- `fn surface_registry_auto_detects_first_implemented_protocol` (line 650-658 region — references `ITerm2`)
- `fn surface_registry_rejects_auto_detect_without_supported_protocol` (line 660-669 region — references `Sixel`)

Add one tiny replacement test to cover the new auto-detect-with-Kitty happy path, so the lost coverage is restored:

```rust
    #[test]
    fn surface_registry_auto_detects_kitty_from_order() {
        let registry = ImageSurfaceRegistry::from_preference(ImageBackendPreference::AutoDetect {
            order: vec![ImageProtocol::Kitty],
        })
        .unwrap();

        assert_eq!(registry.capabilities().protocol, ImageProtocol::Kitty);
    }
```

Insert it near the other `surface_registry_*` tests inside the same `#[cfg(test)] mod tests` block.

- [ ] **Step 6: Build tui-kit**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet 2>&1 | tail -10`
Expected: clean. If the compiler complains about an unused `unreachable!` import or similar, follow the diagnostic.

- [ ] **Step 7: Run tui-kit tests (will fail until Task 8 deletes the matching `terminal.rs` test)**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet 2>&1 | tail -10`
Expected: compile error in `src/terminal.rs` referencing `ImageProtocol::Sixel` (this is `terminal_config_rejects_unimplemented_explicit_protocol_before_entry` at line 283). That's Task 8's problem; proceed.

Do not commit yet — wait until Task 8 so the repo compiles cleanly at every commit boundary.

---

## Task 8: Remove Sixel reference from `terminal.rs` tests

**Files:**
- Modify: `/Users/coleshaffer/Projects/tui-kit/src/terminal.rs:282-300` (test function `terminal_config_rejects_unimplemented_explicit_protocol_before_entry`)

- [ ] **Step 1: Delete the test function**

In `/Users/coleshaffer/Projects/tui-kit/src/terminal.rs`, locate and delete the entire test function (with its `#[test]` attribute) named `terminal_config_rejects_unimplemented_explicit_protocol_before_entry`. It is the only block in `terminal.rs` that names `ImageProtocol::Sixel`. The exact span is approximately lines 282-292:

```rust
    #[test]
    fn terminal_config_rejects_unimplemented_explicit_protocol_before_entry() {
        let error = TerminalConfig {
            image_backend: ImageBackendPreference::Explicit(ImageProtocol::Sixel),
        }
        .validate()
        .unwrap_err();

        assert_eq!(error.path, "terminal.image_backend.protocol");
        assert!(error.reason.contains("not implemented"));
    }
```

Delete those lines plus the surrounding blank lines as needed to leave one blank line between adjacent tests.

- [ ] **Step 2: Build tui-kit**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 3: Run tui-kit tests**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet 2>&1 | tail -5`
Expected: all green. Test count = (Task 1 baseline) + 1 (the Task 2 boxed-forwarding test) + 1 (the new `surface_registry_auto_detects_kitty_from_order` test) - 5 (the five deleted Sixel/iTerm2 tests in `image.rs`) - 1 (the deleted `terminal.rs` test).

- [ ] **Step 4: Build c4tui against the modified tui-kit**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo build --quiet 2>&1 | tail -5`
Expected: clean. c4tui never names `Sixel` or `ITerm2`.

- [ ] **Step 5: Run c4tui tests**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo test --quiet 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 6: Commit (covers both Task 7 and Task 8 — first time the tree is green since Task 6)**

```bash
cd /Users/coleshaffer/Projects/tui-kit
git add src/image.rs src/terminal.rs
git commit -m "reduce ImageProtocol to { Kitty, Noop } and drop unreachable validation paths"
```

---

## Task 9: Slim `src/prelude.rs` (Item #9)

**Files:**
- Modify: `/Users/coleshaffer/Projects/tui-kit/src/prelude.rs` (complete rewrite)

**The rule:** prelude contains *constructors and traits* only. Configuration, placement, error, and internal-state types live behind their module paths (`tui_kit::widgets::image_box::*`, `tui_kit::widgets::image_viewport::*`, `tui_kit::image::*`, `tui_kit::layout::*`).

**Inventory (current prelude → after rewrite):**

Kept (constructors, traits, and types used by typical apps via `use tui_kit::prelude::*`):
- `bar`: `SegmentSlot`, `StatusFragment` (status-bar building blocks)
- `component`: `BufferComponent`, `Cached`, `Component*` ID/outcome/dirty types — these are *the* trait surface
- `elements`: `Element`, `ElementExt`, `ElementOutcome` (the subtrait + extension trait), `EffectElement`
- `events`: all public event types (an app glob-imports to wire the event loop)
- `focus`: `FocusConfig`, `FocusId`, `FocusManager`, `FocusNode`, `FocusScopeKind`
- `image`: `MAIN_PLACEMENT_ID`, `PICKER_PLACEMENT_ID_BASE`, `picker_placement_id`, `ImageProtocol`, `ImageBackendPreference`, `ImageSurface`, `ImageSurfaceRegistry`, `KittyImageRegistry`, `NoopImageSurface`, `ImageCapabilities`, `PlaceOptions`, `TransparencySupport` — all needed at construction time
- `input`: `Key`
- `keymap`: `KeyBinding`, `KeyMap`, `KeyTrigger`, `SpecialKey`
- `layout`: `fit_scale`, `CanvasMetrics`, `CellSize`, `CellOffset`, `CellPixel`, `CellRect`, `CellArea`, `Placement`, `PlacementEngine`, `PixelRect`, `PixelSize`, `MAX_SCALE`, `MIN_SCALE` — the geometric vocabulary
- `scheduler`: `Scheduler`, `Priority`, `Progress`, `Completion`, `RequestScope`, `CancellationReport`, `SchedulerStats`
- `terminal`: `Terminal`, `TerminalConfig`
- `tty`: `stdin_is_terminal`, `stdout_is_terminal`, `terminal_metrics`, `write_stdout_all`
- `watcher`: `WorkspaceWatcher`
- `widgets::dialog`: `Dialog`
- `widgets::grid`: `Grid`, `GridStyle`, `GridCell`, `GridCellPlacement`, `GridColumnMode`, `GridNavigation` (constructors + nav primitives) — but **drop** `GridCellCanvas`, `GridInputOutcome`, `GridRenderState` (internal-state types)
- `widgets::image_box`: `ImageBox`, `ImageBoxState`, `ImageBoxPlan` (constructor + its state/plan return types) — but **drop** `ImageBoxPlacement`, `ImageBoxStyle` (configuration types)
- `widgets::image_viewport`: `ImageViewport`, `ImageViewportElement`, `ImageViewportWidget`, `ImageViewportOptions`, `ImageScale` (constructor + the options struct + the public scale handle) — but **drop** the rest

Removed from prelude (still reachable at their module path):
- `CachedRenderStats` (instrumentation)
- `config::{ConfigError, Validate}` (error type, internal trait — moved out)
- `bar`: no removals
- `elements`: drop `Bordered`, `ContainerElement`, `ElementBorder`, `Focusable`, `KeyResolution`, `KeyScope`, `KeyScopeResolver`, `KeyScopeRole`, `Modal`, `Overlay`, `Padded`, `Padding`, `Panel`, `ScrollY`, `Stack`, `StackConstraint`, `StackDirection`, `TerminalEffect`, `Text`, `TextOverflow`, `Window`, `WindowChrome`, `WindowFocusScope`, `WindowLifecycleEvent`, `WindowRenderStats`, `WindowRepaintPolicy` — these are containers and decorators that apps already reach by name from the `elements` module when they need them. Confirmed via cross-repo grep that c4tui imports zero of these from the prelude (`grep -n "ScrollY\|StackDirection\|...Overlay" /Users/coleshaffer/Projects/c4tui/src/*.rs` → empty).
- `layout`: drop `CellRoundingPolicy`, `ClippedSides`, `ImageAnchorPolicy`, `ImageOverflowPolicy`, `ImagePoint`, `ImageScaleBasis`, `ImageZoomLimitPolicy`, `PlacementAnchor`, `PlacementPolicy`, `TailViewport`, `ViewTransform` (placement policy types — reached via `tui_kit::layout::*` directly, as c4tui already does)
- `widgets::image_viewport`: drop `CanvasUpdate`, `ImageViewportError`, `ImageViewportInitialScale`, `ImageViewportPlacement`, `PixelDistance`, `PixelExtent`, `ResizePolicy`, `RgbaImage`, `ScaledPixelOffset`, `ScaleBasis`, `StepDirection`, `UnscaledPixelOffset`, `ViewportAxis`, `ViewportImage`, `ZoomDirection`, `ZoomFactor` — c4tui imports these from `tui_kit::widgets::image_viewport::*` already
- `widgets::image_box`: drop `ImageBoxPlacement`, `ImageBoxStyle`

- [ ] **Step 1: Replace `src/prelude.rs` with the slim version**

Overwrite `/Users/coleshaffer/Projects/tui-kit/src/prelude.rs` with exactly:

```rust
//! Common imports for production tui-kit consumers.
//!
//! ```ignore
//! use tui_kit::prelude::*;
//! ```
//!
//! Scope: constructors and traits an app reaches for at the import line, plus
//! the small set of return/state types those constructors hand back. Internal
//! state, configuration, placement, and error types live behind their module
//! paths (`tui_kit::widgets::image_box::*`, `tui_kit::layout::*`,
//! `tui_kit::widgets::image_viewport::*`) so glob-importing the prelude does
//! not pollute consumer namespaces with policy enums and error structs.
//!
//! Test harness helpers stay under [`crate::testkit`].

pub use crate::bar::{SegmentSlot, StatusFragment};
pub use crate::component::{
    BufferComponent, Cached, ComponentChildren, ComponentId, ComponentOutcome, DirtyReason,
    DirtyState,
};
pub use crate::elements::{EffectElement, Element, ElementExt, ElementOutcome};
pub use crate::events::{
    AppEvent, AppEventReceiver, AppEventSender, InputEvent, SchedulerEvent, TerminalEvent,
    WatcherEvent,
};
pub use crate::focus::{FocusConfig, FocusId, FocusManager, FocusNode, FocusScopeKind};
pub use crate::image::{
    picker_placement_id, ImageBackendPreference, ImageCapabilities, ImageProtocol, ImageSurface,
    ImageSurfaceRegistry, KittyImageRegistry, NoopImageSurface, PlaceOptions, TransparencySupport,
    MAIN_PLACEMENT_ID, PICKER_PLACEMENT_ID_BASE,
};
pub use crate::input::Key;
pub use crate::keymap::{KeyBinding, KeyMap, KeyTrigger, SpecialKey};
pub use crate::layout::{
    fit_scale, CanvasMetrics, CellArea, CellOffset, CellPixel, CellRect, CellSize, PixelRect,
    PixelSize, Placement, PlacementEngine, MAX_SCALE, MIN_SCALE,
};
pub use crate::scheduler::{
    CancellationReport, Completion, Priority, Progress, RequestScope, Scheduler, SchedulerStats,
};
pub use crate::terminal::{Terminal, TerminalConfig};
pub use crate::tty::{stdin_is_terminal, stdout_is_terminal, terminal_metrics, write_stdout_all};
pub use crate::watcher::WorkspaceWatcher;
pub use crate::widgets::dialog::Dialog;
pub use crate::widgets::grid::{
    Grid, GridCell, GridCellPlacement, GridColumnMode, GridNavigation, GridStyle,
};
pub use crate::widgets::image_box::{ImageBox, ImageBoxPlan, ImageBoxState};
pub use crate::widgets::image_viewport::{
    ImageScale, ImageViewport, ImageViewportElement, ImageViewportOptions, ImageViewportWidget,
};
```

(Notes: `CachedRenderStats` is dropped — only ever needed by tests using `cached.stats()`, which access it through the inherent method; the struct itself is re-exported through `component::CachedRenderStats` if someone explicitly needs the type. Same logic applies to internal stats / error / policy types.)

- [ ] **Step 2: Build tui-kit**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet 2>&1 | tail -10`
Expected: clean build. The library itself does not depend on the prelude.

- [ ] **Step 3: Build visual-tests**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet -p visual-tests 2>&1 | tail -10`
Expected: clean. `visual-tests/src/main.rs` imports `ImageBox, ImageBoxPlan, ImageBoxState, Terminal, TerminalConfig` from prelude — all five survive the slim.

If `cargo build -p visual-tests` fails because the workspace member name differs, run `cargo build --manifest-path /Users/coleshaffer/Projects/tui-kit/visual-tests/Cargo.toml --quiet` instead.

- [ ] **Step 4: Build the example**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo build --quiet --example terminal_dialog 2>&1 | tail -10`
Expected: clean. `terminal_dialog.rs` imports `tui_kit::prelude::*` for `Terminal` and `Dialog` — both kept.

- [ ] **Step 5: Build c4tui against the slimmed prelude**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo build --quiet 2>&1 | tail -10`
Expected: clean. c4tui never imports the prelude — `grep -rn "tui_kit::prelude" /Users/coleshaffer/Projects/c4tui/src/` returns nothing (verified during ground-truth). If this build fails, the failure is the test of that claim — apply the import-path follow-up in the failing file in the same task before committing.

- [ ] **Step 6: Run tui-kit tests**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 7: Run c4tui tests**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo test --quiet 2>&1 | tail -5`
Expected: all green.

- [ ] **Step 8: Commit**

```bash
cd /Users/coleshaffer/Projects/tui-kit
git add src/prelude.rs
git commit -m "slim prelude to constructors and traits"
```

---

## Task 10: Final cross-repo verification

**Files:** none — verification only.

- [ ] **Step 1: `cargo test` in tui-kit**

Run: `cd /Users/coleshaffer/Projects/tui-kit && cargo test 2>&1 | tail -20`
Expected: every binary/lib/test/example target compiles and tests pass. Take note of the final `test result:` line; the count should be `(Task 1 baseline) + 2 (new tests added in Tasks 2 and 7) - 6 (Sixel/iTerm2 tests + the terminal.rs test)`.

- [ ] **Step 2: `cargo build` in c4tui**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo build 2>&1 | tail -10`
Expected: clean build of all targets.

- [ ] **Step 3: `cargo test` in c4tui**

Run: `cd /Users/coleshaffer/Projects/c4tui && cargo test 2>&1 | tail -10`
Expected: all green.

- [ ] **Step 4: Negative-control grep — confirm `Component` trait, Sixel, and iTerm2 are gone from production**

```
cd /Users/coleshaffer/Projects
grep -rn "pub trait Component\b\|ImageProtocol::Sixel\|ImageProtocol::ITerm2" tui-kit/src/ c4tui/src/
```
Expected: empty. (The deleted trait, the deleted variants, and the deleted match arms.)

- [ ] **Step 5: Negative-control grep — confirm no `impl Element for` blocks remain**

```
grep -rn "impl Element for\|impl<.*> Element for" /Users/coleshaffer/Projects/tui-kit/src/
```
Expected: empty. Every implementation now reads `impl BufferComponent for ...`.

- [ ] **Step 6: Positive-control grep — confirm `Element` subtrait is the only `Element` trait reference**

```
grep -n "pub trait Element\b" /Users/coleshaffer/Projects/tui-kit/src/elements.rs
```
Expected: exactly one line, the marker subtrait declaration.

- [ ] **Step 7: Print the final commit graph for human review**

Run: `cd /Users/coleshaffer/Projects/tui-kit && git log --oneline -10`
Expected: a clean per-task commit history covering:
1. add forwarding BufferComponent impl for boxed trait objects
2. drop frame-based Component trait in favor of BufferComponent
3. collapse Element trait into BufferComponent<Event=Key> subtrait
4. refresh elements.rs doc comments after trait collapse
5. reduce ImageProtocol to { Kitty, Noop } and drop unreachable validation paths
6. slim prelude to constructors and traits

(Plus the c4tui commit `drop dead Component import after tui-kit trait collapse` on the c4tui side.)

Phase 1 complete.

---

## Self-review checklist (against the spec)

- **#3 trait collapse:** Tasks 2-6. `Component` deleted (Task 3). `Element` becomes a subtrait of `BufferComponent<Event=Key>` (Task 4) and every impl rewritten (Task 5). `Cached<C>` keeps its inherent `render_to_buffer` — verified untouched. ✓
- **#9 prelude slim:** Task 9. Configuration/policy/error/internal-state types moved out; constructors and traits preserved. ✓
- **#10 ImageProtocol reduction:** Tasks 7-8. Sixel/iTerm2 variants gone; `unsupported_protocol_error` and `image_protocol_is_implemented` helpers deleted; five tests in `image.rs` and one test in `terminal.rs` deleted with a replacement Kitty-auto-detect happy-path test added. ✓
- **No shims / no deprecation comments:** all changes are rip-and-replace; no `#[deprecated]`, no "kept for migration" doc lines. ✓
- **Atomic across repos:** Task 3 modifies both `tui-kit/src/component.rs` and `c4tui/src/app.rs` in the same task; every subsequent task verifies both repos build before its commit. ✓
- **Verification end state:** Task 10 runs `cargo test` in tui-kit and `cargo build` + `cargo test` in c4tui. ✓
