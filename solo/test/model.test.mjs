import test from 'node:test';
import assert from 'node:assert/strict';
import {createWorld,command,advance,canBuild,findPath,DEFINITIONS,alloyRate,publicState} from '../model.mjs';
const build=(w,id,kind,x,z)=>command(w,{id,type:'build',kind,x,z});
test('construction costs once, visibly progresses and survives serialization',()=>{
 const w=createWorld();const before=w.alloy;const a=build(w,'build-a','solar',12,8);assert.equal(w.alloy,before-DEFINITIONS.solar.cost);assert.equal(w.buildings.at(-1).progress,0);
 assert.deepEqual(build(w,'build-a','solar',12,8),a);assert.equal(w.buildings.length,2);assert.throws(()=>build(w,'build-a','habitat',12,8),/reused/);
 advance(w,4);assert.ok(w.buildings.at(-1).progress>0&&w.buildings.at(-1).progress<1);const recovered=JSON.parse(JSON.stringify(w));advance(recovered,20);assert.equal(recovered.buildings.at(-1).progress,1);assert.equal(recovered.alloy,w.alloy+20);
});
test('overlap, water, unknown kinds and unaffordable requests do not spend',()=>{
 const w=createWorld();const before=JSON.stringify(w);assert.throws(()=>build(w,'a','solar',0,8),/overlap|access/);assert.throws(()=>build(w,'b','solar',0,-40),/water|land/);assert.throws(()=>build(w,'c','nuke',12,8),/Unknown/);assert.equal(JSON.stringify(w),before);
 w.alloy=0;assert.throws(()=>build(w,'d','solar',12,8),/alloy/);assert.equal(w.buildings.length,1);
});
test('robot routes around a structure and cannot walk through water',()=>{
 const w=createWorld();const path=findPath(w,{x:-12,z:8},{x:12,z:8});assert.ok(path.length>8);for(const p of path)assert.ok(Math.abs(p.x)>4.8||Math.abs(p.z-8)>4.8);
 command(w,{id:'move-1',type:'move',x:12,z:8});for(let i=0;i<100;i++)advance(w,.2);assert.ok(Math.hypot(w.robot.x-12,w.robot.z-8)<.2);
 assert.throws(()=>command(w,{id:'move-2',type:'move',x:0,z:-40}),/land|route/);
});
test('finite extraction needs a seam and power; outages do not invent output',()=>{
 const w=createWorld();assert.equal(canBuild(w,'extractor',12,8).ok,false);build(w,'mine','extractor',-18,8);advance(w,20);const amount=w.alloy;advance(w,5);assert.ok(w.alloy>amount);assert.ok(w.deposits[0].remaining<600);
 w.deposits[0].remaining=1;advance(w,10);assert.equal(w.deposits[0].remaining,0);const end=w.alloy;advance(w,10);assert.equal(w.alloy,end+10);
});
test('power shortage pauses consumers and solar restores them',()=>{
 const w=createWorld();w.alloy=1000;build(w,'a','habitat',12,8);advance(w,20);build(w,'b','habitat',24,8);advance(w,20);const before=JSON.stringify(w);assert.throws(()=>build(w,'c','workshop',0,24),/power/);assert.equal(JSON.stringify(w),before);build(w,'s','solar',-12,24);advance(w,20);build(w,'c','workshop',0,24);w.buildings=w.buildings.filter(b=>b.id!=='s');advance(w,1);assert.equal(w.buildings.find(b=>b.kind==='workshop').progress,0);build(w,'s2','solar',-12,24);advance(w,30);assert.equal(w.buildings.find(b=>b.kind==='workshop').progress,1);
});
test('commands validate finite coordinates, operation IDs and pause',()=>{
 const w=createWorld();for(const x of [NaN,Infinity,'12',1000001])assert.throws(()=>build(w,'x','solar',x,8));assert.throws(()=>command(w,{id:'',type:'pause'}));command(w,{id:'pause',type:'pause',paused:true});const clock=w.time;advance(w,10);assert.equal(w.time,clock);
});
test('prototype-like kinds and existing entity IDs cannot corrupt a save',()=>{
 const w=createWorld(),before=JSON.stringify(w);for(const kind of ['constructor','toString','__proto__'])assert.throws(()=>build(w,'bad-'+kind,kind,12,8),/Unknown/);assert.throws(()=>build(w,'landing-core','solar',12,8),/identity|exists/);assert.equal(JSON.stringify(w),before);
});
test('operation retries tolerate JSON field order and reserve dangerous IDs',()=>{
 const w=createWorld();const first=command(w,{id:'same',type:'move',x:12,z:8});assert.deepEqual(command(w,{z:8,x:12,type:'move',id:'same'}),first);for(const id of ['__proto__','constructor'])assert.throws(()=>command(w,{id,type:'move',x:12,z:8}));
});
test('prompt creations preserve the free-form brief and queue one job for one paid plot',()=>{
 const w=createWorld(),input={id:'custom-1',type:'build',kind:'creation',x:12,z:8,prompt:'An enormous pink mushroom with a glass observatory',provider:'fixture'};command(w,input);assert.equal(w.alloy,75);assert.equal(w.buildings.at(-1).prompt,input.prompt);assert.equal(w.foundry.requests.length,1);assert.match(w.foundry.requests[0].prompt,/pink mushroom/);assert.equal(w.foundry.requests[0].targetId,'custom-1');command(w,input);assert.equal(w.foundry.requests.length,1);assert.throws(()=>command(w,{...input,id:'bad',x:24,prompt:''}),/Describe/);
});
test('dry land beyond the former survey edge is buildable and mineral prospecting is paid once',()=>{
 const w=createWorld();assert.equal(canBuild(w,'solar',64,8).ok,true);const result=command(w,{id:'prospect',type:'prospect'});assert.ok(result.buildingId);assert.equal(w.alloy,40);assert.equal(w.buildings.at(-1).kind,'extractor');assert.equal(w.buildings.at(-1).progress,0);command(w,{id:'prospect',type:'prospect'});assert.equal(w.alloy,40);
});

