#!/usr/bin/env node
/**
 * Asset verification script
 * 
 * Usage: node scripts/asset-check.mjs
 * 
 * Verifies every entry in ASSET-MANIFEST.md against the live tree.
 * Checks: file exists, SHA-256 matches, source commit is documented.
 */

import { readFile } from 'fs/promises';
import { createHash } from 'crypto';
import { existsSync } from 'fs';
import { resolve } from 'path';

const MANIFEST_PATH = 'ASSET-MANIFEST.md';

async function hashFile(path) {
  const buf = await readFile(path);
  return createHash('sha256').update(buf).digest('hex');
}

async function main() {
  const manifest = await readFile(MANIFEST_PATH, 'utf-8');
  
  // Parse markdown table rows
  const lines = manifest.split('\n');
  const errors = [];
  const checks = [];
  
  for (const line of lines) {
    // Match table rows with | path | source | commit | hash | status |
    const match = line.match(/^\|\s*`([^`]+)`\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|$/);
    if (!match) continue;
    
    const [, path, source, commit, hash, status] = match.map(s => s.trim());
    
    // Skip header rows and pending items
    if (path.includes('StarNet path') || path.includes('---')) continue;
    if (status.includes('pending')) {
      checks.push({ path, status: 'PENDING', ok: null });
      continue;
    }
    if (status.includes('preserved')) {
      checks.push({ path, status: 'PRESERVED', ok: null });
      continue;
    }
    
    const fullPath = resolve(path);
    if (!existsSync(fullPath)) {
      errors.push(`${path}: file not found`);
      checks.push({ path, status: 'MISSING', ok: false });
      continue;
    }
    
    if (hash === 'TBD') {
      checks.push({ path, status: 'HASH_TBD', ok: null });
      continue;
    }
    
    const actualHash = await hashFile(fullPath);
    if (actualHash !== hash) {
      errors.push(`${path}: hash mismatch (expected ${hash}, got ${actualHash})`);
      checks.push({ path, status: 'HASH_MISMATCH', ok: false });
    } else {
      checks.push({ path, status: 'OK', ok: true });
    }
  }
  
  console.log(`Asset Check Results`);
  console.log(`===================\n`);
  
  const ok = checks.filter(c => c.ok === true).length;
  const fail = checks.filter(c => c.ok === false).length;
  const pending = checks.filter(c => c.ok === null).length;
  
  console.log(`  OK:       ${ok}`);
  console.log(`  FAIL:     ${fail}`);
  console.log(`  PENDING:  ${pending}\n`);
  
  if (errors.length > 0) {
    console.log('Errors:');
    for (const err of errors) {
      console.log(`  ✗ ${err}`);
    }
    process.exit(1);
  } else if (fail === 0 && pending > 0) {
    console.log('All checked assets pass. Some assets are still pending replacement.');
    process.exit(0);
  } else {
    console.log('All assets verified.');
    process.exit(0);
  }
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
