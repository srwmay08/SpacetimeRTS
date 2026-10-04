// ============================================================================
// File: client/src/props.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL ENVIRONMENT & RESOURCE PROPS
// ----------------------------------------------------------------------------
// Architectural Note:
// Generates low-poly faceted 3D meshes for harvestable resource nodes and environment props:
// Granite boulders, berry bushes (with decoupled foliage & berries meshes for shadow optimization),
// fallen branches, knapped flint shards, and loose river stones.
// Flat-shaded, vertex-colored, and designed for minimal vertex and draw overhead.
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use crate::voxel_mesh::{MicroVoxelGrid, Prng};
use crate::trees::LowPolyMeshBuilder;

pub fn create_voxel_rock_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.045);
    let slate = [0.35, 0.35, 0.38, 1.0];
    let granite_mid = [0.50, 0.50, 0.53, 1.0];
    let highlight = [0.75, 0.75, 0.78, 1.0];

    grid.fill_sphere(0, 10, 0, 17.0, slate);
    grid.fill_sphere(0, 15, 0, 13.0, granite_mid);
    grid.fill_sphere(-3, 20, -3, 8.0, highlight);
    grid.build_mesh()
}

/// Procedural low-poly faceted berry bush matching the BlendSwap #9440 geometric tree style.
/// Emits a stylized, 8-faceted octahedral jewel berry with crisp flat-shaded normals.
/// Highly optimized: requires only 8 triangles (24 vertices) compared to 20 triangles (60 vertices) of an icosahedron.
fn add_faceted_octahedron_berry(
    builder: &mut LowPolyMeshBuilder,
    center: Vec3,
    radius: f32,
    color: [f32; 4],
) {
    let top = center + Vec3::new(0.0, radius, 0.0);
    let bottom = center - Vec3::new(0.0, radius, 0.0);
    let px = center + Vec3::new(radius, 0.0, 0.0);
    let nx = center - Vec3::new(radius, 0.0, 0.0);
    let pz = center + Vec3::new(0.0, 0.0, radius);
    let nz = center - Vec3::new(0.0, 0.0, radius);

    // 4 upper pyramid faces
    builder.add_flat_triangle(top, px, pz, color);
    builder.add_flat_triangle(top, pz, nx, color);
    builder.add_flat_triangle(top, nx, nz, color);
    builder.add_flat_triangle(top, nz, px, color);

    // 4 lower pyramid faces
    builder.add_flat_triangle(bottom, pz, px, color);
    builder.add_flat_triangle(bottom, nx, pz, color);
    builder.add_flat_triangle(bottom, nz, nx, color);
    builder.add_flat_triangle(bottom, px, nz, color);
}

