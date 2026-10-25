import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createWorld,command,creationRequest,assetForBuilding,validateWorld,publicState,factionAt,adoptFactionDirection} from '../model.mjs';
import {THEMES} from '../../public/themes.mjs';
import {openStore} from '../store.mjs';

const create=(w,id='orchid')=>command(w,{id:'create-'+id,type:'faction-create',factionId:id,name:id,artDirection:'Layered orchid petal roofs with copper ribs and purple glass.'});
const build=(w,id,x,factionId)=>command(w,{id,type:'build',kind:'solar',x,z:8,...(factionId===undefined?{}:{factionId})});
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
 const building=w.buildings.at(-1);assert.equal(building.factionId,'orchid');assert.equal(building.factionAssignment,1);assert.equal(assetForBuilding(w,building),undefined);assert.equal(w.alloy,65);
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


const saveArea=(w,id,factionId,area,extra={})=>command(w,{id,type:'faction-assign',factionId,area,reuseDesign:true,...extra});
const eastArea={minX:6,maxX:30,minZ:0,maxZ:16};

test('saved area authoritatively assigns inhabitants and latest overlapping area wins',()=>{
 const w=createWorld();build(w,'solar-a',12);build(w,'solar-b',24);create(w);create(w,'cobalt');
 saveArea(w,'east','orchid',eastArea,{buildingIds:['solar-b','solar-a']});
 assert.equal(factionAt(w,12,8).id,'orchid');assert.equal(factionAt(w,0,8),null);assert.equal(w.buildings[1].factionId,'orchid');
 assert.equal(w.foundry.requests.at(-1).preferExistingDesign,true);
 saveArea(w,'overlap','cobalt',{minX:20,maxX:40,minZ:0,maxZ:16});
 assert.equal(factionAt(w,24,8).id,'cobalt');assert.equal(factionAt(w,12,8).id,'orchid');assert.equal(w.buildings[2].factionId,'cobalt');
 assert.ok(w.factions.find(f=>f.id==='cobalt').area.order>w.factions.find(f=>f.id==='orchid').area.order);
 const before=structuredClone(w);assert.throws(()=>saveArea(w,'wrong','orchid',eastArea,{buildingIds:['solar-a']}),/changed/);assert.deepEqual(w,before);
});

test('new area building queues a free design while existing in-flight work keeps its frozen targets',()=>{
 const w=createWorld();create(w);saveArea(w,'east','orchid',eastArea);const result=build(w,'solar-a',12,'landing');
 assert.equal(result.factionId,'orchid');const first=w.foundry.requests[0];first.status='running';first.jobId='active-job';const snapshot=structuredClone(first);
 const next=build(w,'solar-b',24,'landing');assert.equal(next.factionId,'orchid');assert.deepEqual(first,snapshot);assert.equal(w.foundry.requests.length,2);
 assert.equal(w.foundry.requests[1].preferExistingDesign,undefined);assert.match(w.foundry.requests[1].prompt,/artistic direction freely/);assert.equal(w.foundry.requests[1].drivesFactionDirection,true);assert.equal(w.foundry.requests[1].artDirection,w.foundry.requests[1].userPrompt);assert.equal(w.foundry.requests[1].prompt.includes(w.factions.find(f=>f.id==='orchid').artDirection),false);assert.equal(w.factions.find(f=>f.id==='orchid').designIntentId,w.foundry.requests[1].id);assert.match(w.foundry.requests[1].userPrompt,/Solar array/);assert.ok(w.foundry.requests[1].prompt.length<=3000);assert.deepEqual(w.foundry.requests[1].targets,[{id:'solar-b',assignment:1}]);assert.equal(w.alloy,30);
 build(w,'solar-b',24,'landing');assert.equal(w.foundry.requests.length,2);assert.equal(w.alloy,30);
 saveArea(w,'east-again','orchid',eastArea);assert.equal(first.status,'running');assert.deepEqual(first.targets,[{id:'solar-a',assignment:1}]);
 command(w,{id:'new-style',type:'faction-style',factionId:'orchid',artDirection:'Tall folded brass blades with slate glass seams.'});
 const latest=w.foundry.requests.at(-1);assert.deepEqual(latest.targets.map(t=>t.id),['solar-a','solar-b']);assert.equal(latest.preferExistingDesign,undefined);assert.equal(latest.factionRevision,2);assert.equal(first.status,'cancelled');
});

