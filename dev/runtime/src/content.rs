//! Bounded, immutable content packs. Rust world state never enters a Lua VM.
use anyhow::{bail, ensure, Context, Result};
use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

const MAX_FILE: usize = 1024 * 1024;
const MAX_PACK: usize = 4 * MAX_FILE;
const INSTRUCTION_CHUNKS: usize = 100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    entry: String,
    files: Vec<String>,
}
struct Pack {
    version: String,
    entry: String,
    scripts: BTreeMap<String, String>,
}

pub struct ContentRuntime {
    root: PathBuf,
    active: Active,
    last_attempt: String,
}
struct Active {
    // Handles own the same VM; keeping Lua makes its lifetime and limits explicit.
    _lua: Lua,
    step: Function,
    budget: Arc<AtomicUsize>,
    version: String,
    world_name: String,
}

impl ContentRuntime {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        // Preserve the supplied path so an atomic deployment symlink swap is visible.
        let root = path.as_ref().to_path_buf();
        let pack = read_pack(&root)?;
        let last_attempt = pack.version.clone();
        Ok(Self {
            root,
            active: Active::load(pack)?,
            last_attempt,
        })
    }
    pub fn reload(&mut self) -> Result<bool> {
        let pack = read_pack(&self.root)?;
        if pack.version == self.last_attempt {
            return Ok(false);
        }
        self.last_attempt = pack.version.clone();
        let candidate = Active::load(pack)?;
        self.active = candidate;
        Ok(true)
    }
    pub fn version(&self) -> &str {
        &self.active.version
    }
    pub fn world_name(&self) -> &str {
        &self.active.world_name
    }
    pub fn step(&self, x: f64, y: f64, dx: f64, dy: f64) -> Result<(f64, f64)> {
        self.active.step(x, y, dx, dy)
    }
}

impl Active {
    fn load(pack: Pack) -> Result<Self> {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )?;
        lua.set_memory_limit(8 * MAX_FILE)?;
        // Lua hooks cannot interrupt native pattern backtracking. Keep those APIs
        // out of this live rules VM; native work must remain bounded by pack size.
        let strings: Table = lua.globals().get("string")?;
        for name in ["find", "match", "gmatch", "gsub"] {
            strings.set(name, Value::Nil)?;
        }
        for (name, source) in &pack.scripts {
            lua.load(source)
                .set_name(name)
                .into_function()
                .with_context(|| format!("invalid Lua module: {name}"))?;
        }
        // No native modules, filesystem, processes, coroutine escapes or swallowed budgets.
        for name in [
            "dofile",
            "loadfile",
            "load",
            "collectgarbage",
            "pcall",
            "xpcall",
            "print",
        ] {
            lua.globals().set(name, Value::Nil)?;
        }
        let budget = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&budget);
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(1000),
            move |_, _| {
                if counter.fetch_add(1, Ordering::Relaxed) >= INSTRUCTION_CHUNKS {
                    return Err(mlua::Error::RuntimeError(
                        "Lua instruction budget exceeded".into(),
                    ));
                }
                Ok(VmState::Continue)
            },
        )?;
        install_bounded_collections(&lua)?;
        let cache = lua.create_table()?;
        let loading = lua.create_table()?;
        let modules = pack.scripts.clone();
        let require = lua.create_function(move |lua, name: String| {
            let cached: Value = cache.raw_get(name.as_str())?;
            if !matches!(cached, Value::Nil) {
                return Ok(cached);
            }
            if loading.raw_get::<bool>(name.as_str()).unwrap_or(false) {
                return Err(mlua::Error::RuntimeError(format!("cyclic module: {name}")));
            }
            let source = modules
                .get(&name)
                .ok_or_else(|| mlua::Error::RuntimeError(format!("undeclared module: {name}")))?;
            loading.raw_set(name.as_str(), true)?;
            let result = lua.load(source).set_name(&name).eval::<Value>();
            loading.raw_set(name.as_str(), Value::Nil)?;
            let value = match result? {
                Value::Nil => Value::Boolean(true),
                other => other,
            };
            cache.raw_set(name.as_str(), value.clone())?;
            Ok(value)
        })?;
        lua.globals().set("require", require)?;
        let source = pack
            .scripts
            .get(&module_name(&pack.entry))
            .context("entry must be a declared .lua file")?;
        let table: Table = lua
            .load(source)
            .set_name(&pack.entry)
            .eval()
            .context("loading Lua entry")?;
        ensure!(
            table.get::<u32>("schema_version")? == 1,
            "unsupported Lua rules schema; expected 1"
        );
        let world_name: String = table.get("world_name")?;
        ensure!(
            !world_name.trim().is_empty() && world_name.len() <= 120,
            "world_name must contain 1–120 bytes"
        );
        let step = table
            .get::<Function>("step")
            .context("rules must export step(x,y,dx,dy)")?;
        let active = Self {
            _lua: lua,
            step,
            budget,
            version: pack.version,
            world_name,
        };
        // Runtime calls remain validated: a probe is not proof for every input.
        active.step(0., 0., 1., 0.)?;
        active.step(0., 0., 0., 1.)?;
        Ok(active)
    }
    fn step(&self, x: f64, y: f64, dx: f64, dy: f64) -> Result<(f64, f64)> {
        ensure!(
            [x, y, dx, dy].iter().all(|v| v.is_finite()),
            "movement must be finite"
        );
        ensure!(
            dx.abs() <= 1. && dy.abs() <= 1.,
            "direction must be between -1 and 1"
        );
        self.budget.store(0, Ordering::Relaxed);
        let (nx, ny): (f64, f64) = self.step.call((x, y, dx, dy)).context("Lua step failed")?;
        ensure!(
            nx.is_finite() && ny.is_finite(),
            "Lua returned non-finite coordinates"
        );
        ensure!(
            (nx - x).hypot(ny - y) <= 128.,
            "Lua exceeded 128-unit movement bound"
        );
        Ok((nx, ny))
    }
}

