//! Common imports for tui-kit consumers.
//!
//! ```ignore
//! use tui_kit::prelude::*;
//! ```

pub use crate::bar::{Segment, SegmentBar, SegmentSlot, StatusFragment};
pub use crate::config::{ConfigError, KitConfig, Validate};
pub use crate::events::{
    AppEvent, AppEventReceiver, AppEventSender, InputEvent, RuntimeEvent, SchedulerEvent,
    TerminalEvent, TickEvent, WatcherEvent,
};
pub use crate::image::{
    picker_placement_id, ImageBackendPreference, ImageCapabilities, ImageProtocol, ImageSurface,
    ImageSurfaceRegistry, KittyImageRegistry, NoopImageSurface, PlaceOptions, TransparencySupport,
    MAIN_PLACEMENT_ID, PICKER_PLACEMENT_ID_BASE,
};
pub use crate::input::Key;
pub use crate::keymap::{KeyBinding, KeyMap, KeyTrigger, SpecialKey};
pub use crate::layout::{
    fit_scale, CanvasMetrics, CellOffset, CellPixel, CellRect, CellRoundingPolicy, CellSize,
    ClippedSides, ImageAnchorPolicy, ImageOverflowPolicy, ImagePoint, ImageScaleBasis,
    ImageZoomLimitPolicy, PixelRect, PixelSize, Placement, PlacementAnchor, PlacementEngine,
    PlacementPolicy, ViewTransform, MAX_SCALE, MIN_SCALE,
};
pub use crate::scheduler::{Completion, Priority, Progress, Scheduler};
pub use crate::terminal::{Terminal, TerminalConfig};
pub use crate::tick::{
    spawn as spawn_tick_source, MissedTickPolicy, TickConfig, TickHandle, TickSourceId,
    TickStartPolicy,
};
pub use crate::tty::{stdin_is_terminal, stdout_is_terminal, terminal_metrics, write_stdout_all};
pub use crate::watcher::WorkspaceWatcher;
pub use crate::widgets::dialog::Dialog;
pub use crate::widgets::picker::{
    Picker, PickerAction, PickerConfig, PickerItem, PickerOutcome, PickerWidget, ThumbnailRequest,
};
