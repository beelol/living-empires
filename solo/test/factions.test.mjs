import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createWorld,command,creationRequest,assetForBuilding,validateWorld,publicState} from '../model.mjs';
import {openStore} from '../store.mjs';

const create=(w,id='orchid')=>command(w,{id:'create-'+id,type:'faction-create',factionId:id,name:id,artDirection:'Layered orchid petal roofs with copper ribs and purple glass.'});
const build=(w,id,x,factionId='landing')=>command(w,{id,type:'build',kind:'solar',x,z:8,factionId});
const assign=(w,id,factionId,buildingIds)=>command(w,{id,type:'faction-assign',factionId,buildingIds});

test('v1 migration pins inherited appearances before removing global fallback',()=>{
 const w=createWorld();build(w,'solar-a',12);build(w,'solar-b',24);
 w.schemaVersion=1;delete w.factions;for(const b of w.buildings){delete b.factionId;delete b.factionAssignment;}
 w.assets.solar={hash:'legacy-family',recipe:{name:'old solar'}};w.buildings[2].asset={hash:'individual'};
 w.assetHistory.push({kind:'solar',hash:'legacy-family',previous:null});w.foundry.requests.push({id:'old-request',kind:'solar',status:'blocked',reason:'Retain failed evidence'});
 const history=structuredClone(w.assetHistory),receipts=structuredClone(w.receipts);validateWorld(w);
 assert.equal(w.schemaVersion,2);assert.equal(w.buildings[1].asset.hash,'legacy-family');assert.equal(w.buildings[2].asset.hash,'individual');
 assert.equal(w.buildings[1].factionId,'landing');assert.equal(w.buildings[1].factionAssignment,1);
 w.assets.solar.hash='unreviewed-global';assert.equal(assetForBuilding(w,w.buildings[1]).hash,'legacy-family');assert.equal(assetForBuilding(w,{kind:'solar'}),undefined);
 assert.deepEqual(w.assetHistory,history);assert.deepEqual(w.receipts,receipts);assert.equal(w.foundry.requests[0].reason,'Retain failed evidence');
});

test('faction updates target selected buildings and keep unrelated assets, economy and history intact',()=>{
 const w=createWorld();build(w,'solar-a',12);build(w,'solar-b',24);create(w);create(w,'cobalt');
 for(const b of w.buildings)b.asset={hash:'retained-'+b.id};
 assign(w,'assign-a','orchid',['solar-a']);assign(w,'assign-b','cobalt',['solar-b']);
 const previousA=w.foundry.requests[0],previousB=w.foundry.requests[1],beforeBuildings=structuredClone(w.buildings),alloy=w.alloy;
 previousA.status='running';previousA.jobId='job-running-a';previousB.status='running';
 const input={id:'style-a',type:'faction-style',factionId:'orchid',artDirection:'Sculpted orchid canopies with copper ribs, petal inlays and slender faceted legs.'};
 const result=command(w,input),request=w.foundry.requests.at(-1);
 assert.equal(previousA.status,'cancelled');assert.match(previousA.reason,/Superseded/);assert.equal(previousA.jobId,'job-running-a');assert.equal(previousB.status,'running');
 assert.deepEqual(request.targets,[{id:'solar-a',assignment:2}]);assert.equal(request.factionRevision,2);assert.equal(request.explicit,true);assert.match(request.prompt,/orchid canopies/);assert.ok(request.prompt.length<=3000);
 assert.equal(w.factions.find(f=>f.id==='orchid').theme,null);assert.equal(w.factions.find(f=>f.id==='cobalt').revision,1);
 assert.deepEqual(w.buildings,beforeBuildings);assert.equal(w.alloy,alloy);assert.equal(w.foundry.enabled,false);
 assert.deepEqual(command(w,input),result);assert.equal(w.foundry.requests.length,3);assert.equal(w.factions.find(f=>f.id==='orchid').revision,2);
});

