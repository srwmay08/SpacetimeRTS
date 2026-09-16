use bevy::prelude::*;

// ----------------------------------------------------------------------------
// CORE LOGICAL ECS COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct LogicalPosition(pub Vec3);
#[derive(Component)] pub struct LogicalRotation(pub Quat);

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
#[derive(Component)] pub struct NavTarget(pub Vec3);

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

#[derive(Component)] pub struct NetworkEntity(pub u64);

/// Links a local rendered building structure to its SpacetimeDB `structure_id`.
#[derive(Component)] pub struct NetworkStructure { pub structure_id: u64 }

#[derive(Component)] pub struct PlayerBody;
#[derive(Component)] pub struct PlayerHead;
#[derive(Component)] pub struct ViewModelArm;
#[derive(Component)] pub struct Kcc { pub is_grounded: bool }
#[derive(Component)] pub struct GroundLootItem { pub loot_id: u64 }
#[derive(Component)] pub struct ResourceNodeItem { pub node_id: u64 } 

// ----------------------------------------------------------------------------
// MODULAR BUILDING & SOCKET COMPONENTS
// ----------------------------------------------------------------------------

/// Defines a mathematical snap point on a structure for modular piece connection.
#[derive(Component, Clone, Debug)]
pub struct Socket {
    pub name: String,
    pub local_offset: Vec3,
    pub is_occupied: bool,
}

/// Marks an entity as a placement hologram previewing a modular piece.
#[derive(Component)]
pub struct BuildHologram;

// ----------------------------------------------------------------------------
// UI & VFX COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct InventoryUiRoot; 
#[derive(Component)] pub struct WoodText;
#[derive(Component)] pub struct OreText;
#[derive(Component)] pub struct FoodText;
#[derive(Component)] pub struct HealthBarUI;

#[derive(Component)] 
pub struct Particle { 
    pub timer: Timer 
}