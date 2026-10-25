import test from 'node:test';
import assert from 'node:assert/strict';
import {describeJob,jobErrorText} from '../public/job-status.mjs';

const start=Date.parse('2026-10-25T22:00:00Z');
const job={id:'job-one',status:'reviewing',createdAt:new Date(start).toISOString(),attempts:[],calls:2,events:[
  {state:'generating',at:new Date(start).toISOString(),message:'Creating scene'},
  {state:'reviewing',at:new Date(start+30000).toISOString(),message:'Reviewing actual views'}
]};

test('active progress reports actual elapsed and stage time without inventing a completion percentage',()=>{
  const value=describeJob(job,start+45000);
  assert.equal(value.timing,'Elapsed 45s · This stage 15s');
  assert.match(value.phase,/review/i);
});
test('accepted duration stays at acceptance when activation occurs later',()=>{
  const completed={...job,status:'active',events:[...job.events,{state:'accepted',at:new Date(start+65000).toISOString()},{state:'active',at:new Date(start+200000).toISOString()}]};
  assert.equal(describeJob(completed,start+500000).timing,'Finished after 1m 5s');
});
test('missing terminal timestamps do not fabricate a duration',()=>{
  assert.equal(describeJob({...job,status:'blocked',events:[],createdAt:undefined}).timing,'');
  assert.equal(describeJob({...job,status:'blocked',events:[]},start+50000).timing,'');
});
test('exhaustion names the actual frozen limits and retains the failure',()=>{
  const exhausted={...job,status:'exhausted',calls:4,attempts:[{},{},{}],stopReason:'No more candidates can be evaluated',pipeline:{contract:{limits:{maxAttempts:3,maxProviderCalls:6,timeBudgetMs:1200000}}},events:[...job.events,{state:'exhausted',at:new Date(start+100000).toISOString()}]};
  const value=describeJob(exhausted,start+500000);
  assert.match(value.reason,/Candidate limit reached \(3\/3\)/);
  assert.doesNotMatch(value.reason,/API call limit reached|Time limit reached/);
  assert.match(value.errorText,/No more candidates/);
});
test('new frozen budgets are displayed without assuming the ordinary candidate ceiling',()=>{
  const limited={...job,status:'exhausted',calls:60,pipeline:{contract:{limits:{maxAttempts:30,maxProviderCalls:60,timeBudgetMs:300000}}},stopReason:'Limit exhausted'};
  assert.match(describeJob(limited).reason,/API call limit reached \(60\/60\)/);
});
test('untimed style completion reports an exhausted call ceiling without inventing a time limit',()=>{
  const limited={...job,status:'exhausted',deadline:null,calls:60,pipeline:{contract:{deadlineMs:0,limits:{maxAttempts:30,maxProviderCalls:60,timeBudgetMs:0}}},events:[...job.events,{state:'exhausted',at:new Date(start+3600000).toISOString()}]};
  const result=describeJob(limited);
  assert.match(result.reason,/API call limit reached \(60\/60\)/);assert.doesNotMatch(result.reason,/Time limit/);
});
test('historical timed jobs still explain their actual elapsed deadline',()=>{
  const limited={...job,status:'exhausted',deadline:start+1200000,pipeline:{contract:{limits:{maxAttempts:3,maxProviderCalls:6,timeBudgetMs:1200000}}},events:[...job.events,{state:'exhausted',at:new Date(start+1201000).toISOString()}]};
  assert.match(describeJob(limited).reason,/Time limit reached \(20 minutes\)/);
});
test('a routine successful poll retains a failed operator action',()=>{
  assert.equal(jobErrorText(job,{previousJob:job,currentError:'Cancel failed; status is unknown',pollError:''}),'Cancel failed; status is unknown');
});
test('a recovered poll clears only its own connection error',()=>{
  assert.equal(jobErrorText(job,{previousJob:job,currentError:'Status request timed out',pollError:'Status request timed out'}),'');
});
test('terminal failures remain visible and switching jobs clears obsolete errors',()=>{
  const failed={...job,status:'blocked',stopReason:'Gemini request rejected'};
  assert.match(jobErrorText(failed,{previousJob:job,currentError:''}),/Gemini request rejected/);
  assert.equal(jobErrorText({...job,id:'job-two'},{previousJob:failed,currentError:'Gemini request rejected'}),'');
});
