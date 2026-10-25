export const THEMES = {
  argonaut: {
    id: 'argonaut', name: 'Argonaut frontier', genre: 'Alien machine civilization',
    description: 'A weathered machine civilization on a luminous alien frontier. Ceramic shells, graphite frames, mineral terrain, cyan energy. Functional industrial silhouettes; no medieval castles, human ornament, or cartoon props.',
    palette: { shell: '#d9ded0', frame: '#34464a', accent: '#80edc4', ground: '#53685d', rock: '#81958a' },
    sky: '#b6d2cf', flora: 'crystal', architecture: 'industrial'
  },
  sylvan: {
    id: 'sylvan', name: 'Verdant sanctuary', genre: 'Ancient forest civilization',
    description: 'An ancient forest world with mossy stone sanctuaries, timber supports, warm amber light, and towering living groves. Organic, grounded silhouettes; no neon circuitry, spaceships, or chrome machines.',
    palette: { shell: '#b4bd92', frame: '#615444', accent: '#ffc577', ground: '#3f614e', rock: '#788475' },
    sky: '#b9cdb1', flora: 'tree', architecture: 'organic'
  },
  ember: {
    id: 'ember', name: 'Ember reach', genre: 'Volcanic frontier civilization',
    description: 'A harsh volcanic frontier with basalt shelters, dark metal braces, oxidized copper, and restrained amber light. Heavy geometric silhouettes and resilient infrastructure; no lush grass, ice castles, or candy colors.',
    palette: { shell: '#9c8872', frame: '#383b40', accent: '#ffad6b', ground: '#55504b', rock: '#76706b' },
    sky: '#c6aba0', flora: 'basalt', architecture: 'industrial'
  }
};

export function fixtureRecipe(themeId = 'argonaut') {
  const theme = THEMES[themeId];
  const part = (id, shape, role, position, size, material) => ({id, shape, role, position, size, material});
  return {
    themeId, name: themeId === 'argonaut' ? 'Aster Relay' : themeId === 'sylvan' ? 'Grove Watch' : 'Cinder Station',
    description: `A traversable frontier sensor outpost in ${theme.name}.`, sensorRadius: 12,
    parts: [
      part('rear', 'box', 'wall', [0, 1.6, -2.5], [6, 3.2, .45], 'shell'),
      part('west', 'box', 'wall', [-2.8, 1.6, 0], [.45, 3.2, 5], 'shell'),
      part('east', 'box', 'wall', [2.8, 1.6, 0], [.45, 3.2, 5], 'shell'),
      part('entry-left', 'box', 'support', [-2, 1.6, 2.5], [1.5, 3.2, .5], 'frame'),
      part('entry-right', 'box', 'support', [2, 1.6, 2.5], [1.5, 3.2, .5], 'frame'),
      part('canopy', 'box', 'roof', [0, 3.5, 0], [6.5, .65, 5.8], 'shell'),
      part('sensor-mast', 'cylinder', 'detail', [0, 4.6, -1], [.2, 1.6, .2], 'frame'),
      part('sensor', 'box', 'detail', [0, 5.6, -1], [1.2, .5, .8], 'accent'),
      part('trim', 'box', 'detail', [0, 3.45, 2.94], [5.8, .12, .08], 'accent'),
      part('terminal', 'box', 'detail', [1.8, .7, -1.6], [.5, 1.4, .6], 'frame')
    ]
  };
}

export function validateRecipe(r, themeId) {
  if (!r || typeof r !== 'object' || r.themeId !== themeId || !THEMES[themeId]) throw new Error('Candidate changed the frozen theme.');
  if (typeof r.name !== 'string' || !r.name.trim() || r.name.length > 100 || typeof r.description !== 'string' || r.description.length > 2000) throw new Error('Invalid candidate identity.');
  if (!Number.isFinite(r.sensorRadius) || r.sensorRadius < 1 || r.sensorRadius > 20) throw new Error('Sensor radius must be 1–20 metres.');
  if (!Array.isArray(r.parts) || r.parts.length < 4 || r.parts.length > 48) throw new Error('Provide 4–48 structural parts.');
  const ids = new Set();
  for (const p of r.parts) {
    if (typeof p.id !== 'string' || p.id.length > 80 || ids.has(p.id)) throw new Error('Part IDs must be unique.');
    ids.add(p.id);
    if (!['box', 'cylinder', 'cone'].includes(p.shape) || !['wall','support','roof','detail'].includes(p.role) || !['shell','frame','accent'].includes(p.material)) throw new Error('Unsupported geometry or theme material.');
    if (!Array.isArray(p.position) || p.position.length !== 3 || p.position.some(v => !Number.isFinite(v) || Math.abs(v) > 10)) throw new Error('Part outside the build envelope.');
    if (!Array.isArray(p.size) || p.size.length !== 3 || p.size.some(v => !Number.isFinite(v) || v <= 0 || v > 12)) throw new Error('Invalid part dimensions.');
    if (p.position[1] - p.size[1] / 2 < -.05) throw new Error('Geometry intersects the ground.');
  }
  return r;
}