/// Generates separate foliage and berry geometry builders deterministically for a given seed.
/// Architectural Reason:
/// Separating foliage from berries allows the bush foliage to cast volumetric shadows onto the
/// terrain while berries receive `NotShadowCaster`, preventing thousands of sub-pixel jewel facets
/// from burdening the 6 cascaded shadow map passes.
pub fn generate_bush_builders(seed: u64) -> (LowPolyMeshBuilder, LowPolyMeshBuilder) {
    let mut rng = Prng::new(seed);
    let mut foliage_builder = LowPolyMeshBuilder::new();
    let mut berries_builder = LowPolyMeshBuilder::new();

    // 1. Branching wooden basal stems (3 optimized tapered prisms)
    let bark = [0.38, 0.26, 0.16, 1.0];
    let stem_count = 3;
    for i in 0..stem_count {
        let ang = (i as f32 / stem_count as f32) * std::f32::consts::TAU + rng.range(-0.25, 0.25);
        let reach = rng.range(0.24, 0.36);
        let lift = rng.range(0.22, 0.35);
        let base = Vec3::new(rng.range(-0.05, 0.05), 0.0, rng.range(-0.05, 0.05));
        let top = Vec3::new(ang.cos() * reach, lift, ang.sin() * reach);
        foliage_builder.add_tapered_prism(base, top, 0.06, 0.03, 4, bark, true, true);
    }

    // 2. Multi-lobed faceted foliage canopies
    let deep_green = [0.18, 0.48, 0.16, 1.0];
    let forest_green = [0.24, 0.58, 0.20, 1.0];
    let vibrant_green = [0.32, 0.68, 0.24, 1.0];
    let sunny_green = [0.42, 0.74, 0.26, 1.0];

    let mut lobe_centers: Vec<(Vec3, Vec3)> = Vec::with_capacity(5);

    // Primary central crown lobe
    let center_y = rng.range(0.48, 0.58);
    let center_blob_pos = Vec3::new(0.0, center_y, 0.0);
    let crx = rng.range(0.60, 0.72);
    let cry = rng.range(0.46, 0.58);
    let crz = rng.range(0.60, 0.72);
    foliage_builder.add_faceted_blob(center_blob_pos, Vec3::new(crx, cry, crz), forest_green, 0, &mut rng, 0.16);
    lobe_centers.push((center_blob_pos, Vec3::new(crx, cry, crz)));

    // Surrounding secondary perimeter foliage lobes
    let lobe_count = 3;
    for i in 0..lobe_count {
        let ang = (i as f32 / lobe_count as f32) * std::f32::consts::TAU + rng.range(-0.25, 0.25);
        let dist = rng.range(0.34, 0.46);
        let ly = rng.range(0.35, 0.48);
        let lobe_pos = Vec3::new(ang.cos() * dist, ly, ang.sin() * dist);
        let lrx = rng.range(0.42, 0.52);
        let lry = rng.range(0.34, 0.44);
        let lrz = rng.range(0.42, 0.52);
        let lobe_col = match i % 3 {
            0 => vibrant_green,
            1 => sunny_green,
            _ => deep_green,
        };
        foliage_builder.add_faceted_blob(lobe_pos, Vec3::new(lrx, lry, lrz), lobe_col, 0, &mut rng, 0.15);
        lobe_centers.push((lobe_pos, Vec3::new(lrx, lry, lrz)));
    }

    // 3. Faceted ruby red berries (clean, sparkling 8-facet octahedra)
    let berry_ruby = [0.92, 0.10, 0.14, 1.0];
    let berry_crimson = [0.80, 0.06, 0.10, 1.0];
    let berry_bright = [0.98, 0.22, 0.26, 1.0];

    let berry_count = 8 + (rng.range(0.0, 3.0) as usize);
    for i in 0..berry_count {
        let (l_pos, l_radii) = lobe_centers[i % lobe_centers.len()];
        let b_ang = rng.range(0.0, std::f32::consts::TAU);
        let b_pitch = rng.range(0.15, 1.20);
        let dir = Vec3::new(b_ang.cos() * b_pitch.cos(), b_pitch.sin(), b_ang.sin() * b_pitch.cos()).normalize();

        let berry_pos = l_pos + Vec3::new(dir.x * l_radii.x, dir.y * l_radii.y, dir.z * l_radii.z) * 0.94;
        let b_rad = rng.range(0.08, 0.11);
        let b_col = match i % 4 {
            0 => berry_bright,
            1 => berry_crimson,
            _ => berry_ruby,
        };
        add_faceted_octahedron_berry(&mut berries_builder, berry_pos, b_rad, b_col);
    }

    (foliage_builder, berries_builder)
}

/// Creates bush foliage mesh (branches and green leaves) capable of casting shadows.
pub fn create_lowpoly_bush_foliage_mesh(seed: u64) -> Mesh {
    let (foliage, _) = generate_bush_builders(seed);
    foliage.build()
}

/// Creates jewel berry mesh to be spawned as a child entity with `NotShadowCaster`.
pub fn create_lowpoly_bush_berries_mesh(seed: u64) -> Mesh {
    let (_, berries) = generate_bush_builders(seed);
    berries.build()
}

/// Combined low-poly bush mesh for single-mesh environments (e.g. editor previews / tests).
pub fn create_lowpoly_bush_mesh(seed: u64) -> Mesh {
    let (mut foliage, berries) = generate_bush_builders(seed);
    foliage.append(berries);
    foliage.build()
}

pub fn create_voxel_bush_mesh() -> Mesh {
    create_lowpoly_bush_mesh(1337)
}

