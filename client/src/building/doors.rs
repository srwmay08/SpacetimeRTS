// ============================================================================
// File: client/src/building/doors.rs
// ============================================================================
// Kinematic door interpolation, server state synchronization, and rotation slerp.

use bevy::prelude::*;
use crate::components::Door;
use crate::network::SpacetimeConnection;
use crate::module_bindings::door_state_table::DoorStateTableAccess;

// ----------------------------------------------------------------------------
// KINEMATIC DOOR ROTATION INTERPOLATION
// ----------------------------------------------------------------------------
// Architectural Note:
// Listens to SpacetimeDB door_state table. Slerps the Kinematic door leaf
// rotation smoothly towards target_rotation. Prevents network jitter while
// keeping Rapier3D's kinematic collider perfectly synchronized with the mesh.

pub fn sync_door_states(
    conn: Res<SpacetimeConnection>,
    mut query: Query<&mut Door>,
) {
    for mut door in query.iter_mut() {
        let is_open = conn.db.db.door_state().structure_id().find(&door.structure_id).map_or(false, |d| d.is_open);
        if is_open != door.is_open {
            door.is_open = is_open;
            door.target_rotation = if is_open {
                Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
            } else {
                Quat::IDENTITY
            };
            door.is_swinging = true;
        }
    }
}

pub fn animate_doors(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Door)>,
) {
    let dt = time.delta_seconds();
    for (mut transform, mut door) in query.iter_mut() {
        if door.is_swinging {
            transform.rotation = transform.rotation.slerp(door.target_rotation, dt * 6.0);
            if transform.rotation.angle_between(door.target_rotation) < 0.01 {
                transform.rotation = door.target_rotation;
                door.is_swinging = false;
            }
        }
    }
}
