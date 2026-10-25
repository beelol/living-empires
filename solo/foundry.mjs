/** Language-neutral Foundry adapter. Only accepted, independently checked packages enter a colony. */
import {createHash} from 'node:crypto';
import {execFile} from 'node:child_process';
import {isDeepStrictEqual} from 'node:util';
import fs from 'node:fs';
import path from 'node:path';
import {validateRecipe,validateTheme,THEMES} from '../public/core.mjs';
const sha=b=>createHash('sha256').update(b).digest('hex');
const requireThat=(value,message)=>{if(!value)throw Error(message);};
export function httpAPI(base){
 const url=new URL(base);requireThat(url.protocol==='http:'&&['127.0.0.1','localhost','[::1]'].includes(url.hostname)&&url.pathname==='/'&&!url.username&&!url.password,'Foundry must use an explicit loopback HTTP URL.');
 return async(route,body)=>{requireThat(route.startsWith('/')&&!route.startsWith('//'),'Invalid Foundry route.');const response=await fetch(new URL(route,url),{method:body?'POST':'GET',...(body?{headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}:{}),signal:AbortSignal.timeout(route==='/api/themes/resolve'?120000:15000),redirect:'error'});const reader=response.body.getReader();let length=0,chunks=[];while(true){const {value,done}=await reader.read();if(done)break;length+=value.byteLength;if(length>16*1024*1024){await reader.cancel();throw Error('Foundry response exceeds 16 MiB.');}chunks.push(value);}const bytes=Buffer.concat(chunks);if(!response.ok){let message;try{message=JSON.parse(bytes).error;}catch{}const e=Error(message||`Foundry HTTP ${response.status}`);e.status=response.status;throw e;}return route.startsWith('/evidence/')?bytes:JSON.parse(bytes);};
}
export function typedVerifier(binary){return async(job,received)=>{
 const input={op:'package',pipeline:{...job.pipeline,...(received?{approved:received}:{})},now:Date.now()};
 const {stdout}=await new Promise((resolve,reject)=>{const child=execFile(binary,[],{timeout:20000,maxBuffer:16*1024*1024},(error,stdout,stderr)=>error?reject(Error('First-party Rust package validation failed. Build the matching world-check binary.')):resolve({stdout,stderr}));child.stdin.on('error',()=>{});child.stdin.end(JSON.stringify(input));});
 return JSON.parse(stdout).value;
};}
export async function verifyPackage({job,api,verifyTyped,guard=()=>{}}){
 const checked=async operation=>{guard();try{const result=await operation();guard();return result;}catch(e){guard();throw e;}};
 requireThat(['accepted','active'].includes(job?.status)&&job.pipeline&&verifyTyped,'Foundry candidate is not accepted or verifier is unavailable.');
 const approved=await checked(()=>verifyTyped(job)),pkg=await checked(()=>api(`/api/jobs/${job.id}/package`)),comparable=structuredClone(pkg);delete comparable.manifest?.adapter;
 requireThat(isDeepStrictEqual(approved,comparable),'HTTP package differs from the first-party approved pipeline.');
 const typed=await checked(()=>verifyTyped(job,comparable));requireThat(isDeepStrictEqual(typed,comparable),'Typed package differs from the exported package.');
 requireThat(pkg.assetId===job.id&&pkg.manifest?.contentSha256===job.acceptedHash&&/^[a-f0-9]{64}$/.test(job.acceptedHash),'Accepted package identity/hash mismatch.');
 requireThat(pkg.manifest.rehearsal===(job.provider==='fixture'),'Package provenance changed.');validateRecipe(pkg.recipe,pkg.theme);requireThat((pkg.recipe.scene?.halfExtent||4)<=4&&pkg.recipe.parts.every(p=>{const diameter=p.shape==='cylinder'?Math.max(p.size[0],p.size[2]):p.shape==='cone'?p.size[0]:null;return Math.abs(p.position[0])+(diameter??p.size[0])/2<=4.05&&Math.abs(p.position[2])+(diameter??p.size[2])/2<=4.05;}),'Reviewed asset exceeds this app’s fixed 8 metre plot.');
 const evidence=pkg.manifest.evidence;requireThat(evidence?.length===3&&new Set(evidence.map(e=>e.sha256)).size===3,'Three distinct renderer images required.');
 const images=[];for(const item of evidence){requireThat(new RegExp(`^/evidence/${job.id}/\\d+-[0-2]\\.png$`).test(item.url),'Evidence image URL is outside the selected job.');const bytes=await checked(()=>api(item.url));requireThat(Buffer.isBuffer(bytes)&&bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))&&sha(bytes)===item.sha256,'Reviewed image bytes changed or invalid PNG.');images.push({sha256:item.sha256,bytes});}
 return {package:pkg,images};
}
const factionIdOf=b=>b.factionId||'landing';
const assignmentOf=b=>b.factionAssignment||1;
const factionOf=(w,id)=>w.factions?.find(f=>f.id===id)||(!w.factions&&id==='landing'?{id:'landing',revision:1,artDirection:'A coherent ivory ceramic robot settlement on a sunlit mineral coast.',theme:THEMES.argonaut}:null);
function targetsFor(w,kind,scope){
 const factionId=scope.factionId||'landing',faction=factionOf(w,factionId);
 if(!faction||(scope.factionRevision!==undefined&&scope.factionRevision!==faction.revision))return [];
 const targets=scope.targets||w.buildings.filter(b=>b.kind===kind&&factionIdOf(b)===factionId).map(b=>({id:b.id,assignment:assignmentOf(b)}));
 return w.buildings.filter(b=>b.kind===kind&&factionIdOf(b)===factionId&&targets.some(t=>t.id===b.id&&t.assignment===assignmentOf(b)));
}
export function activatePackage(w,kind,pkg,scope={}){
 const targets=targetsFor(w,kind,scope),hash=pkg.manifest.contentSha256;
 requireThat(targets.length,'No current building targets in this faction.');
 const changed=targets.filter(b=>b.asset?.hash!==hash);requireThat(changed.length,'These exact bytes are already active.');
 const asset={jobId:pkg.assetId,hash,rehearsal:pkg.manifest.rehearsal,recipe:pkg.recipe,theme:pkg.theme,manifest:pkg.manifest,at:w.time};
 w.assetHistory.push({kind,factionId:scope.factionId||'landing',hash,jobId:pkg.assetId,rehearsal:pkg.manifest.rehearsal,at:w.time,previousByTarget:changed.map(b=>({id:b.id,assignment:assignmentOf(b),asset:b.asset||null}))});
 for(const b of changed){b.asset=asset;if(b.kind==='creation')b.creationStatus='installed';}w.revision++;
 w.events.push({time:w.time,text:`${kind} appearance installed for ${changed.length} building${changed.length===1?'':'s'} in ${factionOf(w,scope.factionId||'landing')?.name||scope.factionId||'Landing'}${pkg.manifest.rehearsal?' (rehearsal)':''}.`});if(w.events.length>80)w.events.shift();
}
export function rollback(w,kind,factionId='landing'){
 const entry=w.assetHistory.findLast(e=>e.kind===kind&&(e.factionId||'landing')===factionId&&!Object.hasOwn(e,'rolledBackAt')&&Array.isArray(e.previousByTarget)&&e.previousByTarget.some(t=>w.buildings.some(b=>b.id===t.id&&b.kind===kind&&factionIdOf(b)===factionId&&assignmentOf(b)===t.assignment&&b.asset?.hash===e.hash)));
 requireThat(entry,'No matching prior appearance for this faction.');
 const restored=[];for(const previous of entry.previousByTarget){const b=w.buildings.find(b=>b.id===previous.id&&b.kind===kind&&factionIdOf(b)===factionId&&assignmentOf(b)===previous.assignment&&b.asset?.hash===entry.hash);if(!b)continue;if(previous.asset)b.asset=previous.asset;else delete b.asset;restored.push(b.id);}
 entry.rolledBackAt=w.time;entry.rollbackTargets=restored;w.revision++;
}
const capabilityContract='Write concise ASCII text: name <=100 characters, genre <=120, biome <=200, scale <=300, description <=1500. Keep each other descriptive field (including shapeLanguage) and each list entry <=400 characters. Use short phrases instead of long prose; check every field length before returning JSON. This family must fit individual 8m x 8m building plots, including supports and clear access; use compact human-scale dimensions, not monumental piers or multi-building complexes. Engine capability contract: meshes are axis-aligned boxes, vertical cylinders and vertical cones only, with three flat palette materials. There is NO per-part rotation, tilt, texture mapping, sphere primitive or real foliage simulation. Do not invent tilted/angled panel requirements, realistic textures, or other unsupported visual checks. Represent organic detail using explicit primitive clusters. Visual checks must remain faithful to the requested common art direction and these capabilities; do not require a particular building role or object in every member of the family.';
class SupersededRequest extends Error{}
export class FoundryController{
 constructor({world,save,api,verifyTyped,dataDir,mode='fixture'}){Object.assign(this,{world,save,api,verifyTyped,dataDir,mode});this.busy=false;if(world.foundry.error==='Twelve jobs retained. Automatic improvements paused at the session ceiling.'){world.foundry.error=null;world.foundry.enabled=true;save();}}
 scope(request){
  // Pin legacy work once. Never allow an old worldwide family request into a new faction.
  if(!request.factionId)request.factionId='landing';
  const faction=factionOf(this.world,request.factionId);
  if(request.factionRevision===undefined)request.factionRevision=faction?.revision;
  if(!request.targets)request.targets=this.world.buildings.filter(b=>b.kind===request.kind&&factionIdOf(b)===request.factionId&&(!request.targetId||b.id===request.targetId)).map(b=>({id:b.id,assignment:assignmentOf(b)}));
  return targetsFor(this.world,request.kind,request);
 }
 current(request){
  const active=this.world.foundry.requests.includes(request)&&['queued','preparing-theme','dispatching','unknown','running'].includes(request.status);
  if(!active)return false;
  if(this.scope(request).length)return true;
  request.status='cancelled';request.reason='Superseded: faction direction or building assignment changed.';this.save();return false;
 }
 guard(request){if(!this.current(request))throw new SupersededRequest('Request was superseded.');}
 async read(request,operation){this.guard(request);try{const result=await operation();this.guard(request);return result;}catch(e){this.guard(request);throw e;}}
 async cancelRemote(request){if(!request.jobId)return;try{await this.api(`/api/jobs/${request.jobId}/cancel`,{});request.remoteCancelled=true;}catch(e){request.remoteCancelError=e.message;}this.save();}
 async step(){if(this.busy)return;this.busy=true;const f=this.world.foundry;let request;
 try{
 request=f.requests.find(r=>['dispatching','unknown','running'].includes(r.status))||f.requests.find(r=>r.status==='queued'&&(r.explicit||r.targetId))||f.requests.find(r=>r.status==='queued'&&f.enabled);if(!request&&!f.enabled)return;
 if(!request){
 const building=this.world.buildings.find(b=>b.kind!=='creation'&&b.progress===1&&!f.requests.some(r=>r.kind===b.kind&&(r.factionId||'landing')===factionIdOf(b)&&r.round===f.round&&(r.factionRevision??1)===(factionOf(this.world,factionIdOf(b))?.revision??1)));if(!building)return;
 const faction=factionOf(this.world,factionIdOf(building));requireThat(faction,'Building faction is missing.');
 request={id:`solo-${this.world.id}-${faction.id}-${building.kind}-${faction.revision}-${f.round}`,kind:building.kind,factionId:faction.id,factionRevision:faction.revision,targets:this.world.buildings.filter(b=>b.kind===building.kind&&b.progress===1&&factionIdOf(b)===faction.id).map(b=>({id:b.id,assignment:assignmentOf(b)})),artDirection:faction.artDirection,round:f.round,status:'queued',provider:this.mode};request.prompt=`Single-player colony / ${request.id}. Refine this ${request.kind} building in the faction's common design language: ${faction.artDirection}. Pass ${f.round}: improve distinctive silhouette, purposeful functional detail, material separation and grounded construction. Fit the fixed 8 metre plot and preserve traversable access. No units or gameplay changes.`;f.requests.push(request);this.save();
 }
 if(!this.current(request)){await this.cancelRemote(request);return;}
 if(['unknown','dispatching'].includes(request.status)){
 const jobs=await this.read(request,()=>this.api('/api/jobs'));const matches=jobs.filter(j=>j.prompt===request.prompt&&j.provider===request.provider);if(matches.length===1){request.jobId=matches[0].id;request.status='running';this.save();}else{request.status='unknown';f.error='Dispatch outcome unknown. Reconcile in Foundry; no automatic paid retry.';this.save();return;}
 }
 if(request.status==='queued'){
 if(!request.theme){
 const faction=factionOf(this.world,request.factionId);
 if(faction.theme&&!(request.targetId&&!this.world.factions)){request.theme=structuredClone(faction.theme);request.themeDraftId=faction.themeDraftId;}
 else{
 request.status='preparing-theme';this.save();
 try{const draft=await this.read(request,()=>this.api('/api/themes/resolve',{prompt:`Create a reusable architectural family style for the following art direction. Describe common shape language, materials, construction details and visual quality across different building purposes.\n${request.artDirection||faction.artDirection||request.userPrompt}\n${capabilityContract}`}));request.theme=validateTheme(draft.theme);request.themeDraftId=draft.draftId;if(this.world.factions){const currentFaction=factionOf(this.world,request.factionId);currentFaction.theme=structuredClone(request.theme);currentFaction.themeDraftId=draft.draftId;}request.status='queued';this.save();}
 catch(e){if(e instanceof SupersededRequest)throw e;request.status='blocked';request.reason='Live artistic contract could not be resolved: '+e.message;f.error=request.reason;this.save();return;}
 }
 }
 this.guard(request);request.status='dispatching';this.save();let job;
 try{job=await this.api('/api/jobs',{provider:request.provider,theme:request.theme,themeId:request.theme.id,sceneHalfExtent:4,prompt:request.prompt,injectDefect:false});}
 catch(e){if(!this.current(request)){request.dispatchOutcome='unknown';request.reconciliationRequired=true;this.save();return;}if(e.cause?.code==='ECONNREFUSED'||e.cause?.errors?.every(error=>error.code==='ECONNREFUSED')){request.status='queued';request.transportError='ECONNREFUSED';f.error='Foundry is offline. Waiting for the local service; no generation was dispatched.';}else if(e.status===409){request.status='queued';f.error='Foundry is busy. Colony play continues; waiting for its current job.';}else if(e.status&&e.status<500){request.status='blocked';request.reason=e.message;f.error=e.message;}else{request.status='unknown';f.error=e.message;}this.save();return;}
 requireThat(typeof job.id==='string'&&/^[a-z0-9-]{1,128}$/.test(job.id),'Invalid Foundry job identity.');request.jobId=job.id;
 if(!this.current(request)){this.save();await this.cancelRemote(request);return;}
 request.status='running';f.error=null;this.save();
 }
 if(request.status==='running'){
 const job=await this.read(request,()=>this.api(`/api/jobs/${request.jobId}`));requireThat(job.id===request.jobId&&job.prompt===request.prompt&&job.provider===request.provider,'Foundry job contract changed.');request.stage=job.status;
 if(['accepted','active'].includes(job.status)){
 const checked=await verifyPackage({job,api:this.api,verifyTyped:this.verifyTyped,guard:()=>this.guard(request)});
 this.guard(request);
 if(this.dataDir){const folder=path.join(this.dataDir,'assets',checked.package.manifest.contentSha256);fs.mkdirSync(folder,{recursive:true,mode:0o700});for(const image of checked.images)fs.writeFileSync(path.join(folder,image.sha256+'.png'),image.bytes,{mode:0o600});fs.writeFileSync(path.join(folder,'package.json'),JSON.stringify(checked.package),{mode:0o600});fs.writeFileSync(path.join(folder,'job.json'),JSON.stringify(job),{mode:0o600});}
 if(this.scope(request).some(b=>b.asset?.hash!==checked.package.manifest.contentSha256))activatePackage(this.world,request.kind,checked.package,request);request.status='installed';request.hash=checked.package.manifest.contentSha256;request.rehearsal=checked.package.manifest.rehearsal;request.installedTargets=this.scope(request).map(b=>b.id);f.error=null;
 }else if(['failed','blocked','cancelled','exhausted'].includes(job.status)){request.status=job.status;request.reason=job.stopReason;f.error=job.stopReason||`Foundry job ${job.status}.`;}
 for(const target of this.scope(request))if(target.kind==='creation')target.creationStatus=request.status;this.save();
 }
 }catch(e){if(e instanceof SupersededRequest){if(request)await this.cancelRemote(request);return;}f.error=e.message;if(request&&this.current(request)){request.status='blocked';request.reason=e.message;}this.save();}finally{this.busy=false;}}
}
