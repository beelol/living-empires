import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID,createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {chromium} from 'playwright';
import {createCandidate,reviewCandidate} from './lib/gemini.mjs';
import {THEMES,fixtureRecipe,injectDefect,recipeSchema,eligible,canAccept} from './public/core.mjs';

const root=path.dirname(fileURLToPath(import.meta.url));
const data=path.join(root,'.data'); fs.mkdirSync(data,{recursive:true});
const port=Number(process.env.PORT||4317), base=`http://127.0.0.1:${port}`;
const apiKey=process.env.GEMINI_API_KEY||process.env.GOOGLE_API_KEY;
const model=process.env.GEMINI_MODEL||'gemini-3.8-flash';
const agent=process.env.GEMINI_AGENT||'antigravity-preview-09-2026';
const jobs=new Map(), controllers=new Map();
const renderTasks=new Map();
let lastRendererSeen=0;
let browser;
const digest=v=>createHash('sha256').update(JSON.stringify(v)).digest('hex');
const sourceRevision=spawnSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).stdout?.trim()||'unknown';
const sourceFingerprint=digest(['server.mjs','public/core.mjs','public/scene.mjs','public/render-worker.mjs','lib/gemini.mjs','crates/living-world-core/src/lib.rs','Cargo.lock','package-lock.json'].map(file=>[file,fs.existsSync(path.join(root,file))?fs.readFileSync(path.join(root,file),'utf8'):null]));
const rustBinary=process.env.RUST_CORE_BIN||path.join(root,'target/debug/world-check');
function verifiedAttempt(job){
  const attempt=job?.attempts.at(-1);
  if(!job||!['accepted','active'].includes(job.status)||job.cancelled||!attempt||digest(attempt.recipe)!==job.acceptedHash||!canAccept(attempt.checks,attempt.images.length))return null;
  if(attempt.imageHashes?.length!==3||!attempt.imageHashes.every((hash,i)=>hash===createHash('sha256').update(fs.readFileSync(path.join(data,...attempt.images[i].split('/').slice(2)))).digest('hex')))return null;
  return attempt;
}
function rustChecks(recipe,themeId){
  if(!fs.existsSync(rustBinary))throw new Error('Rust core is not built. Run cargo build -p living-world-core before starting a run.');
  const result=spawnSync(rustBinary,[],{input:JSON.stringify({recipe,themeId}),encoding:'utf8',timeout:5000});
  if(result.error)throw result.error;
  const value=JSON.parse(result.stdout||'{}');
  if(result.status!==0)throw new Error(value.error||'Rust validation failed');
  return value;
}
const releaseFile=path.join(data,'releases.json');
let releases=fs.existsSync(releaseFile)?JSON.parse(fs.readFileSync(releaseFile,'utf8')):[];
function atomic(file,value){const tmp=file+'.tmp';fs.writeFileSync(tmp,JSON.stringify(value,null,2));fs.renameSync(tmp,file);}
function save(job){atomic(path.join(data,job.id+'.json'),job);}
function event(job,state,message){job.status=state;job.events.push({at:new Date().toISOString(),state,message});save(job);}
for(const file of fs.readdirSync(data).filter(f=>/^[\da-f-]+\.json$/.test(f))){
  const job=JSON.parse(fs.readFileSync(path.join(data,file),'utf8'));
  if(!['accepted','failed','blocked','cancelled','exhausted','active'].includes(job.status)){
    job.status='blocked';job.stopReason='Worker restarted; paid request outcome requires reconciliation before a new run.';
    job.events.push({at:new Date().toISOString(),state:'blocked',message:job.stopReason});save(job);
  }
  jobs.set(job.id,job);
}
function ensureCurrent(job){if(job.cancelled)throw new Error('Cancelled');if(Date.now()>=job.deadline)throw new Error('Job deadline exhausted');}
async function charge(job,label,fn){
  ensureCurrent(job);
  if(job.calls>=6)throw new Error('Six-call job ceiling reached');
  job.calls++; job.callLog.push({number:job.calls,label,status:'dispatched',at:new Date().toISOString()});save(job);
  const entry=job.callLog.at(-1);
  try {const result=await fn();ensureCurrent(job);entry.status='returned';entry.usage=result.usageMetadata||result.usage||null;save(job);return result;}
  catch(e){entry.status='failed_or_unknown';save(job);throw e;}
}
async function generate(job,previous,findings){
  if(job.provider==='fixture'){
    await new Promise(r=>setTimeout(r,350));ensureCurrent(job);
    return previous?{...structuredClone(previous),parts:previous.parts.filter(p=>p.id!=='audit-obstruction')}:fixtureRecipe(job.themeId);
  }
  const prompt=`Generate a complete JSON recipe for a playable 3D sensor outpost in this game world. Output ONLY JSON, no markdown.\nFrozen world theme: ${JSON.stringify(job.theme)}\nUser brief: ${job.prompt}\nContract: ${JSON.stringify(recipeSchema)}\nGeometry uses metres: y up, all dimensions full extents. Ground y=0. Commander starts at (0,0,9), walks straight toward (0,0,0), height 1.5m, radius .28m. Leave an open corridor at least 1.2m wide along x=0,z=0..9. Supports/walls meet ground. Building stays within x/z +-4. Sensor radius 8..20. Use 4..48 parts. Materials are palette tokens ONLY; shape box/cylinder/cone; no arbitrary code. A roof, walls, posts, sensor mast and visual accents should form a coherent outpost. Preserve the same theme, identity, and gameplay role when repairing.\nReference format: ${JSON.stringify(fixtureRecipe(job.themeId))}\nPrevious candidate: ${JSON.stringify(previous||null)}\nIndependent failures to repair: ${JSON.stringify(findings||[])}\nDo not add props that contradict the theme. Change the reference geometry to fit the user's request while respecting the corridor.`;
  job.lastCreatorPrompt=prompt;save(job);
  const result=await charge(job,'Gemini creator',()=>createCandidate({provider:job.provider,apiKey,model,agent,prompt,recipeSchema,interactionId:job.interactionId,environmentId:job.environmentId,signal:controllers.get(job.id).signal}));
  job.interactionId=result.interactionId;job.environmentId=result.environmentId;
  job.providerResponses.push({kind:'creator',response:result.response});save(job);
  return result.recipe;
}
async function getBrowser(){
  if(!browser){
    const chrome=process.env.CHROME_PATH||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
    browser=await chromium.launch({headless:true,...(fs.existsSync(chrome)?{executablePath:chrome}:{})});
  }
  return browser;
}
async function captureHeadless(job,attempt){
  const browser=await getBrowser(), page=await browser.newPage({viewport:{width:1000,height:740},deviceScaleFactor:1});
  const folder=path.join(data,job.id);fs.mkdirSync(folder,{recursive:true});
  try{
    await page.goto(`${base}/render.html?job=${job.id}&attempt=${attempt.number}`,{waitUntil:'networkidle'});
    await page.waitForFunction(()=>window.audit?.ready,{timeout:20000});
    const images=[];
    for(let i=0;i<3;i++){
      await page.evaluate(index=>window.audit.view(index),i);await page.waitForTimeout(120);
      ensureCurrent(job);
      const filename=`${attempt.number}-${i}.png`;
      await page.locator('canvas').screenshot({path:path.join(folder,filename)});
      images.push(`/evidence/${job.id}/${filename}`);
    }
    const movement=await page.evaluate(()=>window.audit.traverse());
    return {images,movement};
  }finally{await page.close();}
}
async function capture(job,attempt){
  if(process.env.AUDIT_RENDERER==='headless'||Date.now()-lastRendererSeen>10000)return captureHeadless(job,attempt);
  return new Promise((resolve,reject)=>{
    const token=randomUUID();
    const cleanup=()=>{clearTimeout(timer);renderTasks.delete(token);controllers.get(job.id)?.signal.removeEventListener('abort',onAbort);};
    const onAbort=()=>{cleanup();reject(new Error('Cancelled during rendering'));};
    const timer=setTimeout(()=>{cleanup();reject(new Error('Studio renderer timed out. Keep the Studio browser open or finish installing Chromium.'));},60000);
    controllers.get(job.id).signal.addEventListener('abort',onAbort,{once:true});
    renderTasks.set(token,{token,jobId:job.id,attemptNumber:attempt.number,candidateHash:attempt.hash,recipe:attempt.recipe,theme:job.theme,
      finish:value=>{cleanup();attempt.renderSource='studio-client';resolve(value);},fail:message=>{cleanup();reject(new Error(message));}});
  });
}
async function review(job,attempt){
  if(job.provider==='fixture')return [
    {id:'THEME-02',status:'pass',label:'Visual theme consistency',detail:'Offline fixture verdict for rehearsal only. No live AI visual assessment.'},
    {id:'LIVE-04',status:'pass',label:'Three actual rendered views',detail:'Three views captured from the actual Three.js client; reviewer verdict is a rehearsal fixture.'}
  ];
  const result=await charge(job,'independent visual reviewer',()=>reviewCandidate({apiKey,model,theme:job.theme,prompt:job.prompt,images:attempt.images.map(url=>fs.readFileSync(path.join(data,...url.split('/').slice(2)))),signal:controllers.get(job.id).signal}));
  job.providerResponses.push({kind:'reviewer',response:result.response});save(job);
  return [{id:'THEME-02',status:result.themeStatus,label:'Visual theme consistency',detail:result.findings},{id:'LIVE-04',status:result.renderStatus,label:'Three actual rendered views',detail:result.findings}];
}
async function run(job){
  let previous=null,findings=[];
  try{
    while(eligible(job)){
      const attempt={number:job.attempts.length+1,checks:[],images:[],createdAt:new Date().toISOString()};job.attempts.push(attempt);
      event(job,'generating',`${attempt.number===1?'Generating':'Repairing'} candidate ${attempt.number}/3 in the frozen ${job.theme.name} theme.`);
      let candidate=await generate(job,previous,findings);ensureCurrent(job);
      if(job.injectDefect&&attempt.number===1)candidate=injectDefect(candidate);
      attempt.recipe=candidate;attempt.hash=digest(candidate);previous=candidate;save(job);
      event(job,'validating',`Checking candidate ${attempt.number}: palette, geometry, collision, supports and sensor range.`);
      try{const evaluation=rustChecks(candidate,job.themeId);attempt.checks=evaluation.checks;attempt.rustMovement=evaluation.movement;attempt.engine=evaluation.engine;}
      catch(e){attempt.checks=[{id:'PACKAGE-01',status:'fail',label:'Valid package and frozen theme',detail:e.message}];findings=attempt.checks;save(job);continue;}
      event(job,'rendering','Capturing three actual game views and running the commander through the client movement path.');
      const evidence=await capture(job,attempt);ensureCurrent(job);attempt.images=evidence.images;attempt.movement=evidence.movement;
      attempt.imageHashes=attempt.images.map(url=>createHash('sha256').update(fs.readFileSync(path.join(data,...url.split('/').slice(2)))).digest('hex'));
      const access=attempt.checks.find(c=>c.id==='GEN-05.access');access.status=access.status==='pass'&&evidence.movement.pass?'pass':'fail';access.trace=evidence.movement.trace;
      if(attempt.rustMovement.pass!==evidence.movement.pass)throw new Error('Browser/Rust movement parity failed. Candidate cannot be accepted.');
      event(job,'reviewing',job.provider==='fixture'?'Applying explicitly simulated visual verdict; functional checks and captures are real.':'Independent Gemini reviewer is inspecting the actual rendered world against the theme contract.');
      attempt.checks.push(...await review(job,attempt));ensureCurrent(job);save(job);
      if(canAccept(attempt.checks,attempt.images.length)){
        job.acceptedHash=attempt.hash;job.stopReason='All six demo checks passed.';event(job,'accepted',job.provider==='fixture'?'Rehearsal candidate accepted. Visual review was simulated.':'All required checks passed. Candidate is ready to activate.');return;
      }
      findings=attempt.checks.filter(c=>c.status!=='pass');
      if(findings.some(c=>c.status==='blocked')){job.stopReason='Review is inconclusive; evidence or operator input required.';event(job,'blocked',job.stopReason);return;}
      event(job,'repairing',findings.map(c=>`${c.id}: ${c.detail}`).join(' '));
    }
    job.stopReason=job.cancelled?'Cancelled':Date.now()>=job.deadline?'20-minute deadline reached':'Three-attempt / six-call ceiling reached';event(job,job.cancelled?'cancelled':'exhausted',job.stopReason);
  }catch(e){
    job.stopReason=job.cancelled?'Cancelled; late results cannot activate.':e.message;
    event(job,job.cancelled?'cancelled':'blocked',job.stopReason);
  }finally{controllers.delete(job.id);}
}
const json=(res,status,value)=>{res.writeHead(status,{'Content-Type':'application/json','Cache-Control':'no-store'});res.end(JSON.stringify(value));};
const mime={'.html':'text/html','.mjs':'text/javascript','.js':'text/javascript','.css':'text/css','.png':'image/png','.json':'application/json'};
async function body(req,limit=12000){let text='';for await(const chunk of req){text+=chunk;if(text.length>limit)throw new Error('Request too large');}return JSON.parse(text||'{}');}
const server=http.createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,base), pathname=decodeURIComponent(url.pathname);
    if(req.method==='POST'){
      if(req.headers.origin && ![base,`http://localhost:${port}`].includes(req.headers.origin))return json(res,403,{error:'Origin rejected'});
      const input=await body(req,pathname==='/api/render-task'?8_000_000:12000);
      if(pathname==='/api/render-task'){
        const task=renderTasks.get(input.token);
        if(!task||task.jobId!==input.jobId||task.attemptNumber!==input.attemptNumber||task.candidateHash!==input.candidateHash)return json(res,409,{error:'Render task is no longer current.'});
        const job=jobs.get(task.jobId);ensureCurrent(job);
        if(input.error){task.fail('Studio renderer failed. Inspect browser rendering support.');return json(res,200,{received:true});}
        if(!Array.isArray(input.images)||input.images.length!==3||typeof input.movement?.pass!=='boolean'||!Array.isArray(input.movement.trace))return json(res,400,{error:'Three PNG views and a movement trace are required.'});
        const buffers=input.images.map(image=>{
          if(typeof image!=='string'||!image.startsWith('data:image/png;base64,'))throw new Error('PNG evidence required');
          const buffer=Buffer.from(image.slice(22),'base64');
          if(buffer.length>2_000_000||!buffer.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])))throw new Error('Invalid PNG evidence');return buffer;
        });
        const folder=path.join(data,job.id);fs.mkdirSync(folder,{recursive:true});
        const images=buffers.map((buffer,i)=>{const file=`${task.attemptNumber}-${i}.png`;fs.writeFileSync(path.join(folder,file),buffer);return `/evidence/${job.id}/${file}`;});
        task.finish({images,movement:input.movement});return json(res,200,{received:true});
      }
      if(pathname==='/api/jobs'){
        if([...jobs.values()].some(j=>!['accepted','active','failed','blocked','cancelled','exhausted'].includes(j.status)))return json(res,409,{error:'A run is already in progress. Cancel it first.'});
        if(!THEMES[input.themeId]||!['fixture','gemini','managed'].includes(input.provider)||typeof input.prompt!=='string'||input.prompt.trim().length<5||input.prompt.length>3000)return json(res,400,{error:'Provide a theme, provider, and 5–3000 character prompt.'});
        if(input.provider!=='fixture'&&!apiKey)return json(res,400,{error:'Configure GEMINI_API_KEY in .env and restart the server. Offline rehearsal is available now.'});
        const job={id:randomUUID(),sourceRevision,sourceFingerprint,prompt:input.prompt,themeId:input.themeId,theme:structuredClone(THEMES[input.themeId]),provider:input.provider,model,agent,injectDefect:input.injectDefect===true,contractVersion:1,createdAt:new Date().toISOString(),deadline:Date.now()+20*60*1000,status:'queued',calls:0,callLog:[],attempts:[],events:[],providerResponses:[]};
        jobs.set(job.id,job);controllers.set(job.id,new AbortController());event(job,'queued','Brief and theme contract frozen. Job limit: three candidates, six paid calls, twenty minutes.');
        json(res,201,job);void run(job);return;
      }
      const match=pathname.match(/^\/api\/jobs\/([\da-f-]+)\/(cancel|activate)$/);
      if(match){
        const job=jobs.get(match[1]);if(!job)return json(res,404,{error:'Unknown job'});
        if(match[2]==='cancel'){
          if(['accepted','active'].includes(job.status))return json(res,409,{error:'This run has finished. Use rollback for an active release.'});
          job.cancelled=true;controllers.get(job.id)?.abort();event(job,'cancelled','Cancelled. No late candidate can activate.');return json(res,200,job);
        }
        const attempt=verifiedAttempt(job);
        if(job.status!=='accepted'||!attempt)return json(res,409,{error:'Only the exact accepted candidate and its reviewed evidence can activate.'});
        const priorJob=jobs.get(releases.at(-1)?.jobId);if(priorJob?.status==='active')event(priorJob,'accepted','Superseded by a newer active package; this candidate remains approved.');
        releases.push({id:randomUUID(),jobId:job.id,hash:attempt.hash,recipe:attempt.recipe,theme:job.theme,rehearsal:job.provider==='fixture',at:new Date().toISOString()});atomic(releaseFile,releases);event(job,'active','Verified package activated in the preview.');return json(res,200,releases.at(-1));
      }
      if(pathname==='/api/rollback'){
        if(releases.length<2)return json(res,409,{error:'No prior release available yet. Activate a second candidate first.'});
        const removed=releases.pop();const removedJob=jobs.get(removed.jobId);if(removedJob)event(removedJob,'accepted','Rolled back from active preview; this candidate remains approved.');
        const restoredJob=jobs.get(releases.at(-1).jobId);if(restoredJob)event(restoredJob,'active','Restored the previous accepted package.');
        atomic(releaseFile,releases);return json(res,200,releases.at(-1));
      }
      return json(res,404,{error:'Unknown action'});
    }
    if(req.method!=='GET')return json(res,405,{error:'Method not allowed'});
    if(pathname==='/api/config')return json(res,200,{keyConfigured:!!apiKey,model,agent,themes:THEMES});
    if(pathname==='/api/render-task'){
      lastRendererSeen=Date.now();const task=renderTasks.values().next().value;
      return json(res,200,task?{token:task.token,jobId:task.jobId,attemptNumber:task.attemptNumber,candidateHash:task.candidateHash,recipe:task.recipe,theme:task.theme}:null);
    }
    if(pathname==='/api/jobs')return json(res,200,[...jobs.values()].map(({providerResponses,lastCreatorPrompt,...job})=>job).reverse());
    if(pathname==='/api/release')return json(res,200,releases.at(-1)||null);
    const packageMatch=pathname.match(/^\/api\/jobs\/([\da-f-]+)\/package$/);
    if(packageMatch){
      const job=jobs.get(packageMatch[1]),attempt=verifiedAttempt(job);
      if(!attempt)return json(res,409,{error:'Package export requires an accepted candidate and unchanged evidence.'});
      const value={format:'living-empires.asset',schemaVersion:1,assetType:'building',assetId:job.id,themeId:job.themeId,theme:job.theme,recipe:attempt.recipe,
        manifest:{version:attempt.number,contentSha256:attempt.hash,themeSha256:digest(job.theme),sourceRevision:job.sourceRevision,sourceFingerprint:job.sourceFingerprint,engine:attempt.engine,contractVersion:job.contractVersion,rehearsal:job.provider==='fixture',createdAt:attempt.createdAt,
          checks:attempt.checks.map(({id,status,detail})=>({id,status,detail})),evidence:attempt.images.map((url,i)=>({view:i+1,url,sha256:attempt.imageHashes[i]})),provider:job.provider,model:job.model,agent:job.provider==='managed'?job.agent:null}};
      res.writeHead(200,{'Content-Type':'application/json','Content-Disposition':`attachment; filename="asset-${job.id}.json"`,'Cache-Control':'no-store'});res.end(JSON.stringify(value,null,2));return;
    }
    const match=pathname.match(/^\/api\/jobs\/([\da-f-]+)$/);
    if(match){const job=jobs.get(match[1]);return json(res,job?200:404,job||{error:'Unknown job'});}
    let file;
    if(pathname==='/vendor/three.js')file=path.join(root,'node_modules/three/build/three.module.js');
    else if(pathname==='/vendor/three.core.js')file=path.join(root,'node_modules/three/build/three.core.js');
    else if(/^\/evidence\/[\da-f-]+\/\d+-[0-2]\.png$/.test(pathname))file=path.join(data,...pathname.split('/').slice(2));
    else {file=path.resolve(root,'public',pathname==='/'?'index.html':'.'+pathname);if(!file.startsWith(path.join(root,'public')+path.sep))return json(res,403,{error:'Forbidden'});}
    if(!fs.existsSync(file)||!fs.statSync(file).isFile())return json(res,404,{error:'Not found'});
    res.writeHead(200,{'Content-Type':mime[path.extname(file)]||'application/octet-stream','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'});fs.createReadStream(file).pipe(res);
  }catch(e){json(res,400,{error:e.message});}
});
server.listen(port,'127.0.0.1',()=>console.log(`Living Empires Studio: ${base}\nGemini: ${apiKey?'configured':'not configured — offline rehearsal available'}`));
process.on('SIGINT',async()=>{for(const c of controllers.values())c.abort();await browser?.close();server.close();process.exit(0);});
