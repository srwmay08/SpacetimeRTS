// ============================================================================
// File: components.rs
// ============================================================================
// ----------------------------------------------------------------------------
// CORE ECS LOGICAL, RENDERING & NETWORK COMPONENTS
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use crate::building::ModularPieceType;

// ----------------------------------------------------------------------------
// CORE LOGICAL ECS COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct LogicalPosition(pub Vec3);
#[derive(Component)] pub struct LogicalRotation(pub Quat);

#[derive(Component, Default, PartialEq, Eq)] 
#[allow(dead_code)] 
pub enum Faction { 
    #[default] 
    Player, 
    Enemy, 
    Neutral 
}

#[derive(Component)] pub struct Selectable;
#[derive(Component)] pub struct Selected;
#[derive(Component)] pub struct SelectionRing;
#[derive(Component)] pub struct MarqueeUI;
#[derive(Component)] pub struct NavTarget(pub Vec3);

// ----------------------------------------------------------------------------
// RENDERING MARKERS & CAMERAS
// ----------------------------------------------------------------------------

#[allow(dead_code)] #[derive(Component)] pub struct FPSMesh;
#[derive(Component)] pub struct RTSProxy;
#[derive(Component)] pub struct FpsCamera;
#[derive(Component)] pub struct RtsCameraRig;
#[derive(Component)] pub struct RtsCameraChild;

// ----------------------------------------------------------------------------
// NETWORKING & ENTITY MARKERS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct NetworkEntity(pub u64);
#[derive(Component)] pub struct NetworkStructure { pub structure_id: u64 }
#[derive(Component)] pub struct NetworkProjectile(pub u64);

#[derive(Component)] pub struct PlayerBody;
#[derive(Component)] pub struct PlayerHead;
#[allow(dead_code)] #[derive(Component)] pub struct ViewModelArm;
#[derive(Component)] pub struct Kcc { pub is_grounded: bool }
#[derive(Component, Debug, Clone, Default)]
pub struct LocomotionState {
    pub time_since_grounded: f32,
    pub jump_buffered_timer: f32,
}

#[derive(Component)]
pub struct ResourceNodeItem {
    pub node_id: u64,
    pub node_type: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct TreeComponent {
    /// 0 = Dead, 1 = Oak, 2 = Pine, 3 = Round
    pub species: u8,
    /// Deterministic variant index [0, 5]
    pub variant: usize,
}

#[allow(unused_imports)]
pub use crate::tree_colors::{Season, SeasonState};

#[derive(Component)] pub struct PeasantUnit { pub entity_id: u64 }

// ----------------------------------------------------------------------------
// VOXEL WORLD, FALLING LOGS & GIB COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct VoxelChunkMarker {
    pub chunk_key: u64,
    #[allow(dead_code)]
    pub chunk_x: i32,
    #[allow(dead_code)]
    pub chunk_y: i32,
    #[allow(dead_code)]
    pub chunk_z: i32,
    pub last_modified_tick: u64,
}

#[derive(Component)]
pub struct VoxelGib {
    pub timer: Timer,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
}

#[derive(Component)]
pub struct FallingTree {
    pub base_pos: Vec3,
    pub fall_dir: Vec3,
    pub initial_rotation: Quat,
    pub angle: f32,
    pub angular_vel: f32,
    pub elapsed: f32,
}

// ----------------------------------------------------------------------------
// MODULAR BUILDING & SOCKET COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component, Clone, Debug)]
pub struct Socket {
    #[allow(dead_code)] pub name: String,
    pub local_offset: Vec3,
    pub local_rotation: Quat, 
    pub is_occupied: bool,
}

#[derive(Component)]
pub struct BuildHologram;

#[derive(Component)]
pub struct BaseInteriorVolume {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Component)]
pub struct InteriorProp;

/// Explicit wrapper around the SpacetimeDB structure id for visual destruction syncing.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpacetimeBuildingId(pub u64);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BuildingHealth {
    pub current: u32,
    pub max: u32,
}

/// Tracks whether a building's mesh is currently displaying its pristine or damaged version.
/// Updated purely based on authoritative health in SpacetimeDB (swaps at <= 50% health).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VisualDamageState {
    #[default]
    Pristine,
    Damaged,
}

/// Kinematic swinging door component attached to the moving door leaf entity.
/// Uses kinematic position/rotation interpolation over the network instead of physical joints.
#[derive(Component, Clone, Debug)]
pub struct Door {
    pub structure_id: u64,
    pub target_rotation: Quat,
    pub is_open: bool,
    pub is_swinging: bool,
}

