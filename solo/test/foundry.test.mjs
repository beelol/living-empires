import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createWorld} from '../model.mjs';
import {verifyPackage,activatePackage,rollback,FoundryController} from '../foundry.mjs';
import {fixtureRecipe,THEMES} from '../../public/core.mjs';
const sha=b=>createHash('sha256').update(b).digest('hex');
function sample(){const images=[0,1,2].map(i=>Buffer.from([137,80,78,71,13,10,26,10,i]));const pkg={assetId:'job-test',recipe:fixtureRecipe(),theme:THEMES.argonaut,manifest:{contentSha256:'a'.repeat(64),rehearsal:true,evidence:images.map((b,i)=>({url:`/evidence/job-test/1-${i}.png`,sha256:sha(b)}))}};return {images,pkg,job:{id:'job-test',status:'accepted',acceptedHash:'a'.repeat(64),pipeline:{},provider:'fixture'}};}
test('exact package and three distinct image hashes are required',async()=>{
 const {images,pkg,job}=sample();const api=async route=>route.endsWith('/package')?structuredClone(pkg):images[Number(route.at(-5))];
 const good=await verifyPackage({job,api,verifyTyped:async()=>structuredClone(pkg)});assert.equal(good.package.assetId,job.id);
 await assert.rejects(()=>verifyPackage({job,api,verifyTyped:async()=>({...pkg,assetId:'other'})}),/differs/);
 const changed=async route=>route.endsWith('/package')?structuredClone(pkg):Buffer.from('changed');await assert.rejects(()=>verifyPackage({job,api:changed,verifyTyped:async()=>pkg}),/image|PNG/);
 job.status='cancelled';await assert.rejects(()=>verifyPackage({job,api,verifyTyped:async()=>pkg}),/accepted/);
});
test('activation and rollback preserve gameplay state; duplicate bytes do not add a revision',()=>{
 const w=createWorld(),{pkg}=sample(),before=JSON.stringify([w.buildings,w.alloy,w.robot,w.deposits]);activatePackage(w,'core',pkg);assert.equal(w.assets.core.hash,pkg.manifest.contentSha256);assert.throws(()=>activatePackage(w,'core',pkg),/already/);rollback(w,'core');assert.equal(w.assets.core,undefined);assert.equal(JSON.stringify([w.buildings,w.alloy,w.robot,w.deposits]),before);
});
test('an uncertain dispatch is retained and never blindly repeated',async()=>{
 const w=createWorld();w.foundry.enabled=true;let posts=0;const c=new FoundryController({world:w,save:()=>{},api:async(route,body)=>{if(body){posts++;throw Error('lost response');}return [];},verifyTyped:async()=>{}});await c.step();assert.equal(posts,1);assert.equal(w.foundry.requests[0].status,'unknown');await c.step();assert.equal(posts,1);assert.equal(w.foundry.requests[0].status,'unknown');
});
test('busy or unavailable Foundry leaves the colony intact',async()=>{
 const w=createWorld();w.foundry.enabled=true;const c=new FoundryController({world:w,save:()=>{},api:async()=>{throw Error('Foundry unavailable');}});const before=JSON.stringify(w.buildings);await c.step();assert.equal(JSON.stringify(w.buildings),before);assert.match(w.foundry.error,/unavailable/);
});
test('cancellation during independent verification cannot install late content',async()=>{
 const w=createWorld(),{images,pkg,job}=sample();let release;const waiting=new Promise(resolve=>release=resolve);let started;const verifying=new Promise(resolve=>started=resolve);w.foundry.enabled=true;
 const request={id:'own',kind:'core',round:1,status:'running',provider:'fixture',prompt:'frozen',jobId:job.id};w.foundry.requests.push(request);job.prompt='frozen';
 const c=new FoundryController({world:w,save:()=>{},api:async route=>route.endsWith('/package')?structuredClone(pkg):route.startsWith('/evidence/')?images[Number(route.at(-5))]:job,verifyTyped:async()=>{started();await waiting;return structuredClone(pkg);}});
 const operation=c.step();await verifying;request.status='cancelled';w.foundry.enabled=false;release();await operation;assert.equal(w.assets.core,undefined);assert.equal(request.status,'cancelled');
});
test('rendered cone and cylinder bounds cannot escape the fixed plot',async()=>{
 for(const shape of ['cylinder','cone']){const {pkg,images,job}=sample();pkg.recipe.parts.push({id:'wide',shape,role:'detail',position:shape==='cylinder'?[3,1,0]:[0,1,3],size:shape==='cylinder'?[1,2,8]:[8,2,1],material:'shell'});await assert.rejects(()=>verifyPackage({job,api:async route=>route.endsWith('/package')?structuredClone(pkg):images[Number(route.at(-5))],verifyTyped:async()=>structuredClone(pkg)}),/fixed 8 metre plot/);}
});
test('an explicit prompt does not resume paused automatic improvements',async()=>{
 const w=createWorld();w.foundry.enabled=false;w.foundry.requests.push({id:'auto',kind:'core',status:'queued',provider:'managed',prompt:'automatic'});const {command}=await import('../model.mjs');command(w,{id:'manual',type:'build',kind:'creation',x:12,z:8,prompt:'A glass mushroom observatory',provider:'fixture'});assert.equal(w.foundry.enabled,false);let sent;const request=w.foundry.requests.at(-1);const c=new FoundryController({world:w,save:()=>{},api:async(route,body)=>{if(route==='/api/themes/resolve')return {theme:THEMES.argonaut,draftId:'theme-test'};if(body){sent=body;return {id:'job-manual'};}return {id:'job-manual',provider:'fixture',prompt:request.prompt,status:'generating'};}});await c.step();assert.match(sent.prompt,/glass mushroom/);assert.equal(w.foundry.requests[0].status,'queued');assert.equal(w.foundry.enabled,false);
});
test('one accepted prompt upgrades only its own object',async()=>{
 const w=createWorld(),{command}=await import('../model.mjs'),{pkg,images,job}=sample();w.foundry.enabled=false;command(w,{id:'object-one',type:'build',kind:'creation',x:12,z:8,prompt:'A glass observatory',provider:'fixture'});command(w,{id:'object-two',type:'build',kind:'creation',x:24,z:8,prompt:'A copper lighthouse',provider:'fixture'});const r=w.foundry.requests[0];r.status='running';r.jobId=job.id;job.prompt=r.prompt;const c=new FoundryController({world:w,save:()=>{},api:async route=>route.endsWith('/package')?structuredClone(pkg):route.startsWith('/evidence/')?images[Number(route.at(-5))]:job,verifyTyped:async()=>structuredClone(pkg)});await c.step();assert.equal(w.buildings.find(b=>b.id==='object-one').asset.hash,job.acceptedHash);assert.equal(w.buildings.find(b=>b.id==='object-two').asset,undefined);assert.equal(w.assets.creation,undefined);
});

test('a player prompt freezes the live inferred palette rather than preset colors',async()=>{
 const w=createWorld(),{command}=await import('../model.mjs');command(w,{id:'palette',type:'build',kind:'creation',x:12,z:8,prompt:'A moss observatory with copper fins and a blue dome',provider:'gemini'});w.foundry.enabled=false;const inferred=structuredClone(THEMES.argonaut);inferred.palette={...inferred.palette,shell:'#63824a',frame:'#b87333',accent:'#168bdf'};let submitted;const r=w.foundry.requests.at(-1);const c=new FoundryController({world:w,save:()=>{},api:async(route,body)=>{if(route==='/api/themes/resolve')return {theme:inferred,draftId:'live-draft'};if(body){submitted=body;return {id:'job-colors'};}return {id:'job-colors',provider:'gemini',prompt:r.prompt,status:'generating'};}});await c.step();assert.deepEqual(submitted.theme.palette,inferred.palette);assert.equal(submitted.sceneHalfExtent,4);assert.equal(r.themeDraftId,'live-draft');assert.equal(r.status,'running');
});
