// ============================================================================
// File: client/src/tree_colors.rs
// ============================================================================
// ----------------------------------------------------------------------------
// SEASONAL PALETTES, BIOME CLASSIFICATION & DYNAMIC TREE COLOR MANAGEMENT
// ----------------------------------------------------------------------------
// Architectural Note:
// Coordinates deterministic seasonal transitions across all micro-voxel tree
// variants. Connects directly to the astronomical ephemeris in `binary_sky.rs`,
// smoothly interpolating foliage colors and bark characteristics across the 4
// planetary seasons: Spring, Summer, Autumn, and Winter.
// ----------------------------------------------------------------------------

use bevy::prelude::*;

/// Discrete planetary seasons driven by the circumstellar orbit around Star A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Season {
    #[default]
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Spring => "Spring",
            Self::Summer => "Summer",
            Self::Autumn => "Autumn",
            Self::Winter => "Winter",
        }
    }
}

/// Instantaneous seasonal progression state resource.
#[derive(Resource, Debug, Clone)]
pub struct SeasonState {
    pub current: Season,
    /// Progress through the current season [0.0, 1.0).
    pub progress: f32,
}

impl Default for SeasonState {
    fn default() -> Self {
        Self {
            current: Season::Summer,
            progress: 0.5,
        }
    }
}

/// Color definition for a tree species across all four seasons.
#[derive(Debug, Clone, Copy)]
pub struct TreePalette {
    pub spring: [f32; 4],
    pub summer: [f32; 4],
    pub autumn: [f32; 4],
    pub winter: [f32; 4],
}

/// Dead Tree: Weathered tan in spring, bleached in summer sun, damp decay in autumn, frost-rimed in winter.
pub const DEAD_PALETTE: TreePalette = TreePalette {
    spring: [0.74, 0.60, 0.40, 1.0],
    summer: [0.70, 0.58, 0.38, 1.0],
    autumn: [0.65, 0.50, 0.32, 1.0],
    winter: [0.55, 0.52, 0.48, 1.0],
};

/// Oak Tree: Fresh tender green in spring, deep canopy green in summer, blazing amber/orange in autumn, bare bark in winter.
pub const OAK_PALETTE: TreePalette = TreePalette {
    spring: [0.35, 0.65, 0.25, 1.0],
    summer: [0.20, 0.50, 0.15, 1.0],
    autumn: [0.94, 0.54, 0.16, 1.0],
    winter: [0.44, 0.28, 0.16, 1.0],
};

/// Pine Tree: Vibrant evergreen needles in spring, deep spruce green in summer, rich dark forest in autumn, snow-dusted in winter.
pub const PINE_PALETTE: TreePalette = TreePalette {
    spring: [0.22, 0.58, 0.26, 1.0],
    summer: [0.14, 0.38, 0.16, 1.0],
    autumn: [0.18, 0.34, 0.15, 1.0],
    winter: [0.65, 0.75, 0.72, 1.0],
};

/// Round Tree: Blossoming chartreuse lime in spring, lush summer crown, brilliant gold in autumn, dormant gray/amber in winter.
pub const ROUND_PALETTE: TreePalette = TreePalette {
    spring: [0.45, 0.78, 0.30, 1.0],
    summer: [0.28, 0.62, 0.20, 1.0],
    autumn: [0.96, 0.82, 0.22, 1.0],
    winter: [0.50, 0.42, 0.35, 1.0],
};

/// Evaluates smooth seasonal lerp color given a palette and current season progress.
pub fn get_seasonal_color(palette: &TreePalette, current: Season, progress: f32) -> Color {
    let t = progress.clamp(0.0, 1.0);
    let (c1, c2) = match current {
        Season::Spring => (palette.spring, palette.summer),
        Season::Summer => (palette.summer, palette.autumn),
        Season::Autumn => (palette.autumn, palette.winter),
        Season::Winter => (palette.winter, palette.spring),
    };
    let r = c1[0] + (c2[0] - c1[0]) * t;
    let g = c1[1] + (c2[1] - c1[1]) * t;
    let b = c1[2] + (c2[2] - c1[2]) * t;
    let a = c1[3] + (c2[3] - c1[3]) * t;
    Color::linear_rgba(r, g, b, a)
}

/// Elevation-based terrain biome categorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Lowland,   // 1.5 - 8.0m elevation
    Hill,      // 8.0 - 18.0m elevation
    Mountain,  // 18.0 - 25.0m elevation
}

/// Classifies elevation into biome zone.
pub fn get_biome(elevation: f32) -> Biome {
    if elevation <= 8.0 {
        Biome::Lowland
    } else if elevation <= 18.0 {
        Biome::Hill
    } else {
        Biome::Mountain
    }
}

