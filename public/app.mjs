import {THEMES,fixtureRecipe} from './core.mjs';
import {createWorld} from './scene.mjs';
import {startRenderWorker} from './render-worker.mjs';
const $=id=>document.getElementById(id),world=createWorld($('world'));
startRenderWorker();
let currentJob=null,lastHash='',busy=false,config,releasePinned=false,pollInFlight=false,sceneRevision=0;
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
function themeChanged(){
  const t=THEMES[$('theme').value];$('theme-description').textContent=t.description;$('palette').replaceChildren(...Object.values(t.palette).map(c=>{const s=el('span',undefined,'swatch');s.style.background=c;return s;}));
  document.documentElement.style.setProperty('--green',t.palette.ground);
  if($('prompt').value===displayedBrief){displayedBrief=briefs[t.id];$('prompt').value=displayedBrief;}
  const civilization={argonaut:'MACHINE COMMANDER',sylvan:'GROVE RANGER',ember:'REACH SCOUT'};
  $('commander-label').textContent=civilization[t.id];
  if(!busy){releasePinned=false;sceneRevision++;const preview=fixtureRecipe(t.id);world.load(preview,t);$('scene-theme').textContent=t.name.toUpperCase();$('scene-name').textContent=preview.name.toUpperCase();$('version-label').textContent='THEME PREVIEW';lastHash='';}
}
function modeNote(){const mode=$('provider').value;$('mode-note').textContent=mode==='fixture'?'REHEARSAL: generation, repair and visual verdicts use fixtures. Rendering and collision tests are real.':mode==='managed'?'Managed Gemini creator + independent Gemini image reviewer. API access is verified when the run starts.':'Direct Gemini recipe generation + independent Gemini image reviewer. Uses the same acceptance contract.';}
function renderChecks(checks=[]){
  $('checks').replaceChildren(...labels.map(([id,label])=>{
    const c=checks.find(c=>c.id===id),row=el('div',undefined,'check '+(c?.status||'not_run'));row.title=c?.detail||'Not evaluated';
    row.append(el('span',c?.status==='pass'?'✓':c?.status==='fail'?'!':c?.status==='blocked'?'?':'○','mark'),el('span',label),el('code',c?.status==='pass'?'PASS':c?.status==='fail'?'FAIL':c?.status==='blocked'?'WAIT':'—'));return row;
  }));$('pass-count').textContent=`${checks.filter(c=>c.status==='pass').length} / 6 VERIFIED`;
}
function renderJob(job){
  currentJob=job;busy=!terminal.includes(job.status);$('start').disabled=busy;$('cancel').hidden=!busy;$('theme').disabled=busy;$('provider').disabled=busy;$('inject').disabled=busy;
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
document.querySelectorAll('[data-view]').forEach(button=>button.onclick=()=>{world.view(Number(button.dataset.view));document.querySelectorAll('[data-view]').forEach(b=>b.classList.toggle('selected',b===button));});
$('enter').onclick=()=>world.goInside();
$('start').onclick=async()=>{
  $('error').textContent='';$('start').disabled=true;releasePinned=false;sceneRevision++;
  try{const job=await api('/api/jobs',{themeId:$('theme').value,prompt:$('prompt').value,provider:$('provider').value,injectDefect:$('inject').checked});lastHash='';renderJob(job);}catch(e){error(e);$('start').disabled=false;}
};
$('cancel').onclick=async()=>{try{renderJob(await api(`/api/jobs/${currentJob.id}/cancel`,{}));}catch(e){error(e);}};
$('activate').onclick=async()=>{try{const r=await api(`/api/jobs/${currentJob.id}/activate`,{});$('release-label').textContent=(r.rehearsal?'REHEARSAL ACTIVE · ':'ACTIVE · ')+r.hash.slice(0,10);renderJob(await api('/api/jobs/'+currentJob.id));}catch(e){error(e);}};
$('rollback').onclick=async()=>{releasePinned=true;sceneRevision++;try{const r=await api('/api/rollback',{});$('theme').value=r.theme.id;themeChanged();releasePinned=true;world.load(r.recipe,r.theme);lastHash='';$('scene-name').textContent=r.recipe.name.toUpperCase();$('scene-theme').textContent=r.theme.name.toUpperCase();$('version-label').textContent='RESTORED RELEASE';$('release-label').textContent='RESTORED · '+r.hash.slice(0,10);$('error').textContent='';}catch(e){releasePinned=false;error(e);}};
try{
  config=await api('/api/config');$('connection').textContent=config.keyConfigured?'GEMINI · CONFIGURED':'OFFLINE REHEARSAL';if(!config.keyConfigured)$('provider').value='fixture';modeNote();
  const jobs=await api('/api/jobs');if(jobs.length){$('theme').value=jobs[0].themeId;$('provider').value=jobs[0].provider;themeChanged();modeNote();renderJob(jobs[0]);}
  const release=await api('/api/release');if(release){$('release-label').textContent=(release.rehearsal?'REHEARSAL ACTIVE · ':'ACTIVE · ')+release.hash.slice(0,10);if(!currentJob){world.load(release.recipe,release.theme);$('scene-name').textContent=release.recipe.name.toUpperCase();$('scene-theme').textContent=release.theme.name.toUpperCase();$('version-label').textContent='ACTIVE RELEASE';}}
}catch(e){error(e);}
setInterval(async()=>{if(currentJob&&busy&&!releasePinned&&!pollInFlight){pollInFlight=true;const revision=sceneRevision,jobId=currentJob.id;try{const job=await api('/api/jobs/'+jobId);if(revision===sceneRevision&&!releasePinned)renderJob(job);}catch(e){if(revision===sceneRevision)error(e);}finally{pollInFlight=false;}}},900);
