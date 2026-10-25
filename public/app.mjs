import {fixtureRecipe} from './core.mjs';
import {THEMES,validateTheme,draftThemeFromPrompt} from './themes.mjs';
import {createWorld} from './scene.mjs';
import {startRenderWorker} from './render-worker.mjs';
const $=id=>document.getElementById(id),world=createWorld($('world'));
startRenderWorker();
let currentJob=null,lastHash='',busy=false,config,releasePinned=false,pollInFlight=false,sceneRevision=0;
let themeContract=null;
const briefs={
  argonaut:'Build a sensor outpost for our machine civilization. Give it a broad ceramic canopy, a glowing cyan relay, and an entrance our commander can walk through.',
  sylvan:'Build a forest watch sanctuary. Use mossy stone, timber supports, warm amber light, and an open entrance our ranger can walk through.',
  ember:'Build a volcanic survey shelter. Use basalt forms, dark metal braces, restrained copper light, and a clear entrance our scout can walk through.'
};
let displayedBrief=briefs.argonaut;
const labels=[['THEME-01','Frozen theme & material palette'],['GEN-05.access','Commander can enter'],['GEN-06.support','Structure meets the ground'],['GEN-05.sensor','Sensor detects the test target'],['THEME-02','Visual theme consistency'],['LIVE-04','Three actual rendered views']];
const terminal=['accepted','active','failed','blocked','cancelled','exhausted'];
function el(tag,text,className){const e=document.createElement(tag);if(text!==undefined)e.textContent=text;if(className)e.className=className;return e;}
async function api(url,body){const response=await fetch(url,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const value=await response.json();if(!response.ok)throw new Error(value.error||'Request failed');return value;}
function error(e){$('error').textContent=e.message;}
function validateContract(value){
  const result=validateTheme(value);
  if(result===false||result?.ok===false)throw new Error(result?.error||'Theme contract is incomplete.');
  if(Array.isArray(result)&&result.length)throw new Error(result.join(' '));
  return result&&typeof result==='object'&&!Array.isArray(result)?result:value;
}
const lines=value=>Array.isArray(value)?value.join('\n'):String(value||'');
function themeFromForm(){
  const value=structuredClone(themeContract);
  document.querySelectorAll('[data-theme-field]').forEach(input=>{value[input.dataset.themeField]=input.value;});
  document.querySelectorAll('[data-theme-list]').forEach(input=>{value[input.dataset.themeList]=input.value.split(/\r?\n/).map(s=>s.trim()).filter(Boolean);});
  value.palette={...value.palette};document.querySelectorAll('[data-palette]').forEach(input=>{value.palette[input.dataset.palette]=input.value;});
  return value;
}
function renderThemeContract(value,{selectPreset=true,preview=true}={}){
  themeContract=structuredClone(value);const preset=themeContract.referencePreset||themeContract.id;
  if(selectPreset){$('theme').value=preset;}
  document.querySelectorAll('[data-theme-field]').forEach(input=>{input.value=themeContract[input.dataset.themeField]??'';});
  document.querySelectorAll('[data-theme-list]').forEach(input=>{input.value=lines(themeContract[input.dataset.themeList]);});
  document.querySelectorAll('[data-palette]').forEach(input=>{input.value=themeContract.palette[input.dataset.palette];});
  $('theme-description').textContent=themeContract.description;
  $('theme-contract-name').textContent=(themeContract.name||'Untitled contract').toUpperCase();
  $('palette').replaceChildren(...Object.values(themeContract.palette).map(color=>{const swatch=el('span',undefined,'swatch');swatch.style.background=color;return swatch;}));
  document.documentElement.style.setProperty('--green',themeContract.palette.ground);
  if(preview&&!busy){releasePinned=false;sceneRevision++;const recipe=fixtureRecipe(themeContract.referencePreset||'argonaut');world.load(recipe,themeContract);$('scene-theme').textContent=themeContract.name.toUpperCase();$('scene-name').textContent=recipe.name.toUpperCase();$('version-label').textContent='THEME PREVIEW';$('commander-label').textContent=`${themeContract.gameplayRole||'SENSOR OUTPOST'} TEST`;lastHash='';}
}
function themeChanged(){
  const preset=THEMES[$('theme').value];if(!preset)return;
  if($('prompt').value===displayedBrief){displayedBrief=briefs[preset.referencePreset||preset.id]||briefs.argonaut;$('prompt').value=displayedBrief;}
  renderThemeContract(preset,{selectPreset:false});$('theme-error').textContent='';
}
function setEditorLocked(locked){
  $('theme').disabled=locked;$('prompt').disabled=locked;$('provider').disabled=locked;$('inject').disabled=locked;
  document.querySelectorAll('#theme-contract input,#theme-contract select,#theme-contract textarea').forEach(input=>input.disabled=locked);
  $('draft-theme').disabled=locked;$('resolve-theme').disabled=locked||config?.themeInferenceEnabled!==true;
}
function modeNote(){const mode=$('provider').value;$('mode-note').textContent=mode==='fixture'?'REHEARSAL: generation, repair and visual verdicts use fixtures. Rendering and collision tests are real.':mode==='managed'?'Managed Gemini creator + independent Gemini image reviewer. API access is verified when the run starts.':'Direct Gemini recipe generation + independent Gemini image reviewer. Uses the same acceptance contract.';}
function renderChecks(checks=[]){
  $('checks').replaceChildren(...labels.map(([id,label])=>{
    const c=checks.find(c=>c.id===id),row=el('div',undefined,'check '+(c?.status||'not_run'));row.title=c?.detail||'Not evaluated';
    row.append(el('span',c?.status==='pass'?'✓':c?.status==='fail'?'!':c?.status==='blocked'?'?':'○','mark'),el('span',label),el('code',c?.status==='pass'?'PASS':c?.status==='fail'?'FAIL':c?.status==='blocked'?'WAIT':'—'));return row;
  }));$('pass-count').textContent=`${checks.filter(c=>c.status==='pass').length} / 6 VERIFIED`;
}
function renderJob(job){
  currentJob=job;busy=!terminal.includes(job.status);$('start').disabled=busy;$('cancel').hidden=!busy;setEditorLocked(busy);
  if(job.theme&&JSON.stringify(themeContract)!==JSON.stringify(job.theme))renderThemeContract(job.theme,{selectPreset:true,preview:false});
  $('run-status').textContent=job.status.toUpperCase();$('run-title').textContent=job.status==='accepted'?'A candidate has earned its place.':job.status==='active'?'The asset is active in the test scene.':job.status==='blocked'?'The loop needs attention.':busy?'The next iteration is taking shape.':'Run complete.';
  $('run-budget').textContent=`${job.attempts.length}/3 CANDIDATES · ${job.calls}/6 CALLS`;
  $('events').replaceChildren(...job.events.slice().reverse().map(event=>{const row=el('div',undefined,'event');row.append(el('time',new Date(event.at).toLocaleTimeString([],{hour:'2-digit',minute:'2-digit',second:'2-digit'})),el('span',event.message));return row;}));
  const stage={queued:'generating',generating:'generating',repairing:'generating',validating:'validating',rendering:'validating',reviewing:'reviewing',accepted:'accepted',active:'accepted'}[job.status];
  document.querySelectorAll('[data-stage]').forEach(e=>e.classList.toggle('running',e.dataset.stage===stage));
  const attempt=job.attempts.at(-1);renderChecks(attempt?.checks);
  if(attempt?.recipe&&attempt.hash!==lastHash){world.load(attempt.recipe,job.theme);lastHash=attempt.hash;$('scene-name').textContent=attempt.recipe.name.toUpperCase();$('scene-theme').textContent=job.theme.name.toUpperCase();$('version-label').textContent=`CANDIDATE ${attempt.number}`;}
  $('activate').disabled=job.status!=='accepted';$('activate').textContent=job.status==='accepted'?(job.provider==='fixture'?'Use rehearsal asset in preview ↗':'Use verified asset in preview ↗'):job.status==='active'?'Asset is active ✓':'Awaiting a verified candidate ↗';
  const packageLink=$('download-package'),downloadable=['accepted','active'].includes(job.status);
  packageLink.hidden=!downloadable;packageLink.href=downloadable?`/api/jobs/${encodeURIComponent(job.id)}/package`:'';
  packageLink.textContent=job.provider==='fixture'?'Download rehearsal asset package ↓':'Download asset package ↓';
  if(job.provider==='fixture')$('connection').textContent='OFFLINE REHEARSAL';
  else $('connection').textContent='GEMINI · '+(job.provider==='managed'?'MANAGED':'DIRECT');
  const signature=job.attempts.map(a=>a.hash+':'+a.images.length+':'+a.checks.map(c=>c.status).join(',')).join('|');
  if($('attempts').dataset.signature!==signature){
    $('attempts').dataset.signature=signature;$('attempts').replaceChildren(...job.attempts.filter(a=>a.recipe).map(a=>{
      const section=el('div',undefined,'attempt'),head=el('div',undefined,'attempt-heading');head.append(el('span',`CANDIDATE ${String(a.number).padStart(2,'0')} · ${a.recipe.name}`),el('span',a.hash.slice(0,12),'attempt-hash'));section.append(head);
      const images=el('div',undefined,'image-grid');for(const [i,url]of a.images.entries()){const f=el('figure'),link=el('a');link.href=url;link.target='_blank';const img=el('img');img.src=url;img.alt=`Candidate ${a.number}, actual game view ${i+1}`;link.append(img);f.append(link,el('figcaption',['01 / OVERVIEW','02 / REVERSE','03 / ENTRANCE'][i]));images.append(f);}section.append(images);
      const failures=a.checks.filter(c=>c.status!=='pass');section.append(el('p',failures.length?failures.map(c=>`${c.id}: ${c.detail}`).join(' '):a.checks.length?'Required checks passed for this candidate.':'Preparing the evaluation…','finding'));return section;
    }));
  }
  if(job.stopReason&&['blocked','exhausted'].includes(job.status))$('error').textContent=job.stopReason;
}
for(const t of Object.values(THEMES)){const option=el('option',t.name+' · '+t.genre);option.value=t.id;$('theme').append(option);}
$('theme').onchange=themeChanged;$('provider').onchange=modeNote;themeChanged();renderChecks();
document.querySelectorAll('[data-theme-field],[data-theme-list],[data-palette]').forEach(input=>input.addEventListener('change',()=>{
  if(!themeContract)return;
  try{themeContract=validateContract(themeFromForm());renderThemeContract(themeContract,{selectPreset:false});$('theme-error').textContent='';}
  catch(e){$('theme-error').textContent=e.message;}
}));
document.querySelectorAll('[data-view]').forEach(button=>button.onclick=()=>{world.view(Number(button.dataset.view));document.querySelectorAll('[data-view]').forEach(b=>b.classList.toggle('selected',b===button));});
$('enter').onclick=()=>world.goInside();
$('draft-theme').onclick=()=>{
  try{
    const result=draftThemeFromPrompt($('prompt').value,$('theme').value),draft=result?.theme||result,checked=validateContract(draft);
    renderThemeContract(checked,{selectPreset:true});
    $('theme-contract').open=true;
    $('theme-draft-note').textContent=`LOCAL KEYWORD DRAFT · Fallback preset: ${checked.referencePreset}. Review the assumptions shown in the contract before generating; this uses no AI semantic inference.`;
    $('theme-error').textContent='';
  }catch(e){$('theme-error').textContent=e.message;}
};
$('resolve-theme').onclick=async()=>{
  if(config?.themeInferenceEnabled!==true)return;
  $('resolve-theme').disabled=true;$('theme-error').textContent='';
  try{
    const result=await api('/api/themes/resolve',{prompt:$('prompt').value}),resolved=validateContract(result.theme);
    renderThemeContract(resolved,{selectPreset:true});
    $('theme-contract').open=true;
    $('theme-draft-note').textContent=`GEMINI-RESOLVED DRAFT · Fallback family: ${resolved.referencePreset}. Review all recorded assumptions before use.`;
  }catch(e){$('theme-error').textContent=e.message;}
  finally{$('resolve-theme').disabled=busy||config?.themeInferenceEnabled!==true;}
};
$('start').onclick=async()=>{
  $('error').textContent='';$('theme-error').textContent='';$('start').disabled=true;releasePinned=false;sceneRevision++;
  try{
    const frozenTheme=validateContract(themeFromForm());renderThemeContract(frozenTheme,{selectPreset:false,preview:false});setEditorLocked(true);
    const job=await api('/api/jobs',{themeId:frozenTheme.id,theme:frozenTheme,prompt:$('prompt').value,provider:$('provider').value,injectDefect:$('inject').checked});lastHash='';renderJob(job);
  }catch(e){error(e);$('theme-error').textContent=e.message;$('start').disabled=false;setEditorLocked(false);}
};
$('cancel').onclick=async()=>{try{renderJob(await api(`/api/jobs/${currentJob.id}/cancel`,{}));}catch(e){error(e);}};
$('activate').onclick=async()=>{try{const r=await api(`/api/jobs/${currentJob.id}/activate`,{});$('release-label').textContent=(r.rehearsal?'REHEARSAL ACTIVE · ':'ACTIVE · ')+r.hash.slice(0,10);renderJob(await api('/api/jobs/'+currentJob.id));}catch(e){error(e);}};
$('rollback').onclick=async()=>{releasePinned=true;sceneRevision++;try{const r=await api('/api/rollback',{});renderThemeContract(r.theme,{selectPreset:true,preview:false});releasePinned=true;world.load(r.recipe,r.theme);lastHash='';$('scene-name').textContent=r.recipe.name.toUpperCase();$('scene-theme').textContent=r.theme.name.toUpperCase();$('version-label').textContent='RESTORED RELEASE';$('release-label').textContent='RESTORED · '+r.hash.slice(0,10);$('error').textContent='';}catch(e){releasePinned=false;error(e);}};
try{
  config=await api('/api/config');$('connection').textContent=config.keyConfigured?'GEMINI · CONFIGURED':'OFFLINE REHEARSAL';if(!config.keyConfigured)$('provider').value='fixture';
  $('resolve-theme').disabled=config.themeInferenceEnabled!==true;$('theme-draft-note').textContent=config.themeInferenceEnabled?'GEMINI RESOLUTION AVAILABLE · Costs one request. Review recorded assumptions before use.':'Gemini theme resolution is disabled until an explicit spending cap is enabled. Local keyword drafts use the selected preset fallback.';modeNote();
  const jobs=await api('/api/jobs');if(jobs.length){if(jobs[0].theme)renderThemeContract(jobs[0].theme,{selectPreset:true,preview:false});$('provider').value=jobs[0].provider;modeNote();renderJob(jobs[0]);}
  const release=await api('/api/release');if(release){$('release-label').textContent=(release.rehearsal?'REHEARSAL ACTIVE · ':'ACTIVE · ')+release.hash.slice(0,10);if(!currentJob){renderThemeContract(release.theme,{selectPreset:true,preview:false});world.load(release.recipe,release.theme);$('scene-name').textContent=release.recipe.name.toUpperCase();$('scene-theme').textContent=release.theme.name.toUpperCase();$('version-label').textContent='ACTIVE RELEASE';}}
}catch(e){error(e);}
setInterval(async()=>{if(currentJob&&busy&&!releasePinned&&!pollInFlight){pollInFlight=true;const revision=sceneRevision,jobId=currentJob.id;try{const job=await api('/api/jobs/'+jobId);if(revision===sceneRevision&&!releasePinned)renderJob(job);}catch(e){if(revision===sceneRevision)error(e);}finally{pollInFlight=false;}}},900);
