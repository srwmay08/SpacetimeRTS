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

#[derive(Component)] pub struct FPSMesh;
#[derive(Component)] pub struct RTSProxy;
#[derive(Component)] pub struct FpsCamera;
#[derive(Component)] pub struct RtsCameraRig;
#[derive(Component)] pub struct RtsCameraChild;

// ----------------------------------------------------------------------------
// NETWORKING & ENTITY MARKERS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct NetworkEntity(pub u64);
#[derive(Component)] pub struct NetworkStructure { pub structure_id: u64 }

#[derive(Component)] pub struct PlayerBody;
#[derive(Component)] pub struct PlayerHead;
#[derive(Component)] pub struct ViewModelArm;
#[derive(Component)] pub struct Kcc { pub is_grounded: bool }

#[derive(Component)]
pub struct ResourceNodeItem {
    pub node_id: u64,
    pub node_type: String,
}

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
}

#[derive(Component)]
pub struct FallingTree {
    pub base_pos: Vec3,
    pub fall_dir: Vec3,
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

#[derive(Component)] pub struct HotbarRoot;
#[derive(Component)] pub struct HotbarSlotUi(pub usize);
#[derive(Component)] pub struct HotbarSlotName(pub usize);
#[derive(Component)] pub struct HotbarSlotCount(pub usize);

#[derive(Resource, Component, Clone, Debug, Default)] 
pub struct ActiveItemSlot(pub usize);

#[derive(Resource, Component, Clone, Debug, Default)] 
pub struct ActiveEquippedItem(pub Option<String>);

#[derive(Resource, Default)]
pub struct CachedPlayerEntity(pub Option<u64>);

#[derive(Component)] pub struct HealthBarFill;
#[derive(Component)] pub struct HealthBarText;

#[derive(Component)] pub struct BuildMenuRoot;
#[derive(Component)] pub struct BuildPieceButton(pub ModularPieceType);

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