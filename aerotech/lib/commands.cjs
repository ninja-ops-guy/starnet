'use strict';
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {StationError}=require('./station.cjs');
// A durable send-once outbox. Unknown requests are never automatically retransmitted.
// This prevents duplicate forwarding, not exactly-once execution in a remote host.
class CommandJournal{
  constructor(directory,client){this.directory=path.join(directory,crypto.createHash('sha256').update(client.url).digest('hex').slice(0,20));this.client=client;fs.mkdirSync(this.directory,{recursive:true,mode:0o700});}
  save(file,value){const temp=file+'.'+crypto.randomUUID()+'.tmp';const fd=fs.openSync(temp,'wx',0o600);try{fs.writeFileSync(fd,JSON.stringify(value));fs.fsyncSync(fd);}finally{fs.closeSync(fd);}fs.renameSync(temp,file);}
  async execute(input){
    if(!input||typeof input!=='object'||Object.keys(input).sort().join(',')!=='action,command_id,expected_revision,project_id' ||
      typeof input.command_id!=='string'||!/^[-a-f0-9]{36}$/.test(input.command_id)||
      typeof input.project_id!=='string'||!/^p-[a-zA-Z0-9_-]{1,80}$/.test(input.project_id)||
      !['run','triage','pause','resume'].includes(input.action)||typeof input.expected_revision!=='string'||!/^[a-f0-9]{64}$/.test(input.expected_revision))throw new StationError('BAD_COMMAND','Invalid command payload.');
    const normalized={command_id:input.command_id,project_id:input.project_id,action:input.action,expected_revision:input.expected_revision};
    const fingerprint=crypto.createHash('sha256').update(JSON.stringify(normalized)).digest('hex');const file=path.join(this.directory,input.command_id+'.json');
    if(fs.existsSync(file)){
      const old=JSON.parse(fs.readFileSync(file,'utf8'));
      if(old.fingerprint!==fingerprint)throw new StationError('IDEMPOTENCY_CONFLICT','This command ID already names a different request.');
      return old.result||{status:'OUTCOME_UNKNOWN',command_id:input.command_id,message:'A previous dispatch is unresolved; it was not resent.'};
    }
    // Serialize check/reservation across separate launcher processes sharing the journal.
    const lock=path.join(this.directory,'dispatch.lock');let lockfd;
    try{lockfd=fs.openSync(lock,'wx',0o600);}catch(e){if(e.code==='EEXIST')throw new StationError('COMMAND_BUSY','Another dispatch is reserving the journal, or a stale lock needs inspection.');throw e;}
    try{
      for(const name of fs.readdirSync(this.directory).filter(x=>x.endsWith('.json'))){
        const old=JSON.parse(fs.readFileSync(path.join(this.directory,name),'utf8'));
        if(old.input?.project_id===input.project_id&&(!old.result||old.result.status==='OUTCOME_UNKNOWN'))throw new StationError('OUTCOME_UNKNOWN','An earlier request for this mission is unresolved. Inspect the host and journal before dispatching more work.');
      }
      const fd=fs.openSync(file,'wx',0o600);try{fs.writeFileSync(fd,JSON.stringify({fingerprint,input:normalized,reserved_at:new Date().toISOString(),result:null}));fs.fsyncSync(fd);}finally{fs.closeSync(fd);}
    }finally{fs.closeSync(lockfd);fs.unlinkSync(lock);}
    let result;
    try{result={...await this.client.command(normalized),command_id:input.command_id};}
    catch(e){result={status:e instanceof StationError&&e.code!=='OUTCOME_UNKNOWN'?'REJECTED':'OUTCOME_UNKNOWN',code:e.code||'COMMAND_ERROR',message:e instanceof StationError?e.message:'Command failed; inspect the host.',command_id:input.command_id};}
    this.save(file,{fingerprint,input:normalized,recorded_at:new Date().toISOString(),result});return result;
  }
}
module.exports={CommandJournal};
