import { THEMES, validateTheme } from './themes.mjs';
export { THEMES, themeSchema, validateTheme, draftThemeFromPrompt } from './themes.mjs';

export function fixtureRecipe(themeOrId = 'argonaut') {
  const theme = typeof themeOrId === 'string' ? THEMES[themeOrId] : validateTheme(themeOrId);
  if (!theme) throw new Error('Unknown theme preset.');
  const { id: themeId, referencePreset } = theme;
  const part = (id, shape, role, position, size, material) => ({id, shape, role, position, size, material});
  const common = { themeId, description: `A traversable sensor outpost in ${theme.name}.`, sensorRadius: 12 };
  let name, parts;
  if (referencePreset === 'sylvan') {
    name = 'Grove Watch';
    parts = [
      part('grove-post-nw', 'cylinder', 'support', [-2.75, 1.55, -2.25], [.32, 3.1, .32], 'frame'),
      part('grove-post-ne', 'cylinder', 'support', [2.75, 1.55, -2.25], [.32, 3.1, .32], 'frame'),
      part('grove-post-sw', 'cylinder', 'support', [-2.75, 1.55, 2.25], [.32, 3.1, .32], 'frame'),
      part('grove-post-se', 'cylinder', 'support', [2.75, 1.55, 2.25], [.32, 3.1, .32], 'frame'),
      part('pavilion-canopy', 'cone', 'roof', [0, 4.15, 0], [6.4, 2.1, 5.5], 'shell'),
      part('ridge-spire', 'cone', 'detail', [0, 5.35, -.15], [1.15, 1.2, 1.15], 'frame'),
      part('lantern-west', 'cylinder', 'detail', [-1.4, 2.85, .15], [.28, .5, .28], 'accent'),
      part('lantern-east', 'cylinder', 'detail', [1.4, 2.85, .15], [.28, .5, .28], 'accent'),
      part('root-stone-west', 'box', 'detail', [-2.2, .22, 1.5], [1.2, .44, .55], 'shell'),
      part('root-stone-east', 'box', 'detail', [2.2, .22, 1.5], [1.2, .44, .55], 'shell'),
      part('wayfinding-mark', 'box', 'detail', [0, 1.15, -1.7], [.12, .5, .12], 'accent'),
    ];
  } else if (referencePreset === 'ember') {
    name = 'Cinder Station';
    parts = [
      part('rampart-west', 'box', 'wall', [-3.35, 1.35, 0], [.65, 2.7, 5.5], 'shell'),
      part('rampart-east', 'box', 'wall', [3.35, 1.35, 0], [.65, 2.7, 5.5], 'shell'),
      part('rampart-rear', 'box', 'wall', [0, 1.35, -2.85], [6.7, 2.7, .5], 'shell'),
      part('entry-bastion-west', 'box', 'wall', [-2.55, 1.05, 2.7], [1.6, 2.1, .55], 'frame'),
      part('entry-bastion-east', 'box', 'wall', [2.55, 1.05, 2.7], [1.6, 2.1, .55], 'frame'),
      part('fortified-roof', 'box', 'roof', [0, 3.05, -.1], [6.8, .5, 5.7], 'shell'),
      part('buttress-west', 'box', 'support', [-2.95, 1.5, -2.3], [.8, 3, .9], 'frame'),
      part('buttress-east', 'box', 'support', [2.95, 1.5, -2.3], [.8, 3, .9], 'frame'),
      part('vent-west', 'cylinder', 'detail', [-1.55, 4, -2.4], [.62, 1.9, .62], 'frame'),
      part('vent-east', 'cylinder', 'detail', [1.55, 4, -2.4], [.62, 1.9, .62], 'frame'),
      part('vent-cap-west', 'cone', 'detail', [-1.55, 5.02, -2.4], [.86, .3, .86], 'accent'),
      part('vent-cap-east', 'cone', 'detail', [1.55, 5.02, -2.4], [.86, .3, .86], 'accent'),
      part('signal-band-west', 'box', 'detail', [-3.35, 1.85, .2], [.68, .18, .45], 'accent'),
      part('signal-band-east', 'box', 'detail', [3.35, 1.85, .2], [.68, .18, .45], 'accent'),
    ];
  } else {
    name = 'Aster Relay';
    parts = [
      part('service-wall-rear', 'box', 'wall', [0, 1.45, -2.8], [6.2, 2.9, .5], 'shell'),
      part('service-wall-west', 'box', 'wall', [-3.05, 1.4, -.1], [.55, 2.8, 5.4], 'shell'),
      part('service-wall-east', 'box', 'wall', [3.05, 1.4, -.1], [.55, 2.8, 5.4], 'shell'),
      part('cantilever-support-west', 'box', 'support', [-2.45, 1.55, 2], [.32, 3.1, .32], 'frame'),
      part('cantilever-support-east', 'box', 'support', [2.45, 1.55, 2], [.32, 3.1, .32], 'frame'),
      part('cantilever-canopy', 'box', 'roof', [.25, 3.35, -.05], [6.8, .5, 5.8], 'shell'),
      part('relay-fin-west', 'box', 'detail', [-1.55, 4.45, -2.45], [.42, 2.1, .3], 'frame'),
      part('relay-fin-east', 'box', 'detail', [1.55, 4.45, -2.45], [.42, 2.1, .3], 'frame'),
      part('sensor-mast', 'cylinder', 'detail', [0, 4.65, -1], [.24, 2.1, .24], 'frame'),
      part('sensor-head', 'box', 'detail', [0, 5.8, -1], [1.05, .45, .75], 'accent'),
      part('signal-strip', 'box', 'detail', [0, 3.35, 2.88], [4.4, .12, .08], 'accent'),
      part('console', 'box', 'detail', [2, .7, -1.6], [.55, 1.4, .55], 'frame'),
    ];
  }
  return { ...common, name, parts };
}

