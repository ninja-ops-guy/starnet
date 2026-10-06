'use strict';
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const os = require('node:os');
const {CommandJournal} = require('./lib/commands.cjs');
const {StationClient, StationError} = require('./lib/station.cjs');
const ROOT = path.join(__dirname, 'public');
const TYPES = {'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.json':'application/json','.png':'image/png','.webp':'image/webp','.svg':'image/svg+xml','.txt':'text/plain'};
function createServer({station, port = 0, control = false, dataDirectory = path.join(os.homedir(),'.aerotech-operations')} = {}) {
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error('Port must be an integer from 0 through 65535.');
  const client = station ? new StationClient(station) : null;
  if(control&&!client)throw new Error('--control requires an explicit --station address.');
  const journal=control?new CommandJournal(path.join(dataDirectory,'commands'),client):null;
  const session = crypto.randomBytes(32).toString('hex');
  const instance = crypto.randomUUID();
  const server = http.createServer(async (req, res) => {
    const actual = server.address().port;
    const hosts = [`127.0.0.1:${actual}`, `localhost:${actual}`];
    const origin = req.headers.origin;
    const okOrigin = !origin || hosts.some(h => origin === 'http://' + h);
    function reply(code, body, type = 'application/json; charset=utf-8') {
      const bytes = Buffer.isBuffer(body) ? body : Buffer.from(type.startsWith('application/json') ? JSON.stringify(body) : body);
      res.writeHead(code, {'Content-Type':type,'Content-Length':bytes.length,'Cache-Control':'no-store',
        'X-Content-Type-Options':'nosniff','Referrer-Policy':'no-referrer',
        'Content-Security-Policy':"default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"});
      res.end(bytes);
    }
    if (!hosts.includes(req.headers.host) || !okOrigin || req.headers['sec-fetch-site'] === 'cross-site') return reply(403,{error:'Request origin is not allowed.'});
    if (!['GET','HEAD','POST'].includes(req.method)) return reply(405,{error:'Method not allowed.'});
    try {
      const u = new URL(req.url, 'http://' + req.headers.host);
      const cookie = (req.headers.cookie || '').split(';').map(x=>x.trim()).includes('aerotech_session=' + session);
      if(req.method==='POST'){
        if(!cookie)return reply(403,{error:'Local session required.'});
        if(!journal||u.pathname!=='/api/command')return reply(405,{error:'Controls are disabled. Relaunch with --control to opt in, or use Command Station.'});
        if(req.headers['content-type']!=='application/json'||!Number.isSafeInteger(Number(req.headers['content-length']))||Number(req.headers['content-length'])<1||Number(req.headers['content-length'])>8192)return reply(400,{error:'Use a bounded JSON command.'});
        let chunks=[],length=0;for await(const chunk of req){length+=chunk.length;if(length>8192)return reply(413,{error:'Command too large.'});chunks.push(chunk);}
        let input;try{input=JSON.parse(Buffer.concat(chunks).toString('utf8'));}catch{return reply(400,{error:'Invalid JSON.'});}
        return reply(200,await journal.execute(input));
      }
      if (u.pathname.startsWith('/api/')) {
        if (!cookie) return reply(403,{error:'Open the AeroTech home page to establish a local session.'});
        if (u.pathname === '/api/status') return reply(200,{product:'AeroTech Operations',version:'0.1.0',instance_id:instance,
          station_url:client?.url || null, adapter:'RESIDUAL Command Station 0.3', access:control?'controlled':'read-only',poll_interval_ms:3000});
        if (!client) return reply(409,{code:'NOT_CONFIGURED',error:'Launch with --station http://127.0.0.1:8765 to connect RESIDUAL.'});
        if (u.pathname === '/api/projects') return reply(200,{projects:await client.projects(),observed_at:new Date().toISOString()});
        if (u.pathname === '/api/project') return reply(200,{...await client.detail(u.searchParams.get('id') || ''),observed_at:new Date().toISOString()});
        if (u.pathname === '/api/artifact') return reply(200,await client.artifact(u.searchParams.get('project') || '',u.searchParams.get('id') || ''));
        return reply(404,{error:'Unknown API route.'});
      }
      if (u.pathname === '/') res.setHeader('Set-Cookie',`aerotech_session=${session}; HttpOnly; SameSite=Strict; Path=/`);
      const relative = u.pathname === '/' ? 'index.html' : decodeURIComponent(u.pathname).slice(1);
      if (relative.split(/[\\/]/).some(p=>p.startsWith('.')) || relative.includes('\0')) return reply(404,{error:'Not found.'});
      const file = path.resolve(ROOT, relative);
      if (!file.startsWith(ROOT + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) return reply(404,{error:'Not found.'});
      const type = TYPES[path.extname(file)];
      if (!type) return reply(404,{error:'Not found.'});
      reply(200,fs.readFileSync(file),type);
    } catch (e) {
      reply(e instanceof StationError ? 502 : 500, {code:e.code || 'READ_FAILED',error:e instanceof StationError ? e.message : 'Could not read station data. Retry the connection.'});
    }
  });
  server.requestTimeout = 15000; server.headersTimeout = 10000;
  return {server, start:()=>new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,'127.0.0.1',()=>resolve(`http://127.0.0.1:${server.address().port}`));})};
}
if (require.main === module) {
  const args = process.argv.slice(2); let station = process.env.AEROTECH_STATION_URL; let port = Number(process.env.AEROTECH_PORT || 0);let control=false;let dataDirectory=process.env.AEROTECH_DATA_DIR;
  for (let i=0;i<args.length;i++) {
    if (args[i] === '--station' && args[i+1]) station = args[++i];
    else if (args[i] === '--port' && args[i+1]) port = Number(args[++i]);
    else if(args[i]==='--control')control=true;
    else if(args[i]==='--data'&&args[i+1])dataDirectory=args[++i];
    else if (args[i] === '--help') {console.log('node server.cjs [--station http://127.0.0.1:8765] [--port 0] [--control] [--data PATH]\nDefault: free loopback port and read-only access. --control enables explicit host run/triage/pause requests.');process.exit(0);}
    else {console.error('Unknown or incomplete option. Use --help.');process.exit(1);}
  }
  try {
    const app = createServer({station,port,control,dataDirectory});
    app.start().then(url=>console.log(`AeroTech Operations 0.1\nOpen ${url}\n${station ? 'RESIDUAL observer configured; source data remains in Command Station.' : 'Offline studio — use the labeled demo or relaunch with --station.'}`))
      .catch(e=>{console.error(e.code === 'EADDRINUSE' ? 'PORT_IN_USE: choose --port 0 for an available port. Existing services were not changed.' : e.message);process.exitCode=1;});
    for (const signal of ['SIGINT','SIGTERM']) process.on(signal,()=>app.server.close(()=>process.exit(0)));
  } catch(e) {console.error(e.message);process.exitCode=1;}
}
module.exports = {createServer};
