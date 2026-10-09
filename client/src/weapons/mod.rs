// ============================================================================
// File: client/src/weapons/mod.rs
// ============================================================================
// ----------------------------------------------------------------------------
// MODULAR WEAPONS ARCHITECTURE & FIRST-PERSON VOXEL VIEWMODELS
// ----------------------------------------------------------------------------
// Architectural Note:
// Modularized into:
//   - `types`: Archetypes, WeaponType, WeaponState, Viewmodel components, hand tracking.
//   - `mesh_builder`: Low-poly vertex/index generators for all weapons, ammunition, and slash arcs.
//   - `melee`: Directional 4-way melee swing kinematics and transform offsets.
//   - `viewmodels`: First-person viewmodel animation, recoil, sway, and 3rd-person body attachments.
//   - `projectiles`: Authoritative server projectile replication and aerodynamic orientation.

pub mod types;
pub mod mesh_builder;
pub mod melee;
pub mod viewmodels;
pub mod projectiles;

pub use types::*;
pub use mesh_builder::*;
#[allow(unused_imports)]
pub use melee::*;
pub use viewmodels::*;
pub use projectiles::*;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{Transform as BevyTransform, *};
    use spacetime_rts_logic::HandSide;
    use crate::components::*;
    use crate::core::*;
    use crate::physics::LinearVelocity;

    #[test]
    fn test_directional_melee_transforms_all_directions() {
        let directions = [
            MeleeSwingDirection::Right,
            MeleeSwingDirection::Left,
            MeleeSwingDirection::Overhead,
            MeleeSwingDirection::Thrust,
        ];
        let phases = [
            MeleeAttackPhase::Idle,
            MeleeAttackPhase::Windup,
            MeleeAttackPhase::Release,
            MeleeAttackPhase::Recovery,
        ];

        for &dir in &directions {
            for &phase in &phases {
                let (pos, rot) = compute_directional_melee_transform(dir, phase, 0.5, 0.5, 0.5);
                assert!(!pos.x.is_nan() && !pos.y.is_nan() && !pos.z.is_nan());
                assert!(!rot.x.is_nan() && !rot.y.is_nan() && !rot.z.is_nan() && !rot.w.is_nan());
                assert!(rot.is_normalized(), "Rotation must be normalized for {:?} {:?}", dir, phase);

                if phase == MeleeAttackPhase::Idle {
                    assert_eq!(pos, Vec3::ZERO);
                    assert_eq!(rot, Quat::IDENTITY);
                }
            }
        }
    }

    #[test]
    fn test_wooden_shield_position_mirroring() {
        let right_pos = get_default_weapon_pos(WeaponType::WoodenShield, false);
        let left_pos = get_default_weapon_pos(WeaponType::WoodenShield, true);

        assert_eq!(right_pos.x, 0.24);
        assert_eq!(left_pos.x, -0.24);
        assert_eq!(right_pos.y, left_pos.y);
        assert_eq!(right_pos.z, left_pos.z);
    }

    #[test]
    fn test_sword_and_board_archetype_properties() {
        assert!(WeaponType::Longsword.is_melee());
        assert!(WeaponType::Longsword.is_one_handed());
        assert!(!WeaponType::Longsword.is_two_handed());

        assert!(WeaponType::WoodenShield.is_one_handed());
        assert!(!WeaponType::WoodenShield.is_two_handed());
    }

    #[test]
    fn test_weapon_systems_ecs_disjointness_and_registration() {
        // Architectural Guard: Verifies that weapon systems have disjoint queries and do not cause B0001 panics
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();

        app.insert_resource(WeaponState::default());
        app.insert_resource(SwingState::default());
        app.insert_resource(EquippedHandSide(HandSide::Right));
        app.insert_resource(ActiveEquippedItem(Some("Longsword".to_string())));
        app.insert_resource(ActiveOffHandItem(Some("Wooden Shield".to_string())));

        app.add_systems(Update, (
            spawn_or_update_view_model_weapon,
            animate_weapon_viewmodel,
            animate_third_person_weapons,
            sync_third_person_weapon_render_layers,
        ));

        // Spawns camera and player body
        let _cam = app.world_mut().spawn((FpsCamera, BevyTransform::default())).id();
        let _player = app.world_mut().spawn((PlayerBody, BevyTransform::default())).id();

        // Run update - this will panic with B0001 if any query parameters conflict!
        app.update();

        assert_eq!(app.world().resource::<WeaponState>().current_weapon, WeaponType::Longsword);
        assert_eq!(app.world().resource::<WeaponState>().offhand_weapon, WeaponType::WoodenShield);
    }

    #[test]
    fn test_all_weapon_meshes_generation() {
        let weapons = [
            WeaponType::None,
            WeaponType::Bow,
            WeaponType::Crossbow,
            WeaponType::HandCrossbow,
            WeaponType::Revolver,
            WeaponType::Shotgun,
            WeaponType::SniperRifle,
            WeaponType::BouncyBombLauncher,
            WeaponType::Runestaff,
            WeaponType::Wand,
            WeaponType::Orb,
            WeaponType::Halberd,
            WeaponType::Longsword,
            WeaponType::Greatsword,
            WeaponType::TwoHandAxe,
            WeaponType::Maul,
            WeaponType::Spear,
            WeaponType::Javelin,
            WeaponType::Trident,
            WeaponType::Rapier,
            WeaponType::Warhammer,
            WeaponType::Club,
            WeaponType::Dagger,
            WeaponType::Handaxe,
            WeaponType::TigerClaws,
            WeaponType::BlackJack,
            WeaponType::Cestus,
            WeaponType::KnuckleDuster,
            WeaponType::FryingPan,
            WeaponType::HolyMackerel,
            WeaponType::Hammer,
            WeaponType::Pickaxe,
            WeaponType::Torch,
            WeaponType::WoodenShield,
        ];

        for w in weapons {
            let mesh = create_lowpoly_weapon_mesh(w);
            let pos_len = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len();
            let norm_len = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap().len();
            let col_len = mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap().len();
            assert_eq!(pos_len, norm_len);
            assert_eq!(pos_len, col_len);
            assert!(pos_len > 0, "Weapon {:?} mesh should not be empty", w);
        }

        let arrow = create_lowpoly_arrow_mesh();
        assert!(arrow.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len() > 0);

        let bolt = create_lowpoly_bolt_mesh();
        assert!(bolt.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len() > 0);

        let slide = create_lowpoly_pumpslide_mesh();
        assert!(slide.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len() > 0);
    }

    #[test]
    fn test_bow_arrow_pullback_and_projectile_trajectory() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(WeaponState {
            current_weapon: WeaponType::Bow,
            bow_charge: 0.75,
            bow_drawing: true,
            ..default()
        });
        app.insert_resource(SwingState::default());
        app.insert_resource(EquippedHandSide(HandSide::Right));

        let arrow_id = app.world_mut().spawn((
            ViewModelBowArrow,
            Visibility::Inherited,
            BevyTransform::from_xyz(-0.01, 0.02, -0.12),
        )).id();

        let proj_id = app.world_mut().spawn((
            ArrowProjectile,
            LinearVelocity(Vec3::new(30.0, -10.0, 0.0)),
            BevyTransform::default(),
        )).id();

        app.add_systems(Update, (
            animate_weapon_viewmodel,
            update_arrow_projectiles,
        ));

        app.update();

        // 1. Arrow should have shifted back along +Z proportional to bow_charge (0.75 * 0.20 = 0.15)
        let arrow_trans = app.world().get::<BevyTransform>(arrow_id).unwrap();
        assert!((arrow_trans.translation.z - (-0.12 + 0.75 * 0.20)).abs() < 1e-4);

        // 2. Projectile should have aligned forward with its velocity vector
        let proj_trans = app.world().get::<BevyTransform>(proj_id).unwrap();
        let expected_dir = Vec3::new(30.0, -10.0, 0.0).normalize();
        let actual_forward = proj_trans.forward().as_vec3();
        assert!((actual_forward.dot(expected_dir) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_weapon_archetypes_and_canonical_slice() {
        assert_eq!(WeaponType::Longsword.archetype(), WeaponArchetype::OneHandBlade);
        assert_eq!(WeaponType::Greatsword.archetype(), WeaponArchetype::TwoHandBlade);
        assert_eq!(WeaponType::Handaxe.archetype(), WeaponArchetype::OneHandAxe);
        assert_eq!(WeaponType::TwoHandAxe.archetype(), WeaponArchetype::TwoHandAxe);
        assert_eq!(WeaponType::Warhammer.archetype(), WeaponArchetype::OneHandBludgeon);
        assert_eq!(WeaponType::Maul.archetype(), WeaponArchetype::TwoHandBludgeon);
        assert_eq!(WeaponType::Spear.archetype(), WeaponArchetype::Polearm);
        assert_eq!(WeaponType::Bow.archetype(), WeaponArchetype::Bow);
        assert_eq!(WeaponType::Crossbow.archetype(), WeaponArchetype::Crossbow);
        assert_eq!(WeaponType::Wand.archetype(), WeaponArchetype::MagicImplement);
        assert_eq!(WeaponType::WoodenShield.archetype(), WeaponArchetype::Shield);
        assert_eq!(WeaponType::Hammer.archetype(), WeaponArchetype::UtilityTool);
        assert!(WeaponType::Longsword.is_canonical_vertical_slice());
        assert!(WeaponType::Bow.is_canonical_vertical_slice());
        assert!(!WeaponType::Revolver.is_canonical_vertical_slice());
        assert!(!WeaponType::Shotgun.is_canonical_vertical_slice());
    }
}
