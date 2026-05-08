//! Common imports for tui-kit consumers.
//!
//! ```ignore
//! use tui_kit::prelude::*;
//! ```

pub use crate::bar::{Segment, SegmentBar, SegmentSlot, StatusFragment};
pub use crate::component::{
    BufferComponent, Cached, CachedRenderStats, Component, ComponentChildren, ComponentId,
    ComponentOutcome, DirtyReason, DirtyState,
};
pub use crate::config::{ConfigError, KitConfig, Validate};
pub use crate::events::{
    AppEvent, AppEventReceiver, AppEventSender, InputEvent, RuntimeEvent, SchedulerEvent,
    TerminalEvent, TickEvent, WatcherEvent,
};
pub use crate::focus::{
    FocusConfig, FocusId, FocusManager, FocusNode, FocusScopeKind, FocusTraversal,
};
pub use crate::image::{
    picker_placement_id, ImageBackendPreference, ImageCapabilities, ImageConfig, ImageProtocol,
    ImageSurface, ImageSurfaceRegistry, KittyImageRegistry, NoopImageSurface, PlaceOptions,
    TransparencySupport, MAIN_PLACEMENT_ID, PICKER_PLACEMENT_ID_BASE,
};
pub use crate::input::Key;
pub use crate::keymap::{KeyBinding, KeyMap, KeyTrigger, SpecialKey};
pub use crate::layout::{
    fit_scale, CanvasMetrics, CellOffset, CellPixel, CellRect, CellRoundingPolicy, CellSize,
    ClippedSides, ImageAnchorPolicy, ImageOverflowPolicy, ImagePoint, ImageScaleBasis,
    ImageZoomLimitPolicy, PixelRect, PixelSize, Placement, PlacementAnchor, PlacementEngine,
    PlacementPolicy, ViewTransform, MAX_SCALE, MIN_SCALE,
};
pub use crate::runtime::RuntimeConfig;
pub use crate::scheduler::{
    CancellationReport, Completion, Priority, Progress, RequestScope, Scheduler, SchedulerConfig,
    SchedulerStats,
};
pub use crate::terminal::{Terminal, TerminalConfig};
pub use crate::testkit::{
    render_stateful_widget, render_widget, test_area, test_cell_pixels, EventScript, MockImageCall,
    MockImageSurface,
};
pub use crate::theme::{ThemeConfig, ThemeRole, REQUIRED_THEME_ROLES};
pub use crate::tick::{
    spawn as spawn_tick_source, MissedTickPolicy, TickConfig, TickHandle, TickSourceId,
    TickStartPolicy,
};
pub use crate::tty::{stdin_is_terminal, stdout_is_terminal, terminal_metrics, write_stdout_all};
pub use crate::watcher::{WatcherConfig, WatcherSourceId, WorkspaceWatcher};
pub use crate::widgets::dialog::{
    Dialog, DialogAction, DialogConfig, DialogDismissPolicy, DialogFocusId, DialogFocusPolicy,
    DialogOutcome, DialogState,
};
pub use crate::widgets::list::{
    ListAction, ListConfig, ListItem, ListItemId, ListOutcome, ListSelectionMode, ListState,
    ListTextOverflow, ListViewport,
};
pub use crate::widgets::picker::{
    Picker, PickerAction, PickerConfig, PickerItem, PickerOutcome, PickerWidget, ThumbnailRequest,
};
pub use crate::widgets::table::{
    TableAction, TableAlignment, TableColumn, TableColumnId, TableColumnLayout, TableColumnSizing,
    TableConfig, TableOutcome, TableRow, TableRowId, TableSelectionMode, TableState, TableViewport,
};
pub use crate::widgets::tabs::{
    PaneChild, PaneId, PaneLayout, PaneLayoutEngine, PaneLayoutEntry, PaneNode, PaneSizePolicy,
    PaneSplitAxis, TabAction, TabConfig, TabId, TabItem, TabOutcome, TabReorderDirection, TabState,
    TabViewport,
};
pub use crate::widgets::tree::{
    TreeAction, TreeCheckboxMode, TreeCheckboxState, TreeConfig, TreeNode, TreeNodeId, TreeOutcome,
    TreeSelectionMode, TreeState, TreeViewport, TreeVisibleNode,
};
