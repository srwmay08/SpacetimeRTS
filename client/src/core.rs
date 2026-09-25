// ============================================================================
// File: core.rs
// ============================================================================
// ----------------------------------------------------------------------------
// GAME CORE STATE, GLOBAL RESOURCES & CONSOLE DEFINITIONS
// ----------------------------------------------------------------------------

#![allow(unexpected_cfgs)]

use bevy::prelude::*;
use avian3d::prelude::*;

// ----------------------------------------------------------------------------
// GAME STATES
// ----------------------------------------------------------------------------

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Connecting,
    InGame,
}

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum CameraMode {
    #[default]
    FPS,
    RTS,
}

// ----------------------------------------------------------------------------
// PHYSICS LAYERS & COLLISION GROUPS
// ----------------------------------------------------------------------------

#[derive(PhysicsLayer, Default)]
pub enum GameLayer {
    #[default]
    Default,
    Terrain,
    Unit,
    Environment,
}

// ----------------------------------------------------------------------------
// GLOBAL RESOURCES
// ----------------------------------------------------------------------------

#[derive(Resource)]
pub struct SwingState {
    pub is_swinging: bool,
    pub timer: Timer,
}

#[derive(Resource, Default)]
pub struct EventTracker {
    pub last_event_id: u64,
}

#[derive(Resource)]
pub struct TelemetryTracker {
    pub last_frame_time: f64,
    #[allow(dead_code)]
    pub frame_drop_threshold: f64,
}

#[derive(Resource, Default)]
pub struct SelectionState {
    pub is_dragging: bool,
    pub start_pos: Option<Vec2>,
    pub end_pos: Option<Vec2>,
}

#[derive(Resource, Default)]
pub struct GeneratedChunks {
    #[allow(dead_code)]
    pub chunks: std::collections::HashSet<(i32, i32)>,
}

#[derive(Resource)] 
pub struct NetworkTickTimer(pub Timer);

#[derive(Resource)]
pub struct NetworkCullingState {
    pub current_chunk: (i32, i32),
    pub radius: i32,
    pub needs_rebuild: bool,
    pub in_interior: bool,
}

impl Default for NetworkCullingState {
    fn default() -> Self {
        Self {
            current_chunk: (0, 0),
            radius: 1,
            needs_rebuild: true,
            in_interior: false,
        }
    }
}

// ----------------------------------------------------------------------------
// DEVELOPER CONSOLE & DRAG-AND-DROP STATE
// ----------------------------------------------------------------------------

#[derive(Resource)]
pub struct ConsoleState {
    pub is_open: bool,
    pub input_buffer: String,
    pub history: Vec<String>,
    pub history_cursor: Option<usize>,
    pub logs: Vec<String>,
    pub cursor_timer: Timer,
    pub show_cursor: bool,
    pub tab_completion_index: usize,
    #[allow(dead_code)]
    pub last_tab_input: String,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self {
            is_open: false,
            input_buffer: String::new(),
            history: Vec::new(),
            history_cursor: None,
            logs: vec![
                "=== REAL-TIME PLAYTEST CONSOLE INITIALIZED ===".to_string(),
                "Type 'help' for command syntax. Press [Tab] to auto-fill items & commands.".to_string(),
            ],
            cursor_timer: Timer::from_seconds(0.45, TimerMode::Repeating),
            show_cursor: true,
            tab_completion_index: 0,
            last_tab_input: String::new(),
        }
    }
}

#[derive(Resource, Default)]
pub struct DragDropState {
    pub is_dragging: bool,
    pub source_slot: Option<usize>,
    pub item_type: String,
    pub count: u32,
    pub current_pos: Vec2,
}