/// Procedural low-poly fallen branch matching the geometric tree aesthetic.
/// Features a tapered main limb with broken sapwood butt end and branching side twigs (~48 vertices).
pub fn create_lowpoly_branch_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut builder = LowPolyMeshBuilder::new();

    let bark_dark = [0.34, 0.22, 0.12, 1.0];
    let bark_mid = [0.46, 0.30, 0.16, 1.0];
    let broken_wood = [0.78, 0.65, 0.46, 1.0];

    // Main stem: 2 slightly angled tapered segments lying flat on the ground
    let p0 = Vec3::new(-0.32, 0.03, rng.range(-0.02, 0.02));
    let p1 = Vec3::new(0.0, 0.04 + rng.range(-0.01, 0.01), rng.range(0.02, 0.05));
    let p2 = Vec3::new(0.35, 0.025, rng.range(-0.04, -0.01));

    // Segment 1 (butt to mid joint)
    builder.add_tapered_prism(p0, p1, 0.038, 0.028, 4, bark_mid, true, false);
    // Broken butt end cap highlight (sapwood ring)
    let butt_offset = (p0 - p1).normalize() * 0.005;
    builder.add_tapered_prism(p0, p0 + butt_offset, 0.036, 0.034, 4, broken_wood, true, false);

    // Segment 2 (mid joint to tip)
    builder.add_tapered_prism(p1, p2, 0.028, 0.016, 4, bark_dark, false, true);

    // Twig fork 1 (branching outward)
    let t1_start = p1 + Vec3::new(-0.06, 0.0, 0.01);
    let t1_end = t1_start + Vec3::new(rng.range(0.10, 0.16), 0.02, rng.range(0.12, 0.18));
    builder.add_tapered_prism(t1_start, t1_end, 0.018, 0.008, 4, bark_mid, true, true);

    // Twig fork 2 (branching opposite side)
    let t2_start = p1 + Vec3::new(0.10, 0.0, -0.01);
    let t2_end = t2_start + Vec3::new(rng.range(0.08, 0.14), 0.015, rng.range(-0.16, -0.10));
    builder.add_tapered_prism(t2_start, t2_end, 0.015, 0.006, 4, bark_dark, true, true);

    builder.build()
}

pub fn create_voxel_branch_mesh() -> Mesh {
    create_lowpoly_branch_mesh(1337)
}

/// Procedural low-poly knapped flint shard.
/// Sharp asymmetric wedge geometry with dorsal ridges, striking platform, and cyan-blue conchoidal highlights (~30 vertices).
pub fn create_lowpoly_flint_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut builder = LowPolyMeshBuilder::new();

    let chert_black = [0.10, 0.12, 0.15, 1.0];
    let chert_dark = [0.16, 0.24, 0.34, 1.0];
    let chert_blue = [0.26, 0.44, 0.60, 1.0];
    let cyan_edge = [0.46, 0.76, 0.92, 1.0];
    let edge_highlight = [0.84, 0.95, 1.0, 1.0];

    // Knapped arrowhead / chert wedge geometry
    // Sharp central ridge, striking platform, tapered razor edges
    let tip = Vec3::new(0.0, 0.02 + rng.range(-0.005, 0.005), 0.18);
    let ridge_mid = Vec3::new(rng.range(-0.01, 0.01), 0.11, 0.02);
    let platform_top = Vec3::new(0.0, 0.08, -0.14);
    let platform_base = Vec3::new(0.0, 0.01, -0.16);

    let edge_left = Vec3::new(-0.10 + rng.range(-0.01, 0.01), 0.02, 0.0);
    let edge_right = Vec3::new(0.10 + rng.range(-0.01, 0.01), 0.02, 0.0);
    let base_mid = Vec3::new(0.0, 0.005, 0.0);

    // Upper sharp facets (knapped dorsal faces)
    builder.add_flat_triangle(ridge_mid, tip, edge_left, cyan_edge);
    builder.add_flat_triangle(ridge_mid, edge_right, tip, edge_highlight);
    builder.add_flat_triangle(platform_top, ridge_mid, edge_left, chert_blue);
    builder.add_flat_triangle(platform_top, edge_right, ridge_mid, chert_dark);

    // Striking platform (proximal butt end)
    builder.add_flat_triangle(platform_top, platform_base, edge_left, chert_black);
    builder.add_flat_triangle(platform_top, edge_right, platform_base, chert_dark);

    // Ventral faces (flatter underside)
    builder.add_flat_triangle(base_mid, edge_left, tip, chert_blue);
    builder.add_flat_triangle(base_mid, tip, edge_right, chert_dark);
    builder.add_flat_triangle(base_mid, platform_base, edge_left, chert_black);
    builder.add_flat_triangle(base_mid, edge_right, platform_base, chert_black);

    builder.build()
}

