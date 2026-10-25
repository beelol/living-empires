#[allow(dead_code)]
#[path = "../src/terrain/mod.rs"]
mod terrain;
use std::collections::HashSet;
use terrain::*;

// Catches nondeterminism, ocean-only worlds, disconnected biome islands, and
// optimistic area accounting that counts unsafe cells as playable land.
#[test]
fn independently_selected_seeds_have_connected_playable_biomes() {
    for seed in [928_751, 44_019, 8_170_321] {
        let world = Terrain::generate(seed);
        assert_eq!(world.info(), Terrain::generate(seed).info());
        assert!(
            world.connected_land_area() >= 4_000_000.0,
            "seed {seed}: {}",
            world.connected_land_area()
        );
        let spawn = world.spawn();
        assert!(world.can_traverse(spawn, [spawn[0] + 17.0, spawn[1] + 12.0]));
        let mut biomes = HashSet::new();
        for y in (0..=384).step_by(4) {
            for x in (0..=384).step_by(4) {
                let s = world.sample(f64::from(x * 8), f64::from(y * 8)).unwrap();
                if s.traversable {
                    biomes.insert(s.biome);
                }
                if x == 0 || y == 0 || x == 384 || y == 384 {
                    assert!(s.water_depth > 0.0);
                }
            }
        }
        assert_eq!(biomes.len(), 5, "seed {seed}: {biomes:?}");
    }
}

// A non-planar cell distinguishes triangle interpolation from bilinear height.
#[test]
fn height_queries_follow_rendered_triangle_and_chunks_share_edges() {
    let world = Terrain::generate(781);
    let a = world.chunk(12, 12).unwrap();
    let b = world.chunk(13, 12).unwrap();
    for row in 0..17 {
        assert_eq!(a.heights[row * 17 + 16], b.heights[row * 17]);
    }
    for (u, v) in [(0.75, 0.25), (0.25, 0.75), (0.5, 0.5)] {
        let expected = if u >= v {
            a.heights[0] * (1.0 - u) + a.heights[1] * (u - v) + a.heights[18] * v
        } else {
            a.heights[0] * (1.0 - v) + a.heights[17] * (v - u) + a.heights[18] * u
        };
        let sample = world
            .sample(1536.0 + f64::from(u) * 8.0, 1536.0 + f64::from(v) * 8.0)
            .unwrap();
        assert!((sample.height - expected).abs() < 0.0001);
        assert_eq!(
            sample,
            a.sample(1536.0 + f64::from(u) * 8.0, 1536.0 + f64::from(v) * 8.0)
                .unwrap()
        );
    }
    assert!(world.sample(f64::NAN, 1.0).is_none());
    assert!(world.sample(3073.0, 1.0).is_none());
    assert!(!world.can_traverse(world.spawn(), [0.0, 0.0]));
    assert!(!world.can_traverse([f64::INFINITY, 0.0], world.spawn()));
}

// Catches lost depletion, silent regeneration, corruption tolerance, and leakage
// of authoritative records into the public geometry transfer.
#[test]
fn saved_geography_and_mutable_deposits_survive_without_regeneration() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("terrain.json");
    let mut world = Terrain::generate(987_321);
    let deposit = world.deposits()[0].clone();
    world.deplete_deposit(deposit.id, 17).unwrap();
    let fingerprint = world.info().fingerprint;
    world.save(&path).unwrap();
    let restored = Terrain::load(&path).unwrap();
    assert_eq!(restored.deposits()[0].remaining, deposit.remaining - 17);
    assert_eq!(restored.info().fingerprint, fingerprint);
    assert_eq!(
        restored.sample(1754.5, 1302.2),
        world.sample(1754.5, 1302.2)
    );
    assert!(world.deplete_deposit(deposit.id, u64::MAX).is_err());
    assert_eq!(world.deposits()[0].remaining, deposit.remaining - 17);
    let mut chunks = Vec::new();
    for y in 0..24 {
        for x in 0..24 {
            chunks.push(world.chunk(x, y).unwrap());
        }
    }
    let public = serde_json::to_string(&(world.info(), &chunks)).unwrap();
    assert!(!public.contains("deposit"));
    assert!(!public.contains("remaining"));
    assert!(!public.contains("seed"));
    let client = Terrain::from_chunks(world.info(), chunks).unwrap();
    assert!(client.deposits().is_empty());
    assert_eq!(client.sample(1754.5, 1302.2), world.sample(1754.5, 1302.2));
    assert!(client.save(temp.path().join("public.json")).is_err());
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    assert!(Terrain::load(&path).is_err());
    let mut corrupted = bytes.clone();
    let i = corrupted.iter().position(|b| *b == b'7').unwrap();
    corrupted[i] = b'8';
    std::fs::write(&path, &corrupted).unwrap();
    assert!(Terrain::load(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupted);
    assert!(Terrain::load(temp.path().join("missing.json")).is_err());
}

