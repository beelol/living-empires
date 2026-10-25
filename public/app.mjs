import {fixtureRecipe} from './core.mjs';
import {THEMES,validateTheme,draftThemeFromPrompt} from './themes.mjs';
import {createWorld} from './scene.mjs';
import {startRenderWorker} from './render-worker.mjs';
const $=id=>document.getElementById(id),world=createWorld($('world'));
startRenderWorker();
let currentJob=null,lastHash='',busy=false,config,releasePinned=false,pollInFlight=false,sceneRevision=0,referenceObjectUrl=null,referenceImage=null,referenceReadPending=false,referenceLoadToken=0,jobsCache=new Map();
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
function editorTheme(theme,themeId){
  const source=structuredClone(theme||THEMES[themeId]||THEMES.argonaut);
  try{return validateContract(source);}catch{
    // Enrich a copy for this editor and preview; persisted legacy job/release history stays untouched.
    const base=structuredClone(THEMES[source.referencePreset]||THEMES[themeId]||THEMES[source.id]||THEMES.argonaut),hydrated={...base};
    for(const key of Object.keys(base))if(key!=='palette'&&source[key]!==undefined)hydrated[key]=source[key];
    hydrated.id=themeId||source.id||base.id;hydrated.referencePreset=base.referencePreset;hydrated.palette={...base.palette,...source.palette};
    return validateContract(hydrated);
  }
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
  $('commander-label').textContent=`${(themeContract.referencePreset||'argonaut').toUpperCase()} / SENSOR TEST`;
  $('palette').replaceChildren(...Object.values(themeContract.palette).map(color=>{const swatch=el('span',undefined,'swatch');swatch.style.background=color;return swatch;}));
  document.documentElement.style.setProperty('--green',themeContract.palette.ground);
  if(preview&&!busy){releasePinned=false;sceneRevision++;const recipe=fixtureRecipe(themeContract.referencePreset||'argonaut');world.load(recipe,themeContract);$('scene-theme').textContent=themeContract.name.toUpperCase();$('scene-name').textContent=recipe.name.toUpperCase();$('version-label').textContent='THEME PREVIEW';lastHash='';}
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
  document.querySelectorAll('#refinement-panel input,#refinement-panel select,#refinement-panel textarea').forEach(input=>input.disabled=locked);
  $('clear-reference').disabled=locked;
  if(locked){$('preview-refinement').disabled=true;$('refine-version').disabled=true;}
}
function modeNote(){const mode=$('provider').value;$('mode-note').textContent=mode==='fixture'?'REHEARSAL: generation, repair and visual verdicts use fixtures. Rendering and collision tests are real.':mode==='managed'?'Managed Gemini creator + independent Gemini image reviewer. API access is verified when the run starts.':'Direct Gemini recipe generation + independent Gemini image reviewer. Uses the same acceptance contract.';}
function renderChecks(checks=[]){
  $('checks').replaceChildren(...labels.map(([id,label])=>{
    const c=checks.find(c=>c.id===id),row=el('div',undefined,'check '+(c?.status||'not_run'));row.title=c?.detail||'Not evaluated';
    row.append(el('span',c?.status==='pass'?'✓':c?.status==='fail'?'!':c?.status==='blocked'?'?':'○','mark'),el('span',label),el('code',c?.status==='pass'?'PASS':c?.status==='fail'?'FAIL':c?.status==='blocked'?'WAIT':'—'));return row;
  }));$('pass-count').textContent=`${checks.filter(c=>c.status==='pass').length} / 6 VERIFIED`;
}
const viewNames=['OVERVIEW','REVERSE','ENTRANCE'];
const list=value=>Array.isArray(value)?value.filter(item=>item!==null&&item!==undefined).map(item=>typeof item==='string'?item:String(item)):typeof value==='string'&&value.trim()?[value]:[];
function appendTextList(parent,title,items,{className='review-block',empty='None recorded.'}={}){
  const block=el('section',undefined,className);block.append(el('h4',title));
  const values=list(items);
  if(!values.length)block.append(el('p',empty,'muted-copy'));
  else{const ul=el('ul');for(const value of values)ul.append(el('li',value));block.append(ul);}
  parent.append(block);return block;
}
function exactEvidencePair(previous,attempt,comparison,job){
  const equalList=(left,right)=>Array.isArray(left)&&Array.isArray(right)&&left.length===right.length&&left.every((value,index)=>value===right[index]);
  if(!previous||!comparison||comparison.candidateHash!==attempt.hash||!equalList(comparison.imageHashes,attempt.imageHashes))return false;
  if((comparison.previousCandidateHash||attempt.previousHash||job.baseCandidateHash)!==previous.hash||!equalList(comparison.previousImageHashes,previous.imageHashes))return false;
  return previous.images?.length===3&&attempt.images?.length===3&&previous.imageHashes?.length===3&&attempt.imageHashes?.length===3;
}
function comparisonPrior(job,attempt,comparison){
  const wanted=comparison?.previousCandidateHash||attempt.previousHash||(attempt.number===1?job.baseCandidateHash:null);
  if(!wanted)return null;
  const local=(job.attempts||[]).find(candidate=>candidate.hash===wanted);
  if(local)return local;
  if(job.baseJobId){const base=jobsCache.get(job.baseJobId),candidate=base?.attempts?.find(item=>item.hash===wanted);if(candidate)return candidate;}
  for(const saved of jobsCache.values()){const candidate=saved.attempts?.find(item=>item.hash===wanted);if(candidate)return candidate;}
  return null;
}
function stableJSON(value){
  if(Array.isArray(value))return `[${value.map(stableJSON).join(',')}]`;
  if(value&&typeof value==='object')return `{${Object.keys(value).sort().map(key=>`${JSON.stringify(key)}:${stableJSON(value[key])}`).join(',')}}`;
  return JSON.stringify(value);
}
function noVisibleChange(previous,attempt){
  if(!previous||!attempt)return false;
  const before=previous.recipe?.parts,after=attempt.recipe?.parts;
  const sameParts=Array.isArray(before)&&Array.isArray(after)&&stableJSON(before)===stableJSON(after);
  const left=previous.imageHashes,right=attempt.imageHashes;
  const sameImages=Array.isArray(left)&&Array.isArray(right)&&left.length===right.length&&left.length>0&&left.every((hash,index)=>hash===right[index]);
  return sameParts||sameImages;
}
function isAuthoredSample(job,attempt){return job.previewKind==='authored_scene'||/^authored/i.test(attempt?.recipe?.name||'');}
function renderPairedViews(parent,previous,attempt,comparison){
  const grid=el('div',undefined,'paired-views');
  for(let i=0;i<3;i++){
    const figure=el('figure',undefined,'paired-view'),caption=el('figcaption',`${String(i+1).padStart(2,'0')} / ${viewNames[i]}`),columns=el('div',undefined,'paired-images');
    for(const [version,url,hash] of [['BEFORE',previous.images[i],previous.imageHashes[i]],['AFTER',attempt.images[i],attempt.imageHashes[i]]]){
      const item=el('div',undefined,'paired-image'),label=el('span',version,'pair-label'),link=el('a');link.href=url;link.target='_blank';link.rel='noopener';
      const img=el('img');img.src=url;img.alt=`${version.toLowerCase()} candidate, ${viewNames[i].toLowerCase()} view`;link.append(img);
      item.append(label,link,el('code',hash,'evidence-hash'));columns.append(item);
    }
    figure.append(caption,columns);grid.append(figure);
  }
  parent.append(grid);
}
function movementSummary(movement){
  const points=Array.isArray(movement.trace)?movement.trace:[];
  const format=point=>Array.isArray(point)?point.join(', '):typeof point==='object'?JSON.stringify(point):String(point);
  const tail=points.slice(-4).map(format).join(' → '),prefix=points.length>4?'… → ':'';
  const row=el('p',`Browser movement ${movement.pass?'passed':'failed'}${tail?` · Trace: ${prefix}${tail}`:''}`,'movement-result '+(movement.pass?'movement-pass':'movement-fail'));
  if(points.length>4)row.title=`Full trace: ${points.map(format).join(' → ')}`;
  return row;
}
function renderLatestComparison(job){
  const attempts=job.attempts||[],attempt=attempts.at(-1),comparison=attempt?.review?.comparison,previous=attempt&&(comparisonPrior(job,attempt,comparison)||job.baseline||null);
  const unchanged=!!attempt&&noVisibleChange(previous,attempt),authored=isAuthoredSample(job,attempt);
  const host=$('attempt-comparison'),signature=JSON.stringify({provider:job.provider,previewKind:job.previewKind,status:job.status,stopReason:job.stopReason,spending:job.spending||config?.spending,baseJobId:job.baseJobId,baseAttemptNumber:job.baseAttemptNumber,baseCandidateHash:job.baseCandidateHash,userFeedback:job.userFeedback,baseline:job.baseline&&{hash:job.baseline.hash,images:job.baseline.images,imageHashes:job.baseline.imageHashes,parts:job.baseline.recipe?.parts},attempt:attempt&&{number:attempt.number,hash:attempt.hash,images:attempt.images,imageHashes:attempt.imageHashes,checks:attempt.checks,review:attempt.review,repairRequested:attempt.repairRequested,decision:attempt.decision,decisionReason:attempt.decisionReason,previousHash:attempt.previousHash,delta:attempt.delta}});
  if(host.dataset.signature===signature)return;
  host.dataset.signature=signature;host.replaceChildren();
  $('evidence-mode').textContent=authored?'AUTHORED SAMPLE':job.provider==='fixture'?'OFFLINE REHEARSAL':'LIVE REVIEW';
  if(!attempt){host.append(el('p','No candidate evidence has been captured yet.','empty-event'));return;}
  const review=attempt.review,head=el('div',undefined,'comparison-heading');
  const title=el('div');title.append(el('span',`VERSION ${String(attempt.number).padStart(2,'0')} · ${attempt.recipe?.name||'Untitled candidate'}`,'eyebrow'),el('p',attempt.hash?`Candidate SHA-256 · ${attempt.hash}`:'Candidate hash pending.','full-hash'));
  if(job.baseCandidateHash)title.append(el('p',`Refines ${job.baseJobId||'saved job'} · version ${job.baseAttemptNumber??'?'} · ${job.baseCandidateHash}`,'lineage-note'));
  if(job.userFeedback)title.append(el('p',`Requested refinement · ${job.userFeedback}`,'lineage-note'));
  const status=unchanged?'no visible change':comparison?.status||attempt.decision||(job.status==='accepted'?'legacy accepted':job.status==='active'?'legacy active':'review pending');
  const badge=el('span',String(status).replaceAll('_',' ').toUpperCase(),'review-status status-'+String(status).toLowerCase().replace(/[^a-z0-9-]+/g,'-'));head.append(title,badge);host.append(head);
  const spending=job.spending||config?.spending;
  if(spending&&Number.isFinite(spending.capUsd))host.append(el('p',`Session allowance · $${spending.capUsd.toFixed(2)} cap · $${Number(spending.allocatedUsd||0).toFixed(2)} reserved · $${Number(spending.remainingUsd||0).toFixed(2)} remaining. Reservations are not actual billed spend.`,'spending-note'));
  if(authored)host.append(el('p','AUTHORED SAMPLE · This fixed recipe was not generated by a provider. Renderer captures and functional checks are real.','rehearsal-notice'));
  else if(job.provider==='fixture')host.append(el('p','OFFLINE REHEARSAL · Generation and visual verdicts are simulated. Renderer captures and functional checks are real. This is not an independent AI creative judgment.','rehearsal-notice'));
  if(attempt.movement)host.append(movementSummary(attempt.movement));
  if(comparison){
    if(unchanged)host.append(el('p','No visible change is evidenced: the saved recipe parts or ordered render hashes match the baseline. Comparative improvement claims are withheld.','comparison-summary'));
    else if(comparison.summary)host.append(el('p',comparison.summary,'comparison-summary'));
    const metrics=el('div',undefined,'review-metrics');
    for(const [label,value] of [['THEME',review.themeStatus],['RENDER',review.renderStatus],['DECISION',unchanged?'NO VISIBLE CHANGE':attempt.decision]])if(value)metrics.append(el('span',`${label} · ${String(value).toUpperCase()}`,'metric'));
    if(metrics.childElementCount)host.append(metrics);
    const failedChecks=(attempt.checks||[]).filter(check=>check.status!=='pass');
    if(failedChecks.length)appendTextList(host,'Functional checks that did not pass',failedChecks.map(check=>`${check.id} · ${check.status}: ${check.detail||'No detail recorded.'}`),{className:'review-block defects'});
    const exact=exactEvidencePair(previous,attempt,comparison,job);
    if(exact){
      host.append(el('h3','Same-view evidence · before and after','comparison-subhead'));
      renderPairedViews(host,previous,attempt,comparison);
    }else if(comparison.previousCandidateHash||attempt.previousHash||job.baseCandidateHash){
      host.append(el('p','The recorded comparison references a prior candidate, but its hash or three image hashes do not match the saved versions. Before/after images are withheld.','evidence-warning'));
    }else if(comparison.status==='initial'){
      host.append(el('p','Initial candidate: no prior version exists for a before/after comparison.','muted-copy'));
    }
    const details=el('div',undefined,'review-details');
    appendTextList(details,'Strengths',comparison.strengths,{className:'review-block strengths',empty:'No strengths recorded.'});
    const defects=Array.isArray(comparison.defects)?comparison.defects.map(item=>{
      if(typeof item==='string')return item;
      const where=[item.view?`View ${item.view}`:'',item.location,item.criterion].filter(Boolean).join(' · ');
      return [where,item.issue,item.repair?`Repair: ${item.repair}`:''].filter(Boolean).join(' — ')||'Unspecified defect';
    }):comparison.defects;
    appendTextList(details,'Defects & requested repairs',defects,{className:'review-block defects',empty:'No defects recorded.'});
    appendTextList(details,'Improvements',unchanged?[]:comparison.improvements,{className:'review-block improvements',empty:unchanged?'No verified visual improvements are evidenced.':'No improvements recorded.'});
    appendTextList(details,'Regressions',comparison.regressions,{className:'review-block regressions',empty:'No regressions recorded.'});
    host.append(details);
    if(list(attempt.repairRequested).length)appendTextList(host,'Repair sent to creator',attempt.repairRequested,{className:'repair-request'});
    if(attempt.decision&&!unchanged)host.append(el('p',`Decision · ${attempt.decision}`,'decision-note'));
  }else{
    const failures=(attempt.checks||[]).filter(check=>check.status!=='pass');
    if(failures.length)appendTextList(host,'Recorded functional failures',failures.map(check=>`${check.id}: ${check.detail||check.status}`),{className:'review-block defects',empty:'No failures recorded.'});
    if(review){
      const summary=typeof review.findings==='string'?review.findings:list(review.findings).join(' · ');
      if(summary)host.append(el('p',summary,'comparison-summary'));
      const statuses=[review.themeStatus&&`Theme · ${review.themeStatus}`,review.renderStatus&&`Render · ${review.renderStatus}`].filter(Boolean);
      if(statuses.length)host.append(el('p',statuses.join(' · '),'muted-copy'));
    }
    host.append(el('p',job.provider==='fixture'?'Legacy rehearsal record: fixtures supplied the visual opinion. Only the listed functional failures and captured renders are real evidence; no independent AI creative judgment or location-matched comparison was saved.':'No structured location-matched comparison was saved for this attempt. Existing functional checks are shown separately; no visual comparison is inferred from this history.','legacy-note'));
    if(attempt.decision)host.append(el('p',`Recorded decision · ${attempt.decision}`,'decision-note'));
    if(list(attempt.repairRequested).length)appendTextList(host,'Repair requested',attempt.repairRequested,{className:'review-block defects'});
  }
  if(attempt.decisionReason&&!unchanged)host.append(el('p',`Decision reason · ${attempt.decisionReason}`,'decision-note'));
  if(job.stopReason&&!['accepted','active'].includes(job.status))host.append(el('p',`Run stopped · ${job.stopReason}`,'stop-reason'));
}
function renderAttemptHistory(job){
  const host=$('attempts'),signature=JSON.stringify({previewKind:job.previewKind,baseline:job.baseline&&{hash:job.baseline.hash,imageHashes:job.baseline.imageHashes,recipe:job.baseline.recipe?.parts},attempts:(job.attempts||[]).map(attempt=>({number:attempt.number,hash:attempt.hash,images:attempt.images,imageHashes:attempt.imageHashes,checks:attempt.checks,review:attempt.review,repairRequested:attempt.repairRequested,decision:attempt.decision,decisionReason:attempt.decisionReason,previousHash:attempt.previousHash,recipe:attempt.recipe?.name,parts:attempt.recipe?.parts,delta:attempt.delta}))});
  if(host.dataset.signature===signature)return;
  host.dataset.signature=signature;host.replaceChildren();
  const attempts=job.attempts||[];$('attempt-count').textContent=`${attempts.length} version${attempts.length===1?'':'s'}`;
  if(!attempts.length){host.append(el('p','No candidate versions yet.','empty-event'));return;}
  for(const attempt of attempts){
    const section=el('article',undefined,'attempt'),head=el('div',undefined,'attempt-heading'),left=el('span',`VERSION ${String(attempt.number).padStart(2,'0')} · ${attempt.recipe?.name||'Candidate'}`),hash=el('code',attempt.hash||'HASH PENDING','attempt-hash');hash.title=attempt.hash||'';head.append(left,hash);section.append(head);
    const authored=isAuthoredSample(job,attempt);section.append(el('p',authored?'AUTHORED SAMPLE':job.provider==='fixture'?'OFFLINE REHEARSAL':'LIVE PROVIDER',`provenance-tag ${job.provider==='fixture'||authored?'rehearsal-tag':'live-tag'}`));
    if(attempt.images?.length){const images=el('div',undefined,'image-grid');for(const [index,url] of attempt.images.entries()){
      const figure=el('figure'),link=el('a');link.href=url;link.target='_blank';link.rel='noopener';const img=el('img');img.src=url;img.alt=`Version ${attempt.number}, actual rendered ${viewNames[index]?.toLowerCase()||`view ${index+1}`}`;link.append(img);figure.append(link,el('figcaption',`${String(index+1).padStart(2,'0')} / ${viewNames[index]||'VIEW'}`));images.append(figure);
    }section.append(images);}
    const failures=(attempt.checks||[]).filter(check=>check.status!=='pass');
    if(failures.length)appendTextList(section,'Functional checks that did not pass',failures.map(check=>`${check.id} · ${check.status}: ${check.detail||'No detail recorded.'}`),{className:'history-findings'});
    else if(attempt.checks?.length)section.append(el('p','Recorded functional checks passed.','finding'));
    const comparison=attempt.review?.comparison,previous=comparisonPrior(job,attempt,comparison)||((attempt.number===1)?job.baseline:null),unchanged=noVisibleChange(previous,attempt);
    if(comparison){
      section.append(el('p',`Review · ${unchanged?'NO VISIBLE CHANGE':String(comparison.status||'recorded').toUpperCase()}${comparison.summary&&!unchanged?` — ${comparison.summary}`:unchanged?' · Comparative improvement claims withheld.':''}`,'comparison-summary'));
      const defects=Array.isArray(comparison.defects)?comparison.defects.map(item=>typeof item==='string'?item:[item.view?`View ${item.view}`:'',item.location,item.criterion,item.issue,item.repair?`Repair: ${item.repair}`:''].filter(Boolean).join(' · ')):comparison.defects;
      if(list(defects).length)appendTextList(section,'Recorded critique',defects,{className:'history-findings'});
      if(list(comparison.regressions).length)appendTextList(section,'Regressions',comparison.regressions,{className:'history-findings'});
    }
    if(attempt.repairRequested?.length)appendTextList(section,'Repair sent to creator',attempt.repairRequested,{className:'repair-request'});
    if(attempt.delta){
      const changes=[['Parts added',attempt.delta.addedParts],['Parts changed',attempt.delta.changedParts],['Parts removed',attempt.delta.removedParts],['Components added',attempt.delta.componentsAdded],['Components changed',attempt.delta.componentsChanged],['Components removed',attempt.delta.componentsRemoved]];
      const recorded=changes.filter(([,items])=>list(items).length);
      if(recorded.length){section.append(el('h4','Recorded recipe changes','delta-heading'));for(const [label,items] of recorded)appendTextList(section,label,items,{className:'recipe-delta'});}
    }
    if(unchanged&&!comparison)section.append(el('p','NO VISIBLE CHANGE against the saved baseline; comparative improvement claims are withheld.','evidence-warning'));
    if(attempt.decision&&!unchanged)section.append(el('p',`Decision · ${attempt.decision}`,'decision-note'));
    if(attempt.decisionReason&&!unchanged)section.append(el('p',`Decision reason · ${attempt.decisionReason}`,'decision-note'));
    if(job.provider==='fixture'&&!attempt.review?.comparison)section.append(el('p','No independent AI creative judgment was recorded for this rehearsal version.','legacy-note'));
    else if(!attempt.review?.comparison)section.append(el('p','No structured visual comparison was saved for this version.','legacy-note'));
    host.append(section);
  }
}
function renderRefinementControls(job){
  const selector=$('base-version'),attempts=(job.attempts||[]).filter(attempt=>attempt.recipe&&attempt.hash),signature=attempts.map(attempt=>`${attempt.number}:${attempt.hash}`).join('|');
  const oldSelection=selector.value;
  if(selector.dataset.job!==job.id||selector.dataset.signature!==signature){
    selector.replaceChildren();
    for(const attempt of attempts){const option=el('option',`VERSION ${String(attempt.number).padStart(2,'0')} · ${attempt.recipe.name} · ${attempt.hash.slice(0,10)}`);option.value=attempt.hash;selector.append(option);}
    selector.dataset.job=job.id;selector.dataset.signature=signature;
    selector.value=attempts.some(attempt=>attempt.hash===oldSelection)?oldSelection:attempts.at(-1)?.hash||'';
  }
  const selected=attempts.find(attempt=>attempt.hash===selector.value),ready=!!selected&&!busy&&terminal.includes(job.status);
  selector.disabled=!attempts.length||busy;
  $('refinement-state').textContent=busy?'Finish the current run first':selected?`${job.provider==='fixture'?'OFFLINE REHEARSAL':'SAVED PROVIDER'} · frozen baseline`:'No finished candidate yet';
  $('base-version-summary').textContent=selected?`Fixed source · job ${job.id} · version ${selected.number} · SHA-256 ${selected.hash}. The source theme and original brief stay frozen.`:'Refinement becomes available after a candidate recipe has been saved.';
  const components=selected?.recipe?.scene?.components;
  const listNode=$('component-list');listNode.replaceChildren();
  if(Array.isArray(components)&&components.length){
    for(const component of components){const row=el('li');row.append(el('strong',component.name||component.id||'Unnamed component'));const details=[component.role,`${Array.isArray(component.partIds)?component.partIds.length:0} recipe parts`].filter(Boolean);row.append(el('span',` · ${details.join(' · ')}`));listNode.append(row);}
  }else listNode.append(el('li',selected?'This saved recipe has no named component metadata; refine its whole outpost.':'Select a saved version to inspect its named components.'));
  $('user-feedback').disabled=!ready;
  $('refine-version').disabled=!ready||!$('user-feedback').value.trim();
  $('preview-refinement').disabled=!ready||$('user-feedback').value.trim().length<5;
  $('clear-reference').disabled=busy;
}
function clearLocalReference(){
  referenceLoadToken++;referenceReadPending=false;
  if(referenceObjectUrl)URL.revokeObjectURL(referenceObjectUrl);
  referenceObjectUrl=null;referenceImage=null;$('reference-file').value='';$('reference-image').removeAttribute('src');$('reference-name').textContent='';$('reference-preview').hidden=true;$('reference-consent').checked=false;$('reference-consent').disabled=true;
}
function readLocalImage(file){
  return new Promise((resolve,reject)=>{
    const reader=new FileReader();
    reader.onerror=()=>reject(new Error('The selected image could not be read locally.'));
    reader.onload=()=>{const value=String(reader.result||''),comma=value.indexOf(',');if(comma<0)reject(new Error('The selected image could not be encoded.'));else resolve(value.slice(comma+1));};
    reader.readAsDataURL(file);
  });
}
async function chooseLocalReference(file){
  $('reference-error').textContent='';clearLocalReference();
  if(!file)return;
  const allowed=['image/png','image/jpeg'];
  if(!allowed.includes(file.type)){ $('reference-error').textContent='Choose a PNG or JPEG image.';$('reference-file').value='';return; }
  if(file.size>2*1024*1024){$('reference-error').textContent='Reference images must be 2 MiB or smaller.';$('reference-file').value='';return;}
  const token=referenceLoadToken;referenceReadPending=true;$('reference-name').textContent='Reading locally…';
  try{
    const data=await readLocalImage(file);
    if(token!==referenceLoadToken)return;
    referenceObjectUrl=URL.createObjectURL(file);referenceImage={mimeType:file.type,data};$('reference-image').src=referenceObjectUrl;$('reference-name').textContent=`${file.name} · ${(file.size/1024/1024).toFixed(2)} MiB`;$('reference-preview').hidden=false;$('reference-consent').disabled=false;
  }catch(e){if(token===referenceLoadToken){$('reference-error').textContent=e.message;clearLocalReference();}}
  finally{if(token===referenceLoadToken)referenceReadPending=false;}
}
function referenceRequestFields(){return referenceImage&&$('reference-consent').checked?{referenceImages:[{mimeType:referenceImage.mimeType,data:referenceImage.data}],referenceConsent:true}:{};}
function referenceConsentError(){if(referenceReadPending)return 'The image is still being read locally. Wait for its preview, then choose whether to consent.';return referenceImage&&!$('reference-consent').checked?'A reference image is selected but consent is off. Check the Google Gemini consent box or clear the image to continue text-only.':'';}
function selectedReferenceMetadata(){return referenceImage?{selected:true,mimeType:referenceImage.mimeType,consent:$('reference-consent').checked,includedInRequest:$('reference-consent').checked,handling:$('reference-consent').checked?'Sent as creative guidance with explicit consent.':'Local only; request is text-only.'}:{selected:false,consent:false,includedInRequest:false};}
function safePreview(value,key=''){
  if(/^(data|base64|imageData)$/i.test(key))return '[redacted]';
  if(key==='referenceImages'&&Array.isArray(value))return value.map(item=>({mimeType:item?.mimeType,data:'[redacted]'}));
  if(Array.isArray(value))return value.map(item=>safePreview(item));
  if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([name,item])=>[name,safePreview(item,name)]));
  return value;
}
function renderJob(job){
  const newJob=currentJob?.id!==job.id;
  if(newJob&&typeof job.prompt==='string'){$('prompt').value=job.prompt;displayedBrief=Object.values(briefs).includes(job.prompt)?job.prompt:'';$('inject').checked=job.injectDefect===true;$('user-feedback').value=job.userFeedback||'';$('refinement-preview-result').replaceChildren();$('refinement-error').textContent='';}
  jobsCache.set(job.id,job);
  currentJob=job;busy=!terminal.includes(job.status);$('start').disabled=busy;$('cancel').hidden=!busy;setEditorLocked(busy);$('contract-version').textContent=`CONTRACT v${job.contractVersion||1}`;
  const editorContract=editorTheme(job.theme,job.themeId);
  if(JSON.stringify(themeContract)!==JSON.stringify(editorContract))renderThemeContract(editorContract,{selectPreset:true,preview:false});
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
  renderLatestComparison(job);renderAttemptHistory(job);renderRefinementControls(job);
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
  $('error').textContent='';$('theme-error').textContent='';const consentError=referenceConsentError();if(consentError){$('reference-error').textContent=consentError;return;}$('reference-error').textContent='';$('start').disabled=true;releasePinned=false;sceneRevision++;
  try{
    const frozenTheme=validateContract(themeFromForm());renderThemeContract(frozenTheme,{selectPreset:false,preview:false});setEditorLocked(true);
    const job=await api('/api/jobs',{themeId:frozenTheme.id,theme:frozenTheme,prompt:$('prompt').value,provider:$('provider').value,injectDefect:$('inject').checked,...referenceRequestFields()});lastHash='';renderJob(job);
  }catch(e){error(e);$('theme-error').textContent=e.message;$('start').disabled=false;setEditorLocked(false);}
};
$('cancel').onclick=async()=>{try{renderJob(await api(`/api/jobs/${currentJob.id}/cancel`,{}));}catch(e){error(e);}};
$('activate').onclick=async()=>{try{const r=await api(`/api/jobs/${currentJob.id}/activate`,{});$('release-label').textContent=(r.rehearsal?'REHEARSAL ACTIVE · ':'ACTIVE · ')+r.hash.slice(0,10);renderJob(await api('/api/jobs/'+currentJob.id));}catch(e){error(e);}};
$('base-version').onchange=()=>{if(currentJob)renderRefinementControls(currentJob);};
$('user-feedback').addEventListener('input',()=>{if(currentJob)renderRefinementControls(currentJob);});
$('reference-file').addEventListener('change',event=>chooseLocalReference(event.target.files?.[0]));
$('clear-reference').onclick=clearLocalReference;
function refinementRequest(){
  const selected=currentJob?.attempts?.find(attempt=>attempt.hash===$('base-version').value&&attempt.recipe),userFeedback=$('user-feedback').value.trim();
  if(!currentJob||!selected)return null;
  return {baseJobId:currentJob.id,baseAttemptNumber:selected.number,baseCandidateHash:selected.hash,userFeedback,prompt:currentJob.prompt,theme:structuredClone(currentJob.theme),provider:$('provider').value,injectDefect:false,...referenceRequestFields()};
}
$('preview-refinement').onclick=async()=>{
  $('refinement-error').textContent='';$('refinement-preview-result').replaceChildren();
  const request=refinementRequest();if(!request)return;
  if(request.userFeedback.length<5){$('refinement-error').textContent='Describe the change in at least five characters.';return;}
  const consentError=referenceConsentError();if(consentError){$('reference-error').textContent=consentError;return;}$('reference-error').textContent='';
  $('preview-refinement').disabled=true;
  try{
    const result=await api('/api/refinement-preview',request),summary=result.summary||result.message||'Request preview prepared. No generation or provider call was made.';
    $('refinement-preview-result').append(el('p',summary,'preview-summary'));
    const previewMetadata={...safePreview(result),referenceImage:selectedReferenceMetadata()};
    const details=el('details',undefined,'preview-details'),pre=el('pre',JSON.stringify(previewMetadata,null,2));details.append(el('summary','Inspect the request metadata · image data hidden'),pre);$('refinement-preview-result').append(details);
  }catch(e){$('refinement-error').textContent=e.message;}
  finally{if(currentJob)renderRefinementControls(currentJob);}
};
$('refine-version').onclick=async()=>{
  $('refinement-error').textContent='';
  const request=refinementRequest();
  if(!request||busy)return;
  if(request.userFeedback.length<5){$('refinement-error').textContent='Describe the change in at least five characters.';return;}
  const consentError=referenceConsentError();if(consentError){$('reference-error').textContent=consentError;return;}$('reference-error').textContent='';
  $('refinement-preview-result').replaceChildren();
  $('refine-version').disabled=true;setEditorLocked(true);
  try{
    const job=await api('/api/jobs',request);lastHash='';renderJob(job);
  }catch(e){$('refinement-error').textContent=e.message;setEditorLocked(false);if(currentJob)renderRefinementControls(currentJob);}
};
window.addEventListener('beforeunload',()=>{if(referenceObjectUrl)URL.revokeObjectURL(referenceObjectUrl);});
$('rollback').onclick=async()=>{releasePinned=true;sceneRevision++;try{const r=await api('/api/rollback',{});renderThemeContract(editorTheme(r.theme,r.theme?.id),{selectPreset:true,preview:false});releasePinned=true;world.load(r.recipe,r.theme);lastHash='';$('scene-name').textContent=r.recipe.name.toUpperCase();$('scene-theme').textContent=r.theme.name.toUpperCase();$('version-label').textContent='RESTORED RELEASE';$('release-label').textContent='RESTORED · '+r.hash.slice(0,10);$('error').textContent='';}catch(e){releasePinned=false;error(e);}};
try{
  config=await api('/api/config');$('connection').textContent=config.keyConfigured?'GEMINI · CONFIGURED':'OFFLINE REHEARSAL';if(!config.keyConfigured)$('provider').value='fixture';
  $('resolve-theme').disabled=config.themeInferenceEnabled!==true;$('theme-draft-note').textContent=config.themeInferenceEnabled?'GEMINI RESOLUTION AVAILABLE · Costs one request. Review recorded assumptions before use.':'Gemini theme resolution is disabled until an explicit spending cap is enabled. Local keyword drafts use the selected preset fallback.';modeNote();
  const jobs=await api('/api/jobs');jobsCache=new Map(jobs.map(job=>[job.id,job]));if(jobs.length){const requestedId=new URLSearchParams(location.search).get('job'),selected=jobs.find(job=>job.id===requestedId)||jobs[0];if(selected.theme)renderThemeContract(selected.theme,{selectPreset:true,preview:false});$('provider').value=selected.provider;modeNote();renderJob(selected);}
  const release=await api('/api/release');if(release){$('release-label').textContent=(release.rehearsal?'REHEARSAL ACTIVE · ':'ACTIVE · ')+release.hash.slice(0,10);if(!currentJob){renderThemeContract(editorTheme(release.theme,release.theme?.id),{selectPreset:true,preview:false});world.load(release.recipe,release.theme);$('scene-name').textContent=release.recipe.name.toUpperCase();$('scene-theme').textContent=release.theme.name.toUpperCase();$('version-label').textContent='ACTIVE RELEASE';}}
}catch(e){error(e);}
setInterval(async()=>{if(currentJob&&busy&&!releasePinned&&!pollInFlight){pollInFlight=true;const revision=sceneRevision,jobId=currentJob.id;try{const job=await api('/api/jobs/'+jobId);if(revision===sceneRevision&&!releasePinned)renderJob(job);}catch(e){if(revision===sceneRevision)error(e);}finally{pollInFlight=false;}}},900);