/// Marker component for selective permeability window panes.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct GlassPane;

/// Client-side purely visual entity animating a server-scheduled lethal fall.
/// Has no Rapier / Avian rigid body; simply animates the fall vector over duration.
#[derive(Component, Clone, Debug)]
pub struct VisualFallHazard {
    pub hazard_id: u64,
    pub kind: String,
    pub origin: Vec3,
    pub dir: Vec3,
    pub length: f32,
    pub elapsed: f32,
    pub duration: f32,
    pub initial_rotation: Quat,
}

/// Architectural Faction Styles based on early 2000s MMO aesthetics:
/// - HighElf (The Pristine Bastion): Polished mint-green marble, lapis/obsidian roofs, gold filigree, sharp sweeping geometry.
/// - Human (The Utilitarian Fortress): Rough-hewn grey stone blocks, red brick walls, iron-banded timber, blocky fortified geometry.
/// Architectural Faction:
/// [ARCHIVED]: Multi-race architectural mesh variations have been archived in
/// `docs/archive/legacy_architectural_factions.rs`. Single Frontier Wood & Stone style is canonical.
/// Retained for network and historical event payload compatibility.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum BuildingFaction {
    #[default]
    Human,
    HighElf,
    DarkElf,
    Barbarian,
}

impl BuildingFaction {

    pub fn name(&self) -> &'static str {
        match self {
            Self::Human => "Frontier Wood & Stone",
            Self::HighElf => "High Elf (Archived)",
            Self::DarkElf => "Dark Elf (Archived)",
            Self::Barbarian => "Barbarian (Archived)",
        }
    }

    pub fn as_prefix(&self) -> &'static str {
        match self {
            Self::Human => "Human",
            Self::HighElf => "HighElf",
            Self::DarkElf => "DarkElf",
            Self::Barbarian => "Barbarian",
        }
    }

    pub fn from_piece_name(name: &str) -> Self {
        if name.starts_with("HighElf") || (name.contains("Elf") && !name.contains("Dark")) {
            Self::HighElf
        } else if name.starts_with("DarkElf") || name.contains("Dark") {
            Self::DarkElf
        } else if name.starts_with("Barbarian") || name.contains("Bear") {
            Self::Barbarian
        } else {
            Self::Human
        }
    }
}

/// Event emitted whenever a building is destroyed (via health zero, collapse cascade, or DB removal).
/// Subscribed to by destruction animation and faction-specific shard/rubble generators.
#[derive(Event, Debug, Clone)]
pub struct BuildingDestructionEvent {
    #[allow(dead_code)]
    pub structure_id: u64,
    pub faction: BuildingFaction,
    pub piece_type: ModularPieceType,
    pub position: Vec3,
    pub rotation: Quat,
}

/// Event to trigger modular building spawns from network listeners or local commands.
#[derive(Event, Debug, Clone)]
pub struct SpawnBuildingEvent {
    pub entity_id: u64,
    pub faction: BuildingFaction,
    pub piece: ModularPieceType,
    pub position: Vec3,
    pub rotation: Quat,
    pub is_blueprint: bool,
}

/// Ephemeral collapse entity that shakes, crumbles, and fades out over duration before leaving a ruin pile.
#[derive(Component, Debug, Clone)]
pub struct BuildingDestructionAnimation {
    #[allow(dead_code)]
    pub faction: BuildingFaction,
    #[allow(dead_code)]
    pub piece_type: ModularPieceType,

    pub elapsed: f32,
    pub duration: f32,
    pub origin: Vec3,
}

/// Harvestable ruin node spawned at the end of a fall animation or tree collapse.
/// Carries a static collider that permanently blocks movement and paths.
#[derive(Component, Clone, Debug)]
pub struct HarvestableRuin {
    pub yield_amount: u32,
    pub node_type: String,
}

/// Fading emissive point light spawned when Dark Elf structures or neon runes are shattered.
#[derive(Component, Debug, Clone)]
pub struct RuneLightDecay {
    pub timer: Timer,
    pub base_intensity: f32,
}

// ----------------------------------------------------------------------------
// UI & VFX COMPONENTS & RESOURCES
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct InventoryUiRoot; 
#[derive(Component, Clone, Copy, Debug)] pub struct InventorySlotIndex(pub usize);
#[derive(Component)] pub struct InventorySlotName(pub usize);
#[derive(Component)] pub struct InventorySlotCount(pub usize);

#[derive(Component)] pub struct DragGhostUi;
#[derive(Component)] pub struct DragGhostText;