test('ordinary premade construction never queues art, even when explicitly factioned or inside a faction area',()=>{
 for(const enabled of [false,true])for(const kind of ['solar','extractor','habitat','workshop']){
  const w=createWorld();w.foundry.enabled=enabled;w.alloy=1000;
  command(w,{id:'faction',type:'faction-create',factionId:'orchid',name:'Orchid',artDirection:'Layered orchid petals with copper ribs.'});
  const areaBuild=kind==='solar';if(areaBuild)command(w,{id:'area',type:'faction-assign',factionId:'orchid',buildingIds:[],area:{minX:6,maxX:30,minZ:0,maxZ:18}});
  const position={solar:[12,8],extractor:[-18,8],habitat:[24,8],workshop:[36,8]}[kind],input={id:kind,type:'build',kind,x:position[0],z:position[1],...(areaBuild?{}:{factionId:'orchid'})};
  const result=command(w,input);assert.equal(result.factionId,'orchid');assert.deepEqual(result.requestIds,[]);assert.equal(w.foundry.requests.length,0);assert.equal(w.factions.find(f=>f.id==='orchid').designIntentId,undefined);
  command(w,input);assert.equal(w.foundry.requests.length,0);assert.equal(w.buildings.filter(b=>b.id===kind).length,1);
 }
});
test('factioned prospect shortcut pays once without requesting premade extractor art',()=>{
 for(const enabled of [false,true]){
  const w=createWorld();w.foundry.enabled=enabled;command(w,{id:'faction',type:'faction-create',factionId:'orchid',name:'Orchid',artDirection:'Layered orchid petals with copper ribs.'});
  const input={id:'prospect-factioned',type:'prospect',factionId:'orchid'},result=command(w,input),alloy=w.alloy;
  assert.equal(w.buildings.find(b=>b.id===result.buildingId).factionId,'orchid');assert.equal(alloy,40);assert.equal(w.foundry.requests.length,0);assert.deepEqual(command(w,input),result);assert.equal(w.alloy,alloy);assert.equal(w.foundry.requests.length,0);
 }
});

