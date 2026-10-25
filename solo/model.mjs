import {depositNear,proceduralDeposit} from './minerals.mjs';
import {THEMES,validateTheme} from '../public/themes.mjs';
/** Single-player rules v2. Pure JSON state; UI and Foundry cannot mint world effects. */
export const DEFINITIONS=Object.freeze({
 core:{name:'Landing core',cost:0,seconds:0,power:8,demand:0,description:'Your colony anchor. Supplies 8 power.'},
 creation:{name:'Prompt creation',cost:25,seconds:6,power:0,demand:0,description:'A fixed-size foundation for anything you describe. 25 alloy.'},
 solar:{name:'Solar array',cost:35,seconds:8,power:12,demand:0,description:'Provides 12 power, even during a shortage.'},
 extractor:{name:'Mineral extractor',cost:60,seconds:12,power:0,demand:5,description:'On a mineral seam: extracts 2 alloy / second. Uses 5 power.'},
 habitat:{name:'Habitat',cost:40,seconds:10,power:0,demand:3,description:'Makes room for 8 settlers. Uses 3 power.'},
 workshop:{name:'Fabrication lab',cost:70,seconds:14,power:0,demand:4,description:'Recycles 0.5 alloy / second. Uses 4 power.'},
});
export const GRID=2,HALF=4.3,ROBOT_RADIUS=.55;
export const snap=n=>Math.round(n/GRID)*GRID;
export const coast=x=>-23+Math.sin(x*.065)*5+Math.sin(x*.16)*1.6;
export function land(x,z){return Number.isFinite(x)&&Number.isFinite(z)&&Math.abs(x)<=1e6&&Math.abs(z)<=1e6&&z>=coast(x)+2;}
const ID=/^[a-zA-Z0-9_-]{1,100}$/;
const activeRequests=new Set(['queued','preparing-theme','dispatching','running','unknown']);
const validId=id=>typeof id==='string'&&ID.test(id)&&!['__proto__','constructor','prototype'].includes(id);
const positive=value=>Number.isSafeInteger(value)&&value>=1;
const bounded=(value,min,max)=>typeof value==='string'&&value.trim().length>=min&&value.length<=max;
const defaultFaction=()=>({id:'landing',name:'Landing',artDirection:'A coherent ivory ceramic robot settlement on a sunlit mineral coast.',revision:1,theme:structuredClone(THEMES.argonaut)});
const validArea=area=>!!area&&['minX','maxX','minZ','maxZ'].every(k=>typeof area[k]==='number'&&Number.isFinite(area[k])&&Math.abs(area[k])<=1e6)&&area.minX<area.maxX&&area.minZ<area.maxZ;
const inArea=(area,x,z)=>x>=area.minX&&x<=area.maxX&&z>=area.minZ&&z<=area.maxZ;
export function factionAt(w,x,z){return w.factions.filter(f=>f.area&&inArea(f.area,x,z)).reduce((latest,f)=>!latest||f.area.order>latest.area.order?f:latest,null);}
function factionFor(w,id='landing'){const faction=w.factions.find(f=>f.id===id);if(!faction)throw Error('Unknown visual faction.');return faction;}
function targetBuildings(w,ids,{empty=false}={}){
 if(!Array.isArray(ids)||(!empty&&!ids.length)||ids.length>100||ids.some(id=>!validId(id))||new Set(ids).size!==ids.length)throw Error('Choose 1–100 unique existing building IDs.');
 const buildings=ids.map(id=>w.buildings.find(b=>b.id===id));if(buildings.some(b=>!b))throw Error('Unknown building in faction selection.');return buildings;
}
function requestFor(w,faction,buildings,id,provider,subject,{drivesFactionDirection=false}={}){
 if(!['fixture','gemini','managed'].includes(provider))throw Error('Choose a Foundry provider.');
 const kind=buildings[0].kind,userPrompt=subject??`A ${DEFINITIONS[kind].name} serving its existing colony function.`;
 const marker=`[colony:${w.id};request:${id};faction:${faction.id};revision:${faction.revision}]`;
 const limits='Keep every part inside a fixed 8 metre-wide plot. Preserve the traversable front-to-centre corridor and grounded supports required by the Foundry contract. Preserve building function; no units or gameplay changes. Use named scene.components with each part assigned exactly once. Elevated ornaments use role detail, not grounded support.';
 const fixed=`${marker}\nRequested subject: ${userPrompt}\n${limits}`;
 const prompt=drivesFactionDirection?fixed+'\nChoose an original artistic direction freely from the requested subject and function. Invent a distinctive silhouette, composition, materials, palette and purposeful details; the existing faction style does not constrain this new design.':fixed+'\nFaction design language: '+faction.artDirection.slice(0,Math.max(0,3000-fixed.length-26));
 if(prompt.length>3000)throw Error('Creation request exceeds its bounded prompt.');
 return {id,kind,round:w.foundry.round,status:'queued',provider,explicit:true,factionId:faction.id,factionRevision:faction.revision,targets:buildings.map(b=>({id:b.id,assignment:b.factionAssignment})),artDirection:drivesFactionDirection?userPrompt:faction.artDirection,userPrompt,prompt,...(drivesFactionDirection?{drivesFactionDirection:true}:{}),...(kind==='creation'?{targetId:buildings[0].id}:{})};
}
export function creationRequest(input,world){
 if(!bounded(input.prompt,5,2000))throw Error('Describe the creation in 5–2000 characters.');
 if(!validId(input.id))throw Error('A stable operation ID is required.');
 const targetId=input.targetId??input.id,existing=world.buildings.find(b=>b.id===targetId),faction=factionFor(world,input.factionId??existing?.factionId??'landing');
 if(!validId(targetId)||input.targetId&&!existing)throw Error('Unknown creation target.');
 if(existing&&(existing.kind!=='creation'||existing.factionId!==faction.id||input.factionAssignment!==undefined&&input.factionAssignment!==existing.factionAssignment))throw Error('Creation target faction assignment changed.');
 const target=existing??{id:targetId,kind:'creation',factionId:faction.id,factionAssignment:1};
 return requestFor(world,faction,[target],'creation-'+input.id,input.provider,input.prompt.trim(),{drivesFactionDirection:true});
}
export function queueFactionUpdates(w,factionId,buildingIds,operationId,{reuseDesign=false}={}){
 if(!validId(operationId))throw Error('A stable operation ID is required.');
 const faction=factionFor(w,factionId),buildings=targetBuildings(w,buildingIds,{empty:true});
 if(buildings.some(b=>b.factionId!==faction.id||!positive(b.factionAssignment)))throw Error('Building faction assignment changed.');
 const groups=new Map();for(const b of buildings){const key=b.kind==='creation'?b.id:b.kind;if(!groups.has(key))groups.set(key,[]);groups.get(key).push(b);}
 const retained=[];const requests=[...groups.values()].map((members,index)=>{
  if(reuseDesign&&members[0].kind==='creation'){const b=members[0],existing=w.foundry.requests.findLast(r=>r.kind==='creation'&&r.factionId===faction.id&&r.factionRevision===faction.revision&&r.status!=='cancelled'&&r.targets?.some(t=>t.id===b.id&&t.assignment===b.factionAssignment));if(existing){retained.push(existing);return null;}}
  if(members[0].kind==='creation'&&!bounded(members[0].prompt,5,2000))throw Error('Creation subject is missing or damaged.');
  const request=requestFor(w,faction,members,`faction-${operationId}-${index+1}`,w.foundry.mode,members[0].kind==='creation'?members[0].prompt:undefined);
  if(reuseDesign&&request.kind!=='creation')request.preferExistingDesign=true;return request;
 }).filter(Boolean);
 const duplicates=requests.filter(r=>w.foundry.requests.some(old=>old.id===r.id));
 if(duplicates.length){
  if(duplicates.length!==requests.length||requests.some(r=>{const old=w.foundry.requests.find(o=>o.id===r.id);return old.prompt!==r.prompt||JSON.stringify(old.targets)!==JSON.stringify(r.targets)||old.provider!==r.provider||!!old.preferExistingDesign!==!!r.preferExistingDesign;}))throw Error('Faction request ID reused with different data.');
  return [...requests.map(r=>w.foundry.requests.find(old=>old.id===r.id)),...retained];
 }
 const targets=new Set(buildings.map(b=>b.id));
 for(const old of w.foundry.requests){
  const overlap=old.targets?.some(t=>targets.has(t.id))||(old.targetId?targets.has(old.targetId):!old.targets&&buildings.some(b=>b.kind===old.kind));
  const currentFamily=reuseDesign&&old.factionId===faction.id&&old.factionRevision===faction.revision;
  if(!currentFamily&&activeRequests.has(old.status)&&(overlap||old.factionId===faction.id&&old.factionRevision!==faction.revision)){old.status='cancelled';old.reason=`Superseded by faction update ${operationId}.`;}
 }
 w.foundry.requests.push(...requests);for(const r of requests)if(r.kind==='creation')w.buildings.find(b=>b.id===r.targetId).creationStatus='queued';return [...requests,...retained];
}
export function assetForBuilding(world,building){return world.schemaVersion>=2?building.asset:(building.asset??world.assets?.[building.kind]);}
export function createWorld(){return {schemaVersion:2,id:'colony-'+globalThis.crypto.randomUUID(),revision:0,time:0,paused:false,alloy:200,robot:{x:0,z:18,path:[]},factions:[defaultFaction()],buildings:[{id:'landing-core',kind:'core',x:0,z:8,progress:1,created:0,factionId:'landing',factionAssignment:1}],deposits:[{id:'west-seam',x:-18,z:8,remaining:600},{id:'east-seam',x:26,z:-4,remaining:900},{id:'north-seam',x:-26,z:32,remaining:1200}],assets:{},assetHistory:[],receipts:{},events:[{time:0,text:'Touchdown. Build a solar array, then extract alloy from a mineral seam.'}],foundry:{enabled:false,mode:'fixture',round:1,requests:[],error:null}};}
function event(w,text){w.events.push({time:w.time,text});if(w.events.length>80)w.events.shift();}
export function power(w){return w.buildings.reduce((a,b)=>{const d=DEFINITIONS[b.kind];if(b.progress===1)a.supply+=d.power;a.demand+=d.demand;return a;},{supply:0,demand:0});}
export function canBuild(w,kind,x,z){
 const d=typeof kind==='string'&&Object.hasOwn(DEFINITIONS,kind)?DEFINITIONS[kind]:null;if(!d||kind==='core')return {ok:false,reason:'Unknown building type.'};
 if(typeof x!=='number'||typeof z!=='number'||!Number.isFinite(x)||!Number.isFinite(z))return {ok:false,reason:'Choose finite map coordinates.'};
 x=snap(x);z=snap(z);
 if(![[-HALF,-HALF],[HALF,-HALF],[-HALF,HALF+2],[HALF,HALF+2]].every(([a,b])=>land(x+a,z+b)))return {ok:false,reason:'Choose dry land; this footprint crosses the coast.'};
 if(w.buildings.length>=100)return {ok:false,reason:'This colony supports 100 buildings.'};
 if(w.buildings.some(b=>Math.abs(x-b.x)<HALF*2+1.4&&Math.abs(z-b.z)<HALF*2+2.2))return {ok:false,reason:'Building or access overlap. Leave room between foundations.'};
 if(Math.abs(x-w.robot.x)<HALF+ROBOT_RADIUS&&Math.abs(z-w.robot.z)<HALF+ROBOT_RADIUS)return {ok:false,reason:'Move your robot off the construction footprint.'};
 if(!findPath(w,w.robot,{x,z:z+HALF+2},{extra:{x,z}}))return {ok:false,reason:'No accessible route to this site.'};
 const deposit=depositNear(w.deposits,x,z,land);
 if(kind==='extractor'&&!deposit)return {ok:false,reason:'Place the extractor directly on a mineral seam.'};
 if(w.alloy<d.cost)return {ok:false,reason:`Need ${d.cost} alloy; you have ${Math.floor(w.alloy)}.`};
 return {ok:true,x,z,depositId:deposit?.id,deposit};
}
export function blocked(w,x,z,extra){return !land(x,z)||[...w.buildings,...(extra?[extra]:[])].some(b=>Math.abs(x-b.x)<HALF+ROBOT_RADIUS&&Math.abs(z-b.z)<HALF+ROBOT_RADIUS);}
export function findPath(w,from,to,{extra}={}){
 if(!land(to.x,to.z)||blocked(w,to.x,to.z,extra))return null;
 const start={x:snap(from.x),z:snap(from.z)},end={x:snap(to.x),z:snap(to.z)};if(blocked(w,end.x,end.z,extra))return null;
 const key=p=>`${p.x},${p.z}`,heuristic=p=>Math.abs(p.x-end.x)+Math.abs(p.z-end.z),open=[{...start,score:heuristic(start)}],came=new Map([[key(start),null]]),cost=new Map([[key(start),0]]);let steps=0;
 while(open.length&&steps++<10000){open.sort((a,b)=>b.score-a.score);const p=open.pop();if(p.x===end.x&&p.z===end.z){const route=[];let at=p;while(at){route.unshift({x:at.x,z:at.z});at=came.get(key(at));}route.shift();return route;}
 for(const [dx,dz] of [[2,0],[-2,0],[0,2],[0,-2]]){const next={x:p.x+dx,z:p.z+dz},nextCost=cost.get(key(p))+2;if(blocked(w,next.x,next.z,extra)||nextCost>=(cost.get(key(next))??Infinity))continue;cost.set(key(next),nextCost);came.set(key(next),p);open.push({...next,score:nextCost+heuristic(next)});}
 }return null;
}
export function mineralCandidates(w){const candidates=[...w.deposits],tileX=Math.floor(w.robot.x/32),tileZ=Math.floor(w.robot.z/32);for(let dx=-3;dx<=3;dx++)for(let dz=-3;dz<=3;dz++){const x=tileX+dx,z=tileZ+dz,deposit=proceduralDeposit(x,z),id=deposit.id;if(w.deposits.some(d=>d.id===id))continue;if(land(deposit.x,deposit.z))candidates.push(deposit);}return candidates.sort((a,b)=>Math.hypot(a.x-w.robot.x,a.z-w.robot.z)-Math.hypot(b.x-w.robot.x,b.z-w.robot.z));}

