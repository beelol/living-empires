import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {createRequire} from 'node:module';
import {randomUUID,createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {command,advance,publicState,creationRequest} from './model.mjs';
import {openStore} from './store.mjs';
import {FoundryController,httpAPI,typedVerifier,rollback} from './foundry.mjs';
const root=path.dirname(fileURLToPath(import.meta.url)),require=createRequire(import.meta.url);
const sourceSha256=createHash('sha256').update(['server.mjs','foundry.mjs','model.mjs','store.mjs'].map(file=>fs.readFileSync(path.join(root,file),'utf8')).join('')).digest('hex');
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim();
function defaultBinary(){const local=path.join(root,'../target/debug/world-check');if(fs.existsSync(local))return local;try{const common=execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{cwd:root,encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();return path.join(path.dirname(common),'target/debug/world-check');}catch{return local;}}
export async function createApp({port=Number(process.env.SOLO_PORT||4320),dataDir=process.env.SOLO_DATA_DIR||path.join(root,'.data'),foundryURL=process.env.SOLO_FOUNDRY_URL||'http://127.0.0.1:4317',foundryEnabled=true,mode=process.env.SOLO_FOUNDRY_MODE||'gemini',binary=process.env.RUST_CORE_BIN||defaultBinary()}={}){
 if(!['gemini','managed'].includes(mode))throw Error('Offline generation is disabled. SOLO_FOUNDRY_MODE must be gemini or managed.');
 const store=openStore(dataDir),w=store.world;if(process.env.SOLO_FOUNDRY_MAINTENANCE==='1')w.foundry.maintenance=true;let interruptedTheme=false;for(const request of w.foundry.requests)if(request.status==='preparing-theme'){request.status='blocked';request.reason='Restart interrupted theme inference. Its paid outcome needs reconciliation; no automatic retry was issued.';request.reconciliationRequired=true;const target=w.buildings.find(b=>b.id===request.targetId);if(target)target.creationStatus='blocked';interruptedTheme=true;}const priorMode=w.foundry.mode;w.foundry.mode=mode;if(priorMode==='fixture'&&mode!=='fixture'){w.foundry.round++;w.foundry.enabled=true;for(const request of w.foundry.requests)if(request.provider==='fixture'&&request.status==='queued')request.status='cancelled';}if(!fs.existsSync(path.join(dataDir,'initialized'))){w.foundry.enabled=foundryEnabled;fs.writeFileSync(path.join(dataDir,'initialized'),'1',{mode:0o600});store.save();}else if(interruptedTheme)store.save();
 const api=httpAPI(foundryURL),controller=new FoundryController({world:w,save:store.save,api,verifyTyped:typedVerifier(binary),dataDir,mode});let url,closed=false,fatal=null;
 const currentScope=request=>{if(!request.factionId)return true;const faction=w.factions.find(f=>f.id===request.factionId);return faction?.revision===request.factionRevision&&request.targets?.some(t=>w.buildings.some(b=>b.id===t.id&&b.factionId===request.factionId&&b.factionAssignment===t.assignment));};
 const cancelRemote=async requests=>{for(const request of requests){if(!request.jobId||request.reuseFromRequestId)continue;try{await api(`/api/jobs/${request.jobId}/cancel`,{});request.remoteCancelled=true;}catch(e){if(e.status!==409)request.remoteCancelError='Stopped locally; remote cancellation needs reconciliation: '+e.message;}try{store.save();}catch(e){fatal=e;throw e;}}};
 const send=(res,status,body)=>{res.writeHead(status,{'Content-Type':'application/json','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'});res.end(JSON.stringify(body));};
 const server=http.createServer(async(req,res)=>{try{
 const host=req.headers.host;if(!host||!/^127\.0\.0\.1:\d+$/.test(host))return send(res,403,{error:'Open this app at its printed 127.0.0.1 address.'});
 const pathname=new URL(req.url,'http://'+host).pathname;
 if(req.method==='POST'){
 if(fatal)return send(res,503,{error:'Saving failed. Stop the app and recover the save before continuing.'});
 if(req.headers.origin&&req.headers.origin!=='http://'+host)return send(res,403,{error:'Cross-origin commands are not allowed.'});
 if(req.headers['content-type']?.split(';')[0]!=='application/json')return send(res,415,{error:'Use application/json.'});
 let body='',size=0;for await(const chunk of req){size+=chunk.length;if(size>16000)return send(res,413,{error:'Command too large.'});body+=chunk;}const input=JSON.parse(body);
 // Work on a transaction copy; acknowledged commands are flushed before reply.
 if(pathname==='/api/v1/command'){if(input.kind==='creation'&&w.foundry.maintenance)throw Error('Foundry connection repair is in progress; your colony is safe.');if(input.kind==='creation'&&input.provider==='fixture')throw Error('Offline generation is disabled. Select live Asset Foundry.');const candidate=structuredClone(w),result=command(candidate,input);const cancelled=[];for(const request of candidate.foundry.requests){const live=w.foundry.requests.find(r=>r.id===request.id);if(live){if(request.status==='cancelled'&&live.status!=='cancelled')cancelled.push(live);Object.assign(live,request);}else w.foundry.requests.push(request);}for(const key of ['revision','paused','alloy','robot','buildings','deposits','receipts','events','factions'])w[key]=candidate[key];try{store.save();}catch(e){fatal=e;throw e;}await cancelRemote(cancelled);return send(res,200,result);}
 if(pathname==='/api/v1/foundry'){
 if(input.action==='maintenance'){if(typeof input.enabled!=='boolean')throw Error('enabled must be boolean.');w.foundry.maintenance=input.enabled;}
 else if(input.action==='toggle'){if(typeof input.enabled!=='boolean')throw Error('enabled must be boolean.');w.foundry.enabled=input.enabled;}
 else if(input.action==='next'){if(w.foundry.requests.some(r=>['running','queued','dispatching','unknown'].includes(r.status)))throw Error('Finish or reconcile the current pass first.');w.foundry.round++;w.foundry.enabled=true;}
 else if(input.action==='rollback'){const factionId=input.factionId||'landing';rollback(w,input.kind,factionId);const cancelled=[];for(const r of w.foundry.requests)if(r.kind===input.kind&&(r.factionId||'landing')===factionId&&['queued','preparing-theme','dispatching','unknown','running'].includes(r.status)){r.status='cancelled';r.reason='Invalidated by appearance rollback.';cancelled.push(r);}store.save();await cancelRemote(cancelled);}
 else if(input.action==='regenerate'){
 const building=w.buildings.find(b=>b.id===input.targetId&&b.kind==='creation');if(!building)throw Error('Unknown prompt creation.');
 if(w.foundry.requests.some(r=>r.targetId===building.id&&['queued','preparing-theme','dispatching','running','unknown'].includes(r.status)))throw Error('This creation already has pending work.');
 const request=creationRequest({id:randomUUID(),targetId:building.id,factionId:building.factionId,factionAssignment:building.factionAssignment,prompt:building.prompt,provider:'gemini'},w);request.replacesRequestId=w.foundry.requests.findLast(r=>r.targetId===building.id)?.id;request.revisionReason='User-authorized regeneration after contract repair; prior outcomes retained.';w.foundry.requests.push(request);building.creationStatus='queued';
 }
 else if(input.action==='reconcile'){
 const request=w.foundry.requests.find(r=>r.id===input.requestId);if(!request||request.status!=='unknown'||request.jobId)throw Error('Only an unknown dispatch without a receipt can be reconciled.');
 if(!currentScope(request))throw Error('This request was superseded by a newer faction assignment or direction.');const jobs=await api('/api/jobs');if(request.status!=='unknown'||!currentScope(request))throw Error('This request was superseded during reconciliation.');const matching=jobs.filter(j=>j.prompt===request.prompt&&j.provider===request.provider),known=w.foundry.requests.filter(r=>r.jobId);if(matching.length>1)throw Error('Multiple matching Foundry jobs exist; retain history and reconcile manually.');
 if(matching.length===1){request.jobId=matching[0].id;request.status='running';}
 else{if(!known.length||!known.every(r=>jobs.some(j=>j.id===r.jobId)))throw Error('Cannot confirm the original Foundry history; no retry was issued.');request.reconciliation={at:Date.now(),matchingJobs:0,priorReceiptsVerified:true,operatorRetry:true};request.status='queued';}
 request.explicit=true;w.foundry.error=null;
 }
 else if(input.action==='retry'){const request=w.foundry.requests.find(r=>r.id===input.requestId);if(!request||!request.jobId||!['blocked','unknown'].includes(request.status))throw Error('Only an existing job can be rechecked; unknown dispatches cannot be retried.');if(!currentScope(request))throw Error('This request was superseded by a newer faction assignment or direction.');const remote=await api(`/api/jobs/${request.jobId}`);if(['blocked','failed','exhausted','cancelled'].includes(remote.status))throw Error('The existing job is terminal. Select the building and start a bounded new generation; its history is retained.');if(!['blocked','unknown'].includes(request.status)||!currentScope(request))throw Error('This request was superseded while it was being rechecked.');request.status='running';request.explicit=true;}
 else if(input.action==='cancel'){const request=w.foundry.requests.find(r=>r.id===input.requestId);if(request&&!request.jobId&&['queued','preparing-theme'].includes(request.status)){request.status='cancelled';request.reason='Cancelled before asset generation dispatch.';const target=w.buildings.find(b=>b.id===request.targetId);if(target)target.creationStatus='cancelled';store.save();return send(res,200,{ok:true});}if(!request?.jobId||!['running','blocked','unknown'].includes(request.status))throw Error('No active owned job to cancel.');request.status='cancelled';const target=w.buildings.find(b=>b.id===request.targetId);if(target)target.creationStatus='cancelled';w.foundry.enabled=false;store.save();try{if(!request.reuseFromRequestId)await api(`/api/jobs/${request.jobId}/cancel`,{});}catch(e){if(e.status!==409)w.foundry.error='Stopped locally. Remote cancellation needs reconciliation: '+e.message;}}
 else throw Error('Unknown Foundry action.');store.save();return send(res,200,{ok:true});
 }
 return send(res,404,{error:'Unknown API route.'});
 }
 if(req.method!=='GET')return send(res,405,{error:'Method not allowed.'});
 if(pathname==='/api/v1/world')return send(res,200,{...publicState(w),foundryURL});
 if(pathname==='/api/v1/health')return send(res,200,{ok:!fatal,app:'first-landing',sourceRoot:path.resolve(root,'..'),sourceSha256,sourceRevision,apiVersion:1,saveVersion:2,mode,foundryURL});
 if(pathname==='/api/v1/export'){res.setHeader('Content-Disposition','attachment; filename="first-landing-colony.json"');return send(res,200,w);}
 let file;if(pathname==='/vendor/three.js')file=require.resolve('three').replace(/three\.cjs$/,'three.module.js');
 else if(pathname==='/vendor/three.core.js')file=require.resolve('three').replace(/three\.cjs$/,'three.core.js');
 else if(pathname==='/minerals.mjs')file=path.join(root,'minerals.mjs');
 else if(pathname==='/model.mjs')file=path.join(root,'model.mjs');
 else if(pathname==='/shared/core.mjs')file=path.join(root,'../public/core.mjs');
 else if((pathname==='/shared/themes.mjs'||pathname==='/public/themes.mjs'))file=path.join(root,'../public/themes.mjs');
 else{const name=pathname==='/'?'index.html':pathname.slice(1);if(!/^[a-zA-Z0-9_.-]+$/.test(name))return send(res,404,{error:'Not found.'});file=path.join(root,'public',name);}
 if(!fs.existsSync(file)||!fs.statSync(file).isFile())return send(res,404,{error:'Not found.'});
 const mime={'.html':'text/html','.css':'text/css','.mjs':'text/javascript','.js':'text/javascript','.svg':'image/svg+xml'};res.writeHead(200,{'Content-Type':mime[path.extname(file)]||'application/octet-stream','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'});fs.createReadStream(file).pipe(res);
 }catch(e){send(res,400,{error:e.message});}});
 try{await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,'127.0.0.1',resolve);});}catch(e){store.close();throw e;}url=`http://127.0.0.1:${server.address().port}`;
 let previous=performance.now(),lastSave=previous;const tick=setInterval(()=>{if(fatal)return;const now=performance.now();advance(w,Math.min((now-previous)/1000,1));previous=now;if(now-lastSave>1000){try{store.save();lastSave=now;}catch(e){fatal=e;w.paused=true;console.error('Colony save failed:',e.message);}}},100);
 const jobs=setInterval(()=>{if(!fatal&&!w.foundry.maintenance)void controller.step();},4000);
 return {url,world:w,async close(){if(closed)return;closed=true;clearInterval(tick);clearInterval(jobs);while(controller.busy)await new Promise(r=>setTimeout(r,50));await new Promise(resolve=>server.close(resolve));store.close();}};
}
if(process.argv[1]&&pathToFileURL(path.resolve(process.argv[1])).href===import.meta.url){const app=await createApp();console.log(`First Landing — single-player colony\n${app.url}\nSave: ${process.env.SOLO_DATA_DIR||path.join(root,'.data')}\nFoundry mode: ${app.world.foundry.mode} (live Foundry)`);for(const signal of ['SIGINT','SIGTERM'])process.once(signal,async()=>{await app.close();process.exit(0);});}