test('default outside-area builds remain unpaid while explicit faction builds request one fresh design',()=>{
 const w=createWorld();build(w,'outside',12);assert.equal(w.foundry.requests.length,0);create(w);build(w,'explicit',24,'orchid');
 assert.equal(w.foundry.requests.length,1);assert.equal(w.foundry.requests[0].preferExistingDesign,undefined);assert.match(w.foundry.requests[0].prompt,/artistic direction freely/);assert.deepEqual(w.foundry.requests[0].targets,[{id:'explicit',assignment:1}]);const queued=structuredClone(w.foundry.requests[0]);build(w,'explicit',24,'orchid');assert.equal(w.foundry.requests.length,1);assert.deepEqual(w.foundry.requests[0],queued);
});

test('creations resolve area membership and retain their subject without duplicate active work',()=>{
 const w=createWorld();create(w);saveArea(w,'east','orchid',eastArea);
 command(w,{id:'custom',type:'build',kind:'creation',x:12,z:8,factionId:'landing',prompt:'A flower-shaped observatory with an open telescope',provider:'fixture'});
 const initial=w.foundry.requests[0];initial.status='running';assert.equal(initial.factionId,'orchid');assert.equal(initial.preferExistingDesign,undefined);
 const result=saveArea(w,'east-again','orchid',eastArea);assert.deepEqual(result.requestIds,[initial.id]);assert.equal(w.foundry.requests.length,1);assert.equal(initial.status,'running');
});

test('empty saved areas survive reopen and reject malformed bounds without mutation',()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'faction-area-'));let store;
 try{
  store=openStore(dir);const w=store.world;create(w);const input={id:'empty-region',type:'faction-assign',factionId:'orchid',area:eastArea,reuseDesign:true,buildingIds:[]};
  command(w,input);const expected=structuredClone(w);command(w,input);assert.deepEqual(w,expected);assert.equal(w.foundry.requests.length,0);
  for(const area of [{...eastArea,minX:NaN},{...eastArea,maxX:1e6+1},{...eastArea,maxX:6},{...eastArea,minZ:'0'}])assert.throws(()=>saveArea(w,'bad','orchid',area),/bounds/);
  assert.deepEqual(w,expected);store.close();store=openStore(dir);assert.deepEqual(store.world,expected);assert.equal(factionAt(store.world,12,8).id,'orchid');
  const broken=structuredClone(store.world);broken.factions[1].area.order=0;assert.throws(()=>validateWorld(broken),/damaged/);
 }finally{store?.close();fs.rmSync(dir,{recursive:true,force:true});}
});

function approvedDirectionSetup(){
 const w=createWorld();create(w);build(w,'older',12,'orchid');const faction=w.factions.find(f=>f.id==='orchid');faction.theme=structuredClone(THEMES.argonaut);
 w.buildings.find(b=>b.id==='older').asset={hash:'previous-visible'};
 command(w,{id:'origin',type:'build',kind:'creation',x:24,z:8,factionId:'orchid',prompt:'An alien greenhouse with branching glass chambers',provider:'gemini'});
 const source=w.foundry.requests.at(-1),hash='a'.repeat(64);Object.assign(source,{status:'installed',jobId:'job-new-direction',hash,rehearsal:false,installedTargets:['origin'],themeDraftId:'fresh-draft'});w.buildings.find(b=>b.id==='origin').asset={hash};
 return {w,faction,source,hash};
}

test('a fresh creation brief is independent of old faction style and helper validation has no side effects',()=>{
 const w=createWorld();create(w);const before=structuredClone(w),request=creationRequest({id:'free',factionId:'orchid',prompt:'A basalt beacon with amber sails',provider:'gemini'},w);
 assert.equal(request.drivesFactionDirection,true);assert.equal(request.artDirection,request.userPrompt);assert.match(request.prompt,/basalt beacon/);assert.equal(request.prompt.includes(w.factions[1].artDirection),false);assert.match(request.prompt,/fixed 8 metre/);assert.ok(request.prompt.length<=3000);assert.deepEqual(w,before);
});

