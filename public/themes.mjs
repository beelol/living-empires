const COLOR = /^#[\da-fA-F]{6}$/;
const SLUG = /^(?!.*--)[a-z][a-z0-9-]{0,63}$/;
const PRESETS = ['argonaut', 'sylvan', 'ember'];
const REQUIRED_PALETTE = ['shell', 'frame', 'accent', 'ground', 'rock'];
const REQUIRED_FIELDS = [
  'id', 'name', 'genre', 'description', 'biome', 'shapeLanguage', 'materialFinish', 'weathering',
  'technology', 'motifs', 'lighting', 'scale', 'gameplayRole', 'exclusions', 'visualChecks',
  'assumptions', 'renderingNotes', 'referencePreset', 'palette', 'sky', 'flora', 'architecture',
];

export const themeSchema = {
  type: 'object',
  additionalProperties: false,
  required: REQUIRED_FIELDS,
  properties: {
    id: { type: 'string', pattern: '^(?!.*--)[a-z][a-z0-9-]{0,63}$' },
    name: { type: 'string', minLength: 1, maxLength: 100 },
    genre: { type: 'string', minLength: 1, maxLength: 120 },
    description: { type: 'string', minLength: 1, maxLength: 2000 },
    biome: { type: 'string', minLength: 1, maxLength: 200 },
    shapeLanguage: { type: 'string', minLength: 1, maxLength: 500 },
    materialFinish: { type: 'string', minLength: 1, maxLength: 500 },
    weathering: { type: 'string', minLength: 1, maxLength: 500 },
    technology: { type: 'string', minLength: 1, maxLength: 500 },
    motifs: { type: 'array', minItems: 1, maxItems: 32, items: { type: 'string', minLength: 1, maxLength: 500 } },
    lighting: { type: 'string', minLength: 1, maxLength: 500 },
    scale: { type: 'string', minLength: 1, maxLength: 300 },
    gameplayRole: { type: 'string', minLength: 1, maxLength: 500 },
    exclusions: { type: 'array', minItems: 1, maxItems: 32, items: { type: 'string', minLength: 1, maxLength: 500 } },
    visualChecks: { type: 'array', minItems: 1, maxItems: 32, items: { type: 'string', minLength: 1, maxLength: 500 } },
    assumptions: { type: 'array', minItems: 0, maxItems: 32, items: { type: 'string', minLength: 1, maxLength: 500 } },
    renderingNotes: { type: 'array', minItems: 1, maxItems: 32, items: { type: 'string', minLength: 1, maxLength: 500 } },
    referencePreset: { type: 'string', enum: PRESETS },
    palette: { type: 'object', additionalProperties: false, required: REQUIRED_PALETTE, properties: Object.fromEntries(REQUIRED_PALETTE.map(k => [k, { type: 'string', pattern: '^#[0-9a-fA-F]{6}$' }])) },
    sky: { type: 'string', pattern: '^#[0-9a-fA-F]{6}$' },
    flora: { type: 'string', enum: ['crystal', 'tree', 'basalt'] },
    architecture: { type: 'string', enum: ['industrial', 'organic', 'fortified'] },
  },
};

