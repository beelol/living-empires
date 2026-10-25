import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fixtureRecipe,injectDefect,deterministicChecks,accessScenario,validateRecipe,eligible,canAccept} from '../public/core.mjs';

test('obstruction is caught and clearing it restores real collision-aware access',()=>{
  const recipe=fixtureRecipe();assert.equal(accessScenario(recipe).pass,true);
  assert.equal(accessScenario(injectDefect(recipe)).pass,false);
  const repaired=injectDefect(recipe);repaired.parts=repaired.parts.filter(p=>p.id!=='audit-obstruction');
  assert.equal(accessScenario(repaired).pass,true);
});
test('generation cannot change the frozen theme or add arbitrary colors',()=>{
  assert.throws(()=>validateRecipe(fixtureRecipe('sylvan'),'argonaut'),/theme/);
  const r=fixtureRecipe();r.parts[0].material='#ff00ff';assert.throws(()=>validateRecipe(r,'argonaut'),/material/);
});
test('missing evidence or a blocked visual assessment prevents acceptance',()=>{
  const checks=[...deterministicChecks(fixtureRecipe(),'argonaut'),{id:'THEME-02',status:'pass'},{id:'LIVE-04',status:'pass'}];
  assert.equal(canAccept(checks,3),true);assert.equal(canAccept(checks,2),false);
  checks[4].status='blocked';assert.equal(canAccept(checks,3),false);
  checks[4].status='pass';checks.push({id:'THEME-02',status:'pass'});assert.equal(canAccept(checks,3),false);
});
test('cancellation, retries, calls and deadlines cannot bypass bounds',()=>{
  const job={cancelled:false,deadline:Date.now()+10000,attempts:[],calls:0};
  assert.equal(eligible(job),true);assert.equal(eligible({...job,cancelled:true}),false);
  assert.equal(eligible({...job,attempts:[1,2,3]}),false);assert.equal(eligible({...job,calls:6}),false);assert.equal(eligible({...job,deadline:0}),false);
});
test('first-party Rust core and browser share movement outcomes for every theme',()=>{
  for(const themeId of ['argonaut','sylvan','ember'])for(const recipe of [fixtureRecipe(themeId),injectDefect(fixtureRecipe(themeId))]){
    const result=spawnSync(process.env.RUST_CORE_BIN||'target/debug/world-check',[],{input:JSON.stringify({recipe,themeId}),encoding:'utf8'});
    assert.equal(result.status,0,result.stderr||String(result.error));
    const output=JSON.parse(result.stdout);assert.equal(output.movement.pass,accessScenario(recipe).pass);
    assert.deepEqual(output.checks.map(c=>[c.id,c.status]),deterministicChecks(recipe,themeId).map(c=>[c.id,c.status]));
  }
});

test('Rust acceptance gates reject theme drift, floating supports, and short sensors',()=>{
  const run=payload=>spawnSync(process.env.RUST_CORE_BIN||'target/debug/world-check',[],{input:JSON.stringify(payload),encoding:'utf8'});
  const recipe=fixtureRecipe();
  const drift=run({recipe:{...recipe,themeId:'sylvan'},themeId:'argonaut'});
  assert.notEqual(drift.status,0);
  assert.match(JSON.parse(drift.stdout).error,/theme/i);

  const floating=structuredClone(recipe);floating.parts[0].position[1]+=.5;
  const floatingResult=run({recipe:floating,themeId:'argonaut'});
  assert.equal(floatingResult.status,0,floatingResult.stderr);
  assert.equal(JSON.parse(floatingResult.stdout).checks.find(c=>c.id==='GEN-06.support').status,'fail');

  const shortSensor={...recipe,sensorRadius:2};
  const sensorResult=run({recipe:shortSensor,themeId:'argonaut'});
  assert.equal(sensorResult.status,0,sensorResult.stderr);
  assert.equal(JSON.parse(sensorResult.stdout).checks.find(c=>c.id==='GEN-05.sensor').status,'fail');
});

test('Rust validates full editable theme contracts without restoring preset exclusions',()=>{
  const theme={
    id:'rusted-orchard',name:'Rusted Orchard',genre:'Agricultural machine frontier',
    description:'A custom orchard civilization among rusted irrigation towers.',biome:'Red grassland',
    shapeLanguage:'Tall arches and broad low platforms',materialFinish:'Oxidized iron and glazed ceramic',
    weathering:'Wind-scoured surfaces with localized rust',technology:'Solar irrigation and seed vaults',
    motifs:['Seed capsules','Irrigation rings'],lighting:'Late afternoon amber light',scale:'Human-scale shelters and monumental towers',
    gameplayRole:'A safe, traversable farming outpost',exclusions:['No military insignia','Avoid dense visual noise'],
    visualChecks:['Entry remains visible','Materials stay distinct'],assumptions:[],
    renderingNotes:['Use warm side light'],referencePreset:'argonaut',
    palette:{shell:'#D9DED0',frame:'#34464A',accent:'#80EDC4',ground:'#53685D',rock:'#81958A'},
    sky:'#B6D2CF',flora:'crystal',architecture:'fortified'
  };
  const recipe=fixtureRecipe();recipe.themeId=theme.id;
  const run=payload=>spawnSync(process.env.RUST_CORE_BIN||'target/debug/world-check',[],{input:JSON.stringify(payload),encoding:'utf8'});
  const valid=run({recipe,themeId:theme.id,theme});
  assert.equal(valid.status,0,valid.stderr);
  const output=JSON.parse(valid.stdout);
  assert.deepEqual(output.theme.exclusions,theme.exclusions);
  assert.equal(output.theme.id,theme.id);

  const noContract=run({recipe,themeId:theme.id});
  assert.notEqual(noContract.status,0);
  assert.match(JSON.parse(noContract.stdout).error,/theme/i);

  const {biome,...missingField}=theme;
  const missing=run({recipe,themeId:theme.id,theme:missingField});
  assert.notEqual(missing.status,0);
  assert.match(JSON.parse(missing.stdout).error,/biome/i);

  const invalidPalette={...theme,palette:{...theme.palette,shell:'rgb(217,222,208)'}};
  const invalid=run({recipe,themeId:theme.id,theme:invalidPalette});
  assert.notEqual(invalid.status,0);
  assert.match(JSON.parse(invalid.stdout).error,/RRGGBB/i);

  const mismatch=run({recipe,themeId:'other-world',theme});
  assert.notEqual(mismatch.status,0);
  assert.match(JSON.parse(mismatch.stdout).error,/themeId/i);
  const recipeMismatch=run({recipe:{...recipe,themeId:'other-world'},themeId:theme.id,theme});
  assert.notEqual(recipeMismatch.status,0);
  assert.match(JSON.parse(recipeMismatch.stdout).error,/theme id/i);
});
