// ============================================================================
// File: core.rs
// ============================================================================
// ----------------------------------------------------------------------------
// GAME CORE STATE, GLOBAL RESOURCES & CONSOLE DEFINITIONS
// ----------------------------------------------------------------------------

#![allow(unexpected_cfgs)]

use std::collections::BTreeSet;
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

#[allow(clippy::upper_case_acronyms)]
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum CameraMode {
    #[default]
    FPS,
    RTS,
}

// ----------------------------------------------------------------------------
// PHYSICS LAYERS & COLLISION GROUPS
// ----------------------------------------------------------------------------

#[derive(PhysicsLayer, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameLayer {
    #[default]
    Default,
    Terrain,
    Unit,
    Environment,
    /// Selective permeability layer: blocks physical movement (Units, Default),
    /// but transparent to vision raycasts / LoS queries.
    Glass,
}

// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------
// DIRECTIONAL MELEE COMBAT TYPES (MOUNT & BLADE / cRPG STYLE)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeleeSwingDirection {
    #[default]
    Right,    // Left-to-Right diagonal slash (mouse flick right +X)
    Left,     // Right-to-Left backhand slash (mouse flick left -X)
    Overhead, // Vertical downward cleave (mouse pull down +Y)
    Thrust,   // Linear forward stab (mouse push up -Y / neutral)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeleeAttackPhase {
    #[default]
    Idle,
    Windup,
    Release,
    Recovery,
}

#[derive(Resource)]
pub struct SwingState {
    pub is_swinging: bool,
    pub timer: Timer,
    pub offhand_is_swinging: bool,
    pub offhand_timer: Timer,

    // Directional Combat
    pub direction: MeleeSwingDirection,
    pub phase: MeleeAttackPhase,
    pub windup_timer: Timer,
    pub release_timer: Timer,
    pub recovery_timer: Timer,

    // Shield Guard / Defense
    pub is_blocking: bool,
    #[allow(dead_code)]
    pub block_stagger_timer: Timer,
}

