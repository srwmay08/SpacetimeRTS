use bevy::prelude::*;

// ----------------------------------------------------------------------------
// CORE LOGICAL ECS COMPONENTS
// ----------------------------------------------------------------------------

/// Represents the authoritative server position from SpacetimeDB.
/// Architectural Note: Used alongside `BevyTransform` for smooth client-side interpolation (lerping),
/// mitigating visual jitter between 50ms server network ticks.
#[derive(Component)] pub struct LogicalPosition(pub Vec3);

/// Represents the authoritative server rotation from SpacetimeDB.
#[derive(Component)] pub struct LogicalRotation(pub Quat);

/// Identifies entity allegiance for combat and RTS targeting.
#[derive(Component, Default, PartialEq, Eq)] 
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

/// Designates an active pathfinding destination for RTS units.
#[derive(Component)] pub struct NavTarget(pub Vec3);

/// Tags a procedurally generated terrain mesh with its logical grid coordinates.
#[derive(Component)]
pub struct TerrainChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

// ----------------------------------------------------------------------------
// RENDERING MARKERS & CAMERAS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct FPSMesh;
#[derive(Component)] pub struct RTSProxy;
#[derive(Component)] pub struct FpsCamera;
#[derive(Component)] pub struct RtsCameraRig;
#[derive(Component)] pub struct RtsCameraChild;

// ----------------------------------------------------------------------------
// NETWORKING & ENTITY MARKERS
// ----------------------------------------------------------------------------

/// Maps a local Bevy Entity to a SpacetimeDB authoritative `entity_id`.
/// Architectural Note: This is strictly required to sync delta updates from the `transform` table back to the correct local mesh.
#[derive(Component)] pub struct NetworkEntity(pub u64);

#[derive(Component)] pub struct PlayerBody;
#[derive(Component)] pub struct PlayerHead;
#[derive(Component)] pub struct ViewModelArm;

/// Kinematic Character Controller state tracking.
#[derive(Component)] pub struct Kcc { pub is_grounded: bool }

/// Links a local dropped item mesh to its SpacetimeDB `ground_loot` table record.
#[derive(Component)] pub struct GroundLootItem { pub loot_id: u64 }

/// Links a local harvestable mesh to its SpacetimeDB `resource_node` table record.
#[derive(Component)] pub struct ResourceNodeItem { pub node_id: u64 } 

// ----------------------------------------------------------------------------
// UI & VFX COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct InventoryUiRoot; 
#[derive(Component)] pub struct WoodText;
#[derive(Component)] pub struct OreText;
#[derive(Component)] pub struct FoodText;

/// Identifies a UI Node as a floating health bar synchronized with server state.
#[derive(Component)] pub struct HealthBarUI;

/// Tracks the lifespan of localized visual effect entities (e.g., resource breaking debris).
#[derive(Component)] 
pub struct Particle { 
    pub timer: Timer 
}