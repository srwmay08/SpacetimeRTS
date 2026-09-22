use bevy::prelude::*;

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

#[derive(Component)]
pub struct TerrainChunk {
    #[allow(dead_code)] pub chunk_x: i32,
    #[allow(dead_code)] pub chunk_z: i32,
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
#[derive(Component)] pub struct ResourceNodeItem { pub node_id: u64 } 
#[derive(Component)] pub struct PeasantUnit { pub entity_id: u64 } // Architectural Note: Added to identify AI entities for RTS right-click routing.

// ----------------------------------------------------------------------------
// MODULAR BUILDING & SOCKET COMPONENTS
// ----------------------------------------------------------------------------

/// Defines a mathematical snap point on a structure for modular piece connection.
#[derive(Component, Clone, Debug)]
pub struct Socket {
    #[allow(dead_code)] pub name: String,
    pub local_offset: Vec3,
    pub local_rotation: Quat, 
    pub is_occupied: bool,
}

/// Marks an entity as a placement hologram previewing a modular piece.
#[derive(Component)]
pub struct BuildHologram;

#[derive(Component)]
pub struct BaseInteriorVolume {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Component)]
pub struct InteriorProp;

// ----------------------------------------------------------------------------
// UI & VFX COMPONENTS
// ----------------------------------------------------------------------------
#[derive(Component)] pub struct InventoryUiRoot; 
#[derive(Component)] pub struct InventorySlotName(pub usize);
#[derive(Component)] pub struct InventorySlotCount(pub usize);

#[derive(Component)] pub struct HealthBarUI;
#[derive(Component)] pub struct BuildUIText;
#[derive(Component)] pub struct InteractionPromptText; // Added for ground item interaction prompts

#[derive(Component)] pub struct ActionBarUiRoot;
#[derive(Component)] pub struct ActionBarButton(pub String);

#[derive(Component)] 
pub struct BerryVisual {
    pub node_id: u64
}

#[derive(Component)] 
pub struct Particle { 
    pub timer: Timer 
}