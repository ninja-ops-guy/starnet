'use strict';
const http = require('node:http');
const https = require('node:https');
const { URL } = require('node:url');

class StationError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
    this.name = 'StationError';
  }
}

class StationClient {
  constructor(url) {
    this.url = url;
    const parsed = new URL(url);
    this.client = parsed.protocol === 'https:' ? https : http;
    this.basePath = parsed.pathname.replace(/\/$/, '');
    this.hostname = parsed.hostname;
    this.port = parsed.port || (parsed.protocol === 'https:' ? 443 : 80);
    this.protocol = parsed.protocol;
  }

  async request(path, method = 'GET', body = null) {
    return new Promise((resolve, reject) => {
      const options = {
        hostname: this.hostname,
        port: this.port,
        path: this.basePath + path,
        method,
        headers: {
          'Accept': 'application/json',
          'Content-Type': body ? 'application/json' : undefined
        }
      };

      if (body) {
        const bodyStr = JSON.stringify(body);
        options.headers['Content-Length'] = Buffer.byteLength(bodyStr);
      }

      const req = this.client.request(options, (res) => {
        let data = '';
        res.on('data', chunk => data += chunk);
        res.on('end', () => {
          try {
            const parsed = JSON.parse(data);
            if (res.statusCode >= 400) {
              reject(new StationError(parsed.code || 'STATION_ERROR', parsed.error || `HTTP ${res.statusCode}`));
            } else {
              resolve(parsed);
            }
          } catch (e) {
            reject(new StationError('PARSE_ERROR', 'Invalid JSON from station'));
          }
        });
      });

      req.on('error', (err) => {
        reject(new StationError('NETWORK_ERROR', err.message));
      });

      req.setTimeout(10000, () => {
        req.destroy();
        reject(new StationError('TIMEOUT', 'Station request timed out'));
      });

      if (body) req.write(JSON.stringify(body));
      req.end();
    });
  }

  async projects() {
    return this.request('/api/v1/projects');
  }

  async detail(id) {
    return this.request(`/api/v1/projects/${encodeURIComponent(id)}`);
  }

  async artifact(projectId, artifactId) {
    return this.request(`/api/v1/projects/${encodeURIComponent(projectId)}/artifacts/${encodeURIComponent(artifactId)}`);
  }

  async command(payload) {
    return this.request('/api/v1/commands', 'POST', payload);
  }
}

module.exports = { StationClient, StationError };