fn install_bounded_collections(lua: &Lua) -> Result<()> {
    // These C functions can loop without allocating or invoking the instruction
    // hook. Cap their work explicitly, including forged __len metamethods.
    lua.load(r#"
        local kind, fail, meta, size, abs, floor = type, error, getmetatable, rawlen, math.abs, math.floor
        local function index(n)
            if kind(n) ~= 'number' or n ~= floor(n) or abs(n) > 4096 then
                fail('native collection index/count exceeds 4096', 2)
            end
            return n
        end
        local function array(t)
            if kind(t) ~= 'table' or meta(t) ~= nil or size(t) > 4096 then
                fail('native collection requires a plain table of at most 4096 entries', 2)
            end
        end
        local rep, move, insert, remove, concat, unpack, sort = string.rep, table.move, table.insert, table.remove, table.concat, table.unpack, table.sort
        string.rep = function(s,n,sep) index(n); return rep(s,n,sep) end
        table.move = function(a,f,e,t,b)
            array(a); if b ~= nil then array(b) end
            index(f); index(e); index(t); index(t + math.max(0,e-f))
            return move(a,f,e,t,b)
        end
        table.insert = function(t,...)
            array(t); if size(t) >= 4096 then fail('table is full',2) end
            return insert(t,...)
        end
        table.remove = function(t,p) array(t); if p ~= nil then index(p) end; return remove(t,p) end
        table.concat = function(t,s,i,j)
            array(t); if i ~= nil then index(i) end; if j ~= nil then index(j) end
            return concat(t,s,i,j)
        end
        table.unpack = function(t,i,j)
            array(t); if i ~= nil then index(i) end; if j ~= nil then index(j) end
            return unpack(t,i,j)
        end
        table.sort = function(t,f) array(t); return sort(t,f) end
    "#).set_name("bounded collection helpers").exec()?;
    Ok(())
}
fn module_name(name: &str) -> String {
    name.trim_end_matches(".lua").replace('/', ".")
}
fn relative_file(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && !name.contains('\\'),
        "invalid content path: {name}"
    );
    ensure!(
        Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "content path must be relative without traversal: {name}"
    );
    Ok(())
}
fn read_file(root: &Path, name: &str) -> Result<Vec<u8>> {
    relative_file(name)?;
    let mut path = root.to_path_buf();
    for component in Path::new(name).components() {
        path.push(component);
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "content files cannot be symlinks: {name}"
        );
    }
    ensure!(
        fs::symlink_metadata(&path)?.is_file(),
        "content entry is not a regular file: {name}"
    );
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Recheck the descriptor below: a concurrent replacement must neither
        // block on a FIFO nor follow a substituted symlink.
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = options
        .open(&path)
        .with_context(|| format!("reading {name}"))?;
    ensure!(
        file.metadata()?.is_file(),
        "content entry is not a file: {name}"
    );
    let mut bytes = Vec::new();
    file.take((MAX_FILE + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_FILE,
        "content file exceeds 1 MiB: {name}"
    );
    Ok(bytes)
}
fn read_pack(root: &Path) -> Result<Pack> {
    // Resolve once per snapshot, not once per file, to avoid mixing two deployments.
    let root = root
        .canonicalize()
        .context("content directory unavailable")?;
    let manifest_bytes = read_file(&root, "manifest.json")?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest_bytes).context("invalid content manifest")?;
    ensure!(
        manifest.schema_version == 1,
        "unsupported content format; expected schema_version 1"
    );
    ensure!(
        !manifest.files.is_empty() && manifest.files.len() <= 128,
        "pack must declare 1–128 files"
    );
    ensure!(
        manifest.entry.ends_with(".lua") && manifest.files.contains(&manifest.entry),
        "Lua entry must be declared in files"
    );
    let mut seen = HashSet::new();
    let mut scripts = BTreeMap::new();
    let mut digest = Sha256::new();
    digest.update(&manifest_bytes);
    let mut total = manifest_bytes.len();
    for name in &manifest.files {
        ensure!(seen.insert(name), "duplicate manifest file: {name}");
        ensure!(name != "manifest.json", "manifest must not list itself");
        let bytes = read_file(&root, name)?;
        total += bytes.len();
        ensure!(total <= MAX_PACK, "development pack exceeds 4 MiB");
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(&bytes);
        if name.ends_with(".lua") {
            let source =
                String::from_utf8(bytes).with_context(|| format!("Lua must be UTF-8: {name}"))?;
            if scripts.insert(module_name(name), source).is_some() {
                bail!("ambiguous Lua module: {name}");
            }
        }
    }
    ensure!(
        read_file(&root, "manifest.json")? == manifest_bytes,
        "manifest changed while reading; retry after save"
    );
    Ok(Pack {
        version: format!("{:x}", digest.finalize()),
        entry: manifest.entry,
        scripts,
    })
}
