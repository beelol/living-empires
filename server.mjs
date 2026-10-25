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
import {fastLimits,retryLimits,remainingTime,armDeadline} from './lib/job-policy.mjs';

const root=path.dirname(fileURLToPath(import.meta.url));
const publicRoot=process.env.ASSET_PUBLIC_DIR||path.join(root,'public');
const data=process.env.ASSET_DATA_DIR||path.join(root,'.data'); fs.mkdirSync(data,{recursive:true});
const spending=new SpendLedger(data,process.env.GOOGLE_SESSION_BUDGET_USD,{sponsoredCredits:process.env.GOOGLE_SPONSORED_CREDITS==='1'});
const port=Number(process.env.PORT||4317), base=`http://127.0.0.1:${port}`;
const apiKey=process.env.GEMINI_API_KEY||process.env.GOOGLE_API_KEY;
const model=process.env.GEMINI_MODEL||'gemini-3.8-flash';
const roleModels=Object.freeze({creator:process.env.CREATOR_MODEL||model,repair:process.env.REPAIR_MODEL||model,reviewer:process.env.REVIEWER_MODEL||model});
const agent=process.env.GEMINI_AGENT||'antigravity-preview-09-2026';
const themeInferenceEnabled=process.env.THEME_INFERENCE_ENABLED==='1';
const jobs=new Map(), controllers=new Map();
const renderTasks=new Map();
let lastRendererSeen=0;
let lastPipelineTime=0;
let inferenceStarting=false;
let browser;
const digest=v=>createHash('sha256').update(JSON.stringify(v)).digest('hex');
const rustBinary=process.env.RUST_CORE_BIN||path.join(root,'target/debug/world-check');
const rustBinarySha256=fs.existsSync(rustBinary)?createHash('sha256').update(fs.readFileSync(rustBinary)).digest('hex'):null;
const sourceRevision=spawnSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).stdout?.trim()||'unknown';
const sourceFingerprint=digest(['server.mjs','public/app.mjs','public/style.css','public/index.html','public/themes.mjs','public/core.mjs','public/scene.mjs','public/render-worker.mjs','lib/spend-ledger.mjs','lib/job-policy.mjs','lib/theme-inference.mjs','lib/native-capture.mjs','game/examples/foundry_audit.rs','crates/living-world-core/src/lib.rs','crates/living-world-core/src/main.rs','crates/living-world-core/src/pipeline.rs','crates/living-world-core/src/provider.rs','crates/living-world-core/fixtures/presets.json','Cargo.lock','package-lock.json'].map(file=>{const source=file.startsWith('public/')?path.join(publicRoot,file.slice(7)):path.join(root,file);return [file,fs.existsSync(source)?fs.readFileSync(source,'utf8'):null];}).concat([['world-check-binary-sha256',rustBinarySha256]]));
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
  if(job.deadline!==null&&job.deadline!==undefined&&Date.now()>=job.deadline)throw new Error('Job deadline exhausted');
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
  ensureCurrent(job);
  const timeout=remainingTime(job.deadline,input.op==='generate'&&input.request.managed?330000:120000);
  const role=input.op==='review'?'reviewer':job.pipeline.attemptCount===0?'creator':'repair';
  const frozenModel=job.pipeline.contract.roleModels?.[role]||job.pipeline.contract.provenance.model;
  if(input.request.model!==frozenModel)throw new Error('Provider role model differs from the frozen contract.');
  const spendId=spending.reserve({jobId:job.id,label,model:input.request.model,managed:input.op==='generate'&&input.request.managed});
  syncPipeline(job,'charge');
  const entry={number:job.pipeline.providerCalls,label,role:input.op==='review'?'reviewer':job.pipeline.attemptCount===0?'creator':'repair',model:input.request.model,agent:input.op==='generate'&&input.request.managed?input.request.agent:null,managed:input.op==='generate'&&input.request.managed,spendId,status:'dispatched',at:new Date().toISOString()};job.callLog.push(entry);job.calls=job.pipeline.providerCalls;job.spending=spending.summary();save(job);
  try{
    const result=await rustAsync(input,controllers.get(job.id)?.signal,timeout);
    if(result.providerError){
      const received=redactReference(job,JSON.parse(JSON.stringify(result.providerError.response).replaceAll(apiKey||'__no_key__','[redacted]')));
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
function referenceInput(input){
  const images=input.referenceImages??[];
  if(!Array.isArray(images)||images.length>1)throw new Error('At most one reference image is supported.');
  if(!images.length){if(input.referenceConsent===true)throw new Error('Select a reference image before consenting.');return {images:[],reference:null};}
  if(input.referenceConsent!==true)throw new Error('Sending a reference to Google Gemini requires the explicit image consent checkbox.');
  const image=images[0];
  if(!image||!['image/png','image/jpeg'].includes(image.mimeType)||typeof image.data!=='string'||image.data.length>2_796_204||!image.data.length||!/^[A-Za-z0-9+/]*={0,2}$/.test(image.data))throw new Error('Reference must be a bounded base64 PNG or JPEG.');
  const bytes=Buffer.from(image.data,'base64');
  if(!bytes.length||bytes.length>2*1024*1024||bytes.toString('base64')!==image.data)throw new Error('Reference image is invalid or exceeds 2 MiB.');
  if(image.mimeType==='image/png'){
    if(bytes.length<57||!bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))||bytes.readUInt32BE(8)!==13||bytes.toString('ascii',12,16)!=='IHDR'||!bytes.readUInt32BE(16)||!bytes.readUInt32BE(20))throw new Error('Reference PNG header is invalid.');
    let offset=8,ended=false,hasData=false;while(offset+12<=bytes.length){const length=bytes.readUInt32BE(offset),type=bytes.toString('ascii',offset+4,offset+8);if(length>bytes.length-offset-12)throw new Error('Reference PNG is truncated.');if(type==='IDAT'&&length>0)hasData=true;offset+=length+12;if(type==='IEND'){ended=length===0&&offset===bytes.length;break;}}
    if(!ended||!hasData)throw new Error('Reference PNG is incomplete.');
  }else if(bytes.length<4||bytes[0]!==255||bytes[1]!==216||bytes[2]!==255||bytes.at(-2)!==255||bytes.at(-1)!==217)throw new Error('Reference JPEG signature or end marker is invalid.');
  return {images:[{mimeType:image.mimeType,data:image.data}],reference:{sha256:createHash('sha256').update(bytes).digest('hex'),mimeType:image.mimeType,bytes:bytes.length,destination:'Google Gemini',consent:true}};
}
function redactReference(job,value){let serialized=JSON.stringify(value);for(const image of job.referenceImages||[])serialized=serialized.replaceAll(image.data,'[reference image omitted]');return JSON.parse(serialized);}
function redactPrepared(value){
  if(Array.isArray(value))return value.map(redactPrepared);
  if(value&&typeof value==='object'){const next={};for(const [key,item]of Object.entries(value))next[key]=key==='data'&&typeof item==='string'&&['image/png','image/jpeg'].includes(value.mimeType||value.mime_type)?'[reference image omitted]':redactPrepared(item);return next;}
  return value;
}
function refinementSource(input,theme){
  if(!input.baseJobId){if(input.componentEdit)throw new Error('Component edits require an exact saved baseline.');return null;}
  const baseJob=jobs.get(input.baseJobId),number=Number(input.baseAttemptNumber),attempt=baseJob?.attempts.find(a=>a.number===number);
  if(!attempt?.recipe||attempt.hash!==input.baseCandidateHash||attempt.images?.length!==3||attempt.imageHashes?.length!==3)throw new Error('Select an existing captured baseline version and its exact candidate hash.');
  if(stableJSON(baseJob.theme)!==stableJSON(theme))throw new Error('Refinement preserves the selected version’s frozen artistic direction. Start a new scene to change that contract.');
  if(typeof input.userFeedback!=='string'||input.userFeedback.trim().length<3||input.userFeedback.length>2000)throw new Error('Describe the requested improvement in 3–2000 characters.');
  for(let i=0;i<3;i++){const file=imagePath(baseJob,attempt.images[i]);if(!file||!fs.existsSync(file)||createHash('sha256').update(fs.readFileSync(file)).digest('hex')!==attempt.imageHashes[i])throw new Error('The selected baseline evidence is missing or changed.');}
  let componentEdit=null,feedback=input.userFeedback.trim();
  if(input.componentEdit){
    const edit=input.componentEdit,components=attempt.recipe.scene?.components;
    if(!edit||typeof edit!=='object'||Object.keys(edit).some(k=>!['mode','componentId'].includes(k))||!['add','modify'].includes(edit.mode)||typeof edit.componentId!=='string'||!(/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/).test(edit.componentId)||edit.componentId.length>64||!components)throw new Error('Choose one valid component edit and a scene baseline.');
    const exists=components.some(c=>c.id===edit.componentId);
    if((edit.mode==='add'&&exists)||(edit.mode==='modify'&&!exists))throw new Error('Added IDs must be new; modified IDs must exist in the baseline.');
    componentEdit={mode:edit.mode,componentId:edit.componentId};
    const name=input.componentName;if(edit.mode==='add'&&(typeof name!=='string'||!name.trim()||name.length>100))throw new Error('Name the new component in at most 100 characters.');
    feedback+=`\nFrozen scope: ${edit.mode} only component ${edit.componentId}${edit.mode==='add'?` named ${name.trim()}`:''}. Preserve all untargeted components and parts exactly, and preserve the baseline name, description, theme, sensor radius and scene extent. A visual variation must change silhouette, layout, proportions, materials or a signature feature meaningfully; palette-only edits are insufficient.`;
  }
  const record=baseJob.pipeline?.candidates.find(c=>c.candidateHash===attempt.hash);
  if(baseJob.pipeline&&(!record||stableJSON(record.recipe)!==stableJSON(attempt.recipe)))throw new Error('Baseline recipe does not match its Rust candidate record.');
  return {jobId:baseJob.id,number,hash:attempt.hash,recipe:structuredClone(attempt.recipe),images:[...attempt.images],imageHashes:[...attempt.imageHashes],componentEdit,userFeedback:feedback,findings:[...attempt.checks.filter(c=>c.status!=='pass').map(c=>`${c.id}: ${c.detail}`),...(attempt.review?.comparison?.defects||[]).map(d=>`View ${d.view}, ${d.location}: ${d.issue}. Repair: ${d.repair}`),...(attempt.review?.comparison?.strengths||[]).map(s=>`Preserve: ${s}`)]};
}
function generationRequest(job,previous,findings){
  const models=job.roleModels||{creator:job.model||roleModels.creator,repair:job.model||roleModels.repair};
  const selectedModel=(job.pipeline?.attemptCount||0)===0?models.creator:models.repair;
  if(job.pipeline&&stableJSON(job.reference||null)!==stableJSON(job.pipeline.contract.reference||null))throw new Error('The frozen reference image consent or identity changed; create a new job.');
  return {theme:job.theme,prompt:job.prompt,userFeedback:job.userFeedback||null,componentEdit:job.componentEdit||null,componentBaseline:job.componentEdit?(job.baseline?.recipe||previous):null,sceneHalfExtent:job.sceneHalfExtent??job.pipeline?.contract.sceneHalfExtent??null,previous:previous||null,findings:findings.map(item=>typeof item==='string'?item:`${item.id}: ${item.detail}`),referenceImages:job.referenceImages||[],referenceConsent:job.reference?.consent===true,referenceSha256:job.reference?.sha256||null,referenceMimeType:job.reference?.mimeType||null,model:selectedModel,agent,managed:job.provider==='managed',interactionId:job.interactionId||null,environmentId:job.environmentId||null};
}
async function generate(job,previous,findings){
  if(job.provider==='fixture'){
    const fixtureBase=previous||job.previewRecipe;
    const result=rustSync({op:'fixture',theme:job.theme,...(fixtureBase?{previous:fixtureBase}: {})});
    await new Promise(resolve=>setTimeout(resolve,350));ensureCurrent(job);return result.recipe;
  }
  const request=generationRequest(job,previous,findings);
  job.creatorInputs=job.creatorInputs||[];job.creatorInputs.push({attempt:job.pipeline.attemptCount+1,baselineHash:job.baseline?.hash||null,previousRecipeHash:job.attempts.at(-1)?.hash||job.baseline?.hash||null,userFeedback:request.userFeedback,findings:request.findings,sceneHalfExtent:request.sceneHalfExtent,reference:job.reference||null,model:request.model,agent:request.managed?request.agent:null});save(job);
  const result=await providerCall(job,'Gemini creator',{op:'generate',request});
  job.interactionId=result.interactionId||job.interactionId;job.environmentId=result.environmentId||job.environmentId;
  job.providerResponses.push({kind:'creator',model:request.model,agent:request.managed?request.agent:null,response:redactReference(job,result.response),usage:result.usage||null});save(job);return result.recipe;
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
    const native=await captureNative({root,dataDir:data,job,attempt,movement:functional.movement,
      binary:process.env.NATIVE_AUDIT_BIN||path.join(root,'target/debug/examples/foundry_audit'),
      signal:controllers.get(job.id)?.signal,
      timeoutMs:remainingTime(job.deadline,90000)});
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
    renderTasks.set(token,{token,jobId:job.id,attemptNumber:attempt.number,candidateHash:attempt.hash,recipe:attempt.recipe,theme:job.theme,sceneHalfExtent:job.sceneHalfExtent??job.pipeline?.contract.sceneHalfExtent??4,
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
  const prior=job.attempts.at(-2)||job.baseline;
  const previous=prior?{candidateHash:prior.hash,imageHashes:prior.imageHashes,images:imageInputs(prior),strengths:prior.review?.comparison?.strengths||[]}:null;
  // Keep the original scene visible to the critic after a rejected intermediate
  // attempt; improving a degraded draft alone must not justify acceptance.
  const retainedBaseline=job.baseline&&prior!==job.baseline?{candidateHash:job.baseline.hash,imageHashes:job.baseline.imageHashes,images:imageInputs(job.baseline),strengths:job.baseline.findings?.filter(finding=>finding.startsWith('Preserve: '))||[]}:null;
  const request={theme:job.theme,prompt:job.prompt,userFeedback:job.userFeedback||null,componentEdit:job.componentEdit||null,retainedBaseline,sceneHalfExtent:job.sceneHalfExtent,candidateHash:attempt.hash,imageHashes:attempt.imageHashes,images:imageInputs(attempt),previous,model:job.roleModels?.reviewer||job.model||roleModels.reviewer};
  job.reviewerInputs=job.reviewerInputs||[];job.reviewerInputs.push({attempt:attempt.number,candidateHash:attempt.hash,previousCandidateHash:previous?.candidateHash||null,retainedBaselineHash:retainedBaseline?.candidateHash||null,componentEdit:request.componentEdit,userFeedback:request.userFeedback});save(job);
  const result=await providerCall(job,'independent comparative visual reviewer',{op:'review',request});
  job.providerResponses.push({kind:'reviewer',model:request.model,agent:null,response:result.response,usage:result.usage||null});save(job);return result;
}
function geometryDelta(previous,current){
  const diff=(before,after)=>{const old=new Map(before.map(p=>[p.id,p])),next=new Map(after.map(p=>[p.id,p]));return {added:[...next.keys()].filter(id=>!old.has(id)),changed:[...next.keys()].filter(id=>old.has(id)&&stableJSON(old.get(id))!==stableJSON(next.get(id))),removed:[...old.keys()].filter(id=>!next.has(id))};};
  const parts=diff(previous?.parts||[],current.parts),components=diff(previous?.scene?.components||[],current.scene?.components||[]);
  return {addedParts:parts.added,changedParts:parts.changed,removedParts:parts.removed,componentsAdded:components.added,componentsChanged:components.changed,componentsRemoved:components.removed};
}
async function prepareAndRun(job,baseSource){
  if(!baseSource)return run(job);
  try{
    event(job,'preparing','Recapturing the selected baseline with this scene’s frozen review cameras before generation.');
    const baseline={number:0,hash:baseSource.hash,recipe:baseSource.recipe,findings:baseSource.findings,images:[],imageHashes:[]};
    job.baseline=baseline;save(job);
    const captured=await capture(job,baseline);ensureCurrent(job);baseline.images=captured.images;baseline.movement=captured.movement;
    baseline.imageHashes=baseline.images.map(url=>createHash('sha256').update(fs.readFileSync(imagePath(job,url))).digest('hex'));
    const refinement={userFeedback:job.userFeedback,...(job.componentEdit?{componentEdit:job.componentEdit}:{}),baselineRecipe:baseline.recipe,baselineEvidence:{candidateHash:baseline.hash,imageHashes:baseline.imageHashes,imageUrls:baseline.images}};
    job.pipeline=rustSync({...job.createInput,refinement}).pipeline;job.baseline=baseline;
    event(job,'queued','Artistic direction, feedback, selected baseline and matched-camera evidence frozen.');
    return run(job);
  }catch(error){job.stopReason=error.message;if(job.pipeline?.status==='active')try{syncPipeline(job,'cancel');}catch{}event(job,job.cancelled?'cancelled':job.deadlineReached?'exhausted':'blocked',error.message);controllers.delete(job.id);}
}
async function executeJob(job,baseSource){
  const stop=armDeadline(job.deadline,controllers.get(job.id),()=>{job.deadlineReached=true;});
  try{return await prepareAndRun(job,baseSource);}finally{stop();}
}
async function run(job){
  let previous=job.baseline?.recipe||null,findings=job.baseline?.findings||[];
  try{
    while(true){
      const canContinue=syncPipeline(job,'continue');
      if(!canContinue){
        const state=job.pipeline.status;
        job.stopReason=state==='exhausted'?'Rust pipeline attempt, provider-call, or time budget exhausted.':'No more candidates can be evaluated.';
        event(job,state==='blocked'?'blocked':state==='cancelled'?'cancelled':'exhausted',job.stopReason);return;
      }
      const attempt={number:job.pipeline.attemptCount+1,checks:[],images:[],imageHashes:[],repairRequested:[...findings],previousHash:job.attempts.at(-1)?.hash||null,createdAt:new Date().toISOString()};
      event(job,'generating',`${attempt.number===1?'Generating':'Repairing'} candidate ${attempt.number}/${job.pipeline.contract.limits.maxAttempts} in the frozen ${job.theme.name} theme.`);
      let candidate=await generate(job,previous,findings);ensureCurrent(job);
      if(job.injectDefect&&attempt.number===1)candidate=injectDefect(candidate);
      const record=syncPipeline(job,'submit',{recipe:candidate});
      candidate=record.recipe;attempt.delta=geometryDelta(previous,candidate);attempt.recipe=candidate;attempt.hash=record.candidateHash;attempt.validationError=record.validationError||null;
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
      const comparisonFailures=comparison&&((comparison.status==='unchanged'&&job.pipeline.contract.completionGoal!=='satisfy-frozen-style')||['regressed','blocked'].includes(comparison.status))?[`Comparative verdict ${comparison.status}: ${comparison.summary}`]:[];
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
    const state=job.cancelled?'cancelled':job.deadlineReached||job.pipeline?.status==='exhausted'?'exhausted':'blocked';
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
      const input=await body(req,pathname==='/api/render-task'?8_000_000:['/api/jobs','/api/refinement-preview'].includes(pathname)?3_000_000:50000);
      if(pathname==='/api/refinement-preview'){
        const reference=referenceInput(input);
        const theme=validateTheme(structuredClone(input.theme)),baseline=refinementSource(input,theme);
        if(!baseline)throw new Error('Choose a saved baseline version before previewing a refinement.');
        if(input.provider==='fixture')return json(res,200,{summary:'Offline fixtures do not use artistic feedback. Select Gemini managed agent to inspect the real refinement request; this preview makes no API call.',request:{provider:'fixture',baseCandidateHash:baseline.hash,userFeedback:baseline.userFeedback,referenceImages:'local only; not included'}});
        if(!['managed','gemini'].includes(input.provider))throw new Error('Select a generation engine.');
        const sceneHalfExtent=Number(baseline.recipe.scene?.halfExtent??jobs.get(baseline.jobId).sceneHalfExtent??4);
        const request=generationRequest({theme,prompt:input.prompt||jobs.get(baseline.jobId).prompt,provider:input.provider,userFeedback:baseline.userFeedback,componentEdit:baseline.componentEdit,sceneHalfExtent,roleModels,reference:reference.reference,referenceImages:reference.images},baseline.recipe,baseline.findings);
        const prepared=redactPrepared(rustSync({op:'prepare-generation',request}));
        return json(res,200,{summary:'Prepared locally only: the selected baseline recipe, your feedback, critic targets and frozen scene extent reach the creator request. No API call or reference-image transmission occurred.',request:{baseJobId:baseline.jobId,baseAttemptNumber:baseline.number,baseCandidateHash:baseline.hash,userFeedback:baseline.userFeedback,componentEdit:baseline.componentEdit,sceneHalfExtent,reference:reference.reference,...prepared}});
      }
      if(pathname==='/api/themes/resolve'){
        if(!themeInferenceEnabled)return json(res,403,{error:'Live theme inference is prepared but disabled pending an operator-approved spending ceiling. Use the editable contract or labeled local draft.'});
        if(!apiKey)return json(res,400,{error:'Configure GEMINI_API_KEY to resolve a theme with Gemini.'});
        const spendId=spending.reserve({jobId:'theme-draft',label:'theme inference',model,managed:false});
        let draft;try{draft=await inferTheme({prompt:input.prompt,apiKey,model,signal:AbortSignal.timeout(120000)});spending.settle(spendId,draft.usage);}catch(error){if(error.themeInference){const id=randomUUID();spending.settle(spendId,error.themeInference.usage);atomic(path.join(data,`theme-draft-${id}.json`),{id,sourceRevision,model,prompt:input.prompt,status:'invalid',error:error.message,...error.themeInference,normalizations:error.themeInference.normalizations||[]});}else spending.unknown(spendId);throw error;}
        const id=randomUUID();atomic(path.join(data,`theme-draft-${id}.json`),{id,sourceRevision,model,prompt:input.prompt,theme:draft.theme,normalizations:draft.normalizations||[],usage:draft.usage,response:draft.response});
        return json(res,200,{theme:draft.theme,normalizations:draft.normalizations||[],usage:draft.usage,draftId:id,requiresReview:true});
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
        if(inferenceStarting)return json(res,409,{error:'Art direction is already being inferred. Wait for that brief to freeze.'});
        if([...jobs.values()].some(j=>!['accepted','active','failed','blocked','cancelled','exhausted'].includes(j.status)))return json(res,409,{error:'A run is already in progress. Cancel it first.'});
        const reference=referenceInput(input);
        if(reference.images.length&&input.provider==='fixture')throw new Error('Reference images require a Gemini engine; clear the image to run an offline fixture.');
        if(!['fixture','gemini','managed'].includes(input.provider)||typeof input.prompt!=='string'||input.prompt.trim().length<5||input.prompt.length>3000)return json(res,400,{error:'Provide a generation engine and a 5–3000 character creative brief.'});
        let theme=validateTheme(structuredClone(input.theme||THEMES[input.themeId]||(input.inferTheme===true?THEMES.argonaut:null)));
        let artDirection=null;
        if(input.inferTheme===true){
          if(input.baseJobId)throw new Error('Saved-version refinement preserves its existing frozen art direction.');
          if(!themeInferenceEnabled)throw new Error('Automatic Gemini art direction is disabled in this runtime.');
          if(input.provider==='fixture'||!apiKey)throw new Error('Automatic art direction requires a configured Gemini engine.');
          if(typeof input.prompt!=='string'||input.prompt.trim().length<5||input.prompt.length>3000)throw new Error('Provide a 5–3000 character creative brief.');
          const id=randomUUID(),startedAt=Date.now(),spendId=spending.reserve({jobId:`art-direction-${id}`,label:'prompt art direction',model,managed:false});
          const draftFile=path.join(data,`theme-draft-${id}.json`);
          atomic(draftFile,{id,status:'dispatched',model,prompt:input.prompt,startedAt,spendId});
          inferenceStarting=true;
          try{
            const draft=await inferTheme({prompt:input.prompt,apiKey,model,signal:AbortSignal.timeout(120000)});
            spending.settle(spendId,draft.usage);theme=validateTheme(draft.theme);
            artDirection={id,model,spendId,startedAt,usage:draft.usage||null,normalizations:draft.normalizations||[],themeSha256:digest(theme)};
            atomic(draftFile,{...artDirection,status:'returned',prompt:input.prompt,theme,response:draft.response});
          }catch(error){
            const returned=error.themeInference||null;
            if(returned)spending.settle(spendId,returned.usage);else spending.unknown(spendId);
            atomic(draftFile,{id,status:'failed_or_unknown',model,prompt:input.prompt,startedAt,spendId,error:error.message,...(returned||{}),normalizations:returned?.normalizations||[]});
            throw error;
          }
          finally{inferenceStarting=false;}
        }
        if((input.inferTheme!==true&&input.themeId&&input.themeId!==theme.id)||!['fixture','gemini','managed'].includes(input.provider)||typeof input.prompt!=='string'||input.prompt.trim().length<5||input.prompt.length>3000)return json(res,400,{error:'Provide a matching theme contract, provider, and 5–3000 character prompt.'});
        if(input.provider!=='fixture'&&!apiKey)return json(res,400,{error:'Configure GEMINI_API_KEY in .env and restart the server. Offline rehearsal is available now.'});
        const baseSource=refinementSource(input,theme);
        if(baseSource&&input.provider==='fixture')return json(res,400,{error:'Offline fixtures only demonstrate the known obstruction repair; they cannot apply artistic feedback. Select a live engine after the specific paid request is approved.'});
        const sceneHalfExtent=input.provider==='fixture'?(input.previewRecipe?.scene?.halfExtent??null):Number(baseSource?(baseSource.recipe.scene?.halfExtent??jobs.get(baseSource.jobId).sceneHalfExtent??4):(input.sceneHalfExtent??12));
        if(sceneHalfExtent!==null&&(!Number.isFinite(sceneHalfExtent)||sceneHalfExtent<4||sceneHalfExtent>16))throw new Error('Scene half extent must be 4–16 metres.');
        const now=Math.max(Date.now(),lastPipelineTime+1);lastPipelineTime=now;
        let retryGroupId=null,limits=fastLimits(),completionGoal=baseSource?null:'satisfy-frozen-style';
        if(input.retryOf){
          const prior=jobs.get(input.retryOf);
          if(!prior||prior.status!=='blocked'||!prior.callLog?.length||prior.callLog.some(call=>!['returned','returned_error'].includes(call.status))||prior.callLog.at(-1).responseStatus!=='incomplete')throw new Error('A retry requires a reconciled incomplete provider response; unknown calls cannot be retried.');
          if(stableJSON(prior.theme)!==stableJSON(theme)||prior.prompt!==input.prompt)throw new Error('A bounded retry preserves the original theme and user brief.');
          retryGroupId=prior.retryGroupId||prior.id;
          const group=[...jobs.values()].filter(j=>j.id===retryGroupId||j.retryGroupId===retryGroupId);
          const rootJob=jobs.get(retryGroupId),calls=group.reduce((n,j)=>n+(j.callLog?.length||0),0),attempts=group.reduce((n,j)=>n+j.attempts.length,0);
          if(!rootJob)throw new Error('The original retry group record is unavailable.');
          limits=retryLimits(rootJob,attempts,calls,now);
          completionGoal=rootJob.pipeline.contract.completionGoal||null;
        }
        const createInput={op:'create',theme,prompt:input.prompt,now,...(completionGoal?{completionGoal}:{}),limits,...(input.provider!=='fixture'?{roleModels}:{}),...(reference.reference?{reference:reference.reference}:{}),...(sceneHalfExtent!==null?{sceneHalfExtent}:{}),provenance:{rehearsal:input.provider==='fixture',provider:input.provider,model:input.provider==='fixture'?'fixture':roleModels.creator,agent:input.provider==='managed'?agent:null,sourceRevision,sourceFingerprint}};
        const pipeline=rustSync(createInput).pipeline;
        const job={id:pipeline.contract.jobId,pipeline,createInput,sceneHalfExtent,artDirection,inferTheme:input.inferTheme===true,retryOf:input.retryOf||null,retryGroupId,roleModels:input.provider==='fixture'?null:roleModels,reference:reference.reference,baseJobId:baseSource?.jobId||null,baseAttemptNumber:baseSource?.number||null,baseCandidateHash:baseSource?.hash||null,userFeedback:baseSource?.userFeedback||null,componentEdit:baseSource?.componentEdit||null,sourceRevision,sourceFingerprint,rustBinarySha256,prompt:input.prompt,themeId:theme.id,theme,themeHash:pipeline.contract.themeSha256,provider:input.provider,model:input.provider==='fixture'?'fixture':roleModels.creator,agent:input.provider==='managed'?agent:null,interactionId:null,environmentId:null,injectDefect:input.injectDefect===true,contractVersion:2,createdAt:new Date(pipeline.contract.createdAtMs).toISOString(),deadline:pipeline.contract.limits.timeBudgetMs===0?null:pipeline.contract.deadlineMs,status:baseSource?'preparing':'queued',calls:0,callLog:[],attempts:[],events:[],providerResponses:[]};
        Object.defineProperty(job,'referenceImages',{value:reference.images,enumerable:false});
        if(input.previewRecipe){if(input.provider!=='fixture')throw new Error('Authored recipe previews are offline only.');rustChecks(input.previewRecipe,theme.id,theme);job.previewRecipe=structuredClone(input.previewRecipe);job.previewKind='authored_scene';job.sceneHalfExtent=input.previewRecipe.scene?.halfExtent||4;}
        jobs.set(job.id,job);
        if(artDirection){
          syncPipeline(job,'charge');job.callLog.push({number:1,label:'prompt art direction',role:'art_director',model:artDirection.model,agent:null,managed:false,spendId:artDirection.spendId,status:'returned',usage:artDirection.usage,at:new Date(artDirection.startedAt).toISOString()});job.spending=spending.summary();
        }
        controllers.set(job.id,new AbortController());event(job,job.status,baseSource?'Preparing matched baseline evidence; no generation dispatched yet.':`Brief and style frozen. Up to ${pipeline.contract.limits.maxAttempts} candidates and ${pipeline.contract.limits.maxProviderCalls} paid calls ${pipeline.contract.limits.timeBudgetMs?`within ${Math.ceil(pipeline.contract.limits.timeBudgetMs/1000)} seconds`:'with no overall timeout'}; stop on independent acceptance.`);
        json(res,201,job);void executeJob(job,baseSource);return;
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
    if(pathname==='/api/config')return json(res,200,{service:'asset-foundry',sourceRoot:root,sourceRevision,protocolVersion:1,packageVersion:1,rustBinarySha256,capabilities:rustSync({op:'capabilities'}),jobLimits:fastLimits(),completionGoal:'satisfy-frozen-style',keyConfigured:!!apiKey,model,roleModels,agent,themes:THEMES,themeInferenceEnabled,spending:spending.summary()});
    if(pathname==='/api/render-task'){
      lastRendererSeen=Date.now();const task=renderTasks.values().next().value;
      return json(res,200,task?{token:task.token,jobId:task.jobId,attemptNumber:task.attemptNumber,candidateHash:task.candidateHash,recipe:task.recipe,theme:task.theme,sceneHalfExtent:task.sceneHalfExtent}:null);
    }
    if(pathname==='/api/jobs')return json(res,200,[...jobs.values()].sort((a,b)=>Date.parse(b.createdAt)-Date.parse(a.createdAt)).map(({providerResponses,lastCreatorPrompt,...job})=>({...job,spending:spending.summary()})));
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
    if(match){const job=jobs.get(match[1]);return json(res,job?200:404,job?{...job,spending:spending.summary()}:{error:'Unknown job'});}
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
