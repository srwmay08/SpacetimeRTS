#![allow(unexpected_cfgs)] // Architectural Note: Suppresses third-party Avian3D macro warnings.

use bevy::prelude::*;
use avian3d::prelude::*;

// ----------------------------------------------------------------------------
// GAME STATES
// ----------------------------------------------------------------------------

/// Tracks the top-level execution state of the client.
/// Architectural Note: Separating `Connecting` and `InGame` ensures we do not 
/// boot heavy rendering or physics systems until the SpacetimeDB identity is fully resolved.
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Connecting,
    InGame,
}

/// Determines the active control scheme and network culling bounds.
/// Architectural Note: FPS mode relies on high-frequency micro-data streaming.
/// RTS mode triggers a SpacetimeDB subscription rebuild to stream macro-data.
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum CameraMode {
    #[default]
    FPS,
    RTS,
}

// ----------------------------------------------------------------------------
// PHYSICS LAYERS & COLLISION GROUPS
// ----------------------------------------------------------------------------

/// Collision layers for Avian3D physics.
/// Architectural Note: Isolating `Terrain` from `Unit` and `Environment` minimizes 
/// expensive continuous collision detection (CCD) checks during high-density RTS movement.
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

/// Manages client-side prediction for the harvesting tool animation.
/// Architectural Note: We predict the swing locally for responsiveness, while the actual 
/// resource gathering logic executes authoritatively in the SpacetimeDB reducer.
#[derive(Resource)]
pub struct SwingState {
    pub is_swinging: bool,
    pub timer: Timer,
}

/// Tracks the highest combat event ID processed from SpacetimeDB.
/// Architectural Note: Since SpacetimeDB pushes the entire table state on updates, 
/// we must track the `last_event_id` locally to avoid re-triggering visual effects for old events.
#[derive(Resource, Default)]
pub struct EventTracker {
    pub last_event_id: u64,
}

/// Monitors frame times to detect UI/Physics stutters.
/// Architectural Note: Important for ensuring our ECS interpolation logic doesn't 
/// desync heavily from the fixed SpacetimeDB server tick.
#[derive(Resource)]
pub struct TelemetryTracker {
    pub last_frame_time: f64,
    pub frame_drop_threshold: f64,
}

/// Tracks the active RTS box-selection state.
#[derive(Resource, Default)]
pub struct SelectionState {
    pub is_dragging: bool,
    pub start_pos: Option<Vec2>,
    pub end_pos: Option<Vec2>,
}

/// Caches the generated chunk coordinates to avoid redundant generation.
#[derive(Resource, Default)]
pub struct GeneratedChunks {
    pub chunks: std::collections::HashSet<(i32, i32)>,
}

/// Controls the frequency at which we push client transforms to SpacetimeDB.
/// Architectural Note: Locked to 0.05s (20Hz) to preserve server TeV compute energy.
#[derive(Resource)] 
pub struct NetworkTickTimer(pub Timer);

/// Manages the dynamic SpacetimeDB SQL subscription for spatial partitioning.
/// Architectural Note: Tracks the client's current chunk and rebuilds queries 
/// when boundaries are crossed to isolate the network load strictly to the required perimeter.
#[derive(Resource)]
pub struct NetworkCullingState {
    pub current_chunk: (i32, i32),
    pub radius: i32,
    pub needs_rebuild: bool,
}

impl Default for NetworkCullingState {
    fn default() -> Self {
        Self {
            current_chunk: (0, 0),
            radius: 1, // Default FPS 3x3 perimeter limit
            needs_rebuild: true,
        }
    }
}