pub fn create_voxel_flint_mesh() -> Mesh {
    create_lowpoly_flint_mesh(1337)
}

/// Procedural low-poly loose river stone / granite pebble.
/// Polyhedral faceted geometry with 20 flat facets and subtle quartz flecking (~60 vertices).
pub fn create_lowpoly_stone_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut builder = LowPolyMeshBuilder::new();

    let granite_base = [0.52, 0.50, 0.48, 1.0];

    let rx = rng.range(0.24, 0.28);
    let ry = rng.range(0.13, 0.17);
    let rz = rng.range(0.20, 0.24);
    let center = Vec3::new(0.0, ry * 0.9, 0.0);

    builder.add_faceted_blob(center, Vec3::new(rx, ry, rz), granite_base, 0, &mut rng, 0.18);

    builder.build()
}

pub fn create_voxel_stone_mesh() -> Mesh {
    create_lowpoly_stone_mesh(1337)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lowpoly_bush_mesh_attributes_and_budget() {
        let bush = create_lowpoly_bush_mesh(1337);
        assert!(bush.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(bush.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(bush.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert!(bush.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
        assert!(bush.indices().is_some());

        let vert_count = bush.count_vertices();
        assert!(vert_count >= 100, "Bush should have faceted detail: {vert_count}");
        assert!(vert_count <= 2500, "Bush exceeds low-poly budget: {vert_count}");
    }

    #[test]
    fn test_lowpoly_bush_foliage_and_berries_split() {
        let foliage = create_lowpoly_bush_foliage_mesh(1337);
        let berries = create_lowpoly_bush_berries_mesh(1337);

        assert!(foliage.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(berries.attribute(Mesh::ATTRIBUTE_POSITION).is_some());

        let f_verts = foliage.count_vertices();
        let b_verts = berries.count_vertices();
        assert!(f_verts >= 100, "Foliage should have substantial verts: {f_verts}");
        assert!(b_verts >= 48, "Berries should have octahedral facets: {b_verts}");
        assert!(b_verts < f_verts, "Berries mesh must be significantly smaller than foliage");
    }

    #[test]
    fn test_lowpoly_bush_seed_variety() {
        let b1 = create_lowpoly_bush_mesh(1111);
        let b2 = create_lowpoly_bush_mesh(2222);
        let pos1 = b1.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        let pos2 = b2.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        assert_ne!(pos1[0], pos2[0], "Different seeds should produce varied geometry");
    }

    #[test]
    fn test_lowpoly_ground_clutter_meshes() {
        let branch = create_lowpoly_branch_mesh(1337);
        let flint = create_lowpoly_flint_mesh(1337);
        let stone = create_lowpoly_stone_mesh(1337);

        // Verify attribute compliance
        for (mesh, name, min_v, max_v) in [
            (&branch, "Branch", 24, 250),
            (&flint, "Flint", 18, 100),
            (&stone, "Stone", 30, 150),
        ] {
            assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some(), "{name} missing POSITION");
            assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some(), "{name} missing NORMAL");
            assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some(), "{name} missing COLOR");
            assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some(), "{name} missing UV");
            assert!(mesh.indices().is_some(), "{name} missing INDICES");

            let vert_count = mesh.count_vertices();
            assert!(
                vert_count >= min_v && vert_count <= max_v,
                "{name} vertex count {vert_count} outside expected range [{min_v}, {max_v}]"
            );
        }
    }

    #[test]
    fn test_voxel_forwarders_compatibility() {
        let bush = create_voxel_bush_mesh();
        assert!(bush.count_vertices() >= 100);

        let branch = create_voxel_branch_mesh();
        assert!(branch.count_vertices() >= 24);

        let flint = create_voxel_flint_mesh();
        assert!(flint.count_vertices() >= 18);

        let stone = create_voxel_stone_mesh();
        assert!(stone.count_vertices() >= 30);
    }
}