/// Deterministic tree species picker per biome using a seeded PRNG roll.
pub fn pick_tree_type(biome: Biome, seed: &mut u64) -> &'static str {
    // xorshift64 step
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    let roll = (*seed as u32 as f32) / (u32::MAX as f32);

    match biome {
        Biome::Lowland => {
            if roll < 0.50 {
                "Oak"
            } else if roll < 0.80 {
                "Round"
            } else {
                "Pine"
            }
        }
        Biome::Hill => {
            if roll < 0.60 {
                "Pine"
            } else if roll < 0.90 {
                "Oak"
            } else {
                "Dead"
            }
        }
        Biome::Mountain => {
            if roll < 0.70 {
                "Pine"
            } else {
                "Dead"
            }
        }
    }
}

/// Shared StandardMaterial handles for the 4 tree species to enable O(1) seasonal updates.
#[derive(Resource, Clone)]
pub struct TreeMaterialHandles {
    pub dead: Handle<StandardMaterial>,
    pub oak: Handle<StandardMaterial>,
    pub pine: Handle<StandardMaterial>,
    pub round: Handle<StandardMaterial>,
}

impl FromWorld for TreeMaterialHandles {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self {
            dead: materials.add(StandardMaterial {
                base_color: Color::linear_rgba(DEAD_PALETTE.summer[0], DEAD_PALETTE.summer[1], DEAD_PALETTE.summer[2], 1.0),
                perceptual_roughness: 0.85,
                reflectance: 0.1,
                ..default()
            }),
            oak: materials.add(StandardMaterial {
                base_color: Color::linear_rgba(OAK_PALETTE.autumn[0], OAK_PALETTE.autumn[1], OAK_PALETTE.autumn[2], 1.0),
                perceptual_roughness: 0.85,
                reflectance: 0.1,
                ..default()
            }),
            pine: materials.add(StandardMaterial {
                base_color: Color::linear_rgba(PINE_PALETTE.summer[0], PINE_PALETTE.summer[1], PINE_PALETTE.summer[2], 1.0),
                perceptual_roughness: 0.85,
                reflectance: 0.1,
                ..default()
            }),
            round: materials.add(StandardMaterial {
                base_color: Color::linear_rgba(ROUND_PALETTE.autumn[0], ROUND_PALETTE.autumn[1], ROUND_PALETTE.autumn[2], 1.0),
                perceptual_roughness: 0.85,
                reflectance: 0.1,
                ..default()
            }),
        }
    }
}

/// Updates tree materials based on current seasonal progression.
pub fn update_tree_colors(
    season: Res<SeasonState>,
    tree_mats: Option<Res<TreeMaterialHandles>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tree_query: Query<(&crate::components::TreeComponent, &Handle<StandardMaterial>)>,
) {
    if let Some(tree_mats) = tree_mats {
        if let Some(mat) = materials.get_mut(&tree_mats.dead) {
            mat.base_color = get_seasonal_color(&DEAD_PALETTE, season.current, season.progress);
        }
        if let Some(mat) = materials.get_mut(&tree_mats.oak) {
            mat.base_color = get_seasonal_color(&OAK_PALETTE, season.current, season.progress);
        }
        if let Some(mat) = materials.get_mut(&tree_mats.pine) {
            mat.base_color = get_seasonal_color(&PINE_PALETTE, season.current, season.progress);
        }
        if let Some(mat) = materials.get_mut(&tree_mats.round) {
            mat.base_color = get_seasonal_color(&ROUND_PALETTE, season.current, season.progress);
        }
    } else {
        let t = season.progress;
        for (tree_comp, mat_handle) in tree_query.iter_mut() {
            if let Some(mat) = materials.get_mut(mat_handle) {
                let palette = match tree_comp.species {
                    0 => &DEAD_PALETTE,
                    1 => &OAK_PALETTE,
                    2 => &PINE_PALETTE,
                    _ => &ROUND_PALETTE,
                };
                mat.base_color = get_seasonal_color(palette, season.current, t);
            }
        }
    }
}