test('new colonies receive one alloy per running second and expose the live rate',()=>{
 const w=createWorld();assert.equal(w.alloy,100);assert.equal(alloyRate(w),1);assert.equal(publicState(w).alloyRate,1);advance(w,10);assert.equal(w.alloy,110);command(w,{id:'pause-income',type:'pause',paused:true});advance(w,10);assert.equal(w.alloy,110);
});
test('generated solar and wind only provide their fixed output after paid build, accepted design and completion',async()=>{
 const {power,buildingDefinition}=await import('../model.mjs');
 for(const prompt of ['A copper solar panel array','A green windmill']){const w=createWorld(),input={id:'generator',type:'build',kind:'creation',x:12,z:8,prompt,provider:'gemini',function:'extractor',power:99999,cost:0};command(w,input);const b=w.buildings.at(-1);assert.equal(b.functionVersion,1);assert.equal(buildingDefinition(b).power,12);assert.equal(w.alloy,65);advance(w,10);assert.equal(power(w).supply,8);const asset={hash:'a'.repeat(64),recipe:{name:'exact generated appearance'}};b.asset=asset;assert.equal(power(w).supply,20);assert.equal(b.asset,asset);assert.equal(b.prompt,prompt);command(w,input);assert.equal(w.buildings.length,2);}
});
test('generated extractors require valid seam and accepted finished design for finite +2 alloy/sec',()=>{
 const w=createWorld(),input={id:'generated-mine',type:'build',kind:'creation',x:-18,z:8,prompt:'A copper mineral extractor',provider:'gemini'};assert.throws(()=>command(w,{...input,x:12}),/seam/);command(w,input);const b=w.buildings.at(-1);advance(w,20);const before=w.alloy;assert.equal(alloyRate(w),1);const asset={hash:'b'.repeat(64)};b.asset=asset;assert.equal(alloyRate(w),3);advance(w,5);assert.equal(w.alloy-before,15);w.deposits[0].remaining=0;assert.equal(alloyRate(w),1);const end=w.alloy;advance(w,5);assert.equal(w.alloy-end,5);
});
test('legacy generated functions migrate once without changing approved appearance or economy',async()=>{
 const {validateWorld,power}=await import('../model.mjs'),w=createWorld();w.alloy=321;const asset={hash:'c'.repeat(64),recipe:{name:'original windmill'}};w.buildings.push({id:'legacy-wind',kind:'creation',prompt:'A copper windmill',x:12,z:8,progress:1,asset,factionId:'landing',factionAssignment:1});validateWorld(w);const building=w.buildings.at(-1);assert.equal(building.function,'wind');assert.equal(building.functionVersion,1);assert.equal(w.alloy,321);assert.deepEqual(building.asset,asset);assert.equal(power(w).supply,20);building.prompt='A solar array';const saved=JSON.stringify(w);validateWorld(w);assert.equal(w.buildings.at(-1).function,'wind');assert.equal(JSON.stringify(w),saved);
});
test('invalid saved creation function versions and prototype names are rejected atomically',async()=>{
 const {validateWorld}=await import('../model.mjs');for(const change of [{function:'constructor',functionVersion:1},{function:'solar',functionVersion:2},{functionVersion:1}]){const w=createWorld();w.buildings.push({id:'bad-creation',kind:'creation',prompt:'A solar array',x:12,z:8,progress:1,factionId:'landing',factionAssignment:1,...change});const before=structuredClone(w);assert.throws(()=>validateWorld(w),/Damaged/);assert.deepEqual(w,before);}
});
test('canonical creation roles and costs ignore untrusted payload fields and failed reservations are atomic',async()=>{
 const {creationBlueprint,power}=await import('../model.mjs');for(const prompt of ['power plant','powerplant','fusion reactor','windmills']){const blueprint=creationBlueprint(prompt);assert.equal(blueprint.power,12);assert.deepEqual(Object.keys(blueprint).sort(),['function','name','cost','seconds','power','demand','description'].sort());}
 const w=createWorld();w.alloy=1000;for(let i=0;i<4;i++)command(w,{id:'load-'+i,type:'build',kind:'creation',x:12+i*12,z:8,prompt:'A stone monument',provider:'gemini',function:'solar',power:99999,cost:0});assert.equal(power(w).demand,8);assert.equal(power(w).supply,8);assert.equal(w.alloy,900);const before=JSON.stringify(w);assert.throws(()=>command(w,{id:'overload',type:'build',kind:'creation',x:0,z:24,prompt:'A stone monument',provider:'gemini'}),/power/);assert.equal(JSON.stringify(w),before);assert.throws(()=>command(w,{id:'prospect-overload',type:'prospect'}),/seam/);assert.equal(JSON.stringify(w),before);
});

test('legacy extractors bind finite existing seams once without inventing geography',async()=>{
 const {validateWorld}=await import('../model.mjs');
 for(const [x,z,remaining] of [[-18,8,600],[-18,8,0],[12,8,600]]){
  const w=createWorld();w.deposits[0].remaining=remaining;const deposits=structuredClone(w.deposits);
  w.buildings.push({id:'old-mine',kind:'creation',prompt:'A mineral extractor',x,z,progress:1,asset:{hash:'d'.repeat(64)},factionId:'landing',factionAssignment:1});
  validateWorld(w);const b=w.buildings.at(-1);assert.equal(b.function,'extractor');assert.equal(b.depositId,x===-18?'west-seam':undefined);assert.deepEqual(w.deposits,deposits);
  const saved=JSON.stringify(w);validateWorld(w);assert.equal(JSON.stringify(w),saved);
  const before=w.alloy;advance(w,5);assert.equal(w.alloy-before,x===-18&&remaining>0?15:5);assert.equal(w.deposits[0].remaining,x===-18&&remaining>0?590:remaining);
 }
});