test('accepted changed art adopts once and queues only other current members without recursive proposals',()=>{
 const {w,faction,source,hash}=approvedDirectionSetup(),old=w.foundry.requests[0],before=structuredClone([w.alloy,w.robot,w.buildings,w.deposits]),originalPrompt=source.prompt,theme=structuredClone(THEMES.sylvan);old.status='running';
 const result=adoptFactionDirection(w,source,theme,hash);assert.equal(result.status,'adopted');assert.equal(faction.revision,2);assert.equal(source.factionRevision,1);assert.equal(source.adoptedFactionRevision,2);assert.equal(source.prompt,originalPrompt);assert.equal(w.foundry.requests.find(r=>r.id===source.id),source);assert.equal(old.status,'cancelled');
 assert.equal(faction.theme,null);assert.equal(faction.themeDraftId,undefined);assert.deepEqual(faction.designOrigin,{requestId:source.id,jobId:source.jobId,sourceHash:hash});assert.ok(faction.artDirection.length<=2000);assert.equal(faction.artDirection.includes('greenhouse'),false);
 const propagated=w.foundry.requests.filter(r=>result.requestIds.includes(r.id));assert.equal(propagated.length,1);assert.deepEqual(propagated[0].targets,[{id:'older',assignment:1}]);assert.equal(propagated[0].drivesFactionDirection,undefined);assert.match(propagated[0].prompt,/Faction design language/);assert.deepEqual([w.alloy,w.robot,w.buildings,w.deposits],before);
 const after=structuredClone(w);assert.deepEqual(adoptFactionDirection(w,source,theme,hash),result);assert.deepEqual(w,after);assert.equal(adoptFactionDirection(w,propagated[0],theme,hash),null);assert.deepEqual(w,after);assert.equal(source.themeDraftId,'fresh-draft');
});

test('equivalent visual style ignores descriptive identity, role and environment and queues nothing',()=>{
 const {w,faction,source,hash}=approvedDirectionSetup(),theme=structuredClone(THEMES.argonaut);theme.id='new-role';theme.name='A new building';theme.gameplayRole='A greenhouse';theme.palette.ground='#abcdef';theme.visualChecks=['Different role requirement'];theme.shapeLanguage='  '+theme.shapeLanguage.toUpperCase().replaceAll(' ','  ')+'  ';theme.motifs.reverse();
 const older=w.buildings.find(b=>b.id==='older'),installed=w.foundry.requests[0];older.asset={hash:'b'.repeat(64),theme:structuredClone(THEMES.argonaut),rehearsal:false};Object.assign(installed,{status:'installed',hash:older.asset.hash,installedTargets:[older.id]});
 const count=w.foundry.requests.length;assert.equal(adoptFactionDirection(w,source,theme,hash).status,'unchanged');assert.equal(faction.revision,1);assert.equal(w.foundry.requests.length,count);assert.equal(source.adoptedFactionRevision,1);
});

test('newer free prompts, explicit faction updates and reassignment each prevent stale direction adoption',()=>{
 for(const mode of ['prompt','style','reassign']){
  const {w,faction,source,hash}=approvedDirectionSetup();
  if(mode==='prompt')build(w,'newest',36,'orchid');
  if(mode==='style')command(w,{id:'latest-style',type:'faction-style',factionId:'orchid',artDirection:'New explicit titanium faceted city direction.'});
  if(mode==='reassign')assign(w,'move-origin','landing',['origin']);
  const savedFaction=structuredClone(faction),count=w.foundry.requests.length;assert.equal(adoptFactionDirection(w,source,THEMES.sylvan,hash).status,'superseded');assert.deepEqual(faction,savedFaction);assert.equal(w.foundry.requests.length,count);
  if(mode==='style')assert.equal(faction.designIntentId,'style-latest-style');
 }
});

test('failed adoption validation leaves faction, requests and assets atomically unchanged; rehearsal never propagates',()=>{
 const {w,source,hash}=approvedDirectionSetup();w.buildings.find(b=>b.id==='older').kind='creation';w.buildings.find(b=>b.id==='older').prompt='';const before=structuredClone(w);
 assert.throws(()=>adoptFactionDirection(w,source,THEMES.sylvan,hash),/subject/);assert.deepEqual(w,before);source.rehearsal=true;const rehearsal=structuredClone(w);assert.equal(adoptFactionDirection(w,source,THEMES.sylvan,hash),null);assert.deepEqual(w,rehearsal);
});


