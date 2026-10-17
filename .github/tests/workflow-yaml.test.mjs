// Checks every file under .github/ that GitHub parses as YAML for the mistakes GitHub rejects
// a whole workflow over: a key repeated in one mapping, and tab indentation. CI has no YAML
// library, so this is a small line scanner for the block style these files use.
// Run: node --test '.github/tests/*.test.mjs'
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root = new URL('..', import.meta.url).pathname;

function yamlFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) return yamlFiles(p);
    return /\.ya?ml$/.test(e.name) ? [p] : [];
  });
}

// Returns one message per problem found in `text`.
export function problems(text) {
  const out = [];
  const stack = []; // { indent, keys: Map<key, line> }
  let block = null; // indent of the key that opened a block scalar
  text.split('\n').forEach((raw, i) => {
    const n = i + 1;
    if (/^\t| \t/.test(raw.match(/^\s*/)[0])) out.push(`line ${n}: tab in indentation`);
    const indent = raw.match(/^ */)[0].length;
    const body = raw.slice(indent);
    if (block !== null) {
      if (body === '' || indent > block) return;
      block = null;
    }
    if (body === '' || body.startsWith('#') || body === '---') return;
    let item = 0;
    let rest = body;
    while (rest.startsWith('- ')) {
      item += 2;
      rest = rest.slice(2);
    }
    const at = indent + item;
    if (item) {
      // A list item starts a new mapping at its content's column.
      while (stack.length && stack[stack.length - 1].indent >= at) stack.pop();
      stack.push({ indent: at, keys: new Map() });
    } else {
      while (stack.length && stack[stack.length - 1].indent > at) stack.pop();
      if (!stack.length || stack[stack.length - 1].indent < at) stack.push({ indent: at, keys: new Map() });
    }
    const m = rest.match(/^("[^"]*"|'[^']*'|[^\s#'"{[][^:#]*?)\s*:(\s|$)(.*)$/);
    if (!m) return;
    const key = m[1].replace(/^["']|["']$/g, '');
    const keys = stack[stack.length - 1].keys;
    if (keys.has(key)) out.push(`line ${n}: key \`${key}\` repeats line ${keys.get(key)}`);
    else keys.set(key, n);
    if (/^[|>][-+0-9]*\s*(#.*)?$/.test(m[3].trim())) block = at;
  });
  return out;
}

test('every YAML file under .github/ has no repeated keys or tab indentation', () => {
  const files = yamlFiles(root);
  assert.ok(files.length > 0);
  const found = files.flatMap((f) => problems(fs.readFileSync(f, 'utf8')).map((p) => `${path.relative(root, f)}: ${p}`));
  assert.deepEqual(found, []);
});

test('a repeated key in one mapping is found', () => {
  const yml = 'permissions:\n  contents: read\n  pull-requests: read\n  pull-requests: write\n';
  assert.deepEqual(problems(yml), ['line 4: key `pull-requests` repeats line 3']);
});

test('the same key in sibling mappings and list items is fine', () => {
  const yml = [
    'jobs:',
    '  a:',
    '    runs-on: x',
    '    steps:',
    '      - uses: y',
    '        with:',
    '          ref: z',
    '      - uses: y',
    '        with:',
    '          ref: z',
    '  b:',
    '    runs-on: x',
  ].join('\n');
  assert.deepEqual(problems(yml), []);
});

test('block scalars are skipped', () => {
  const yml = 'run: |\n  a: 1\n  a: 2\nrun2: x\n';
  assert.deepEqual(problems(yml), []);
});

test('tab indentation is found', () => {
  assert.deepEqual(problems('a:\n\tb: 1\n'), ['line 2: tab in indentation']);
});