export function collides(recipe, x, z) {
  return recipe.parts.some(p => {
    const bottom = p.position[1] - p.size[1] / 2;
    const top = p.position[1] + p.size[1] / 2;
    return bottom < 1.5 && top > .08 && Math.abs(x-p.position[0]) < p.size[0]/2+.28 && Math.abs(z-p.position[2]) < p.size[2]/2+.28;
  });
}

// Shared with the real client: collision-aware movement, with no audit teleport.
export function moveCommander(recipe, current, target, dt = 1/60) {
  const dx = target.x-current.x, dz = target.z-current.z, distance = Math.hypot(dx,dz);
  if (distance < .06) return {...current};
  const step = Math.min(distance, 4*Math.max(0,Math.min(dt,.05))), x = current.x+dx/distance*step, z = current.z+dz/distance*step;
  if (!collides(recipe,x,z)) return {x,z};
  return {...current};
}

export function accessScenario(recipe) {
  let position = {x:0,z:9}; const trace = [{...position}];
  for (let i=0;i<300;i++) {
    position = moveCommander(recipe,position,{x:0,z:0});
    if (i%15===0) trace.push({...position});
  }
  return {pass:Math.hypot(position.x,position.z)<.3, position, trace};
}

export function deterministicChecks(recipe, themeId) {
  validateRecipe(recipe,themeId);
  const route = accessScenario(recipe);
  const structural = recipe.parts.filter(p=>['wall','support'].includes(p.role));
  return [
    {id:'THEME-01',status:'pass',label:'Frozen theme & material palette',detail:`${THEMES[themeId].name}; all materials resolve through the theme palette.`},
    {id:'GEN-05.access',status:route.pass?'pass':'fail',label:'Commander can enter',detail:route.pass?'Normal movement reaches the interior.':`Movement blocked at z=${route.position.z.toFixed(2)}m. Clear the front-to-centre route for a 0.56m-wide commander.`,trace:route.trace},
    {id:'GEN-06.support',status:structural.length && structural.every(p=>Math.abs(p.position[1]-p.size[1]/2)<.08)?'pass':'fail',label:'Structure meets the ground',detail:'All support and wall bases must be within 8cm of ground.'},
    {id:'GEN-05.sensor',status:recipe.sensorRadius>=8?'pass':'fail',label:'Sensor detects the test target',detail:`A target 8m from the relay ${recipe.sensorRadius>=8?'is':'is not'} within the ${recipe.sensorRadius}m sensor radius.`}
  ];
}

export function injectDefect(recipe) {
  const copy=structuredClone(recipe);
  copy.parts.push({id:'audit-obstruction',shape:'box',role:'wall',position:[0,1,2.5],size:[2.1,2,.6],material:'frame'});
  return copy;
}

export const recipeSchema = {
  type:'object', required:['themeId','name','description','sensorRadius','parts'],
  properties:{themeId:{type:'string'},name:{type:'string'},description:{type:'string'},sensorRadius:{type:'number'},parts:{type:'array',items:{type:'object',required:['id','shape','role','position','size','material'],properties:{id:{type:'string'},shape:{type:'string',enum:['box','cylinder','cone']},role:{type:'string',enum:['wall','support','roof','detail']},position:{type:'array',items:{type:'number'},minItems:3,maxItems:3},size:{type:'array',items:{type:'number'},minItems:3,maxItems:3},material:{type:'string',enum:['shell','frame','accent']}}}}}
};

export function eligible(job) {
  return !job.cancelled && Date.now()<job.deadline && job.attempts.length<3 && job.calls<6;
}

export function canAccept(checks, imageCount) {
  const required=['THEME-01','GEN-05.access','GEN-06.support','GEN-05.sensor','THEME-02','LIVE-04'];
  return imageCount>=3 && required.every(id=>checks.filter(c=>c.id===id).length===1 && checks.find(c=>c.id===id).status==='pass');
}
