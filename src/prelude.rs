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
pub use crate::elements::{
    EffectElement, Element, ElementExt, ElementOutcome, ImageViewportElement,
};
pub use crate::events::{
    AppEvent, AppEventReceiver, AppEventSender, SchedulerEvent, TerminalEvent, WatcherEvent,
};
pub use crate::focus::{FocusConfig, FocusId, FocusManager, FocusNode, FocusScopeKind};
pub use crate::image::{
    picker_placement_id, ImageBackendPreference, ImageCapabilities, ImageProtocol, ImageSurface,
    ImageSurfaceRegistry, KittyImageRegistry, NoopImageSurface, PlaceOptions, TransparencySupport,
    MAIN_PLACEMENT_ID, PICKER_PLACEMENT_ID_BASE,
};
pub use crate::input::{InputEvent, KeyEvent, MouseEvent};
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
    ImageScale, ImageViewport, ImageViewportOptions, ImageViewportWidget,
};
