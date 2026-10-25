import fs from 'node:fs';
import path from 'node:path';
import {randomUUID} from 'node:crypto';

// One runtime owns this session ledger. Reservations survive crashes and unknown
// responses; this is a conservative application allowance, not Google's invoice.
export class SpendLedger {
  constructor(folder, capUsd, {sponsoredCredits=false}={}) {
    this.sponsoredCredits=sponsoredCredits===true;
    this.file=path.join(folder,'google-spend-ledger.json');
    this.capUsd=Number(capUsd)||0;
    this.state=fs.existsSync(this.file)?JSON.parse(fs.readFileSync(this.file,'utf8')):{schemaVersion:1,capUsd:this.capUsd,pricing:{model:'gemini-3.8-flash',inputPerMillion:.75,outputPerMillion:3.75,source:'https://ai.google.dev/gemini-api/docs/pricing',checkedAt:'2026-10-25'},entries:[]};
    this.state.capUsd=this.capUsd;
    const probe=path.join(folder,'managed-probe.json');
    if(fs.existsSync(probe)&&!this.state.entries.some(e=>e.id==='earlier-managed-probe')){
      const saved=JSON.parse(fs.readFileSync(probe,'utf8'));
      this.state.entries.push({id:'earlier-managed-probe',status:'prior_usage_conservative_allowance',allowanceUsd:5,usage:saved.usage||null,note:'Earlier managed probe included. Exact historical tool/sandbox invoice unavailable; retain $5 allowance rather than claim a measured dollar cost.'});
    }
    this.save();
  }
  save(){const temporary=this.file+'.tmp';fs.writeFileSync(temporary,JSON.stringify(this.state,null,2),{mode:0o600});fs.renameSync(temporary,this.file);}
  summary(){const allocatedUsd=this.state.entries.reduce((sum,e)=>sum+e.allowanceUsd,0);return {sponsoredCredits:this.sponsoredCredits,capUsd:this.sponsoredCredits?null:this.capUsd,allocatedUsd,remainingUsd:this.sponsoredCredits?null:Math.max(0,this.capUsd-allocatedUsd),entries:this.state.entries.length,note:'Conservative allowances include prior managed usage and unresolved calls; not an invoice.'};}
  reserve({jobId,label,model,managed}){
    if(!this.sponsoredCredits&&model!=='gemini-3.8-flash')throw new Error('Session pricing is verified only for gemini-3.8-flash; configure its price before another model.');
    const allowanceUsd=this.sponsoredCredits?0:managed?5:2;
    if(!this.sponsoredCredits&&(this.capUsd<=0||this.summary().remainingUsd<allowanceUsd))throw new Error('Shared Google session spending allowance exhausted or not configured.');
    const entry={id:randomUUID(),jobId,label,model,managed,allowanceUsd,sponsoredCredits:this.sponsoredCredits,status:'reserved',at:new Date().toISOString()};this.state.entries.push(entry);this.save();return entry.id;
  }
  settle(id,usage){
    const entry=this.state.entries.find(e=>e.id===id);if(!entry)throw new Error('Missing spend reservation');
    entry.usage=usage||null;entry.status=usage?'returned':'unknown_usage';
    const total=Number(usage?.totalTokenCount??usage?.total_tokens);
    // Charge all reported tokens at the highest token price. Managed calls also
    // retain their tool allowance because their token report is not a full bill.
    if(!entry.sponsoredCredits&&!entry.managed&&Number.isFinite(total)&&total>0){const upper=total*3.75/1e6;entry.allowanceUsd=Math.max(.000001,upper);}
    this.save();
    if(!this.sponsoredCredits&&this.summary().allocatedUsd>this.capUsd)throw new Error('Reported usage exceeded the shared allowance; further dispatch is blocked.');
  }
  unknown(id){const entry=this.state.entries.find(e=>e.id===id);if(entry){entry.status='failed_or_unknown';this.save();}}
}
