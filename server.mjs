import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID,createHash} from 'node:crypto';
import {spawn,spawnSync} from 'node:child_process';
import {chromium} from 'playwright';
import {inferTheme} from './lib/theme-inference.mjs';
import {validateTheme} from './public/themes.mjs';
import {THEMES,injectDefect,canAccept} from './public/core.mjs';
import {captureNative} from './lib/native-capture.mjs';
import {SpendLedger} from './lib/spend-ledger.mjs';

const root=path.dirname(fileURLToPath(import.meta.url));
const publicRoot=process.env.ASSET_PUBLIC_DIR||path.join(root,'public');
const data=process.env.ASSET_DATA_DIR||path.join(root,'.data'); fs.mkdirSync(data,{recursive:true});
const spending=new SpendLedger(data,process.env.GOOGLE_SESSION_BUDGET_USD,{sponsoredCredits:process.env.GOOGLE_SPONSORED_CREDITS==='1'});
const port=Number(process.env.PORT||4317), base=`http://127.0.0.1:${port}`;
const apiKey=process.env.GEMINI_API_KEY||process.env.GOOGLE_API_KEY;
const model=process.env.GEMINI_MODEL||'gemini-3.8-flash';
const agent=process.env.GEMINI_AGENT||'antigravity-preview-09-2026';
const themeInferenceEnabled=process.env.THEME_INFERENCE_ENABLED==='1';
const jobs=new Map(), controllers=new Map();
const renderTasks=new Map();
let lastRendererSeen=0;
let lastPipelineTime=0;
let browser;
const digest=v=>createHash('sha256').update(JSON.stringify(v)).digest('hex');
const rustBinary=process.env.RUST_CORE_BIN||path.join(root,'target/debug/world-check');
const rustBinarySha256=fs.existsSync(rustBinary)?createHash('sha256').update(fs.readFileSync(rustBinary)).digest('hex'):null;
const sourceRevision=spawnSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).stdout?.trim()||'unknown';
const sourceFingerprint=digest(['server.mjs','public/app.mjs','public/style.css','public/index.html','public/themes.mjs','public/core.mjs','public/scene.mjs','public/render-worker.mjs','lib/spend-ledger.mjs','lib/theme-inference.mjs','lib/native-capture.mjs','game/examples/foundry_audit.rs','crates/living-world-core/src/lib.rs','crates/living-world-core/src/main.rs','crates/living-world-core/src/pipeline.rs','crates/living-world-core/src/provider.rs','crates/living-world-core/fixtures/presets.json','Cargo.lock','package-lock.json'].map(file=>{const source=file.startsWith('public/')?path.join(publicRoot,file.slice(7)):path.join(root,file);return [file,fs.existsSync(source)?fs.readFileSync(source,'utf8'):null];}).concat([['world-check-binary-sha256',rustBinarySha256]]));
function rustSync(input){
  if(!fs.existsSync(rustBinary))throw new Error('Rust core is not built. Run cargo build -p living-world-core before starting a run.');
  const result=spawnSync(rustBinary,[],{cwd:root,input:JSON.stringify(input),encoding:'utf8',timeout:15000,maxBuffer:16*1024*1024});
  if(result.error)throw result.error;
  let value={};try{value=JSON.parse(result.stdout||'{}');}catch{throw new Error('Rust core returned invalid JSON.');}
  if(result.status!==0)throw new Error(value.error||'Rust core operation failed.');
  return value;
}
function rustAsync(input,signal,timeoutMs=120000){
  if(!fs.existsSync(rustBinary))return Promise.reject(new Error('Rust core is not built. Run cargo build -p living-world-core before starting a run.'));
  return new Promise((resolve,reject)=>{
    const child=spawn(rustBinary,[],{cwd:root,stdio:['pipe','pipe','ignore']});let stdout='',settled=false,timer;
    const finish=(error,value)=>{if(settled)return;settled=true;clearTimeout(timer);signal?.removeEventListener('abort',abort);error?reject(error):resolve(value);};
    const abort=()=>{child.kill('SIGTERM');finish(new Error('Cancelled'));};
    if(signal?.aborted){abort();return;}
    signal?.addEventListener('abort',abort,{once:true});
    timer=setTimeout(()=>{child.kill('SIGTERM');finish(new Error('Rust provider operation timed out.'));},timeoutMs);
    child.stdout.on('data',chunk=>{stdout+=chunk;if(stdout.length>16*1024*1024){child.kill('SIGTERM');finish(new Error('Rust provider response exceeded the size limit.'));}});
    child.on('error',()=>finish(new Error('Could not start the Rust provider process.')));
    child.stdin.on('error',()=>finish(new Error('Rust provider request could not be delivered.')));
    child.on('close',code=>{
      if(settled)return;
      let value={};try{value=JSON.parse(stdout||'{}');}catch{finish(new Error('Rust provider returned invalid JSON.'));return;}
      if(code!==0){finish(new Error(value.error||'Rust provider operation failed.'));return;}
      finish(null,value);
    });
    child.stdin.end(JSON.stringify(input));
  });
}
function verifiedAttempt(job){
  if(job?.pipeline){
    if(!job||!['accepted','active'].includes(job.status)||job.pipeline.status!=='approved'||job.cancelled)return null;
    try{
      const result=rustSync({op:'package',pipeline:job.pipeline,now:Date.now()}),canonical=result.value,attempt=job.attempts.at(-1),record=job.pipeline.candidates.at(-1);
      if(stableJSON(job.theme)!==stableJSON(job.pipeline.contract.theme)||job.themeHash!==job.pipeline.contract.themeSha256)return null;
      if(!canonical||!attempt||!record||canonical.manifest.contentSha256!==record.candidateHash||attempt.hash!==record.candidateHash||canonical.assetId!==job.id||attempt.imageHashes?.length!==3)return null;
      for(let i=0;i<3;i++){
        const file=imagePath(job,attempt.images[i]);
        if(!file||!fs.existsSync(file))return null;
        const hash=createHash('sha256').update(fs.readFileSync(file)).digest('hex');
        if(hash!==attempt.imageHashes[i]||canonical.manifest.evidence[i]?.sha256!==hash)return null;
      }
      return attempt;
    }catch{return null;}
  }
  const attempt=job?.attempts.at(-1);
  if(job?.themeHash&&digest(job.theme)!==job.themeHash)return null;
  if(!job||!['accepted','active'].includes(job.status)||job.cancelled||!attempt||digest(attempt.recipe)!==job.acceptedHash||!canAccept(attempt.checks,attempt.images.length))return null;
  if(attempt.imageHashes?.length!==3||!attempt.imageHashes.every((hash,i)=>hash===createHash('sha256').update(fs.readFileSync(path.join(data,...attempt.images[i].split('/').slice(2)))).digest('hex')))return null;
  return attempt;
}
function rustChecks(recipe,themeId,theme){return rustSync({recipe,themeId,theme});}
function imagePath(job,url){
  const match=typeof url==='string'&&url.match(new RegExp(`^/evidence/${job.id}/(\\d+-[0-2]\\.png)$`));
  return match?path.join(data,job.id,match[1]):null;
}
const releaseFile=path.join(data,'releases.json');
let releases=fs.existsSync(releaseFile)?JSON.parse(fs.readFileSync(releaseFile,'utf8')):[];
function atomic(file,value){const tmp=file+'.tmp';fs.writeFileSync(tmp,JSON.stringify(value,null,2));fs.renameSync(tmp,file);}
function save(job){atomic(path.join(data,job.id+'.json'),job);}
function event(job,state,message){job.status=state;job.events.push({at:new Date().toISOString(),state,message});save(job);}
for(const file of fs.readdirSync(data).filter(f=>/^(?:[\da-f-]{36}|job-[a-z0-9-]{1,120})\.json$/.test(f))){
  const job=JSON.parse(fs.readFileSync(path.join(data,file),'utf8'));
  if(job.pipeline&&!['approved','blocked','exhausted','cancelled'].includes(job.pipeline.status)){
    try{job.pipeline=rustSync({op:'cancel',pipeline:job.pipeline,now:Date.now()}).pipeline;job.cancelled=true;job.stopReason='Server restarted before the run completed.';}
    catch{job.stopReason='Pipeline state could not be resumed safely after restart.';}
  }
  if(job.status==='active'&&releases.at(-1)?.jobId!==job.id){job.status='accepted';save(job);}
  if(!['accepted','failed','blocked','cancelled','exhausted','active'].includes(job.status)){
    job.status=job.pipeline?.status==='cancelled'?'cancelled':'blocked';job.stopReason=job.stopReason||'Worker restarted; paid request outcome requires reconciliation before a new run.';
    job.events.push({at:new Date().toISOString(),state:'blocked',message:job.stopReason});save(job);
  }
  jobs.set(job.id,job);
}
function ensureCurrent(job){
  if(job.cancelled)throw new Error('Cancelled');
  if(Date.now()>=job.deadline)throw new Error('Job deadline exhausted');
  if(job.pipeline){if(stableJSON(job.theme)!==stableJSON(job.pipeline.contract.theme))throw new Error('Frozen theme contract changed; create a new job.');if(!syncPipeline(job,'active'))throw new Error('Rust pipeline is no longer active.');return;}
  if(job.themeHash&&digest(job.theme)!==job.themeHash)throw new Error('Frozen theme contract changed; create a new job.');
}
function stableJSON(value){
  if(Array.isArray(value))return `[${value.map(stableJSON).join(',')}]`;
  if(value&&typeof value==='object')return `{${Object.keys(value).sort().map(key=>`${JSON.stringify(key)}:${stableJSON(value[key])}`).join(',')}}`;
  return JSON.stringify(value);
}
function syncPipeline(job,op,extra={}){
  const result=rustSync({op,pipeline:job.pipeline,now:Date.now(),...extra});
  job.pipeline=result.pipeline;job.calls=job.pipeline.providerCalls;save(job);return result.value;
}
async function providerCall(job,label,input){
  const spendId=spending.reserve({jobId:job.id,label,model:input.request.model,managed:input.op==='generate'&&input.request.managed});
  syncPipeline(job,'charge');
  const entry={number:job.pipeline.providerCalls,label,spendId,status:'dispatched',at:new Date().toISOString()};job.callLog.push(entry);job.calls=job.pipeline.providerCalls;job.spending=spending.summary();save(job);
  try{
    const result=await rustAsync(input,controllers.get(job.id)?.signal,input.op==='generate'&&input.request.managed?330000:120000);
    if(result.providerError){
      const received=JSON.parse(JSON.stringify(result.providerError.response).replaceAll(apiKey||'__no_key__','[redacted]'));
      const usage=received?.usageMetadata||received?.usage||received?.usage_metadata||null;
      job.providerResponses.push({kind:'provider-error',response:received,usage});
      job.interactionId=received?.id||job.interactionId;job.environmentId=received?.environment_id||job.environmentId;
      spending.settle(spendId,usage);entry.status='returned_error';entry.responseStatus=received?.status||null;entry.usage=usage;job.spending=spending.summary();save(job);
      throw new Error(result.providerError.message||'Provider response could not be used.');
    }
    spending.settle(spendId,result.usage);job.spending=spending.summary();
    ensureCurrent(job);entry.status='returned';entry.usage=result.usage||null;save(job);return result;
  }catch(error){if(entry.status!=='returned_error'){spending.unknown(spendId);entry.status='failed_or_unknown';}job.spending=spending.summary();save(job);throw error;}
}
function reviewFindings(value){return typeof value==='string'?value.trim():String(value||'').trim();}
async function generate(job,previous,findings){
  if(job.provider==='fixture'){
    const result=rustSync({op:'fixture',theme:job.theme,...(previous?{previous}: {})});
    await new Promise(resolve=>setTimeout(resolve,350));ensureCurrent(job);return result.recipe;
  }
  const request={theme:job.theme,prompt:job.prompt,previous:previous||null,findings:findings.map(item=>typeof item==='string'?item:`${item.id}: ${item.detail}`),model,agent,managed:job.provider==='managed',interactionId:job.interactionId||null,environmentId:job.environmentId||null};
  const result=await providerCall(job,'Gemini creator',{op:'generate',request});
  job.interactionId=result.interactionId||job.interactionId;job.environmentId=result.environmentId||job.environmentId;
  job.providerResponses.push({kind:'creator',response:result.response,usage:result.usage||null});save(job);return result.recipe;
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
  if(process.env.AUDIT_RENDERER==='bevy'){
    const functional=await captureHeadless(job,attempt);
    ensureCurrent(job);
    const native=await captureNative({root,job,attempt,movement:functional.movement,
      binary:process.env.NATIVE_AUDIT_BIN||path.join(root,'target/debug/examples/foundry_audit'),
      signal:controllers.get(job.id)?.signal,
      timeoutMs:Math.min(90000,Math.max(1,job.deadline-Date.now()))});
    ensureCurrent(job);attempt.renderSource=native.renderSource;
    attempt.functionalHarness=native.functionalHarness;attempt.nativeEvidence=native.nativeEvidence;
    return native;
  }
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
  if(job.provider==='fixture'){
    const previous=job.attempts.at(-2),restored=previous?.movement?.pass===false&&attempt.movement?.pass===true;
    const comparison={candidateHash:attempt.hash,imageHashes:attempt.imageHashes,previousCandidateHash:previous?.hash||null,previousImageHashes:previous?.imageHashes||[],status:previous?(restored?'improved':'unchanged'):'initial',summary:previous?(restored?'Deterministic rehearsal: the previously blocked traversal now passes. Creative quality was not judged.':'Deterministic rehearsal: no new traversal improvement is evidenced. Creative quality was not judged.'):'Initial rehearsal capture. Visual judgment is simulated; functional checks are real.',strengths:['Frozen fixture identity and material palette retained.'],defects:[],improvements:restored?['Collision-aware traversal changed from fail to pass; no creative improvement is claimed.']:[],regressions:[],simulated:true};
    return {themeStatus:'pass',renderStatus:'pass',findings:'Offline rehearsal visual verdict; no live AI review.',comparison,usage:null};
  }
  const imageInputs=record=>record.images.map((url,i)=>{
    const file=imagePath(job,url);if(!file||!fs.existsSync(file))throw new Error('Comparison image is missing.');
    const bytes=fs.readFileSync(file),hash=createHash('sha256').update(bytes).digest('hex');
    if(hash!==record.imageHashes[i])throw new Error('Comparison evidence changed after capture.');
    return bytes.toString('base64');
  });
  const prior=job.attempts.at(-2);
  const previous=prior?{candidateHash:prior.hash,imageHashes:prior.imageHashes,images:imageInputs(prior),strengths:prior.review?.comparison?.strengths||[]}:null;
  const request={theme:job.theme,prompt:job.prompt,candidateHash:attempt.hash,imageHashes:attempt.imageHashes,images:imageInputs(attempt),previous,model};
  const result=await providerCall(job,'independent comparative visual reviewer',{op:'review',request});
  job.providerResponses.push({kind:'reviewer',response:result.response,usage:result.usage||null});save(job);return result;
}
async function run(job){
  let previous=null,findings=[];
  try{
    while(true){
      const canContinue=syncPipeline(job,'continue');
      if(!canContinue){
        const state=job.pipeline.status;
        job.stopReason=state==='exhausted'?'Rust pipeline attempt, provider-call, or time budget exhausted.':'No more candidates can be evaluated.';
        event(job,state==='blocked'?'blocked':state==='cancelled'?'cancelled':'exhausted',job.stopReason);return;
      }
      const attempt={number:job.pipeline.attemptCount+1,checks:[],images:[],imageHashes:[],repairRequested:[...findings],previousHash:job.attempts.at(-1)?.hash||null,createdAt:new Date().toISOString()};
      event(job,'generating',`${attempt.number===1?'Generating':'Repairing'} candidate ${attempt.number}/3 in the frozen ${job.theme.name} theme.`);
      let candidate=await generate(job,previous,findings);ensureCurrent(job);
      if(job.injectDefect&&attempt.number===1)candidate=injectDefect(candidate);
      const record=syncPipeline(job,'submit',{recipe:candidate});
      candidate=record.recipe;attempt.recipe=candidate;attempt.hash=record.candidateHash;attempt.validationError=record.validationError||null;
      attempt.checks=record.checks||[];
      if(record.validationError)attempt.checks=[{id:'THEME-01',status:'fail',label:'Valid recipe and frozen theme',detail:record.validationError}];
      job.attempts.push(attempt);previous=candidate;save(job);
      event(job,'validating',`Rust recorded candidate ${attempt.number}: ${record.validationError?'recipe/theme validation failed.':'palette, geometry, collision, supports and sensor checks completed.'}`);
      if(!record.validationError){
        const evaluation=rustChecks(candidate,job.themeId,job.theme);
        if(JSON.stringify(evaluation.checks)!==JSON.stringify(record.checks))throw new Error('Rust pipeline and validation CLI checks diverged.');
        attempt.rustMovement=evaluation.movement;attempt.engine=evaluation.engine;
      }
      event(job,'rendering','Capturing three actual game views and running the commander through the client movement path.');
      let evidence;
      try{evidence=await capture(job,attempt);ensureCurrent(job);}
      catch(e){if(record.validationError){syncPipeline(job,'cancel');job.stopReason='Invalid candidate could not be rendered for required evidence.';event(job,'blocked',job.stopReason);return;}throw e;}
      attempt.images=evidence.images;attempt.movement=evidence.movement;
      attempt.imageHashes=attempt.images.map(url=>{const file=imagePath(job,url);if(!file||!fs.existsSync(file))throw new Error('Captured evidence file is missing.');return createHash('sha256').update(fs.readFileSync(file)).digest('hex');});
      const access=attempt.checks.find(c=>c.id==='GEN-05.access');
      if(access){access.status=access.status==='pass'&&evidence.movement.pass?'pass':'fail';access.trace=evidence.movement.trace;}
      if(!record.validationError&&attempt.rustMovement.pass!==evidence.movement.pass)throw new Error('Browser/Rust movement parity failed. Candidate cannot be accepted.');
      event(job,'reviewing',job.provider==='fixture'?'Applying explicitly simulated visual verdict; functional checks and captures are real.':'Independent Gemini reviewer is inspecting the actual rendered world against the theme contract.');
      const visual=record.validationError?{themeStatus:'fail',renderStatus:'fail',findings:record.validationError,usage:null}:await review(job,attempt);
      attempt.review={themeStatus:visual.themeStatus,renderStatus:visual.renderStatus,findings:visual.findings,comparison:visual.comparison||null,simulated:job.provider==='fixture'};
      ensureCurrent(job);
      const visualFindings=reviewFindings(visual.findings),allPass=visual.themeStatus==='pass'&&visual.renderStatus==='pass';
      const comparison=visual.comparison;
      const comparisonDefects=(comparison?.defects||[]).map(d=>`View ${d.view}, ${d.location} (${d.criterion}): ${d.issue} Repair: ${d.repair}`);
      const regressions=(comparison?.regressions||[]).map(d=>`Regression: ${d}`);
      const comparisonFailures=comparison&&['unchanged','regressed','blocked'].includes(comparison.status)?[`Comparative verdict ${comparison.status}: ${comparison.summary}`]:[];
      const defects=[...(allPass?[]:[visualFindings||'Visual review did not pass.']),...comparisonDefects,...regressions,...comparisonFailures];
      const summary=allPass?(visualFindings||'Visual theme and render review passed.'):null;
      attempt.checks.push(
        {id:'THEME-02',status:visual.themeStatus,label:'Visual theme consistency',detail:visualFindings||summary||'Theme visual check completed.'},
        {id:'LIVE-04',status:visual.renderStatus,label:'Three actual rendered views',detail:visualFindings||summary||'Three rendered views reviewed.'}
      );
      save(job);
      const verdict=syncPipeline(job,'decide',{review:{themeStatus:visual.themeStatus,renderStatus:visual.renderStatus,findings:defects,summary,...(comparison?{comparison}:{})},evidence:{candidateHash:attempt.hash,imageHashes:attempt.imageHashes,imageUrls:attempt.images}});
      attempt.decision=verdict;
      attempt.decisionReason=verdict==='accept'?(summary||'Required checks passed with a justified review.'):[...attempt.checks.filter(c=>c.status!=='pass').map(c=>`${c.id}: ${c.detail}`),...defects,comparison?.summary].filter(Boolean).join(' ')||`Rust decision: ${verdict}`;
      save(job);
      if(verdict==='accept'){
        job.acceptedHash=attempt.hash;job.stopReason=summary||'All required checks passed.';event(job,'accepted',job.provider==='fixture'?'Rehearsal candidate accepted. Visual review was simulated.':'All required checks passed. Candidate is ready to activate.');return;
      }
      if(verdict==='blocked'||verdict==='exhausted'){
        job.stopReason=verdict==='blocked'?'Review is inconclusive; evidence or operator input required.':'Rust pipeline attempt, provider-call, or time budget exhausted.';event(job,verdict,job.stopReason);return;
      }
      findings=[...attempt.checks.filter(check=>check.status!=='pass').map(check=>`${check.id}: ${check.detail}`),...defects,...(comparison?.strengths||[]).map(s=>`Preserve this strength: ${s}`)];
      event(job,'repairing',findings.join(' '));
    }
  }catch(e){
    if(job.pipeline?.status==='active'&&!job.cancelled){try{syncPipeline(job,'cancel');}catch{}}
    job.stopReason=job.cancelled?'Cancelled; late results cannot activate.':e.message;
    const state=job.cancelled?'cancelled':job.pipeline?.status==='exhausted'?'exhausted':'blocked';
    event(job,state,job.stopReason);
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
      const input=await body(req,pathname==='/api/render-task'?8_000_000:50000);
      if(pathname==='/api/themes/resolve'){
        if(!themeInferenceEnabled)return json(res,403,{error:'Live theme inference is prepared but disabled pending an operator-approved spending ceiling. Use the editable contract or labeled local draft.'});
        if(!apiKey)return json(res,400,{error:'Configure GEMINI_API_KEY to resolve a theme with Gemini.'});
        const spendId=spending.reserve({jobId:'theme-draft',label:'theme inference',model,managed:false});
        let draft;try{draft=await inferTheme({prompt:input.prompt,apiKey,model,signal:AbortSignal.timeout(120000)});spending.settle(spendId,draft.usage);}catch(error){spending.unknown(spendId);throw error;}
        const id=randomUUID();atomic(path.join(data,`theme-draft-${id}.json`),{id,sourceRevision,model,prompt:input.prompt,theme:draft.theme,usage:draft.usage,response:draft.response});
        return json(res,200,{theme:draft.theme,usage:draft.usage,draftId:id,requiresReview:true});
      }
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
        const theme=validateTheme(structuredClone(input.theme||THEMES[input.themeId]));
        if((input.themeId&&input.themeId!==theme.id)||!['fixture','gemini','managed'].includes(input.provider)||typeof input.prompt!=='string'||input.prompt.trim().length<5||input.prompt.length>3000)return json(res,400,{error:'Provide a matching theme contract, provider, and 5–3000 character prompt.'});
        if(input.provider!=='fixture'&&!apiKey)return json(res,400,{error:'Configure GEMINI_API_KEY in .env and restart the server. Offline rehearsal is available now.'});
        const now=Math.max(Date.now(),lastPipelineTime+1);lastPipelineTime=now;
        const pipeline=rustSync({op:'create',theme,prompt:input.prompt,now,provenance:{rehearsal:input.provider==='fixture',provider:input.provider,model:input.provider==='fixture'?'fixture':model,agent:input.provider==='managed'?agent:null,sourceRevision,sourceFingerprint}}).pipeline;
        const job={id:pipeline.contract.jobId,pipeline,sourceRevision,sourceFingerprint,rustBinarySha256,prompt:input.prompt,themeId:theme.id,theme,themeHash:pipeline.contract.themeSha256,provider:input.provider,model:input.provider==='fixture'?'fixture':model,agent:input.provider==='managed'?agent:null,interactionId:null,environmentId:null,injectDefect:input.injectDefect===true,contractVersion:2,createdAt:new Date(pipeline.contract.createdAtMs).toISOString(),deadline:pipeline.contract.deadlineMs,status:'queued',calls:0,callLog:[],attempts:[],events:[],providerResponses:[]};
        jobs.set(job.id,job);controllers.set(job.id,new AbortController());event(job,'queued','Brief and theme contract frozen. Job limit: three candidates, six paid calls, twenty minutes.');
        json(res,201,job);void run(job);return;
      }
      const match=pathname.match(/^\/api\/jobs\/([\da-z-]{1,128})\/(cancel|activate)$/);
      if(match){
        const job=jobs.get(match[1]);if(!job)return json(res,404,{error:'Unknown job'});
        if(match[2]==='cancel'){
          if(['accepted','active'].includes(job.status))return json(res,409,{error:'This run has finished. Use rollback for an active release.'});
          job.cancelled=true;controllers.get(job.id)?.abort();if(job.pipeline?.status==='active')try{syncPipeline(job,'cancel');}catch{}event(job,'cancelled','Cancelled. No late candidate can activate.');return json(res,200,job);
        }
        const attempt=verifiedAttempt(job);
        if(job.status!=='accepted'||!attempt)return json(res,409,{error:'Only the exact accepted candidate and its reviewed evidence can activate.'});
        const priorJob=jobs.get(releases.at(-1)?.jobId);if(priorJob?.status==='active')event(priorJob,'accepted','Superseded by a newer active package; this candidate remains approved.');
        const canonical=job.pipeline?rustSync({op:'package',pipeline:job.pipeline,now:Date.now()}).value:{recipe:attempt.recipe};
        if(canonical.manifest?.contentSha256!==undefined&&canonical.manifest.contentSha256!==attempt.hash)return json(res,409,{error:'Rust package hash does not match the accepted candidate.'});
        releases.push({id:randomUUID(),jobId:job.id,hash:attempt.hash,recipe:canonical.recipe,theme:job.theme,rehearsal:job.provider==='fixture',at:new Date().toISOString()});atomic(releaseFile,releases);event(job,'active','Verified package activated in the preview.');return json(res,200,releases.at(-1));
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
    if(pathname==='/api/config')return json(res,200,{keyConfigured:!!apiKey,model,agent,themes:THEMES,themeInferenceEnabled,spending:spending.summary()});
    if(pathname==='/api/render-task'){
      lastRendererSeen=Date.now();const task=renderTasks.values().next().value;
      return json(res,200,task?{token:task.token,jobId:task.jobId,attemptNumber:task.attemptNumber,candidateHash:task.candidateHash,recipe:task.recipe,theme:task.theme}:null);
    }
    if(pathname==='/api/jobs')return json(res,200,[...jobs.values()].sort((a,b)=>Date.parse(b.createdAt)-Date.parse(a.createdAt)).map(({providerResponses,lastCreatorPrompt,...job})=>job));
    if(pathname==='/api/release')return json(res,200,releases.at(-1)||null);
    const packageMatch=pathname.match(/^\/api\/jobs\/([\da-z-]{1,128})\/package$/);
    if(packageMatch){
      const job=jobs.get(packageMatch[1]),attempt=verifiedAttempt(job);
      if(!attempt)return json(res,409,{error:'Package export requires an accepted candidate and unchanged evidence.'});
      const value=job.pipeline?rustSync({op:'package',pipeline:job.pipeline,now:Date.now()}).value:{format:'living-empires.asset',schemaVersion:1,assetType:'building',assetId:job.id,themeId:job.themeId,theme:job.theme,recipe:attempt.recipe,manifest:{version:attempt.number,contentSha256:attempt.hash,themeSha256:job.themeHash,sourceRevision:job.sourceRevision,sourceFingerprint:job.sourceFingerprint,engine:attempt.engine,contractVersion:job.contractVersion,rehearsal:job.provider==='fixture',createdAt:attempt.createdAt,checks:attempt.checks,evidence:attempt.images.map((url,i)=>({view:i+1,url,sha256:attempt.imageHashes[i]})),provider:job.provider,model:job.model,agent:job.agent}};
      value.manifest.adapter={renderer:attempt.renderSource||'headless-chromium',sourceUrls:[],
        ...(attempt.nativeEvidence?{nativeEvidenceSha256:digest(attempt.nativeEvidence),functionalHarness:attempt.functionalHarness}: {})};
      res.writeHead(200,{'Content-Type':'application/json','Content-Disposition':`attachment; filename="asset-${job.id}.json"`,'Cache-Control':'no-store'});res.end(JSON.stringify(value,null,2));return;
    }
    const match=pathname.match(/^\/api\/jobs\/([\da-z-]{1,128})$/);
    if(match){const job=jobs.get(match[1]);return json(res,job?200:404,job||{error:'Unknown job'});}
    let file;
    if(pathname==='/vendor/three.js')file=path.join(root,'node_modules/three/build/three.module.js');
    else if(pathname==='/vendor/three.core.js')file=path.join(root,'node_modules/three/build/three.core.js');
    else if(/^\/evidence\/[\da-z-]{1,128}\/\d+-[0-2]\.png$/.test(pathname))file=path.join(data,...pathname.split('/').slice(2));
    else {file=path.resolve(publicRoot,pathname==='/'?'index.html':'.'+pathname);if(!file.startsWith(publicRoot+path.sep))return json(res,403,{error:'Forbidden'});}
    if(!fs.existsSync(file)||!fs.statSync(file).isFile())return json(res,404,{error:'Not found'});
    res.writeHead(200,{'Content-Type':mime[path.extname(file)]||'application/octet-stream','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'});fs.createReadStream(file).pipe(res);
  }catch(e){json(res,400,{error:e.message});}
});
server.listen(port,'127.0.0.1',()=>console.log(`Living Empires Studio: ${base}\nGemini: ${apiKey?'configured':'not configured — offline rehearsal available'}`));
process.on('SIGINT',async()=>{for(const c of controllers.values())c.abort();await browser?.close();server.close();process.exit(0);});
