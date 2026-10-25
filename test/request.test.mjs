import test from 'node:test';
import assert from 'node:assert/strict';
import {requestJSON} from '../public/request.mjs';

const stalled=()=>new Promise(()=>{});
async function outcome(promise) {
  let timer;
  try{return await Promise.race([promise.then(value=>({value}),error=>({error})),new Promise(resolve=>{timer=setTimeout(()=>resolve({stalled:true}),150);})]);}
  finally{clearTimeout(timer);}
}

test('stalled status headers time out and permit a later successful refresh',async()=>{
  let calls=0,signal;
  const fetchImpl=async(_url,options)=>{calls++;signal=options.signal;return calls===1?stalled():new Response('{"status":"accepted"}');};
  const result=await outcome(requestJSON('/api/jobs/job-one',undefined,{fetchImpl,timeoutMs:20}));
  assert.equal(result.stalled,undefined,'request stayed pending and would hold the polling slot');
  assert.equal(result.error?.name,'TimeoutError');
  assert.equal(signal.aborted,true);
  assert.deepEqual(await requestJSON('/api/jobs/job-one',undefined,{fetchImpl}),{status:'accepted'});
  assert.equal(calls,2);
});

test('the deadline includes consuming the response body',async()=>{
  const result=await outcome(requestJSON('/api/jobs/job-one',undefined,{timeoutMs:20,fetchImpl:async()=>({ok:true,json:stalled})}));
  assert.equal(result.stalled,undefined,'response headers must not disable the body deadline');
  assert.equal(result.error?.name,'TimeoutError');
});

test('timed-out creation retains an unknown outcome without repeating the paid action',async()=>{
  let calls=0,request;
  const result=await outcome(requestJSON('/api/jobs',{prompt:'a lookout'},{timeoutMs:20,fetchImpl:async(_url,options)=>{calls++;request=options;return stalled();}}));
  assert.equal(result.stalled,undefined);
  assert.equal(result.error?.outcomeUnknown,true);
  assert.match(result.error.message,/unknown|may still/i);
  assert.equal(calls,1);
  assert.equal(request.method,'POST');
  assert.deepEqual(JSON.parse(request.body),{prompt:'a lookout'});
});

test('a network failure after dispatch warns of an unknown action outcome',async()=>{
  let calls=0;
  await assert.rejects(requestJSON('/api/jobs/job-one/cancel',{}, {fetchImpl:async()=>{calls++;throw new TypeError('network lost');}}),error=>{
    assert.equal(error.outcomeUnknown,true);assert.match(error.message,/unknown|may still/i);return true;
  });
  assert.equal(calls,1);
});

test('an explicit server rejection preserves its reason and does not retry',async()=>{
  let calls=0;
  await assert.rejects(requestJSON('/api/jobs',{}, {fetchImpl:async()=>{calls++;return new Response('{"error":"A run is already in progress"}',{status:409});}}),error=>{
    assert.equal(error.message,'A run is already in progress');assert.equal(error.outcomeUnknown,false);return true;
  });
  assert.equal(calls,1);
});

test('a successful action returns its receipt without mutation',async()=>{
  const receipt={id:'job-one',status:'queued'};
  assert.deepEqual(await requestJSON('/api/jobs',{prompt:'a lookout'}, {fetchImpl:async()=>new Response(JSON.stringify(receipt))}),receipt);
});

test('a non-JSON client rejection keeps its known HTTP status',async()=>{
  await assert.rejects(requestJSON('/api/jobs',{}, {fetchImpl:async()=>new Response('Already running',{status:409})}),error=>{
    assert.equal(error.outcomeUnknown,false);assert.match(error.message,/409/);return true;
  });
});

test('a server failure keeps its reason and an uncertain action outcome',async()=>{
  await assert.rejects(requestJSON('/api/jobs',{}, {fetchImpl:async()=>new Response('{"error":"Persistence failed"}',{status:500})}),error=>{
    assert.equal(error.outcomeUnknown,true);assert.match(error.message,/Persistence failed/);return true;
  });
});