function commitRequests(w,candidate){
 for(const next of candidate.foundry.requests){const old=w.foundry.requests.find(r=>r.id===next.id);if(old)Object.assign(old,next);else w.foundry.requests.push(next);}
}

const normalizeStyle=value=>typeof value==='string'?value.trim().replace(/\s+/g,' ').toLowerCase():value;
function styleSignature(theme){return theme?JSON.stringify({architecture:normalizeStyle(theme.architecture),shapeLanguage:normalizeStyle(theme.shapeLanguage),materialFinish:normalizeStyle(theme.materialFinish),weathering:normalizeStyle(theme.weathering),technology:normalizeStyle(theme.technology),motifs:[...new Set(theme.motifs.map(normalizeStyle))].sort(),palette:['shell','frame','accent'].map(k=>normalizeStyle(theme.palette[k]))}):null;}
export function adoptFactionDirection(w,request,theme,sourceHash){
 if(request.directionPropagation)return request.directionPropagation;
 if(!request.drivesFactionDirection||request.rehearsal)return null;
 if(!w.foundry.requests.includes(request)||request.status!=='installed'||request.hash!==sourceHash||!(/^[a-f0-9]{64}$/).test(sourceHash)||!Array.isArray(request.installedTargets)||!request.installedTargets.length)throw Error('Only an installed independently approved design can propose faction direction.');
 const faction=w.factions.find(f=>f.id===request.factionId),sourceCurrent=request.installedTargets.some(id=>w.buildings.some(b=>b.id===id&&b.factionId===request.factionId&&b.asset?.hash===sourceHash&&request.targets?.some(t=>t.id===id&&t.assignment===b.factionAssignment)));
 if(!faction||!sourceCurrent||faction.revision!==request.factionRevision||faction.designIntentId!==request.id){request.directionPropagation={status:'superseded',sourceHash,reason:'A newer faction direction, building request or assignment takes precedence.'};return request.directionPropagation;}
 validateTheme(theme);
 const signature=styleSignature(theme),styleChanged=styleSignature(faction.theme)!==signature,originIds=new Set(request.installedTargets);
 if(styleChanged&&!Number.isSafeInteger(faction.revision+1))throw Error('Faction revision counter is exhausted.');
 const ownsTarget=(r,b)=>r.factionId===faction.id&&r.kind===b.kind&&r.targets?.some(t=>t.id===b.id&&t.assignment===b.factionAssignment);
 const aligned=b=>b.asset?.rehearsal===false&&styleSignature(b.asset.theme)===signature&&w.foundry.requests.some(r=>r.status==='installed'&&(r.adoptedFactionRevision??r.factionRevision)===faction.revision&&r.hash===b.asset.hash&&ownsTarget(r,b)&&r.installedTargets?.includes(b.id));
 const stale=w.buildings.filter(b=>b.factionId===faction.id&&!originIds.has(b.id)&&(styleChanged||!aligned(b))),retained=new Set(),ids=[];
 for(const b of stale){const pending=!styleChanged&&w.foundry.requests.findLast(r=>activeRequests.has(r.status)&&!r.drivesFactionDirection&&r.factionRevision===faction.revision&&ownsTarget(r,b)&&!r.targets.some(t=>originIds.has(t.id)));if(pending)retained.add(pending.id);else ids.push(b.id);}
 const candidate=structuredClone(w),next=factionFor(candidate,faction.id),source=candidate.foundry.requests.find(r=>r.id===request.id),previousRevision=faction.revision;
 if(styleChanged)next.revision++;
 if(styleChanged||stale.length){next.theme=null;delete next.themeDraftId;}
 next.artDirection=`Shape: ${theme.shapeLanguage.slice(0,400)}. Materials: ${theme.materialFinish.slice(0,400)}. Motifs: ${theme.motifs.join('; ').slice(0,600)}. Technology: ${theme.technology.slice(0,400)}. Colors: ${theme.palette.shell}, ${theme.palette.frame}, ${theme.palette.accent}.`.slice(0,2000);next.designOrigin={requestId:request.id,jobId:request.jobId,sourceHash};
 const fullId='adopt-'+request.id,operationId=fullId.length<=100?fullId:'adopt-'+request.id.slice(0,70)+'-'+w.foundry.requests.indexOf(request),requests=styleChanged||ids.length?queueFactionUpdates(candidate,faction.id,ids,operationId):[],requestIds=[...retained,...requests.map(r=>r.id)];
 source.adoptedFactionRevision=next.revision;source.directionPropagation={status:styleChanged||stale.length?'adopted':'unchanged',styleChanged,previousRevision,factionRevision:next.revision,sourceHash,requestIds,reason:styleChanged?'The independently approved new design defines this faction direction; other current members are queued for review.':stale.length?'The approved design confirms this direction; only members still missing its approved appearance are being reconciled.':'Every current member already has an approved appearance matching this visual direction.'};
 Object.assign(faction,next);if(!Object.hasOwn(next,'themeDraftId'))delete faction.themeDraftId;commitRequests(w,candidate);for(const b of w.buildings)if(ids.includes(b.id)&&b.kind==='creation')b.creationStatus=candidate.buildings.find(t=>t.id===b.id).creationStatus;w.revision++;
 return request.directionPropagation;
}