impl Default for SwingState {
    fn default() -> Self {
        Self {
            is_swinging: false,
            timer: Timer::from_seconds(0.25, TimerMode::Once),
            offhand_is_swinging: false,
            offhand_timer: Timer::from_seconds(0.25, TimerMode::Once),
            direction: MeleeSwingDirection::Right,
            phase: MeleeAttackPhase::Idle,
            windup_timer: Timer::from_seconds(0.12, TimerMode::Once),
            release_timer: Timer::from_seconds(0.26, TimerMode::Once),
            recovery_timer: Timer::from_seconds(0.16, TimerMode::Once),
            is_blocking: false,
            block_stagger_timer: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}

/// Tracks smoothed mouse movement vectors to classify directional attacks
#[derive(Resource, Debug, Clone, Default)]
pub struct MouseFlickTracker {
    pub smoothed_dx: f32,
    pub smoothed_dy: f32,
}

impl MouseFlickTracker {
    pub fn update(&mut self, dx: f32, dy: f32) {
        self.smoothed_dx = self.smoothed_dx * 0.75 + dx * 0.25;
        self.smoothed_dy = self.smoothed_dy * 0.75 + dy * 0.25;
    }

    pub fn decay(&mut self, factor: f32) {
        self.smoothed_dx *= factor;
        self.smoothed_dy *= factor;
    }

    pub fn classify(&self) -> MeleeSwingDirection {
        let abs_x = self.smoothed_dx.abs();
        let abs_y = self.smoothed_dy.abs();
        let threshold = 1.0;

        if abs_x > abs_y && abs_x > threshold {
            if self.smoothed_dx > 0.0 {
                MeleeSwingDirection::Right
            } else {
                MeleeSwingDirection::Left
            }
        } else if abs_y >= abs_x && abs_y > threshold {
            if self.smoothed_dy > 0.0 {
                MeleeSwingDirection::Overhead
            } else {
                MeleeSwingDirection::Thrust
            }
        } else {
            MeleeSwingDirection::Right // Default slash
        }
    }
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
    // AI_RULES.md Rule 2.1 #3: BTreeSet enforces deterministic ordering without randomized SipHash
    #[allow(dead_code)]
    pub chunks: BTreeSet<(i32, i32)>,
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
            radius: 2,
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

// ----------------------------------------------------------------------------
// FRAMERATE LIMITER STATE & PACING SYSTEM
// ----------------------------------------------------------------------------

/// Resource controlling client framerate limits and pacing.
#[derive(Resource, Debug, Clone)]
pub struct FpsLimiterState {
    /// Target maximum frames per second. If None or <= 0, framerate is uncapped.
    pub target_fps: Option<u32>,
    /// Instant when the previous frame completed execution.
    pub last_frame_instant: Option<std::time::Instant>,
}

impl Default for FpsLimiterState {
    fn default() -> Self {
        Self {
            target_fps: None, // Uncapped by default (VSync disabled via AutoNoVsync)
            last_frame_instant: None,
        }
    }
}

/// Enforces the maximum FPS cap by pacing frame execution in the `Last` schedule.
/// Uses a hybrid OS-sleep + sub-millisecond spin loop to guarantee microsecond precision
/// without excessive CPU busy-waiting.
pub fn enforce_fps_limit(mut limiter: ResMut<FpsLimiterState>) {
    let Some(target_fps) = limiter.target_fps else {
        limiter.last_frame_instant = Some(std::time::Instant::now());
        return;
    };

    if target_fps == 0 {
        limiter.last_frame_instant = Some(std::time::Instant::now());
        return;
    }

    let target_frame_duration = std::time::Duration::from_secs_f64(1.0 / target_fps as f64);

    if let Some(last_instant) = limiter.last_frame_instant {
        let elapsed = last_instant.elapsed();
        if elapsed < target_frame_duration {
            let remaining = target_frame_duration - elapsed;
            // Coarse sleep for durations longer than 2ms, preserving CPU budget
            if remaining > std::time::Duration::from_millis(2) {
                std::thread::sleep(remaining - std::time::Duration::from_millis(1));
            }
            // Sub-millisecond spin-loop for pinpoint frame boundary alignment
            while last_instant.elapsed() < target_frame_duration {
                std::hint::spin_loop();
            }
        }
    }

    limiter.last_frame_instant = Some(std::time::Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fps_limiter_defaults_uncapped() {
        let limiter = FpsLimiterState::default();
        assert_eq!(limiter.target_fps, None);
        assert!(limiter.last_frame_instant.is_none());
    }

    #[test]
    fn test_fps_limiter_pacing_system() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<FpsLimiterState>();
        app.add_systems(Update, enforce_fps_limit);

        // Run 1 tick uncapped
        app.update();
        let limiter = app.world().resource::<FpsLimiterState>();
        assert!(limiter.last_frame_instant.is_some());

        // Set to 60 FPS
        app.world_mut().resource_mut::<FpsLimiterState>().target_fps = Some(60);
        app.update();
        let limiter = app.world().resource::<FpsLimiterState>();
        assert_eq!(limiter.target_fps, Some(60));

        // Uncap again
        app.world_mut().resource_mut::<FpsLimiterState>().target_fps = None;
        app.update();
        let limiter = app.world().resource::<FpsLimiterState>();
        assert_eq!(limiter.target_fps, None);
    }

    #[test]
    fn test_mouse_flick_directional_classification() {
        let mut tracker = MouseFlickTracker::default();
        // Default classification when below threshold is Right
        assert_eq!(tracker.classify(), MeleeSwingDirection::Right);

        // Right flick
        tracker.update(10.0, 0.0);
        assert_eq!(tracker.classify(), MeleeSwingDirection::Right);

        // Left flick
        let mut tracker = MouseFlickTracker::default();
        tracker.update(-10.0, 0.0);
        assert_eq!(tracker.classify(), MeleeSwingDirection::Left);

        // Overhead cleave (+Y in mouse delta)
        let mut tracker = MouseFlickTracker::default();
        tracker.update(0.0, 10.0);
        assert_eq!(tracker.classify(), MeleeSwingDirection::Overhead);

        // Forward thrust (-Y in mouse delta)
        let mut tracker = MouseFlickTracker::default();
        tracker.update(0.0, -10.0);
        assert_eq!(tracker.classify(), MeleeSwingDirection::Thrust);
    }

    #[test]
    fn test_mouse_flick_decay() {
        let mut tracker = MouseFlickTracker {
            smoothed_dx: 10.0,
            smoothed_dy: 5.0,
        };
        tracker.decay(0.5);
        assert!((tracker.smoothed_dx - 5.0).abs() < 1e-4);
        assert!((tracker.smoothed_dy - 2.5).abs() < 1e-4);
    }

    #[test]
    fn test_swing_state_defaults() {
        let swing = SwingState::default();
        assert!(!swing.is_swinging);
        assert!(!swing.is_blocking);
        assert_eq!(swing.phase, MeleeAttackPhase::Idle);
        assert_eq!(swing.direction, MeleeSwingDirection::Right);
        assert_eq!(swing.windup_timer.duration().as_secs_f32(), 0.12);
        assert_eq!(swing.release_timer.duration().as_secs_f32(), 0.26);
        assert_eq!(swing.recovery_timer.duration().as_secs_f32(), 0.16);
    }
}