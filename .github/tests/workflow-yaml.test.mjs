// Checks every file under .github/ that GitHub parses as YAML for the mistakes GitHub rejects
// a whole workflow over: a key repeated in one mapping, and tab indentation. CI has no YAML
// library, so this is a small line scanner for the block style these files use. It also
// checks that each pull_request_target workflow fails closed on an untrusted base.
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

// pull_request_target takes the workflow file from the PR's base branch, and a stacked PR's
// base is another PR's unreviewed branch. Every such workflow fails closed on a base other
// than the default branch or wave/*.
const workflow = (name) => fs.readFileSync(path.join(root, 'workflows', name), 'utf8');
// The text of one top-level job, from its `  <id>:` line to the next job.
const job = (text, id) => {
  const m = new RegExp(`^  ${id}:\\n([\\s\\S]*?)(?=^  [a-z][a-z0-9-]*:\\n|(?![\\s\\S]))`, 'm').exec(text);
  assert.ok(m, `job ${id} exists`);
  return m[1];
};

test('every pull_request_target workflow has a trusted-base guard', () => {
  const targets = fs.readdirSync(path.join(root, 'workflows'))
    .filter((f) => /\.ya?ml$/.test(f) && /^  pull_request_target:/m.test(workflow(f)))
    .sort();
  // A new pull_request_target workflow gets its own guard and its own test here.
  assert.deepEqual(targets, ['governance.yml', 'review-gate.yml']);
});

test('governance: the tool job checks the base before it checks out or builds anything', () => {
  const text = workflow('governance.yml');
  assert.match(text, /^  BASE_REF: \$\{\{ github\.event\.pull_request\.base\.ref \}\}$/m);
  assert.match(text, /^  DEFAULT_BRANCH: \$\{\{ github\.event\.repository\.default_branch \}\}$/m);
  const tool = job(text, 'tool');
  const guard = tool.indexOf('case "$BASE_REF" in');
  assert.ok(guard >= 0, 'the tool job has the base guard');
  assert.match(tool, /^ {12}"\$DEFAULT_BRANCH"\|wave\/\?\*\) ;;$/m);
  assert.match(tool, /^ {14}exit 1 ;;$/m);
  assert.ok(guard < tool.indexOf('uses:'), 'the guard is the first step, before any action runs');
  // Every job that runs the tool needs it, and fails when it didn't succeed.
  for (const id of ['governance', 'pr-title', 'lane-guard']) {
    const j = job(text, id);
    assert.match(j, /^ {4}needs: tool$/m, `${id} needs tool`);
    assert.match(j, /\[ "\$TOOL" = success \] \|\| \{ echo "::error::the tool job did not succeed/, `${id} fails when tool did not succeed`);
  }
});

test('review-gate: decide() is given the base and the default branch', () => {
  const text = workflow('review-gate.yml');
  assert.match(text, /^ +base: pr\.base\.ref,$/m);
  assert.match(text, /^ +defaultBranch: pr\.base\.repo\.default_branch,$/m);
  const decide = text.slice(text.indexOf('// BEGIN decide'), text.indexOf('// END decide'));
  assert.ok(decide.indexOf('input.base') < decide.indexOf('if (!maintainer)'), 'the base is checked first');
});
