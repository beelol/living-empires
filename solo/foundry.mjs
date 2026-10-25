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
 return async(route,body)=>{requireThat(route.startsWith('/')&&!route.startsWith('//'),'Invalid Foundry route.');const response=await fetch(new URL(route,url),{method:body?'POST':'GET',...(body?{headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}:{}),signal:AbortSignal.timeout(15000),redirect:'error'});const reader=response.body.getReader();let length=0,chunks=[];while(true){const {value,done}=await reader.read();if(done)break;length+=value.byteLength;if(length>16*1024*1024){await reader.cancel();throw Error('Foundry response exceeds 16 MiB.');}chunks.push(value);}const bytes=Buffer.concat(chunks);if(!response.ok){let message;try{message=JSON.parse(bytes).error;}catch{}const e=Error(message||`Foundry HTTP ${response.status}`);e.status=response.status;throw e;}return route.startsWith('/evidence/')?bytes:JSON.parse(bytes);};
}
export function typedVerifier(binary){return async(job,received)=>{
 const input={op:'package',pipeline:{...job.pipeline,...(received?{approved:received}:{})},now:Date.now()};
 const {stdout}=await new Promise((resolve,reject)=>{const child=execFile(binary,[],{timeout:20000,maxBuffer:16*1024*1024},(error,stdout,stderr)=>error?reject(Error('First-party Rust package validation failed. Build the matching world-check binary.')):resolve({stdout,stderr}));child.stdin.on('error',()=>{});child.stdin.end(JSON.stringify(input));});
 return JSON.parse(stdout).value;
};}
export async function verifyPackage({job,api,verifyTyped}){
 requireThat(['accepted','active'].includes(job?.status)&&job.pipeline&&verifyTyped,'Foundry candidate is not accepted or verifier is unavailable.');
 const approved=await verifyTyped(job),pkg=await api(`/api/jobs/${job.id}/package`),comparable=structuredClone(pkg);delete comparable.manifest?.adapter;
 requireThat(isDeepStrictEqual(approved,comparable),'HTTP package differs from the first-party approved pipeline.');
 const typed=await verifyTyped(job,comparable);requireThat(isDeepStrictEqual(typed,comparable),'Typed package differs from the exported package.');
 requireThat(pkg.assetId===job.id&&pkg.manifest?.contentSha256===job.acceptedHash&&/^[a-f0-9]{64}$/.test(job.acceptedHash),'Accepted package identity/hash mismatch.');
 requireThat(pkg.manifest.rehearsal===(job.provider==='fixture'),'Package provenance changed.');validateRecipe(pkg.recipe,pkg.theme);requireThat((pkg.recipe.scene?.halfExtent||4)<=4&&pkg.recipe.parts.every(p=>{const diameter=p.shape==='cylinder'?Math.max(p.size[0],p.size[2]):p.shape==='cone'?p.size[0]:null;return Math.abs(p.position[0])+(diameter??p.size[0])/2<=4.05&&Math.abs(p.position[2])+(diameter??p.size[2])/2<=4.05;}),'Reviewed asset exceeds this app’s fixed 8 metre plot.');
 const evidence=pkg.manifest.evidence;requireThat(evidence?.length===3&&new Set(evidence.map(e=>e.sha256)).size===3,'Three distinct renderer images required.');
 const images=[];for(const item of evidence){requireThat(new RegExp(`^/evidence/${job.id}/\\d+-[0-2]\\.png$`).test(item.url),'Evidence image URL is outside the selected job.');const bytes=await api(item.url);requireThat(Buffer.isBuffer(bytes)&&bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))&&sha(bytes)===item.sha256,'Reviewed image bytes changed or invalid PNG.');images.push({sha256:item.sha256,bytes});}
 return {package:pkg,images};
}
export function activatePackage(w,kind,pkg){
 requireThat(w.buildings.some(b=>b.kind===kind),'No building family to upgrade.');const hash=pkg.manifest.contentSha256;
 requireThat(w.assets[kind]?.hash!==hash,'These exact bytes are already active.');
 w.assetHistory.push({kind,hash,rehearsal:pkg.manifest.rehearsal,at:w.time,previous:w.assets[kind]||null});if(w.assetHistory.length>24)w.assetHistory.shift();
 w.assets[kind]={hash,rehearsal:pkg.manifest.rehearsal,recipe:pkg.recipe,theme:pkg.theme,manifest:pkg.manifest,at:w.time};w.revision++;
 w.events.push({time:w.time,text:`${kind} visual upgrade installed${pkg.manifest.rehearsal?' (rehearsal)':''}.`});if(w.events.length>80)w.events.shift();
}
export function rollback(w,kind){const index=w.assetHistory.findLastIndex(e=>e.kind===kind);requireThat(index>=0,'No prior appearance for this family.');const entry=w.assetHistory[index];if(entry.previous)w.assets[kind]=entry.previous;else delete w.assets[kind];w.assetHistory.splice(index,1);w.revision++;}
export class FoundryController{
 constructor({world,save,api,verifyTyped,dataDir,mode='fixture'}){Object.assign(this,{world,save,api,verifyTyped,dataDir,mode});this.busy=false;}
 async step(){if(this.busy)return;this.busy=true;const f=this.world.foundry;
 try{
 let request=f.requests.find(r=>['dispatching','unknown','running'].includes(r.status))||f.requests.find(r=>r.status==='queued'&&(r.targetId||f.enabled));if(!request&&!f.enabled)return;
 if(!request){
 const kind=this.world.buildings.find(b=>b.kind!=='creation'&&b.progress===1&&!f.requests.some(r=>r.kind===b.kind&&r.round===f.round))?.kind;if(!kind)return;
 if(f.requests.length>=12){f.error='Twelve jobs retained. Automatic improvements paused at the session ceiling.';f.enabled=false;this.save();return;}
 request={id:`solo-${this.world.id}-${kind}-${f.round}`,kind,round:f.round,status:'queued',provider:this.mode};request.prompt=`Single-player colony / ${request.id}. Refine this ${kind} building for a coherent ivory ceramic robot settlement on a sunlit mineral coast. Pass ${f.round}: improve silhouette, functional detail, material separation and ground contact. Keep the existing traversable outpost contract and frozen footprint. No units or gameplay changes.`;f.requests.push(request);this.save();
 }
 if(['unknown','dispatching'].includes(request.status)){
 const jobs=await this.api('/api/jobs');const matches=jobs.filter(j=>j.prompt===request.prompt&&j.provider===request.provider);if(matches.length===1){request.jobId=matches[0].id;request.status='running';this.save();}else{request.status='unknown';f.error='Dispatch outcome unknown. Reconcile in Foundry; no automatic paid retry.';this.save();return;}
 }
 if(request.status==='queued'){
 if(request.targetId&&!request.theme){
 request.status='preparing-theme';this.save();
 try{const draft=await this.api('/api/themes/resolve',{prompt:request.userPrompt});request.theme=validateTheme(draft.theme);request.themeDraftId=draft.draftId;request.status='queued';this.save();}
 catch(e){request.status='blocked';request.reason='Live artistic contract could not be resolved: '+e.message;f.error=request.reason;this.save();return;}
 }
 request.status='dispatching';this.save();let job;try{job=await this.api('/api/jobs',{provider:request.provider,themeId:'argonaut',sceneHalfExtent:4,...(request.targetId?{theme:request.theme,themeId:request.theme.id}:{} ),prompt:request.prompt,injectDefect:false});}catch(e){if(e.cause?.code==='ECONNREFUSED'||e.cause?.errors?.every(error=>error.code==='ECONNREFUSED')){request.status='queued';request.transportError='ECONNREFUSED';f.error='Foundry is offline. Waiting for the local service; no generation was dispatched.';}else if(e.status===409){request.status='queued';f.error='Foundry is busy. Colony play continues; waiting for its current job.';}else if(e.status&&e.status<500){request.status='blocked';f.error=e.message;}else{request.status='unknown';f.error=e.message;}this.save();return;}
 requireThat(typeof job.id==='string'&&/^[a-z0-9-]{1,128}$/.test(job.id),'Invalid Foundry job identity.');request.jobId=job.id;request.status='running';f.error=null;this.save();
 }
 if(request.status==='running'){
 const job=await this.api(`/api/jobs/${request.jobId}`);requireThat(job.id===request.jobId&&job.prompt===request.prompt&&job.provider===request.provider,'Foundry job contract changed.');request.stage=job.status;
 if(['accepted','active'].includes(job.status)){
 const checked=await verifyPackage({job,api:this.api,verifyTyped:this.verifyTyped});
 // A user can cancel while the independent verifier or image downloads await.
 if(request.status!=='running'||!f.requests.includes(request)||request.jobId!==job.id)return;
 if(this.dataDir){const folder=path.join(this.dataDir,'assets',checked.package.manifest.contentSha256);fs.mkdirSync(folder,{recursive:true,mode:0o700});for(const image of checked.images)fs.writeFileSync(path.join(folder,image.sha256+'.png'),image.bytes,{mode:0o600});fs.writeFileSync(path.join(folder,'package.json'),JSON.stringify(checked.package),{mode:0o600});fs.writeFileSync(path.join(folder,'job.json'),JSON.stringify(job),{mode:0o600});}
 if(request.targetId){const target=this.world.buildings.find(b=>b.id===request.targetId);requireThat(target&&target.kind==='creation','Creation target no longer exists.');const pkg=checked.package;target.asset={hash:pkg.manifest.contentSha256,recipe:pkg.recipe,theme:pkg.theme,manifest:pkg.manifest,rehearsal:pkg.manifest.rehearsal};target.creationStatus='installed';this.world.revision++;}else if(this.world.assets[request.kind]?.hash!==checked.package.manifest.contentSha256)activatePackage(this.world,request.kind,checked.package);request.status='installed';request.hash=checked.package.manifest.contentSha256;request.rehearsal=checked.package.manifest.rehearsal;f.error=null;
 }else if(['failed','blocked','cancelled','exhausted'].includes(job.status)){request.status=job.status;request.reason=job.stopReason;f.error=job.stopReason||`Foundry job ${job.status}.`;}
 if(request.targetId){const target=this.world.buildings.find(b=>b.id===request.targetId);if(target)target.creationStatus=request.status;}this.save();
 }
 }catch(e){f.error=e.message;const active=f.requests.find(r=>r.status==='running');if(active){active.status='blocked';active.reason=e.message;}this.save();}finally{this.busy=false;}}
}