test('reassignment increments membership, cancels old target work and invalidates a stale regeneration',()=>{
 const w=createWorld();create(w);create(w,'cobalt');
 command(w,{id:'custom',type:'build',kind:'creation',x:12,z:8,factionId:'orchid',prompt:'A celestial observatory held by three layered petals',provider:'fixture'});
 const initial=w.foundry.requests[0],building=w.buildings.at(-1);assert.equal(initial.explicit,true);
 assign(w,'move-custom','cobalt',['custom']);assert.equal(initial.status,'cancelled');assert.equal(building.factionAssignment,2);
 assert.throws(()=>creationRequest({id:'stale',targetId:'custom',factionId:'orchid',factionAssignment:1,prompt:building.prompt,provider:'fixture'},w),/assignment changed/);
 const current=creationRequest({id:'fresh',targetId:'custom',factionId:'cobalt',factionAssignment:2,prompt:building.prompt,provider:'fixture'},w);
 assert.deepEqual(current.targets,[{id:'custom',assignment:2}]);assert.equal(current.targetId,'custom');assert.equal(current.userPrompt,building.prompt);
 assign(w,'apply-again','cobalt',['custom']);assert.equal(building.factionAssignment,2);assert.equal(w.foundry.requests[1].status,'cancelled');
});

test('invalid selections and reused operation IDs leave the complete world unchanged',()=>{
 const w=createWorld();build(w,'solar-a',12);create(w);const before=structuredClone(w);
 for(const buildingIds of [[],['solar-a','solar-a'],['solar-a','missing']])assert.throws(()=>assign(w,'invalid','orchid',buildingIds));
 assert.throws(()=>command(w,{id:'invalid-style',type:'faction-style',factionId:'orchid',artDirection:'bad'}));assert.deepEqual(w,before);
 const input={id:'assign',type:'faction-assign',factionId:'orchid',buildingIds:['solar-a']};command(w,input);const after=structuredClone(w);command(w,input);assert.deepEqual(w,after);
 assert.throws(()=>command(w,{...input,factionId:'landing'}),/reused/);assert.deepEqual(w,after);
});

test('new construction inherits its requested faction without a global appearance grant',()=>{
 const w=createWorld();create(w);w.assets.solar={hash:'legacy-global'};build(w,'solar-a',12,'orchid');
 const building=w.buildings.at(-1);assert.equal(building.factionId,'orchid');assert.equal(building.factionAssignment,1);assert.equal(assetForBuilding(w,building),undefined);assert.equal(w.alloy,165);
});

test('faction membership, current prompt, pinned asset and failed history survive durable reopen',()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'faction-save-'));let store;
 try{
  store=openStore(dir);const w=store.world;create(w);build(w,'solar-a',12);assign(w,'assign-a','orchid',['solar-a']);
  w.buildings.at(-1).asset={hash:'last-approved'};w.foundry.requests[0].status='blocked';w.foundry.requests[0].reason='Independent review rejected floating supports';
  store.save();const expected=structuredClone(w);store.close();store=openStore(dir);assert.deepEqual(store.world,expected);assert.equal(store.world.schemaVersion,2);
 }finally{store?.close();fs.rmSync(dir,{recursive:true,force:true});}
});

test('damaged membership is rejected without partially migrating a v1 save',()=>{
 const w=createWorld();w.schemaVersion=1;delete w.factions;w.buildings.push({...w.buildings[0]});const before=structuredClone(w);
 assert.throws(()=>validateWorld(w),/Damaged/);assert.deepEqual(w,before);
 const v2=createWorld();v2.buildings[0].factionId='missing';assert.throws(()=>validateWorld(v2),/Damaged/);
});


test('public history exposes rollback evidence and scoped targets without prior asset payloads',()=>{
 const w=createWorld();
 w.assetHistory.push({kind:'core',factionId:'landing',hash:'current-hash',jobId:'job-current',rehearsal:false,at:10,rolledBackAt:20,rollbackTargets:['landing-core'],previousByTarget:[{id:'landing-core',assignment:1,asset:{hash:'old-hash',recipe:{parts:['large historical recipe']}}}]});
 const entry=publicState(w).assetHistory[0];
 assert.equal(entry.jobId,'job-current');assert.equal(entry.rolledBackAt,20);assert.deepEqual(entry.rollbackTargets,['landing-core']);
 assert.deepEqual(entry.previousByTarget,[{id:'landing-core',assignment:1}]);assert.equal(JSON.stringify(entry).includes('large historical recipe'),false);
 assert.equal(w.assetHistory[0].previousByTarget[0].asset.hash,'old-hash');
});