function resolveTheme(themeOrId) {
  if (typeof themeOrId === 'string') {
    const theme = THEMES[themeOrId];
    if (!theme) throw new Error('Unknown theme preset.');
    return theme;
  }
  return validateTheme(themeOrId);
}

export function validateRecipe(r, themeOrId) {
  let theme;
  try { theme = resolveTheme(themeOrId); } catch { throw new Error('Candidate changed the frozen theme.'); }
  if (!r || typeof r !== 'object' || r.themeId !== theme.id) throw new Error('Candidate changed the frozen theme.');
  if (typeof r.name !== 'string' || !r.name.trim() || r.name.length > 100 || typeof r.description !== 'string' || r.description.length > 2000) throw new Error('Invalid candidate identity.');
  if (!Number.isFinite(r.sensorRadius) || r.sensorRadius < 1 || r.sensorRadius > 20) throw new Error('Sensor radius must be 1–20 metres.');
  if (!Array.isArray(r.parts) || r.parts.length < 4 || r.parts.length > 48) throw new Error('Provide 4–48 structural parts.');
  const ids = new Set();
  for (const p of r.parts) {
    if (typeof p.id !== 'string' || p.id.length > 80 || ids.has(p.id)) throw new Error('Part IDs must be unique.');
    ids.add(p.id);
    if (!['box', 'cylinder', 'cone'].includes(p.shape) || !['wall','support','roof','detail'].includes(p.role) || !['shell','frame','accent'].includes(p.material) || !(p.material in theme.palette)) throw new Error('Unsupported geometry or theme material.');
    if (!Array.isArray(p.position) || p.position.length !== 3 || p.position.some(v => !Number.isFinite(v) || Math.abs(v) > 10)) throw new Error('Part outside the build envelope.');
    if (!Array.isArray(p.size) || p.size.length !== 3 || p.size.some(v => !Number.isFinite(v) || v <= 0 || v > 12)) throw new Error('Invalid part dimensions.');
    if (Math.abs(p.position[0]) + p.size[0] / 2 > 4.05 || Math.abs(p.position[2]) + p.size[2] / 2 > 4.05) throw new Error('Building must stay within x/z ±4m.');
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
  const theme = resolveTheme(themeId);
  validateRecipe(recipe,theme);
  const route = accessScenario(recipe);
  const structural = recipe.parts.filter(p=>['wall','support'].includes(p.role));
  return [
    {id:'THEME-01',status:'pass',label:'Frozen theme & material palette',detail:`${theme.name}; all materials resolve through the theme palette.`},
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
