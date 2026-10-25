//! Renderer geometry contracts. Coordinates are metres and triangles match authority.
#[derive(Default)]
pub(super) struct Geometry {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}
pub(super) fn grid_indices(cells: u32) -> Vec<u32> {
    let mut result = Vec::with_capacity((cells * cells * 6) as usize);
    for z in 0..cells {
        for x in 0..cells {
            let a = z * (cells + 1) + x;
            let b = a + cells + 1;
            result.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        }
    }
    result
}
pub(super) fn normal_at(
    x: f32,
    z: f32,
    extent: f32,
    spacing: f32,
    height: impl Fn(f32, f32) -> f32,
) -> [f32; 3] {
    let (left, right) = ((x - spacing).max(0.), (x + spacing).min(extent));
    let (back, front) = ((z - spacing).max(0.), (z + spacing).min(extent));
    let dx = (height(right, z) - height(left, z)) / (right - left);
    let dz = (height(x, front) - height(x, back)) / (front - back);
    let len = (dx * dx + 1. + dz * dz).sqrt();
    [-dx / len, 1. / len, -dz / len]
}
pub(super) fn visible_chunks(
    x: f32,
    z: f32,
    chunk_size: f32,
    axis: u32,
    radius: i32,
) -> Vec<(u32, u32)> {
    let cx = ((x / chunk_size).floor() as i32).clamp(0, axis as i32 - 1);
    let cz = ((z / chunk_size).floor() as i32).clamp(0, axis as i32 - 1);
    let mut result = Vec::new();
    for z in (cz - radius).max(0)..=(cz + radius).min(axis as i32 - 1) {
        for x in (cx - radius).max(0)..=(cx + radius).min(axis as i32 - 1) {
            result.push((x as u32, z as u32));
        }
    }
    result.sort_by_key(|(x, z)| (*x as i32 - cx).pow(2) + (*z as i32 - cz).pow(2));
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn triangles_use_authority_diagonal_and_upward_winding() {
        assert_eq!(grid_indices(1), vec![0, 2, 3, 0, 3, 1]);
        let indices = grid_indices(16);
        assert_eq!(indices.len(), 16 * 16 * 6);
        assert!(indices.iter().all(|i| *i < 17 * 17));
        for tri in indices.chunks_exact(3) {
            let p: Vec<_> = tri
                .iter()
                .map(|i| [(*i % 17) as i32, (*i / 17) as i32])
                .collect();
            let cross = (p[1][1] - p[0][1]) * (p[2][0] - p[0][0])
                - (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]);
            assert_eq!(cross, 1);
        }
    }
    #[test]
    fn normals_match_plane_even_at_world_edges() {
        for (x, z) in [(0., 0.), (128., 128.), (3072., 3072.)] {
            let n = normal_at(x, z, 3072., 8., |x, z| 0.25 * x + 0.5 * z);
            let len = (0.25_f32.powi(2) + 1. + 0.5_f32.powi(2)).sqrt();
            for (a, b) in n.into_iter().zip([-0.25 / len, 1. / len, -0.5 / len]) {
                assert!((a - b).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn residency_is_bounded_sorted_and_clipped() {
        for (x, z) in [(0., 0.), (1536., 1536.), (3072., 3072.), (-100., 9000.)] {
            let chunks = visible_chunks(x, z, 128., 24, 5);
            assert!(!chunks.is_empty());
            assert!(chunks.len() <= 121);
            assert!(chunks.iter().all(|(x, z)| *x < 24 && *z < 24));
            let mut unique = chunks.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), chunks.len());
        }
        assert_eq!(visible_chunks(130., 130., 128., 24, 5)[0], (1, 1));
    }
}
