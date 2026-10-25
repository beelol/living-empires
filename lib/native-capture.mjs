import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {spawn} from 'node:child_process';

const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const png=Buffer.from([137,80,78,71,13,10,26,10]);
const viewNames=['front','three-quarter','rear'];
const ensure=(ok,message)=>{if(!ok)throw new Error(message);};

async function fileHash(file){
  const hash=createHash('sha256');
  for await(const chunk of fs.createReadStream(file))hash.update(chunk);
  return hash.digest('hex');
}

function render(binary,input,output,signal,timeout){
  return new Promise((resolve,reject)=>{
    let finished=false;
    const child=spawn(binary,[input,output],{stdio:['ignore','ignore','pipe']});
    let log='';
    const done=(error)=>{if(finished)return;finished=true;clearTimeout(timer);signal?.removeEventListener('abort',abort);error?reject(error):resolve(log);};
    const abort=()=>{child.kill('SIGTERM');done(new Error('Native capture cancelled'));};
    const timer=setTimeout(()=>{child.kill('SIGTERM');done(new Error('Native capture deadline exceeded'));},timeout);
    child.stderr.on('data',b=>{if(log.length<32768)log+=b.toString().slice(0,32768-log.length);});
    child.on('error',()=>done(new Error('Native renderer could not start')));
    child.on('close',code=>done(code===0?null:new Error(`Native renderer failed (${code}); evidence remains unapproved`)));
    if(signal?.aborted)abort();else signal?.addEventListener('abort',abort,{once:true});
  });
}

/** Native visual evidence plus an explicitly separate existing browser movement result. */
export async function captureNative({root,binary,job,attempt,movement,signal,timeoutMs=90000}){
  ensure(/^[a-z0-9-]{1,128}$/.test(job.id)&&Number.isInteger(attempt.number)&&attempt.number>=1&&attempt.number<=3,'Invalid native capture job');
  ensure(/^[a-f0-9]{64}$/.test(attempt.hash),'Invalid candidate fingerprint');
  ensure(binary&&fs.existsSync(binary),'Build the native foundry_audit example before native review');
  const folder=path.join(root,'.data',job.id);fs.mkdirSync(folder,{recursive:true});
  const attemptFolder=fs.mkdtempSync(path.join(folder,`native-${attempt.number}-`));
  const output=path.join(attemptFolder,'views'),input=path.join(attemptFolder,'candidate-preview.json');
  const candidate={format:'living-empires.asset',schemaVersion:1,assetType:'building',assetId:job.id,
    themeId:job.themeId,theme:job.theme,recipe:attempt.recipe,manifest:{contentSha256:attempt.hash,
      rehearsal:job.provider==='fixture',provider:job.provider,model:job.model,renderOnly:true}};
  fs.writeFileSync(input,JSON.stringify(candidate),{mode:0o600});
  const rendererBinarySha256=await fileHash(binary);
  const log=await render(binary,input,output,signal,timeoutMs);
  fs.writeFileSync(path.join(attemptFolder,'renderer.log'),log,{mode:0o600});
  const manifest=JSON.parse(fs.readFileSync(path.join(output,'capture-manifest.json'),'utf8'));
  ensure(manifest.package?.candidateHash===attempt.hash,'Native capture candidate differs from requested candidate');
  ensure(Array.isArray(manifest.views)&&manifest.views.length===3,'Native capture must provide three views');
  const views=viewNames.map((name,i)=>{
    const file=`view-${i+1}-${name}.png`;
    ensure(manifest.views[i].file===file,'Unexpected native view path');
    const bytes=fs.readFileSync(path.join(output,file));
    // Three base64 frames plus theme/prompt must fit world-check's 12MB input ceiling.
    ensure(bytes.length<=2_900_000&&bytes.subarray(0,8).equals(png),'Native view is missing, invalid or too large');
    return {view:i+1,sha256:sha(bytes),atUnixSeconds:Math.floor(fs.statSync(path.join(output,file)).mtimeMs/1000),
      sourceFile:path.relative(folder,path.join(output,file)),publicFile:`${attempt.number}-${i}.png`,bytes};
  });
  ensure(new Set(views.map(v=>v.sha256)).size===3,'Native views must be distinct');
  // Validate the complete capture before replacing the candidate's public review images.
  for(const view of views){
    const target=path.join(folder,view.publicFile);
    if(fs.existsSync(target))fs.copyFileSync(target,path.join(attemptFolder,`browser-${view.view}.png`));
    fs.writeFileSync(target,view.bytes,{mode:0o600});
  }
  const nativeEvidence={renderer:'Bevy 0.20 native',candidateSha256:attempt.hash,
    rendererBinarySha256,coordinateMapping:manifest.coordinateMapping||null,
    sourceRevision:job.sourceRevision,views:views.map(({bytes,...v})=>v),
    functionalHarness:'three-browser',nativeTraversal:'not_run'};
  fs.writeFileSync(path.join(attemptFolder,'evidence.json'),JSON.stringify(nativeEvidence,null,2),{mode:0o600});
  return {images:views.map(v=>`/evidence/${job.id}/${v.publicFile}`),movement,
    renderSource:'bevy-native',functionalHarness:'three-browser',nativeEvidence};
}
