# Terrain core stream

Owner: terrain_core subagent, coordinated by terrain integration owner. Base:
`db64e4d40dd2f0e8ec0fde5801c2a40d23272bea`. Read the entire README and AGENTS.md.
Primary ACs: EXP-01, EXP-02, EXP-03, EXP-09, EXP-10, EXP-11. Affected:
EXP-04, PLY-02, PLY-03, SET-02, SET-05, WORLD-05. Owned paths:
`dev/runtime/src/terrain/`, `dev/runtime/tests/terrain.rs`, this record.

Dependencies: parent owns authoritative world, network, export handoff and release
integration. Renderer owner consumes terrain v1 through `ClientState.terrain`.
Existing diplomacy owner retains runtime lib.rs and dependencies until handoff.
No shared schema or dependency file changed in this stream. Public packet structs
are additive; no existing world format changed here. No private deposits or seed
are sent in TerrainInfo/TerrainChunk. Deposit depletion is an authority primitive;
caller still owns consent, cost, idempotency and durable transaction acknowledgement.

## Frozen v1 interface

All public records derive Clone, Debug, serde Serialize/Deserialize, PartialEq;
Terrain is Clone/Debug but deliberately cannot serialize as a public packet.
`WORLD_SIZE:f32=3072`, `CELL_SIZE:f32=8`, `CELLS:u32=384`,
`CHUNK_CELLS:u32=16`. Origin (0,0); map x/y maps renderer x/z, height Y up.

- `Biome`: Coast, Grassland, Wetland, Jungle, LuminousGrove, in that weight order.
- `TerrainInfo`: schema_version:u32, generator_version:u32, cells:u32,
  cell_size:f32, chunk_cells:u32, sea_level:f32, fingerprint:String.
  `validate(&self)->anyhow::Result<()>` rejects unsupported dimensions/version.
- `TerrainChunk`: chunk_x:u32, chunk_y:u32, heights:Vec<f32>,
  moisture:Vec<f32>, geology:Vec<f32>, biome_weights:Vec<[f32;5]>.
  Arrays are 17x17 row-major vertices. `validate(&self,&TerrainInfo)->Result<()>`
  checks coordinate/length/finiteness/ranges/normalized weights.
  `sample(&self,x:f64,y:f64)->Option<Surface>` uses world coordinates.
- `Surface`: height:f32, slope:f32 (rise/run), water_depth:f32, biome:Biome,
  biome_weights:[f32;5], moisture:f32, geology:f32, movement_multiplier:f32,
  traversable:bool. Movement multiplier is informational for the parent to apply.
- `Terrain::generate(seed:u64)->Terrain`; `info(&self)->TerrainInfo`;
  `sample(&self,x:f64,y:f64)->Option<Surface>`;
  `chunk(&self,x:u32,y:u32)->Option<TerrainChunk>`;
  `can_traverse(&self,from:[f64;2],to:[f64;2])->bool`;
  `spawn(&self)->[f64;2]`; `connected_land_area(&self)->f64` (square metres);
  `from_chunks(info:TerrainInfo,chunks:Vec<TerrainChunk>)->Result<Terrain>`;
  `load(path:impl AsRef<Path>)->Result<Terrain>`;
  `save(&self,path:impl AsRef<Path>)->Result<()>`.
- Private-authority access: `deposits(&self)->&[Deposit]`;
  `deplete_deposit(&mut self,id:u64,quantity:u64)->Result<()>`.
  Deposit fields: id:u64, x:f64, y:f64, kind:DepositKind (Metal/Crystal),
  initial_quantity:u64, remaining:u64. Invalid overdraw leaves quantities intact.

Triangles use diagonal 00→11: Bevy upward winding [00,01,11] and [00,11,10],
where 01 is next row. Exact triangle interpolation provides height and slope;
renderer computes smooth normals using globally shared samples. Ground is safe at
height >=0.2m and rise/run <=0.7. Straight segment checks split at every crossed
triangle edge, evaluate endpoints and interval interiors, and reject invalid input.
This is terrain-only collision: buildings/units/routing remain separate dependencies.

## Stability, scale, and persistence

Generator bakes smooth seeded shorelines, low-frequency relief, moisture/geology,
continuous normalized biome weights and private finite deposits. Land area counts
only complete safe cells in the largest four-neighbour component, excluding ocean
and partially safe shoreline cells. Spawn is the central safe plateau.

The save envelope is ARGTRN01, little-endian u64 payload length, SHA256 of payload,
then JSON baked records. A 64MiB cap bounds reads before payload allocation.
Load validates all dimensions, fields, quantities and geometry hash and never calls
the generator. Missing, corrupt or incompatible records fail without overwriting.
Save creates a unique sibling temporary file, fsyncs, renames, and fsyncs the
parent directory. Parent owns serialization with the world transaction lock.
Geometry fingerprint remains unchanged when a deposit is mined. Public reconstruction
requires all 576 unique validated chunks, exact shared edges and final fingerprint;
its zero-deposit Terrain cannot overwrite an authoritative save.

## Evidence and limits

Red: generation/geometry tests failed on unimplemented generation; persistence test
failed on absent deposits. Green results and final package suite recorded at handoff.
Independent data tests select seeds 928751, 44019, 8170321 and flood-fill from spawn
using authoritative segment checks. These are implementation checks, not README
acceptance passes. Actual two-client journey/screenshots and independent visual
judgments belong to parent audit. Mining/building persistence remains blocked on
those gameplay systems. Frontier expansion, sensor/fog filtering, settlement
collision, pathfinding, approved LIVE release and reference-device performance are
not implemented by this core and cannot be marked passed.

Final core evidence: all 7 terrain tests passed; measured safe connected areas
5.880, 5.881, 5.931 km² respectively; all five biomes reachable from spawn in
each seed. `cargo clippy -p argon-dev --test terrain --offline -- -D warnings`
and explicit rustfmt check passed. Runtime package suite first run: content 13/13
passed, seven existing HTTP tests failed because local socket binding was denied.
Escalated rerun: existing content tests special_files_cannot_block_content_loading,
native_pattern_work_cannot_stall_loading_or_movement, and
native_collection_operations_cannot_bypass_instruction_limits hit their one-second
subprocess deadlines under concurrent load (10 other content tests passed).
Serial rerun was requested twice but automatic permission approval review timed out
before execution both times. Parent must run serial runtime/workspace checks on the
integrated revision; no passing full-suite claim is made here.

Explicit handoff: terrain integration parent owns these three paths after this
candidate commit; core owner will release its claim after committing. Parent owns
remaining integration/full-suite/native audit work and any resulting corrections.
