/** Language-neutral Foundry adapter. Only accepted, independently checked packages enter a colony. */
import {createHash} from 'node:crypto';
import {execFile} from 'node:child_process';
import {isDeepStrictEqual} from 'node:util';
import fs from 'node:fs';
import path from 'node:path';
import {validateRecipe,validateTheme,THEMES} from '../public/core.mjs';
import {adoptFactionDirection} from './model.mjs';
const sha=b=>createHash('sha256').update(b).digest('hex');
const requireThat=(value,message)=>{if(!value)throw Error(message);};
export function httpAPI(base){
 const url=new URL(base);requireThat(url.protocol==='http:'&&['127.0.0.1','localhost','[::1]'].includes(url.hostname)&&url.pathname==='/'&&!url.username&&!url.password,'Foundry must use an explicit loopback HTTP URL.');
 return async(route,body)=>{requireThat(route.startsWith('/')&&!route.startsWith('//'),'Invalid Foundry route.');const response=await fetch(new URL(route,url),{method:body?'POST':'GET',...(body?{headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}:{}),signal:AbortSignal.timeout(route==='/api/themes/resolve'?120000:15000),redirect:'error'});const reader=response.body.getReader();let length=0,chunks=[];while(true){const {value,done}=await reader.read();if(done)break;length+=value.byteLength;if(length>16*1024*1024){await reader.cancel();throw Error('Foundry response exceeds 16 MiB.');}chunks.push(value);}const bytes=Buffer.concat(chunks);if(!response.ok){let details;try{details=JSON.parse(bytes);}catch{}const e=Error(details?.error||`Foundry HTTP ${response.status}`);e.status=response.status;if(details?.reconciliationRequired)e.reconciliationRequired=true;if(details?.draftId)e.draftId=details.draftId;throw e;}return route.startsWith('/evidence/')?bytes:JSON.parse(bytes);};
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
// Only public job summary fields belong in the colony; never copy provider responses or usage.
const progressText=value=>{
 const text=typeof value==='string'?value:Array.isArray(value)?value.filter(v=>typeof v==='string').slice(0,20).join('; '):'';
 return text.replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g,'').replace(/AIza[\w-]{20,}/g,'[redacted]').replace(/Bearer\s+[^\s,;]+/gi,'Bearer [redacted]').replace(/((?:api[_ -]?key|access[_ -]?token|authorization)\s*[:=]\s*)[^\s,;]+/gi,'$1[redacted]').replace(/((?:provider|response|HTTP)[^{}\n]*)\{[\s\S]*$/i,'$1[response body omitted]').trim().slice(0,500);
};
const isoTime=value=>{if(typeof value!=='string'||value.length>64)return null;const time=Date.parse(value);return Number.isFinite(time)?new Date(time).toISOString():null;};
const progressCount=value=>Number.isSafeInteger(value)&&value>=0?value:null;
const familyOnlyInference='Shared style for DIFFERENT building functions. Extract reusable construction/material language, e.g. stepped/layered profiles, dark timber joints, warm infill. Object names are examples, never required components. Neither require NOR prohibit purpose-specific equipment: sports or solar. Never require a hut, cabin, cottage, chimney or woodstore. gameplayRole preserves each target function from its own brief, not the source role. Shared exclusions cover renderer capabilities only; visualChecks assess building parts only.';
const familyCapability='ASCII field limits: name100, genre120, biome200, scale300, description1500; other text/list entries400 characters. Compact 8m x 8m building plots, grounded supports, clear access. Axis-aligned boxes, vertical cylinders/cones only; no tilt, rotation, textures, spheres or simulated foliage. Building-part colors/visualChecks use ONLY exact palette.shell/frame/accent. palette.ground/rock are ENVIRONMENT ONLY, never building colors.';
function validateSharedTheme(theme){
 const partColors=new Set(['shell','frame','accent'].map(key=>theme.palette[key].toLowerCase()));
 for(const check of theme.visualChecks)requireThat(!/\b(?:huts?|cabins?|cottages?|chimneys?|wood[ -]?stores?)\b/i.test(check),'Shared faction visual checks require source-specific domestic components. Resolve reusable construction and materials before generation.');
 for(const check of theme.visualChecks){for(const color of check.match(/#[0-9a-f]{3,8}\b/gi)||[]){const normalized=color.length===4?'#'+[...color.slice(1)].map(c=>c+c).join(''):color;requireThat(partColors.has(normalized.toLowerCase()),`Shared faction visual check requires unavailable building color ${color}; use only palette.shell/frame/accent. Resolve a new family before generation.`);}}
 for(const exclusion of theme.exclusions)requireThat(!/\b(?:sports?|basketball|courts?|hoops?|backboards?|solar|photovoltaic|collectors?)\b/i.test(exclusion),'Shared faction exclusions conflict with supported sports or solar functions. Resolve a role-neutral family before generation.');
}
function themeInferencePrompt(request,faction){
 if(request.drivesFactionDirection)return `Infer an original art direction from the complete user brief below. Choose distinctive silhouette, composition, palette, material treatment and memorable functional details freely; no previous faction style constrains this design. Preserve the requested purpose and explicit details. Return a valid theme contract with concrete renderable visual checks.\n${familyCapability}\nUser brief: ${request.userPrompt||request.artDirection}`;
 return `${familyOnlyInference}\n${familyCapability}\nFaction art direction: ${request.artDirection||faction.artDirection||request.userPrompt}`;
}
class SupersededRequest extends Error{}
export class FoundryController{
 constructor({world,save,api,verifyTyped,dataDir,mode='fixture'}){Object.assign(this,{world,api,verifyTyped,dataDir,mode});this.save=()=>{for(const building of world.buildings.filter(b=>b.kind==='creation')){const request=world.foundry.requests.findLast(r=>r.targetId===building.id);if(request)building.creationStatus=request.status;}save();};this.busy=false;if(world.foundry.error==='Twelve jobs retained. Automatic improvements paused at the session ceiling.'){world.foundry.error=null;world.foundry.enabled=true;this.save();}}
 progress(request,job=null,detail){
  const old=request.progress||{},now=new Date().toISOString(),events=Array.isArray(job?.events)?job.events:[],attempts=Array.isArray(job?.attempts)?job.attempts:[];
  request.startedAt=isoTime(job?.createdAt)||isoTime(request.startedAt)||now;
  const latestEvent=events.findLast(e=>typeof e?.message==='string'),review=attempts.findLast(a=>progressText(a?.review?.comparison?.summary)||progressText(a?.review?.findings))?.review;
  const numbers=attempts.map(a=>progressCount(a?.number));for(const event of events){if(['generating','repairing','validating'].includes(event?.state)){const number=typeof event.message==='string'?event.message.match(/candidate\s+(\d+)/i)?.[1]:null;if(number)numbers.push(progressCount(Number(number)));}}
  const known=numbers.filter(n=>n!==null),terminal=['installed','blocked','failed','cancelled','exhausted','unknown'].includes(request.status);
  const stage=terminal?request.status:job?.status||(request.status==='running'?(old.stage||request.stage||request.status):request.status);
  const latestAttempt=attempts.at(-1),failedCheck=Array.isArray(latestAttempt?.checks)?latestAttempt.checks.filter(check=>check?.status==='fail').map(check=>check.detail):[];
  const observedFailure=['blocked','failed','cancelled','exhausted'].includes(job?.status)?progressText(latestAttempt?.validationError)||progressText(failedCheck):'';
  const summary=detail??(observedFailure||null)??(terminal?(old.stage===request.status&&old.detail?old.detail:request.reason||(request.status==='installed'?'Accepted design installed on its current targets.':null)):null)??latestEvent?.message??old.detail;
  request.progress={jobId:progressText(request.jobId||job?.id||old.jobId)||null,stage:progressText(stage),attempt:known.length?Math.max(...known):progressCount(old.attempt),calls:progressCount(job?.calls)??progressCount(old.calls),startedAt:request.startedAt,observedAt:now,detail:progressText(summary),critique:progressText(review?.comparison?.summary)||progressText(review?.findings)||progressText(old.critique),repair:progressText(events.findLast(e=>e?.state==='repairing')?.message)||progressText(old.repair),acceptedHash:[request.hash,job?.acceptedHash,old.acceptedHash].find(hash=>typeof hash==='string'&&/^[a-f0-9]{64}$/.test(hash))||null};
 }
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
 sameFamily(a,b){return a!==b&&a.kind===b.kind&&(a.factionId||'landing')===(b.factionId||'landing')&&(a.adoptedFactionRevision??a.factionRevision)===(b.adoptedFactionRevision??b.factionRevision);}
 reusableSource(request,source){
  if(!source||source.reuseFromRequestId||source.status!=='installed'||!this.sameFamily(request,source)||typeof source.jobId!=='string'||!/^[a-f0-9]{64}$/.test(source.hash||''))return false;
  if(factionOf(this.world,request.factionId)?.revision!==(source.adoptedFactionRevision??source.factionRevision))return false;
  if(this.world.assetHistory.some(entry=>entry.kind===source.kind&&(entry.factionId||'landing')===source.factionId&&entry.hash===source.hash&&Object.hasOwn(entry,'rolledBackAt')))return false;
  return this.world.buildings.some(b=>b.kind===source.kind&&factionIdOf(b)===source.factionId&&b.asset?.hash===source.hash&&b.asset?.jobId===source.jobId);
 }
 reuseSource(request){
  const source=this.world.foundry.requests.find(r=>r.id===request.reuseFromRequestId);
  requireThat(request.preferExistingDesign&&request.kind!=='creation'&&this.reusableSource(request,source)&&request.jobId===source.jobId&&request.sourceHash===source.hash&&request.sourcePrompt===source.prompt&&request.sourceProvider===source.provider,'Existing approved design is no longer eligible for reuse.');return source;
 }
 prepareReuse(request){
  const requests=this.world.foundry.requests,source=requests.findLast(r=>this.reusableSource(request,r));
  if(source){request.reuseFromRequestId=source.id;request.sourcePrompt=source.prompt;request.sourceProvider=source.provider;request.sourceHash=source.hash;request.jobId=source.jobId;request.status='running';request.reason='Rechecking the existing approved faction design.';this.save();return true;}
  const prior=requests.findLast(r=>this.sameFamily(request,r)&&!['queued','preparing-theme','dispatching','running'].includes(r.status));
  if(prior){request.status='blocked';request.reason=`Existing faction design cannot be inherited: ${prior.reason||prior.status}. Update the faction explicitly to request new art.`;this.world.foundry.error=request.reason;this.save();return false;}
  const pending=requests.find(r=>this.sameFamily(request,r)&&['queued','preparing-theme','dispatching','running'].includes(r.status));
  // An earlier queued request may be deliberately paused. Do not turn it into duplicate paid work.
  if(pending&&requests.indexOf(pending)<requests.indexOf(request)){request.reason='Waiting for the existing faction design request.';this.save();return false;}
  return true;
 }
 guard(request){if(!this.current(request))throw new SupersededRequest('Request was superseded.');if(request.reuseFromRequestId)this.reuseSource(request);}
 async read(request,operation){this.guard(request);try{const result=await operation();this.guard(request);return result;}catch(e){this.guard(request);throw e;}}
 async cancelRemote(request){if(!request.jobId||request.reuseFromRequestId)return;try{await this.api(`/api/jobs/${request.jobId}/cancel`,{});request.remoteCancelled=true;}catch(e){request.remoteCancelError=e.message;}this.save();}
 async backfillProgress(){
  const request=this.world.foundry.requests.findLast(r=>['installed','blocked','failed','cancelled','exhausted'].includes(r.status)&&typeof r.jobId==='string'&&/^[a-z0-9-]{1,128}$/.test(r.jobId));
  if(!request||request.progress)return false;
  // One read for the latest finished job when upgrading an older save; never replay its work.
  const jobId=request.jobId;this.progress(request);this.save();
  try{const job=await this.api(`/api/jobs/${jobId}`);const prompt=request.reuseFromRequestId?request.sourcePrompt:request.prompt,provider=request.reuseFromRequestId?request.sourceProvider:request.provider;requireThat(job.id===jobId&&job.prompt===prompt&&job.provider===provider,'Saved Foundry job contract changed.');if(this.world.foundry.requests.includes(request)&&request.jobId===jobId){this.progress(request,job);this.save();}}
  catch{if(this.world.foundry.requests.includes(request)&&request.jobId===jobId){this.progress(request,null,request.reason||'Saved job details are unavailable.');this.save();}}return true;
 }
 async step(){if(this.busy)return;this.busy=true;const f=this.world.foundry;let request;
 try{
 request=f.requests.find(r=>['dispatching','unknown','running'].includes(r.status))||f.requests.find(r=>r.status==='queued'&&(r.explicit||r.targetId))||f.requests.find(r=>r.status==='queued'&&f.enabled);if(!request&&await this.backfillProgress())return;if(!request&&!f.enabled)return;
 if(!request){
 const building=this.world.buildings.find(b=>b.kind!=='creation'&&b.progress===1&&!f.requests.some(r=>r.kind===b.kind&&(r.factionId||'landing')===factionIdOf(b)&&r.round===f.round&&(r.factionRevision??1)===(factionOf(this.world,factionIdOf(b))?.revision??1)));if(!building)return;
 const faction=factionOf(this.world,factionIdOf(building));requireThat(faction,'Building faction is missing.');
 request={id:`solo-${this.world.id}-${faction.id}-${building.kind}-${faction.revision}-${f.round}`,kind:building.kind,factionId:faction.id,factionRevision:faction.revision,targets:this.world.buildings.filter(b=>b.kind===building.kind&&b.progress===1&&factionIdOf(b)===faction.id).map(b=>({id:b.id,assignment:assignmentOf(b)})),artDirection:faction.artDirection,round:f.round,status:'queued',provider:this.mode};request.prompt=`Single-player colony / ${request.id}. Refine this ${request.kind} building in the faction's common design language: ${faction.artDirection}. Pass ${f.round}: improve distinctive silhouette, purposeful functional detail, material separation and grounded construction. Fit the fixed 8 metre plot and preserve traversable access. No units or gameplay changes.`;f.requests.push(request);this.save();
 }
 this.progress(request);this.save();
 if(!this.current(request)){await this.cancelRemote(request);return;}
 if(['unknown','dispatching'].includes(request.status)){
 const jobs=await this.read(request,()=>this.api('/api/jobs'));const matches=jobs.filter(j=>j.prompt===request.prompt&&j.provider===request.provider);if(matches.length===1){request.jobId=matches[0].id;request.status='running';this.save();}else{request.status='unknown';f.error='Dispatch outcome unknown. Reconcile in Foundry; no automatic paid retry.';this.save();return;}
 }
 if(request.status==='queued'&&request.preferExistingDesign&&request.kind!=='creation'&&!this.prepareReuse(request))return;
 if(request.status==='queued'){
 request.startedAtMs??=Date.now();this.save();
 if(!request.theme){
 const faction=factionOf(this.world,request.factionId);
 if(!request.drivesFactionDirection&&faction.theme&&!(request.targetId&&!this.world.factions)){request.theme=structuredClone(faction.theme);request.themeDraftId=faction.themeDraftId;}
 else{
 request.status='preparing-theme';this.progress(request,null,'Resolving the faction art direction.');this.save();let themeResponseReceived=false;
 try{const draft=await this.read(request,()=>this.api('/api/themes/resolve',{prompt:themeInferencePrompt(request,faction)}));themeResponseReceived=true;request.theme=validateTheme(draft.theme);request.themeDraftId=draft.draftId;if(!request.drivesFactionDirection)validateSharedTheme(request.theme);if(this.world.factions&&!request.drivesFactionDirection){const currentFaction=factionOf(this.world,request.factionId);currentFaction.theme=structuredClone(request.theme);currentFaction.themeDraftId=draft.draftId;}request.status='queued';this.save();}
 catch(e){if(e instanceof SupersededRequest)throw e;const refused=e.cause?.code==='ECONNREFUSED'||(e.cause?.errors?.length&&e.cause.errors.every(error=>error.code==='ECONNREFUSED'));if(e.reconciliationRequired||(!themeResponseReceived&&!refused&&(!e.status||e.status>=500)))request.reconciliationRequired=true;if(e.draftId)request.themeDraftId=e.draftId;request.status='blocked';request.reason='Live artistic contract could not be resolved: '+e.message+(request.reconciliationRequired?' Paid outcome needs reconciliation before another attempt.':'');f.error=request.reason;this.save();return;}
 }
 }
 if(!request.drivesFactionDirection)validateSharedTheme(request.theme);
 const timeBudgetMs=0; // The frozen attempt and call ceilings bound work; latency remains visible per phase.
 this.guard(request);request.status='dispatching';this.progress(request,null,'Submitting the frozen building request.');this.save();let job;
 try{job=await this.api('/api/jobs',{timeBudgetMs,provider:request.provider,theme:request.theme,themeId:request.theme.id,sceneHalfExtent:4,prompt:request.prompt,injectDefect:false});}
 catch(e){if(!this.current(request)){request.dispatchOutcome='unknown';request.reconciliationRequired=true;this.save();return;}if(e.cause?.code==='ECONNREFUSED'||e.cause?.errors?.every(error=>error.code==='ECONNREFUSED')){request.status='queued';request.transportError='ECONNREFUSED';f.error='Foundry is offline. Waiting for the local service; no generation was dispatched.';}else if(e.status===409){request.status='queued';f.error='Foundry is busy. Colony play continues; waiting for its current job.';}else if(e.status&&e.status<500){request.status='blocked';request.reason=e.message;f.error=e.message;}else{request.status='unknown';f.error=e.message;}this.save();return;}
 requireThat(typeof job.id==='string'&&/^[a-z0-9-]{1,128}$/.test(job.id),'Invalid Foundry job identity.');request.jobId=job.id;
 if(!this.current(request)){this.save();await this.cancelRemote(request);return;}
 request.status='running';f.error=null;this.save();
 }
 if(request.status==='running'){
 const job=await this.read(request,()=>this.api(`/api/jobs/${request.jobId}`));this.progress(request,job);this.save();const source=request.reuseFromRequestId?this.reuseSource(request):request;requireThat(job.id===request.jobId&&job.prompt===source.prompt&&job.provider===source.provider,'Foundry job contract changed.');if(request.reuseFromRequestId)requireThat(job.acceptedHash===request.sourceHash,'Existing approved design hash changed.');request.stage=job.status;request.timings={...request.timings,...job.timings};const candidate=job.attempts?.at(-1);if(candidate?.images?.length===3&&candidate.hash)request.preview={candidateNumber:candidate.number,candidateHash:candidate.hash,images:candidate.images,approved:false};
 if(['accepted','active'].includes(job.status)){
 const installingAt=Date.now();const checked=await verifyPackage({job,api:this.api,verifyTyped:this.verifyTyped,guard:()=>this.guard(request)});
 this.guard(request);
 if(this.dataDir){const folder=path.join(this.dataDir,'assets',checked.package.manifest.contentSha256);fs.mkdirSync(folder,{recursive:true,mode:0o700});for(const image of checked.images)fs.writeFileSync(path.join(folder,image.sha256+'.png'),image.bytes,{mode:0o600});fs.writeFileSync(path.join(folder,'package.json'),JSON.stringify(checked.package),{mode:0o600});fs.writeFileSync(path.join(folder,'job.json'),JSON.stringify(job),{mode:0o600});}
 if(this.scope(request).some(b=>b.asset?.hash!==checked.package.manifest.contentSha256))activatePackage(this.world,request.kind,checked.package,request);request.status='installed';request.timings={...request.timings,installationMs:Date.now()-installingAt,totalMs:Date.now()-request.startedAtMs};request.hash=checked.package.manifest.contentSha256;request.rehearsal=checked.package.manifest.rehearsal;request.installedTargets=this.scope(request).map(b=>b.id);adoptFactionDirection(this.world,request,checked.package.theme,checked.package.manifest.contentSha256);if(request.reuseFromRequestId)request.reason='Approved existing faction design reused after exact package and image verification; no new generation call.';f.error=null;
 }else if(['failed','blocked','cancelled','exhausted'].includes(job.status)){request.status=job.status;request.reason=job.stopReason;f.error=job.stopReason||`Foundry job ${job.status}.`;}
 for(const target of this.scope(request))if(target.kind==='creation')target.creationStatus=request.status;this.save();
 }
 }catch(e){if(e instanceof SupersededRequest){if(request)await this.cancelRemote(request);return;}f.error=e.message;if(request&&this.current(request)){request.status='blocked';request.reason=e.message;}this.save();}finally{try{if(request){this.progress(request);this.save();}}finally{this.busy=false;}}}
}
