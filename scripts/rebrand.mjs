#!/usr/bin/env node
/**
 * Rebrand script: StarNet → AeroTech Staff
 * 
 * Usage: node scripts/rebrand.mjs [--dry-run]
 * 
 * This script performs SAFE string replacements across the codebase.
 * It skips binary files, node_modules, and .git.
 * 
 * It deliberately does NOT touch code identifiers (variables, classes,
 * localStorage keys, CSS classes) to minimize breakage.
 */

import { readFile, writeFile, readdir, stat } from 'fs/promises';
import { join, extname } from 'path';
import { createHash } from 'crypto';

const DRY_RUN = process.argv.includes('--dry-run');
const ROOT = process.cwd();

// Extensions to process
const TEXT_EXTS = new Set([
  '.html', '.js', '.mjs', '.css', '.json', '.md', '.txt',
  '.rs', '.toml', '.yml', '.yaml', '.xml', '.svg'
]);

// Directories to skip
const SKIP_DIRS = new Set([
  'node_modules', '.git', 'target', 'dist', 'frontend-embed',
  'voice-deps', 'sidecar/voice-model'
]);

// Safe replacements: user-facing strings only
// Format: [regex, replacement, description]
const REPLACEMENTS = [
  // Titles and labels
  [/StarNet(?!\w)/g, 'AeroTech Staff', 'product name'],
  [/STARNET(?!\w)/g, 'AEROTECH STAFF', 'uppercase product name'],
  
  // Feature branding
  [/StarNet Remote/g, 'AeroTech Remote', 'remote feature'],
  [/StarNet Subscription/g, 'AeroTech Subscription', 'subscription UI'],
  [/StarNet Originals/g, 'AeroTech Originals', 'skill attribution'],
  [/StarNet export/g, 'AeroTech export', 'export file description'],
  
  // URLs (placeholder — update when domain is registered)
  [/www\.starnetos\.com/g, 'aerotech.staff', 'website URL'],
  [/starnetos\.com/g, 'aerotech.staff', 'website URL'],
  
  // Descriptions
  [/the StarNet sidecar/g, 'the AeroTech sidecar', 'architecture docs'],
  [/a StarNet export file/g, 'an AeroTech export file', 'import UI help'],
];

// Files that are allowed to have code-identifier replacements
// (these are config/docs, not runtime code)
const ALLOW_CODE_REPLACEMENTS = new Set([
  'package.json',
  'tauri.conf.json',
  'README.md',
  'REBRAND.md',
  'CONTRIBUTING.md',
  'SECURITY.md',
  'PRIVACY.md',
  'TERMS.md',
  'docs/',
]);

// Additional replacements for config/docs files
const CONFIG_REPLACEMENTS = [
  ["starnet", "aerotech", 'package/config identifier'],
  ["skynet", "aerotech", 'bundle identifier'],
];

async function walk(dir, callback) {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      await walk(path, callback);
    } else {
      await callback(path);
    }
  }
}

async function processFile(filePath) {
  const relPath = filePath.slice(ROOT.length + 1);
  const ext = extname(filePath);
  
  if (!TEXT_EXTS.has(ext)) return null;
  
  let content = await readFile(filePath, 'utf-8');
  let original = content;
  let changes = [];
  
  // Determine which replacement set to use
  const isConfigFile = ALLOW_CODE_REPLACEMENTS.some(p => 
    relPath === p || relPath.startsWith(p)
  );
  
  const replacements = isConfigFile 
    ? [...REPLACEMENTS, ...CONFIG_REPLACEMENTS]
    : REPLACEMENTS;
  
  for (const [pattern, replacement, desc] of replacements) {
    const matches = content.match(pattern);
    if (matches) {
      content = content.replace(pattern, replacement);
      changes.push(`${desc}: ${matches.length} occurrence(s)`);
    }
  }
  
  if (content === original) return null;
  
  if (!DRY_RUN) {
    await writeFile(filePath, content, 'utf-8');
  }
  
  return { path: relPath, changes };
}

async function main() {
  console.log(`AeroTech Staff Rebrand ${DRY_RUN ? '(DRY RUN)' : ''}`);
  console.log('=' .repeat(50));
  
  const results = [];
  
  await walk(ROOT, async (filePath) => {
    try {
      const result = await processFile(filePath);
      if (result) results.push(result);
    } catch (err) {
      console.error(`Error processing ${filePath}: ${err.message}`);
    }
  });
  
  console.log(`\nProcessed ${results.length} file(s):\n`);
  
  for (const { path, changes } of results) {
    console.log(`  ${path}`);
    for (const change of changes) {
      console.log(`    → ${change}`);
    }
  }
  
  if (DRY_RUN) {
    console.log('\n(Dry run — no files were modified)');
    console.log('Run without --dry-run to apply changes.');
  } else {
    console.log('\nRebrand complete. Review the changes with:');
    console.log('  git diff --stat');
    console.log('\nThen verify with the checklist in REBRAND.md');
  }
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