#[derive(Component)] pub struct CraftRecipeButton(pub String);
#[derive(Component)] #[allow(dead_code)] pub struct CraftRecipeText;

#[derive(Component)] #[allow(dead_code)] pub struct HotbarRoot;
#[derive(Component)] pub struct HotbarSlotUi(pub usize);
#[derive(Component)] pub struct HotbarSlotName(pub usize);
#[derive(Component)] pub struct HotbarSlotCount(pub usize);

#[derive(Resource, Component, Clone, Debug, Default)] 
pub struct ActiveItemSlot(pub usize);

#[derive(Resource, Component, Clone, Debug, Default)] 
pub struct ActiveEquippedItem(pub Option<String>);

#[derive(Resource, Component, Clone, Debug, Default)] 
pub struct ActiveOffHandItem(pub Option<String>);

#[derive(Resource, Default)]
pub struct CachedPlayerEntity(pub Option<u64>);

#[derive(Component)] pub struct HealthBarFill;
#[derive(Component)] pub struct HealthBarText;

#[derive(Component)] pub struct BuildMenuRoot;
#[derive(Component)] pub struct BuildPieceButton(pub ModularPieceType);
#[derive(Component)] pub struct BuildTemplateButton(pub crate::templates::BuildingTemplateType);

#[derive(Component)] pub struct HealthBarUI(pub u64);
#[derive(Component)] pub struct BuildUIText;
#[derive(Component)] pub struct InteractionPromptText;

#[derive(Component)] pub struct ActionBarUiRoot;
#[derive(Component)] pub struct ActionBarButton(pub String);

// ----------------------------------------------------------------------------
// DEVELOPER CONSOLE COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct ConsoleRoot;
#[derive(Component)] pub struct ConsoleLogText;
#[derive(Component)] pub struct ConsoleInputText;
#[derive(Component)] pub struct ConsoleSuggestionsText;

#[derive(Component)] 
pub struct BerryVisual {
    #[allow(dead_code)]
    pub node_id: u64,
}

#[derive(Component)] 
pub struct Particle { 
    pub timer: Timer, 
}

/// Tag for in-flight arrow projectiles requiring aerodynamic trajectory alignment.
#[derive(Component)]
pub struct ArrowProjectile;

// ----------------------------------------------------------------------------
// RETICLE-ADJACENT HUD & CROSSHAIR CUSTOMIZATION
// ----------------------------------------------------------------------------
// Architectural Note: Positions vital fighting telemetry directly adjacent to the
// center crosshair (ammo counter, tactical cooldowns, critical health alerts) so
// the player's eyes never leave the center of the screen during engagements.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CrosshairColorPreset {
    #[default]
    Cyan,
    BrightGreen,
    Magenta,
    Yellow,
    White,
    Red,
    Amber,
}