export function command(w,input){
 if(!input||typeof input!=='object'||typeof input.id!=='string'||!/^[a-zA-Z0-9_-]{1,100}$/.test(input.id)||['__proto__','constructor','prototype'].includes(input.id))throw Error('A stable operation ID is required.');
 const signature=JSON.stringify(Object.fromEntries(Object.keys(input).sort().map(k=>[k,input[k]])));if(Object.hasOwn(w.receipts,input.id)){if(w.receipts[input.id].signature!==signature)throw Error('Operation ID reused with different data.');return structuredClone(w.receipts[input.id].result);}
 if(Object.keys(w.receipts).length>=4096)throw Error('Command history is full; export this colony before starting a new one.');
 let result={ok:true,id:input.id};
 if(input.type==='prospect'){
 if(w.alloy<DEFINITIONS.extractor.cost)throw Error('Need 60 alloy to place a mineral extractor.');
 const candidate=structuredClone(w),deposit=mineralCandidates(w).find(d=>d.remaining>0&&canBuild({...w,deposits:[...w.deposits,d]},'extractor',d.x,d.z).ok);if(!deposit)throw Error('No accessible mineral seam nearby. Move your robot to another area.');
 if(!candidate.deposits.some(d=>d.id===deposit.id))candidate.deposits.push(deposit);result=command(candidate,{id:input.id,type:'build',kind:'extractor',x:deposit.x,z:deposit.z,...(input.factionId!==undefined?{factionId:input.factionId}:{})});Object.assign(w,candidate);
 }else if(input.type==='build'){
 if(w.buildings.some(b=>b.id===input.id))throw Error('Building identity already exists.');
 const site=canBuild(w,input.kind,input.x,input.z);if(!site.ok)throw Error(site.reason);
 const areaFaction=factionAt(w,site.x,site.z),faction=areaFaction??factionFor(w,input.factionId??'landing');
 const creation=input.kind==='creation'?creationRequest({...input,targetId:undefined,factionAssignment:undefined,factionId:faction.id},w):null;
 const b={id:input.id,kind:input.kind,x:site.x,z:site.z,progress:0,created:w.time,factionId:faction.id,factionAssignment:1,...(input.kind==='extractor'?{depositId:site.depositId}:{})};
 const design=!creation&&(areaFaction||input.factionId!==undefined)?requestFor(w,faction,[b],'building-'+input.id,w.foundry.mode,undefined,{drivesFactionDirection:true}):null;
 if(input.kind==='extractor'&&!w.deposits.some(p=>p.id===site.depositId))w.deposits.push({...site.deposit});
 w.alloy-=DEFINITIONS[input.kind].cost;if(creation){b.prompt=creation.userPrompt;b.name=creation.userPrompt.slice(0,70);b.creationStatus='queued';w.foundry.requests.push(creation);}if(design)w.foundry.requests.push(design);if(creation||design)faction.designIntentId=(creation||design).id;w.buildings.push(b);result.buildingId=b.id;result.factionId=faction.id;result.requestIds=[creation,design].filter(Boolean).map(r=>r.id);event(w,`${DEFINITIONS[b.kind].name} construction started.`);
 // Recalculate the remaining route when a new foundation crosses it.
 if(w.robot.path.length){const destination=w.robot.path.at(-1);w.robot.path=findPath(w,w.robot,destination)||[];}
 }else if(input.type==='faction-create'){
 if(!validId(input.factionId)||w.factions.some(f=>f.id===input.factionId))throw Error('Choose a unique faction ID.');
 if(!bounded(input.name,1,100))throw Error('Faction name must contain 1–100 characters.');
 if(!bounded(input.artDirection,5,2000))throw Error('Faction art direction must contain 5–2000 characters.');
 w.factions.push({id:input.factionId,name:input.name.trim(),artDirection:input.artDirection.trim(),revision:1,theme:null});result.factionId=input.factionId;
 }else if(input.type==='faction-assign'){
 const faction=factionFor(w,input.factionId);
 if(input.reuseDesign!==undefined&&typeof input.reuseDesign!=='boolean')throw Error('Reuse design must be true or false.');
 let area,buildings;
 if(input.area!==undefined){
  if(!validArea(input.area))throw Error('Choose finite faction area bounds with positive width and depth.');
  const order=Math.max(0,...w.factions.map(f=>f.area?.order??0))+1;if(!positive(order))throw Error('Faction area order is exhausted.');
  const {minX,maxX,minZ,maxZ}=input.area;area={minX,maxX,minZ,maxZ,order};buildings=w.buildings.filter(b=>inArea(area,b.x,b.z));
  if(input.buildingIds!==undefined){const supplied=targetBuildings(w,input.buildingIds,{empty:true});if(supplied.length!==buildings.length||supplied.some(b=>!buildings.includes(b)))throw Error('Selected buildings changed; select the faction area again.');}
 }else buildings=targetBuildings(w,input.buildingIds);
 const buildingIds=buildings.map(b=>b.id);
 if(buildings.some(b=>!positive(b.factionAssignment)||b.factionId!==faction.id&&!Number.isSafeInteger(b.factionAssignment+1)))throw Error('Faction assignment counter is exhausted.');
 // Plan all requests on a copy so failed scope or prompt validation mutates nothing.
 const candidate=structuredClone(w);for(const b of candidate.buildings.filter(b=>buildingIds.includes(b.id)))if(b.factionId!==faction.id){b.factionId=faction.id;b.factionAssignment++;}
 if(area)factionFor(candidate,faction.id).area=area;
 const requests=queueFactionUpdates(candidate,faction.id,buildingIds,input.id,{reuseDesign:input.reuseDesign===true});
 for(const b of buildings){const next=candidate.buildings.find(n=>n.id===b.id);b.factionId=next.factionId;b.factionAssignment=next.factionAssignment;if(b.kind==='creation')b.creationStatus=next.creationStatus;}
 if(area)faction.area=area;commitRequests(w,candidate);result.factionId=faction.id;result.requestIds=requests.map(r=>r.id);
 }else if(input.type==='faction-style'){
 const faction=factionFor(w,input.factionId);
 if(!bounded(input.artDirection,5,2000))throw Error('Faction art direction must contain 5–2000 characters.');
 if(!Number.isSafeInteger(faction.revision+1))throw Error('Faction revision counter is exhausted.');
 const candidate=structuredClone(w),next=factionFor(candidate,faction.id);next.artDirection=input.artDirection.trim();next.revision++;next.theme=null;next.designIntentId='style-'+input.id;
 const requests=queueFactionUpdates(candidate,faction.id,candidate.buildings.filter(b=>b.factionId===faction.id).map(b=>b.id),input.id);
 Object.assign(faction,next);commitRequests(w,candidate);for(const b of w.buildings)if(b.factionId===faction.id&&b.kind==='creation')b.creationStatus='queued';result.factionId=faction.id;result.requestIds=requests.map(r=>r.id);
 }else if(input.type==='move'){
 if(typeof input.x!=='number'||typeof input.z!=='number'||!Number.isFinite(input.x)||!Number.isFinite(input.z))throw Error('Choose finite map coordinates.');
 const path=findPath(w,w.robot,{x:input.x,z:input.z});if(!path)throw Error('No dry-land route to that location.');w.robot.path=path;
 }else if(input.type==='pause'){if(typeof input.paused!=='boolean')throw Error('Pause must be true or false.');w.paused=input.paused;
 }else throw Error('Unknown colony command.');
 w.revision++;w.receipts[input.id]={signature,result};return structuredClone(result);
}
export function advance(w,seconds){
 if(w.paused||!Number.isFinite(seconds)||seconds<=0)return;
 let remaining=Math.min(seconds,60);while(remaining>0){const dt=Math.min(remaining,.25);remaining-=dt;w.time+=dt;const p=power(w),powered=p.supply>=p.demand;
 for(const b of w.buildings){const def=DEFINITIONS[b.kind];if(b.progress<1){if(powered||def.demand===0){b.progress=Math.min(1,b.progress+dt/def.seconds);if(b.progress>=1-1e-9){b.progress=1;event(w,`${def.name} is online.`);}}continue;}
 if(powered&&b.kind==='extractor'){const deposit=w.deposits.find(d=>d.id===b.depositId);if(deposit){const amount=Math.min(deposit.remaining,dt*2);deposit.remaining-=amount;w.alloy+=amount;}}
 if(powered&&b.kind==='workshop')w.alloy+=dt*.5;
 }
 const target=w.robot.path[0];if(target){const dx=target.x-w.robot.x,dz=target.z-w.robot.z,d=Math.hypot(dx,dz),s=Math.min(d,dt*6);const x=w.robot.x+(d?dx/d*s:0),z=w.robot.z+(d?dz/d*s:0);if(blocked(w,x,z)){w.robot.path=[];event(w,'Robot route blocked. Choose another destination.');}else{w.robot.x=x;w.robot.z=z;if(d<=s+.001)w.robot.path.shift();}}
 }w.revision++;
}
export function publicState(w){const p=power(w);return {schemaVersion:w.schemaVersion,factions:w.factions,id:w.id,revision:w.revision,time:w.time,paused:w.paused,alloy:w.alloy,robot:w.robot,buildings:w.buildings,deposits:w.deposits,assets:w.assets,assetHistory:w.assetHistory.map(({kind,hash,jobId,rehearsal,at,factionId,rolledBackAt,rollbackTargets,previousByTarget})=>({kind,hash,jobId,rehearsal,at,factionId,rolledBackAt,rollbackTargets,previousByTarget:previousByTarget?.map(({id,assignment})=>({id,assignment}))})),power:p,population:w.buildings.filter(b=>b.kind==='habitat'&&b.progress===1).length*8,populationCap:1+w.buildings.filter(b=>b.kind==='habitat'&&b.progress===1).length*8,events:w.events,foundry:w.foundry,definitions:DEFINITIONS};}
export function validateWorld(w){
 const damaged=()=>{throw Error('Unsupported or damaged colony save; preserve the file for recovery.');};
 if(![1,2].includes(w?.schemaVersion)||(typeof w.id!=='string'||!(/^(?:first-landing|colony-[a-f0-9-]{36})$/).test(w.id))||!Number.isFinite(w.alloy)||w.alloy<0||!Number.isFinite(w.time)||!Array.isArray(w.buildings)||w.buildings.length<1||w.buildings.length>100||!w.robot||!Array.isArray(w.robot.path)||!w.receipts||!w.foundry||!Array.isArray(w.foundry.requests)||!w.assets||typeof w.assets!=='object'||Array.isArray(w.assets)||!Array.isArray(w.assetHistory))damaged();
 const candidate=w.schemaVersion===1?structuredClone(w):w;
 if(candidate.schemaVersion===1){
  candidate.factions=[defaultFaction()];
  for(const b of candidate.buildings){if(!b.asset&&candidate.assets[b.kind])b.asset=structuredClone(candidate.assets[b.kind]);b.factionId='landing';b.factionAssignment=1;}
  candidate.schemaVersion=2;
 }
 if(!Array.isArray(candidate.factions)||!candidate.factions.length)damaged();
 const factions=new Set();for(const f of candidate.factions){
  if(!f||!validId(f.id)||factions.has(f.id)||!bounded(f.name,1,100)||!bounded(f.artDirection,5,2000)||!positive(f.revision)||f.theme===undefined)damaged();
  if(f.area!==undefined&&(!validArea(f.area)||!positive(f.area.order)))damaged();
  if(f.theme!==null)validateTheme(f.theme);factions.add(f.id);
 }
 if(!factions.has('landing'))damaged();
 const ids=new Set();for(const b of candidate.buildings){if(!b||!validId(b.id)||ids.has(b.id)||!Object.hasOwn(DEFINITIONS,b.kind)||!Number.isFinite(b.x)||!Number.isFinite(b.z)||!Number.isFinite(b.progress)||b.progress<0||b.progress>1||!factions.has(b.factionId)||!positive(b.factionAssignment))throw Error('Damaged building or faction assignment record.');ids.add(b.id);}
 const requests=new Set();for(const r of candidate.foundry.requests){
  if(!r||typeof r.id!=='string'||!r.id.length||requests.has(r.id))damaged();requests.add(r.id);
  if(r.factionId===undefined&&r.factionRevision===undefined&&r.targets===undefined)continue; // Historical v1 requests stay readable, restricted to landing by adapter.
  if(!factions.has(r.factionId)||!positive(r.factionRevision)||!Array.isArray(r.targets)||!r.targets.length||r.targets.length>100||new Set(r.targets.map(t=>t?.id)).size!==r.targets.length||r.targets.some(t=>!t||!ids.has(t.id)||!positive(t.assignment)))damaged();
 }
 if(w.schemaVersion===1)Object.assign(w,candidate);return w;
}