#[test]
fn malformed_or_mixed_public_chunks_are_rejected() {
    let world = Terrain::generate(4);
    let info = world.info();
    let mut bad_info = info.clone();
    bad_info.cells = u32::MAX;
    assert!(bad_info.validate().is_err());
    let mut chunk = world.chunk(0, 0).unwrap();
    chunk.heights[0] = f32::NAN;
    assert!(chunk.validate(&info).is_err());
    chunk.heights[0] = 9000.0;
    assert!(chunk.validate(&info).is_err());
    let mut chunk = world.chunk(0, 0).unwrap();
    chunk.biome_weights[0] = [0.0; 5];
    assert!(chunk.validate(&info).is_err());
    let mut chunk = world.chunk(0, 0).unwrap();
    chunk.heights.pop();
    assert!(chunk.validate(&info).is_err());
    assert!(Terrain::from_chunks(info.clone(), vec![]).is_err());
    let mut chunks: Vec<_> = (0..24)
        .flat_map(|y| (0..24).map(move |x| (x, y)))
        .map(|(x, y)| world.chunk(x, y).unwrap())
        .collect();
    chunks[1].heights[0] += 1.0; // Shared edge cannot disagree even before hash.
    assert!(Terrain::from_chunks(info, chunks).is_err());
}

#[test]
fn a_nonplanar_fixture_proves_triangle_height_and_slope() {
    let mut chunk = TerrainChunk {
        chunk_x: 0,
        chunk_y: 0,
        heights: vec![0.0; 289],
        moisture: vec![0.5; 289],
        geology: vec![0.5; 289],
        biome_weights: vec![[0.0, 1.0, 0.0, 0.0, 0.0]; 289],
    };
    chunk.heights[1] = 8.0;
    chunk.heights[17] = 16.0;
    chunk.heights[18] = 40.0;
    assert_eq!(chunk.sample(6.0, 2.0).unwrap().height, 14.0);
    assert_eq!(chunk.sample(2.0, 6.0).unwrap().height, 18.0);
    assert_eq!(chunk.sample(4.0, 4.0).unwrap().height, 20.0);
    assert!((chunk.sample(6.0, 2.0).unwrap().slope - 17.0_f32.sqrt()).abs() < 0.00001);
    assert!(!chunk.sample(6.0, 2.0).unwrap().traversable);
}

#[test]
fn authenticated_envelope_still_rejects_invalid_records() {
    use sha2::{Digest, Sha256};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("world.terrain");
    let world = Terrain::generate(129);
    world.save(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let original: serde_json::Value = serde_json::from_slice(&bytes[48..]).unwrap();
    for case in 0..5 {
        let mut value = original.clone();
        match case {
            0 => value["info"]["cells"] = u32::MAX.into(),
            1 => value["heights"][0] = 700.0.into(),
            2 => value["deposits"][0]["remaining"] = u64::MAX.into(),
            3 => value["deposits"][0]["x"] = (-1.0).into(),
            _ => value["heights"][0] = (-3.0).into(), // Valid range, wrong geometry hash.
        }
        let payload = serde_json::to_vec(&value).unwrap();
        let mut encoded = b"ARGTRN01".to_vec();
        encoded.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        encoded.extend_from_slice(&Sha256::digest(&payload));
        encoded.extend_from_slice(&payload);
        std::fs::write(&path, &encoded).unwrap();
        assert!(Terrain::load(&path).is_err(), "case {case}");
        assert_eq!(std::fs::read(&path).unwrap(), encoded);
    }
    let mut too_large = bytes[..48].to_vec();
    too_large[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    std::fs::write(&path, too_large).unwrap();
    assert!(Terrain::load(&path).is_err());
}

// Independent flood fill from the actual spawn proves the five biome samples
// are part of the same reachable land, rather than separate showcase islands.
#[test]
fn every_biome_is_reachable_from_spawn_on_three_seeds() {
    use std::collections::VecDeque;
    for seed in [928_751, 44_019, 8_170_321] {
        let world = Terrain::generate(seed);
        let mut seen = vec![false; 193 * 193];
        let mut queue = VecDeque::from([(96usize, 96usize)]);
        seen[96 * 193 + 96] = true;
        let mut biomes = HashSet::new();
        while let Some((x, y)) = queue.pop_front() {
            let here = [(x * 16) as f64, (y * 16) as f64];
            biomes.insert(world.sample(here[0], here[1]).unwrap().biome);
            for (nx, ny) in [
                (x.saturating_sub(1), y),
                (x + 1, y),
                (x, y.saturating_sub(1)),
                (x, y + 1),
            ] {
                if nx >= 193 || ny >= 193 || seen[ny * 193 + nx] {
                    continue;
                }
                let next = [(nx * 16) as f64, (ny * 16) as f64];
                if world.can_traverse(here, next) {
                    seen[ny * 193 + nx] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        assert_eq!(biomes.len(), 5, "seed {seed}: {biomes:?}");
        println!(
            "seed {seed}: connected safe area {:.3} km², {} reachable biome types",
            world.connected_land_area() / 1_000_000.0,
            biomes.len()
        );
    }
}
