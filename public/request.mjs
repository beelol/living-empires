// Shared by Studio reads and actions; one invocation dispatches one request.
export async function requestJSON(url, body, {fetchImpl=globalThis.fetch,timeoutMs}={}) {
  const action=body!==undefined;
  // Creating a job can first wait for the server's bounded theme inference.
  timeoutMs??=action?(['/api/jobs','/api/themes/resolve'].includes(url)?135000:15000):10000;
  const options=action?{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}:{};
  const controller=new AbortController();
  let timer;
  const unknownMessage='The request outcome is unknown; the action may still have completed. Check saved scenes or refresh its status before submitting again.';
  const deadline=new Promise((_,reject)=>{
    timer=setTimeout(()=>{
      const error=new Error(action?unknownMessage:'Studio status request timed out. Status refresh will retry; the job has not been cancelled.');
      error.name='TimeoutError';error.outcomeUnknown=action;
      reject(error);controller.abort();
    },timeoutMs);
  });
  try {
    return await Promise.race([deadline,(async()=>{
      const response=await fetchImpl(url,{...options,signal:controller.signal});
      let value;
      try{value=await response.json();}
      catch(error){if(response.ok)throw error;value={};}
      if(!response.ok){
        const error=new Error(value.error||`Request failed (${response.status})`);
        error.outcomeUnknown=action&&response.status>=500;
        if(error.outcomeUnknown)error.message+=`. ${unknownMessage}`;
        throw error;
      }
      return value;
    })()]);
  } catch(error) {
    if(action&&error.outcomeUnknown===undefined){
      const uncertain=new Error(unknownMessage,{cause:error});uncertain.outcomeUnknown=true;throw uncertain;
    }
    throw error;
  } finally {clearTimeout(timer);}
}