test('object-specific approved source stays intact while propagation resolves a new generic family contract',()=>{
 const {w,faction,source,hash}=approvedDirectionSetup(),court=structuredClone(THEMES.sylvan);court.gameplayRole='Basketball court';court.visualChecks=['A visible basketball backboard and court lines'];
 const origin=w.buildings.find(b=>b.id==='origin');origin.asset={hash,theme:court,manifest:{contentSha256:hash},rehearsal:false};const exactSource=structuredClone(origin.asset);faction.themeDraftId='old-family-draft';
 const result=adoptFactionDirection(w,source,court,hash);assert.equal(result.status,'adopted');assert.equal(faction.theme,null);assert.equal(faction.themeDraftId,undefined);assert.deepEqual(origin.asset,exactSource);
 const child=w.foundry.requests.find(r=>result.requestIds.includes(r.id));assert.equal(child.kind,'solar');assert.equal(child.theme,undefined);assert.equal(child.drivesFactionDirection,undefined);assert.equal(child.prompt.includes('backboard and court lines'),false);
});

test('equivalent cached direction reconciles stale members and retains active matching propagation once',()=>{
 const {w,faction,source,hash}=approvedDirectionSetup();build(w,'aligned',36);build(w,'pending',48);
 const aligned=w.buildings.find(b=>b.id==='aligned'),pending=w.buildings.find(b=>b.id==='pending');for(const b of [aligned,pending]){b.factionId='orchid';b.factionAssignment=1;}
 aligned.asset={hash:'c'.repeat(64),theme:structuredClone(THEMES.argonaut),rehearsal:false};pending.asset={hash:'d'.repeat(64),theme:structuredClone(THEMES.sylvan),rehearsal:false};
 const installed={id:'current-aligned',kind:'solar',factionId:'orchid',factionRevision:1,status:'installed',hash:aligned.asset.hash,targets:[{id:'aligned',assignment:1}],installedTargets:['aligned']},running={id:'current-pending',kind:'solar',factionId:'orchid',factionRevision:1,status:'running',jobId:'active-direction',targets:[{id:'pending',assignment:1}]};w.foundry.requests.push(installed,running);const runningBefore=structuredClone(running),alignedBefore=structuredClone(aligned);
 const result=adoptFactionDirection(w,source,THEMES.argonaut,hash);assert.equal(result.status,'adopted');assert.equal(result.styleChanged,false);assert.equal(faction.revision,1);assert.equal(faction.theme,null);assert.deepEqual(faction.designOrigin,{requestId:source.id,jobId:source.jobId,sourceHash:hash});assert.ok(result.requestIds.includes(running.id));assert.deepEqual(running,runningBefore);assert.deepEqual(aligned,alignedBefore);
 const fresh=w.foundry.requests.filter(r=>result.requestIds.includes(r.id)&&r.id!==running.id);assert.equal(fresh.length,1);assert.deepEqual(fresh[0].targets,[{id:'older',assignment:1}]);const after=structuredClone(w);adoptFactionDirection(w,source,THEMES.argonaut,hash);assert.deepEqual(w,after);
});

test('matching style on an old revision or mismatched style on the current revision still needs convergence',()=>{
 for(const variant of ['old-revision','wrong-style']){
  const {w,faction,source,hash}=approvedDirectionSetup(),older=w.buildings.find(b=>b.id==='older'),installed=w.foundry.requests[0];older.asset={hash:'b'.repeat(64),theme:structuredClone(variant==='wrong-style'?THEMES.sylvan:THEMES.argonaut),rehearsal:false};Object.assign(installed,{status:'installed',hash:older.asset.hash,installedTargets:['older'],factionRevision:variant==='old-revision'?0:1});
  const result=adoptFactionDirection(w,source,THEMES.argonaut,hash);assert.equal(result.status,'adopted');assert.equal(result.styleChanged,false);assert.equal(faction.revision,1);assert.equal(result.requestIds.length,1);assert.deepEqual(w.foundry.requests.at(-1).targets,[{id:'older',assignment:1}]);
 }
});
