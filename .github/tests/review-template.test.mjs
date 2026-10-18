// Checks the review round template, and REVIEW.md's marker example, against the gate's parser:
// decide() from .github/workflows/review-gate.yml, cut out of the deployed file verbatim.
// Run: node --test '.github/tests/*.test.mjs'
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';

const read = (path) => fs.readFileSync(new URL(path, import.meta.url), 'utf8');
const yml = read('../workflows/review-gate.yml');
const begin = yml.indexOf('// BEGIN decide');
const end = yml.indexOf('// END decide');
assert.ok(begin >= 0 && end > begin, 'review-gate.yml has the BEGIN/END decide block');
const source = yml.slice(begin, end);
const decide = new Function(`${source}\nreturn decide;`)();

// The verdicts the gate's marker regex accepts.
const parserVerdicts = new Set(/verdict=\(([a-z|]+)\)/.exec(source)[1].split('|'));

const H = 'a'.repeat(40);
const LOGIN = 'reviewer-bot';
const ROLE = 'reviewer-0';
const SESSION = 'session_REVIEWER1';
const T0 = '2026-01-01T00:00:00Z';
const MARKER_START = /^<!-- review v1\b/;

const run = (body) => decide({
  base: 'main',
  defaultBranch: 'main',
  head: H,
  comments: [{ id: 1, user: { login: LOGIN }, created_at: T0, updated_at: T0, body }],
  commits: [{ sha: 'c'.repeat(40), parents: [{}], author: { login: 'builder' },
    commit: { message: 'feat: x\n\nClaude-Session: https://claude.ai/code/session_BUILDER1\n' } }],
  onMain: [],
  pushed: false,
  labels: [],
  body: '',
  overrideLabeledBy: null,
  maintainer: 'the-maintainer',
  reviewerLogin: LOGIN,
  roles: [ROLE],
});

// What the gate does with each verdict, for the current head.
const expected = { pass: 'success', fail: 'failure', escalate: 'failure', recuse: 'pending' };

// Fills every `key=<placeholder>` in a marker line, by field name.
const fill = (line, verdict) => {
  const values = { verdict, sha: H, round: '1', reviewer: ROLE, 'reviewer-session': SESSION };
  return line.replace(/([a-z-]+)=<[^>]*>/g, (all, key) => {
    assert.ok(key in values, `unknown placeholder field ${key} in ${line}`);
    return `${key}=${values[key]}`;
  });
};
// The verdicts a placeholder such as `verdict=<pass|fail>` offers.
const offered = (line) => {
  const m = /verdict=<([a-z|]+)>/.exec(line) || /verdict=([a-z]+)/.exec(line);
  assert.ok(m, `marker line names its verdict: ${line}`);
  return m[1].split('|');
};

const template = read('../../docs/agents/review/review-template.md');
const forms = [...template.matchAll(/^````markdown\n([\s\S]*?)^````$/gm)].map((m) => m[1]);

test('the template has a full form and a short form', () => assert.equal(forms.length, 2));

forms.forEach((form, i) => {
  const name = i === 0 ? 'full form' : 'short form';
  const lines = form.split('\n').filter((l) => MARKER_START.test(l));
  const line = lines[0] ?? '';

  test(`${name}: exactly one line starts a marker`, () => assert.equal(lines.length, 1));

  test(`${name}: every verdict it offers is one the gate parses`, () => {
    for (const v of offered(line)) assert.ok(parserVerdicts.has(v), `${v} is not a verdict the gate parses`);
  });

  for (const verdict of line ? offered(line) : []) {
    test(`${name}, filled in as ${verdict}: the gate reads ${expected[verdict]}`, () => {
      const filled = fill(line, verdict);
      assert.doesNotMatch(filled, /=</, `every placeholder in the marker is filled: ${filled}`);
      const body = form.replace(line, filled);
      const [state, why] = run(body);
      assert.equal(state, expected[verdict], why);
      if (verdict !== 'recuse') assert.match(why, /\(round 1\)/);
    });
  }
});

test('the full form offers pass, fail and escalate; the short form recuse, and pass or fail for a bot rebase', () => {
  const full = forms[0].split('\n').find((l) => MARKER_START.test(l));
  const short = forms[1].split('\n').find((l) => MARKER_START.test(l));
  assert.deepEqual(offered(full).sort(), ['escalate', 'fail', 'pass']);
  assert.deepEqual(offered(short).sort(), ['fail', 'pass', 'recuse']);
});

test('the parser accepts exactly the four verdicts REVIEW.md names', () =>
  assert.deepEqual([...parserVerdicts].sort(), Object.keys(expected).sort()));

test('REVIEW.md\'s marker example, filled in, passes the gate', () => {
  const review = read('../../REVIEW.md');
  const examples = review.split('\n').filter((l) => MARKER_START.test(l));
  assert.equal(examples.length, 1, 'REVIEW.md shows one marker example');
  const values = { sha: H, round: '1', reviewer: ROLE, 'reviewer-session': SESSION };
  const filled = examples[0].replace(/([a-z-]+)=<[^>]*>/g, (all, key) => {
    assert.ok(key in values, `unknown placeholder field ${key}`);
    return `${key}=${values[key]}`;
  });
  assert.doesNotMatch(filled, /=</);
  const [state, why] = run(`## Review · round 1 · ${H.slice(0, 7)} · full\n\n${filled}\n`);
  assert.equal(state, 'success', why);
});
