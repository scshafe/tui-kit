//! Common imports for production tui-kit consumers.
//!
//! ```ignore
//! use tui_kit::prelude::*;
//! ```
//!
//! Test harness helpers stay under [`crate::testkit`] instead of the prelude so
//! application imports do not accidentally couple runtime code to test-only
//! surfaces.

pub use crate::bar::{SegmentSlot, StatusFragment};
pub use crate::component::{
    BufferComponent, Cached, CachedRenderStats, Component, ComponentChildren, ComponentId,
    ComponentOutcome, DirtyReason, DirtyState,
};
pub use crate::config::{ConfigError, Validate};
pub use crate::elements::{
    Bordered, ContainerElement, EffectElement, Element, ElementBorder, ElementExt, ElementOutcome,
    Focusable, ImageViewportElement, KeyResolution, KeyScope, KeyScopeResolver, KeyScopeRole,
    Modal, Overlay, Padded, Padding, Panel, ScrollY, Stack, StackConstraint, StackDirection,
    TerminalEffect, Text, TextOverflow, Window, WindowChrome, WindowFocusScope,
    WindowLifecycleEvent, WindowRenderStats, WindowRepaintPolicy,
};
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
    fit_scale, CanvasMetrics, CellArea, CellOffset, CellPixel, CellRect, CellRoundingPolicy,
    CellSize, ClippedSides, ImageAnchorPolicy, ImageOverflowPolicy, ImagePoint, ImageScaleBasis,
    ImageZoomLimitPolicy, PixelRect, PixelSize, Placement, PlacementAnchor, PlacementEngine,
    PlacementPolicy, TailViewport, ViewTransform, MAX_SCALE, MIN_SCALE,
};
pub use crate::scheduler::{
    CancellationReport, Completion, Priority, Progress, RequestScope, Scheduler, SchedulerStats,
};
pub use crate::terminal::{Terminal, TerminalConfig};
pub use crate::tty::{stdin_is_terminal, stdout_is_terminal, terminal_metrics, write_stdout_all};
pub use crate::watcher::WorkspaceWatcher;
pub use crate::widgets::dialog::Dialog;
pub use crate::widgets::grid::{
    Grid, GridCell, GridCellCanvas, GridCellPlacement, GridInputOutcome, GridNavigation,
    GridRenderState, GridStyle,
};
pub use crate::widgets::image_box::{
    ImageBox, ImageBoxPlacement, ImageBoxPlan, ImageBoxState, ImageBoxStyle,
};
pub use crate::widgets::image_viewport::{
    CanvasUpdate, ImageScale, ImageViewport, ImageViewportError, ImageViewportInitialScale,
    ImageViewportOptions, ImageViewportPlacement, ImageViewportWidget, PixelDistance, PixelExtent,
    ResizePolicy, RgbaImage, ScaleBasis, ScaledPixelOffset, StepDirection, UnscaledPixelOffset,
    ViewportAxis, ViewportImage, ZoomDirection, ZoomFactor,
};
