const terminal=['accepted','active','failed','blocked','cancelled','exhausted'];
const duration=ms=>{const seconds=Math.floor(ms/1000);return seconds>=60?`${Math.floor(seconds/60)}m ${seconds%60}s`:`${seconds}s`;};
export function describeJob(job,now=Date.now()) {
  const limits=job.pipeline?.contract?.limits||{},attempt=job.attempts?.at(-1),stopped=['failed','blocked','exhausted','cancelled'].includes(job.status);
  const events=job.events||[],finished=terminal.includes(job.status);
  // Activation/rollback events can happen long after generation has finished.
  const end=finished?Date.parse(events.find(event=>terminal.includes(event.state))?.at):now;
  const start=Date.parse(job.createdAt),stageStart=Date.parse(events.findLast(event=>event.state===job.status)?.at);
  const elapsed=Number.isFinite(start)&&Number.isFinite(end)&&end>=start?duration(end-start):null;
  const stageTime=!finished&&Number.isFinite(stageStart)&&now>=stageStart?duration(now-stageStart):null;
  const timing=[elapsed&&(finished?`Finished after ${elapsed}`:`Elapsed ${elapsed}`),stageTime&&`This stage ${stageTime}`].filter(Boolean).join(' · ');
  const phase={queued:'Waiting to start',generating:'Generating scene',repairing:'Revising scene',rendering:'Capturing actual renders',reviewing:'Independent visual review in progress',validating:'Checking geometry',preparing:'Preparing the selected scene',accepted:'Scene accepted',active:'Scene active',blocked:'Run blocked',failed:'Run failed',exhausted:'Run limit reached',cancelled:'Run cancelled'}[job.status]||job.status;
  const failure=attempt?.validationError||(attempt?.checks||[]).find(check=>check.status==='fail'||check.status==='blocked')?.detail||attempt?.decisionReason;
  const reached=[],attempts=job.attempts?.length||0,calls=job.calls||0,deadline=Number(job.deadline??job.pipeline?.contract?.deadlineMs);
  if(job.status==='exhausted'){
    if(attempts>=(limits.maxAttempts??3))reached.push(`Candidate limit reached (${attempts}/${limits.maxAttempts??3})`);
    if(calls>=(limits.maxProviderCalls??6))reached.push(`API call limit reached (${calls}/${limits.maxProviderCalls??6})`);
    if(Number.isFinite(deadline)&&Number.isFinite(end)&&end>=deadline)reached.push(`Time limit reached (${Math.round((limits.timeBudgetMs??1200000)/60000)} minutes)`);
  }
  const reason=stopped?[...reached,job.stopReason||phase,failure&&`Last issue: ${failure}`].filter(Boolean).join('. '):job.status==='reviewing'?'The critic is inspecting three actual renderer views.':!finished?events.at(-1)?.message||'Work is in progress.':job.stopReason||'';
  return {phase,reason,limits,timing,errorText:stopped?reason:''};
}
export function jobErrorText(job,{previousJob,currentError='',pollError=''}={}){
  const failure=describeJob(job).errorText;
  if(failure)return failure;
  if(previousJob?.id!==job.id||currentError===pollError||currentError===(previousJob&&describeJob(previousJob).errorText))return '';
  return currentError;
}
