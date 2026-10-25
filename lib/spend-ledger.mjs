import fs from 'node:fs';
import path from 'node:path';
import {randomUUID} from 'node:crypto';

// Verified exact model IDs only. Argon pricing expires on the date specified;
// startup/dispatch must fail closed after expiry until the table is refreshed.
export const PRICING_BY_MODEL = Object.freeze({
  'gemini-3.8-flash': Object.freeze({inputPerMillion:.75,outputPerMillion:3.75,source:'https://ai.google.dev/gemini-api/docs/pricing',checkedAt:'2026-10-25'}),
  'gemini-4-argon': Object.freeze({inputPerMillion:2,outputPerMillion:10,source:'https://ai.google.dev/gemini-api/docs/latest-model',checkedAt:'2026-10-25',validUntil:'2026-11-30'}),
});

const pricingFor=(model,now=new Date())=>{
  const price=PRICING_BY_MODEL[model];
  if(!price)throw new Error(`Session pricing is not verified for exact model ID: ${model}`);
  if(price.validUntil&&now.toISOString().slice(0,10)>price.validUntil)throw new Error(`Verified pricing expired for ${model}; refresh pricing before dispatch.`);
  return price;
};

// One runtime owns this session ledger. Reservations survive crashes and unknown
// responses; this is a conservative application allowance, not Google's invoice.
export class SpendLedger {
  constructor(folder, capUsd, options={}) {
    const {sponsoredCredits=false,now=()=>new Date()}=typeof options==='function'?{now:options}:options;
    this.now=now;
    this.sponsoredCredits=sponsoredCredits===true;
    this.file=path.join(folder,'google-spend-ledger.json');
    this.capUsd=Number(capUsd)||0;
    this.state=fs.existsSync(this.file)?JSON.parse(fs.readFileSync(this.file,'utf8')):{schemaVersion:1,capUsd:this.capUsd,pricing:{model:'gemini-3.8-flash',inputPerMillion:.75,outputPerMillion:3.75,source:'https://ai.google.dev/gemini-api/docs/pricing',checkedAt:'2026-10-25'},entries:[]};
    this.state.pricingByModel??=structuredClone(PRICING_BY_MODEL);
    this.state.capUsd=this.capUsd;
    const probe=path.join(folder,'managed-probe.json');
    if(fs.existsSync(probe)&&!this.state.entries.some(e=>e.id==='earlier-managed-probe')){
      const saved=JSON.parse(fs.readFileSync(probe,'utf8'));
      this.state.entries.push({id:'earlier-managed-probe',status:'prior_usage_conservative_allowance',allowanceUsd:5,usage:saved.usage||null,note:'Earlier managed probe included. Exact historical tool/sandbox invoice unavailable; retain $5 allowance rather than claim a measured dollar cost.'});
    }
    this.save();
  }
  save(){const temporary=this.file+'.tmp';fs.writeFileSync(temporary,JSON.stringify(this.state,null,2),{mode:0o600});fs.renameSync(temporary,this.file);}
  summary(){const allocatedUsd=this.state.entries.reduce((sum,e)=>sum+(Number(e.allowanceUsd)||0),0);return {sponsoredCredits:this.sponsoredCredits,capUsd:this.sponsoredCredits?null:this.capUsd,allocatedUsd,remainingUsd:this.sponsoredCredits?null:Math.max(0,this.capUsd-allocatedUsd),entries:this.state.entries.length,note:'Conservative allowances include prior managed usage and unresolved calls; not an invoice.'};}
  reserve({jobId,label,model,managed}){
    if(!this.sponsoredCredits)pricingFor(model,this.now());
    const allowanceUsd=this.sponsoredCredits?0:managed?5:2;
    if(!this.sponsoredCredits&&(this.capUsd<=0||this.summary().remainingUsd<allowanceUsd))throw new Error('Shared Google session spending allowance exhausted or not configured.');
    const entry={id:randomUUID(),jobId,label,model,managed,allowanceUsd,sponsoredCredits:this.sponsoredCredits,status:'reserved',at:this.now().toISOString()};this.state.entries.push(entry);this.save();return entry.id;
  }
  settle(id,usage){
    const entry=this.state.entries.find(e=>e.id===id);if(!entry)throw new Error('Missing spend reservation');
    entry.usage=usage||null;entry.status=usage?'returned':'unknown_usage';
    const total=Number(usage?.totalTokenCount??usage?.total_tokens);
    // Charge all reported tokens at the highest token price. Managed calls also
    // retain their tool allowance because their token report is not a full bill.
    if(!entry.sponsoredCredits&&Number.isFinite(total)&&total>0){const upper=total*pricingFor(entry.model,this.now()).outputPerMillion/1e6;entry.allowanceUsd=entry.managed?Math.max(5,upper):Math.max(.000001,upper);}
    this.save();
    if(!this.sponsoredCredits&&this.summary().allocatedUsd>this.capUsd)throw new Error('Reported usage exceeded the shared allowance; further dispatch is blocked.');
  }
  unknown(id){const entry=this.state.entries.find(e=>e.id===id);if(entry){entry.status='failed_or_unknown';this.save();}}
}
