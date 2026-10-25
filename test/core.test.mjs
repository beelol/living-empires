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