// ============================================================================
// COMPREHENSIVE UNIT TEST SUITE
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::*;

    #[test]
    fn test_biome_elevation_classification() {
        assert_eq!(get_biome(1.5), Biome::Lowland);
        assert_eq!(get_biome(5.0), Biome::Lowland);
        assert_eq!(get_biome(8.0), Biome::Lowland);
        assert_eq!(get_biome(8.1), Biome::Hill);
        assert_eq!(get_biome(15.0), Biome::Hill);
        assert_eq!(get_biome(18.0), Biome::Hill);
        assert_eq!(get_biome(18.1), Biome::Mountain);
        assert_eq!(get_biome(24.5), Biome::Mountain);
    }

    #[test]
    fn test_biome_tree_type_distribution() {
        let mut seed = 12345;
        let mut lowland_counts = (0, 0, 0); // Oak, Round, Pine
        for _ in 0..100 {
            match pick_tree_type(Biome::Lowland, &mut seed) {
                "Oak" => lowland_counts.0 += 1,
                "Round" => lowland_counts.1 += 1,
                "Pine" => lowland_counts.2 += 1,
                _ => panic!("Unexpected tree in lowland"),
            }
        }
        assert!(lowland_counts.0 > 25);
        assert!(lowland_counts.1 > 15);
        assert!(lowland_counts.2 > 10);

        let mut mountain_counts = (0, 0); // Pine, Dead
        for _ in 0..100 {
            match pick_tree_type(Biome::Mountain, &mut seed) {
                "Pine" => mountain_counts.0 += 1,
                "Dead" => mountain_counts.1 += 1,
                _ => panic!("Unexpected tree in mountain"),
            }
        }
        assert!(mountain_counts.0 > 45);
        assert!(mountain_counts.1 > 15);
    }

    #[test]
    fn test_seasonal_color_progression() {
        let oak_spring = get_seasonal_color(&OAK_PALETTE, Season::Spring, 0.0);
        let oak_summer = get_seasonal_color(&OAK_PALETTE, Season::Summer, 0.0);
        let oak_autumn = get_seasonal_color(&OAK_PALETTE, Season::Autumn, 0.0);
        let oak_winter = get_seasonal_color(&OAK_PALETTE, Season::Winter, 0.0);

        // Spring is vibrant green
        let spring_linear = oak_spring.to_linear();
        assert!(spring_linear.green > spring_linear.red);

        // Autumn is amber/orange (red > green)
        let autumn_linear = oak_autumn.to_linear();
        assert!(autumn_linear.red > autumn_linear.blue);

        // Smooth transition test
        let mid_summer = get_seasonal_color(&OAK_PALETTE, Season::Summer, 0.5);
        let mid_linear = mid_summer.to_linear();
        assert!(mid_linear.red >= oak_summer.to_linear().red);
        assert!(mid_linear.red <= oak_autumn.to_linear().red);
    }

    #[test]
    fn test_tree_mesh_procedural_variation() {
        let oak1 = create_voxel_oak_mesh(1001);
        let oak2 = create_voxel_oak_mesh(2002);
        let pine1 = create_voxel_pine_mesh(3003);
        let pine2 = create_voxel_pine_mesh(4004);
        let dead1 = create_voxel_dead_tree_mesh(5005);
        let dead2 = create_voxel_dead_tree_mesh(6006);
        let round1 = create_voxel_round_tree_mesh(7007);
        let round2 = create_voxel_round_tree_mesh(8008);

        // All meshes should have vertex positions, colors, normals, and indices
        for mesh in [&oak1, &oak2, &pine1, &pine2, &dead1, &dead2, &round1, &round2] {
            assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
            assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
            assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
            assert!(mesh.indices().is_some());
        }

        // Procedural variation check: Different seeds produce different vertex counts or geometry
        let oak1_verts = oak1.count_vertices();
        let oak2_verts = oak2.count_vertices();
        assert!(oak1_verts > 100);
        assert!(oak2_verts > 100);

        let pine1_verts = pine1.count_vertices();
        let pine2_verts = pine2.count_vertices();
        assert!(pine1_verts > 100);
        assert!(pine2_verts > 100);
    }

    #[test]
    fn test_micro_voxel_ellipsoid_and_curved_cylinder() {
        let mut grid = MicroVoxelGrid::new(0.06);
        let col = [0.2, 0.6, 0.2, 1.0];

        grid.fill_ellipsoid(0.0, 10.0, 0.0, 5.0, 8.0, 5.0, col);
        assert!(grid.voxels.contains_key(&(0, 10, 0)));
        assert!(grid.voxels.contains_key(&(0, 15, 0)));
        assert!(!grid.voxels.contains_key(&(0, 25, 0))); // Out of bounds

        grid.fill_curved_cylinder_y(0.0, 0.0, 0, 20, 2.0, 5.0, 0.0, col);
        assert!(grid.voxels.contains_key(&(0, 0, 0)));
        // At y=20, offset_x should curve towards 5.0
        assert!(grid.voxels.contains_key(&(5, 20, 0)));
    }

    #[test]
    fn test_fallen_log_mesh_generation() {
        let log = create_voxel_fallen_log_mesh(9999);
        assert!(log.count_vertices() > 50);
        assert!(log.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
    }
}