impl CrosshairColorPreset {
    pub fn to_color(self) -> Color {
        match self {
            Self::Cyan => Color::srgb(0.0, 1.0, 1.0),
            Self::BrightGreen => Color::srgb(0.0, 1.0, 0.2),
            Self::Magenta => Color::srgb(1.0, 0.0, 1.0),
            Self::Yellow => Color::srgb(1.0, 0.95, 0.1),
            Self::White => Color::srgb(1.0, 1.0, 1.0),
            Self::Red => Color::srgb(1.0, 0.15, 0.15),
            Self::Amber => Color::srgb(1.0, 0.75, 0.0),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Cyan => Self::BrightGreen,
            Self::BrightGreen => Self::Magenta,
            Self::Magenta => Self::Yellow,
            Self::Yellow => Self::White,
            Self::White => Self::Red,
            Self::Red => Self::Amber,
            Self::Amber => Self::Cyan,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Cyan => "Cyan",
            Self::BrightGreen => "Bright Green",
            Self::Magenta => "Magenta",
            Self::Yellow => "Yellow",
            Self::White => "White",
            Self::Red => "Red",
            Self::Amber => "Amber",
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct CrosshairSettings {
    pub enabled: bool,
    pub color_preset: CrosshairColorPreset,
    pub thickness: f32,         // Line thickness in pixels (1.0 - 8.0)
    pub length: f32,            // Line length in pixels (2.0 - 30.0)
    pub gap: f32,               // Center gap in pixels (0.0 - 40.0)
    pub dot: bool,              // Center dot toggle
    pub dot_size: f32,          // Center dot diameter in pixels
    pub outline: bool,          // High-contrast black outline
    pub outline_thickness: f32, // Outline border thickness in pixels
    pub opacity: f32,           // Crosshair alpha opacity (0.2 - 1.0)
    pub is_dynamic: bool,       // Dynamic bloom/spread vs rock-solid Static
}

impl Default for CrosshairSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            color_preset: CrosshairColorPreset::Cyan, // Default high-contrast cyan
            thickness: 2.0,
            length: 8.0,
            gap: 5.0,
            dot: false,
            dot_size: 2.0,
            outline: true,
            outline_thickness: 1.0,
            opacity: 1.0,
            is_dynamic: true, // Teaches spread/velocity by default; toggleable to static
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrosshairArmDir {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Component)] pub struct ReticleHudRoot;
#[derive(Component)] pub struct ReticleCrosshairArm(pub CrosshairArmDir);
#[derive(Component)] pub struct ReticleCrosshairDot;
#[allow(dead_code)]
#[deprecated(note = "Archived to docs/archive/legacy_firearms.rs. Disabled in favor of fantasy genre & Bevy Taffy HUD.")]
#[derive(Component)] pub struct ReticleAmmoText;
#[derive(Component)] pub struct ReticleBowChargeBar;
#[allow(dead_code)] #[derive(Component)] pub struct ReticleAbilityDashText;
#[allow(dead_code)] #[derive(Component)] pub struct ReticleAbilitySmokeText;
#[allow(dead_code)] #[derive(Component)] pub struct ReticleAbilityIntelText;
#[allow(dead_code)] #[derive(Component)] pub struct ReticleAbilityLiftText;
#[derive(Component)] pub struct ReticleCriticalHealthAlert;
#[derive(Component)] pub struct ReticleHitMarker;
#[derive(Component)] pub struct ReticleHitMarkerTick;

#[derive(Component)] pub struct CrosshairMenuRoot;
#[derive(Component)] pub struct CrosshairMenuButton(pub String);
#[derive(Component)] pub struct CrosshairMenuText;

#[derive(Resource, Default)]
pub struct CrosshairMenuState {
    pub is_open: bool,
}

// ----------------------------------------------------------------------------
// TACTICAL ABILITIES & SENSORY STATE
// ----------------------------------------------------------------------------

#[derive(Resource)]
pub struct TacticalAbilityState {
    pub cooldowns: spacetime_rts_logic::TacticalAbilityCooldowns,
    pub is_dashing: bool,
    pub dash_timer: Timer,
    pub dash_velocity: Vec3,
}

impl Default for TacticalAbilityState {
    fn default() -> Self {
        Self {
            cooldowns: spacetime_rts_logic::TacticalAbilityCooldowns::new(),
            is_dashing: false,
            dash_timer: Timer::from_seconds(0.22, TimerMode::Once),
            dash_velocity: Vec3::ZERO,
        }
    }
}

#[derive(Component)]
pub struct SmokeCloudMarker {
    pub timer: Timer,
    #[allow(dead_code)]
    pub radius: f32,
}

#[derive(Component)]
pub struct IntelDartMarker {
    pub pings_left: u32,
    pub ping_timer: Timer,
    pub radius: f32,
}

#[derive(Component)]
pub struct IntelSonarPulseVisual {
    pub timer: Timer,
    pub max_radius: f32,
}

#[derive(Resource)]
pub struct HitMarkerState {
    pub timer: Timer,
    pub is_crit: bool,
    pub is_armor: bool,
}

impl Default for HitMarkerState {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            is_crit: false,
            is_armor: false,
        }
    }
}

// ----------------------------------------------------------------------------
// SPARRING YARD & PRACTICE ENTITY COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component, Debug, Clone)]
pub struct TrainingDummy {
    pub wobble_timer: Timer,
    pub wobble_angle: f32,
    pub wobble_axis: Vec3,
    pub base_rotation: Quat,
}

#[derive(Component, Debug, Clone)]
pub struct SparringGoblin {
    pub health: f32,
    pub max_health: f32,
    pub is_blocking: bool,
    pub block_timer: Timer,
    pub stagger_timer: Timer,
    pub attack_cooldown: Timer,
    pub home_pos: Vec3,
}

#[derive(Resource, Clone, Default)]
pub struct CombatAudioHandles {
    pub dink: Handle<AudioSource>,
    pub armor_break: Handle<AudioSource>,
    pub bodyshot_tick: Handle<AudioSource>,
    pub dash_whoosh: Handle<AudioSource>,
    pub sonar_ping: Handle<AudioSource>,
    pub smoke_hiss: Handle<AudioSource>,
    pub sword_clang: Handle<AudioSource>,
    pub shield_block: Handle<AudioSource>,
    pub flesh_impact: Handle<AudioSource>,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SparringMode(pub bool);