export function validateTheme(contract) {
  if (!contract || typeof contract !== 'object' || Array.isArray(contract)) throw new Error('Theme contract must be an object.');
  for (const key of REQUIRED_FIELDS) if (!(key in contract)) throw new Error(`Theme contract is missing ${key}.`);
  if (Object.keys(contract).some(key => !REQUIRED_FIELDS.includes(key))) throw new Error('Theme contract contains unsupported fields.');
  if (typeof contract.id !== 'string' || !SLUG.test(contract.id)) throw new Error('Theme id must be a lowercase slug.');
  const bounded = (key, min, max) => {
    const value = contract[key];
    if (typeof value !== 'string' || value.trim().length < min || new TextEncoder().encode(value).length > max) throw new Error(`Theme ${key} must be ${min}–${max} characters.`);
  };
  for (const [key, max] of Object.entries({ name: 100, genre: 120, description: 2000, biome: 200, shapeLanguage: 500, materialFinish: 500, weathering: 500, technology: 500, lighting: 500, scale: 300, gameplayRole: 500 })) bounded(key, 1, max);
  const byteLength = value => new TextEncoder().encode(value).length;
  const list = (key, maxItems, minLength, maxLength, minItems = 1) => {
    const value = contract[key];
    if (!Array.isArray(value) || value.length < minItems || value.length > maxItems || value.some(v => typeof v !== 'string' || v.trim().length < minLength || byteLength(v) > maxLength)) {
      throw new Error(`Theme ${key} must contain ${minItems}–${maxItems} bounded strings.`);
    }
  };
  list('motifs', 32, 1, 500); list('exclusions', 32, 1, 500); list('visualChecks', 32, 1, 500);
  list('assumptions', 32, 1, 500, 0); list('renderingNotes', 32, 1, 500);
  if (!contract.palette || typeof contract.palette !== 'object' || Array.isArray(contract.palette) || Object.keys(contract.palette).length !== 5 || REQUIRED_PALETTE.some(k => !COLOR.test(contract.palette[k] || ''))) throw new Error('Theme palette must provide five #RRGGBB colors.');
  if (!COLOR.test(contract.sky)) throw new Error('Theme sky must be a #RRGGBB color.');
  if (!PRESETS.includes(contract.referencePreset)) throw new Error('Theme referencePreset is invalid.');
  if (!['crystal', 'tree', 'basalt'].includes(contract.flora)) throw new Error('Theme flora is invalid.');
  if (!['industrial', 'organic', 'fortified'].includes(contract.architecture)) throw new Error('Theme architecture is invalid.');
  return contract;
}

const presets = {
  argonaut: {
    id: 'argonaut', name: 'Argonaut frontier', genre: 'Alien machine civilization',
    description: 'A weathered machine civilization on a luminous alien frontier. Ceramic shells, graphite frames, mineral terrain, and cyan energy define functional industrial silhouettes.',
    biome: 'Wind-scoured mineral plain with luminous crystal growths', shapeLanguage: 'Broad cantilevered plates, paired relay fins, and precise cylindrical masts',
    materialFinish: 'Matte pale ceramic shells, graphite alloy frames, and translucent cyan accents', weathering: 'Fine dust abrasion, chipped ceramic edges, and mineral deposits near the ground',
    technology: 'Advanced relay equipment integrated into practical field infrastructure', motifs: ['paired relay fins', 'ceramic canopy', 'cyan signal marks'],
    lighting: 'Cool diffuse sky light with restrained cyan accents', scale: 'Human-scale outpost with a prominent signal mast',
    gameplayRole: 'Traversable sensor relay that marks a safe route across the frontier', exclusions: ['medieval castle forms', 'human ornament', 'cartoon props', 'lush forest growth'],
    visualChecks: ['canopy reads as a broad cantilevered ceramic plate', 'paired relay fins remain visible above the roofline', 'graphite frame contrasts with the pale shell'],
    assumptions: ['The site is an exposed alien frontier outpost.', 'The relay should remain readable at human scale.'],
    renderingNotes: ['Keep cyan accents sparse and legible.', 'Show mineral ground contact beneath all structural supports.', 'Micro weathering and texture are contract metadata, not separately rendered detail.'],
    referencePreset: 'argonaut', palette: { shell: '#d9ded0', frame: '#34464a', accent: '#80edc4', ground: '#53685d', rock: '#81958a' },
    sky: '#b6d2cf', flora: 'crystal', architecture: 'industrial',
  },
  sylvan: {
    id: 'sylvan', name: 'Verdant sanctuary', genre: 'Ancient forest civilization',
    description: 'An ancient forest world with mossy stone sanctuaries, timber supports, warm amber light, and towering living groves. Organic, grounded silhouettes define the architecture.',
    biome: 'Deep temperate forest with moss, roots, and tall living trees', shapeLanguage: 'Conical grove pavilion, slender posts, and branching organic lines',
    materialFinish: 'Weathered timber, moss-softened stone, and warm amber lantern glass', weathering: 'Moss in joints, softened stone edges, and root marks at ground level',
    technology: 'Quiet wayfinding and sensor craft integrated into a living sanctuary', motifs: ['conical grove pavilion', 'amber lanterns', 'branch-like braces'],
    lighting: 'Soft green canopy light with warm amber lantern pools', scale: 'Human-scale pavilion nested beneath towering forest growth',
    gameplayRole: 'Traversable sanctuary relay that guides travelers through dense woodland', exclusions: ['neon circuitry', 'spaceships', 'chrome machines', 'industrial floodlights'],
    visualChecks: ['roof silhouette reads as a grove pavilion', 'slender supports leave the central path open', 'timber and stone feel grounded in the forest'],
    assumptions: ['The requested setting is interpreted as a forest sanctuary.', 'Amber light is used as the default navigation signal.'],
    renderingNotes: ['Use warm accents sparingly against mossy surfaces.', 'Avoid sharp metallic highlights.', 'Micro moss and texture are contract metadata, not separately rendered detail.'],
    referencePreset: 'sylvan', palette: { shell: '#b4bd92', frame: '#615444', accent: '#ffc577', ground: '#3f614e', rock: '#788475' },
    sky: '#b9cdb1', flora: 'tree', architecture: 'organic',
  },
  ember: {
    id: 'ember', name: 'Ember reach', genre: 'Volcanic frontier civilization',
    description: 'A harsh volcanic frontier with basalt shelters, dark metal braces, oxidized copper, and restrained amber light. Heavy geometric silhouettes signal resilient infrastructure.',
    biome: 'Volcanic ash plain with exposed basalt and sparse heat-tolerant growth', shapeLanguage: 'Low fortified walls, paired vent stacks, and compact buttresses',
    materialFinish: 'Rough dark basalt, soot-dark metal, and muted oxidized copper', weathering: 'Soot, heat cracking, and fine ash accumulation around wall bases',
    technology: 'Heat-tolerant sensor and shelter infrastructure for a hostile frontier', motifs: ['basalt ramparts', 'twin vent stacks', 'copper signal bands'],
    lighting: 'Smoky warm sky light with restrained amber signal glows', scale: 'Compact human-scale shelter with heavy protective massing',
    gameplayRole: 'Traversable fortified sensor station that protects a route across volcanic terrain', exclusions: ['lush grass', 'ice castles', 'candy colors', 'delicate forest pavilions'],
    visualChecks: ['low basalt walls define a fortified silhouette', 'twin vent stacks rise above the shelter', 'the center approach remains open and readable'],
    assumptions: ['The site is a volcanic frontier station.', 'The outpost prioritizes shelter and durability.'],
    renderingNotes: ['Keep copper and amber accents restrained.', 'Make ash and basalt distinct without relying on glow effects.', 'Micro soot and texture are contract metadata, not separately rendered detail.'],
    referencePreset: 'ember', palette: { shell: '#9c8872', frame: '#383b40', accent: '#ffad6b', ground: '#55504b', rock: '#76706b' },
    sky: '#c6aba0', flora: 'basalt', architecture: 'fortified',
  },
};

export const THEMES = Object.freeze(Object.fromEntries(Object.entries(presets).map(([id, value]) => [id, validateTheme(value)])));

export function draftThemeFromPrompt(prompt, selectedPresetId = 'argonaut') {
  if (typeof prompt !== 'string' || prompt.trim().length < 1 || prompt.length > 3000) throw new Error('Prompt must be 1–3000 characters.');
  const lower = prompt.toLowerCase();
  const sylvanHits = ['forest', 'moss', 'wood', 'grove', 'tree', 'sanctuary'].filter(word => lower.includes(word));
  const emberHits = ['volcano', 'basalt', 'lava', 'ash', 'ember'].filter(word => lower.includes(word));
  const referencePreset = sylvanHits.length && sylvanHits.length >= emberHits.length ? 'sylvan' : emberHits.length ? 'ember' : PRESETS.includes(selectedPresetId) ? selectedPresetId : 'argonaut';
  const base = THEMES[referencePreset];
  const selection = sylvanHits.length && referencePreset === 'sylvan' ? `forest terms (${sylvanHits.join(', ')})` : emberHits.length && referencePreset === 'ember' ? `volcanic terms (${emberHits.join(', ')})` : `the selected/default preset (${referencePreset})`;
  return validateTheme({ ...structuredClone(base), id: 'custom-draft', name: 'Local theme draft',
    assumptions: [`Local keyword-based preset draft selected ${referencePreset} from ${selection}.`, 'This local draft does not claim to interpret or satisfy every part of the prompt.', 'Preset details and palette use the selected reference as defaults.'],
  });
}
