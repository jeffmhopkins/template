// Tests decide() from .github/workflows/review-gate.yml, cut out of the deployed file verbatim.
// Run: node --test '.github/tests/*.test.mjs'
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';

const yml = fs.readFileSync(new URL('../workflows/review-gate.yml', import.meta.url), 'utf8');
const begin = yml.indexOf('// BEGIN decide');
const end = yml.indexOf('// END decide');
assert.ok(begin >= 0 && end > begin, 'review-gate.yml has the BEGIN/END decide block');
const decide = new Function(`${yml.slice(begin, end)}\nreturn decide;`)();

const H = 'a'.repeat(40);
const OLD = 'b'.repeat(40);
const LOGIN = 'reviewer-bot';
const REVIEWER = 'session_REVIEWER1';
const OTHER_REVIEWER = 'session_REVIEWER2';
const BUILDER = 'session_BUILDER1';
const T0 = '2026-01-01T00:00:00Z';

const marker = (verdict, round, o = {}) => ({
  user: { login: o.login ?? LOGIN },
  created_at: T0,
  updated_at: o.edited ? '2026-01-01T00:05:00Z' : T0,
  body: `Round ${round}\n\nB1: …\n\n<!-- review v1 verdict=${verdict} sha=${o.sha ?? H} round=${round} reviewer-session=${o.session ?? REVIEWER} -->\n`,
});
const commit = (o = {}) => ({
  sha: o.sha ?? 'c'.repeat(40),
  parents: o.merge ? [{}, {}] : [{}],
  author: { login: o.author ?? 'builder' },
  commit: { message: o.message ?? `feat: x\n\nClaude-Session: https://claude.ai/code/${BUILDER}\n` },
});
const run = (o = {}) => decide({
  head: H,
  comments: o.comments ?? [],
  commits: o.commits ?? [commit()],
  labels: o.labels ?? [],
  body: o.body ?? '',
  overrideLabeledBy: o.overrideLabeledBy ?? null,
  maintainer: 'the-maintainer',
  reviewerLogin: o.reviewerLogin ?? LOGIN,
  sessions: o.sessions ?? [REVIEWER, `https://claude.ai/code/${OTHER_REVIEWER}`],
});
const state = (o) => run(o)[0];

test('no markers: pending', () => assert.equal(state({}), 'pending'));
test('pass from the configured reviewer: success', () => assert.equal(state({ comments: [marker('pass', 1)] }), 'success'));
test('fail: failure', () => assert.equal(state({ comments: [marker('fail', 1)] }), 'failure'));
test('marker from another login is ignored', () =>
  assert.equal(state({ comments: [marker('pass', 1, { login: 'someone' })] }), 'pending'));
test('marker from an unlisted session is ignored', () =>
  assert.equal(state({ comments: [marker('pass', 1, { session: 'session_ROGUE' })] }), 'pending'));
test('edited marker comment is ignored', () =>
  assert.equal(state({ comments: [marker('pass', 1, { edited: true })] }), 'pending'));
test('edited pass does not mask an unedited fail', () =>
  assert.equal(state({ comments: [marker('fail', 1), marker('pass', 2, { edited: true })] }), 'failure'));
test('marker for an older SHA only: pending', () =>
  assert.equal(state({ comments: [marker('pass', 1, { sha: OLD })] }), 'pending'));
test('session given as a URL matches the listed id', () =>
  assert.equal(state({ comments: [marker('pass', 1, { session: OTHER_REVIEWER })] }), 'success'));
test('fail beats pass within a round', () =>
  assert.equal(state({ comments: [marker('pass', 2), marker('fail', 2, { session: OTHER_REVIEWER }), marker('pass', 2)] }), 'failure'));
test('highest round decides', () =>
  assert.equal(state({ comments: [marker('fail', 1), marker('pass', 2)] }), 'success'));
test('later fail beats earlier pass', () =>
  assert.equal(state({ comments: [marker('pass', 1), marker('fail', 2)] }), 'failure'));
test('rounds compare numerically (10 > 9)', () =>
  assert.equal(state({ comments: [marker('fail', 9), marker('pass', 10)] }), 'success'));
test('recusal: a reviewer that authored a commit is ignored', () => {
  const commits = [commit({ message: `fix: y\n\nClaude-Session: ${REVIEWER}\n` })];
  assert.equal(state({ commits, comments: [marker('pass', 1)] }), 'pending');
  assert.equal(state({ commits, comments: [marker('pass', 1), marker('pass', 1, { session: OTHER_REVIEWER })] }), 'success');
});
test('recused fail does not block an independent pass', () => {
  const commits = [commit(), commit({ message: `fix: y\n\nClaude-Session: ${REVIEWER}\n` })];
  assert.equal(state({ commits, comments: [marker('fail', 3), marker('pass', 3, { session: OTHER_REVIEWER })] }), 'success');
});
test('non-merge commit without a trailer: failure', () =>
  assert.equal(state({ commits: [commit({ message: 'feat: z\n' })], comments: [marker('pass', 1)] }), 'failure'));
test('merge commit without a trailer is fine', () =>
  assert.equal(state({ commits: [commit(), commit({ merge: true, message: "Merge branch 'main'\n" })], comments: [marker('pass', 1)] }), 'success'));
test('dependabot commit without a trailer is fine', () =>
  assert.equal(state({ commits: [commit({ author: 'dependabot[bot]', message: 'build(deps): bump x\n' })], comments: [marker('pass', 1)] }), 'success'));
test('unconfigured gate fails', () => {
  assert.equal(state({ reviewerLogin: '', comments: [marker('pass', 1)] }), 'failure');
  assert.equal(state({ sessions: [], comments: [marker('pass', 1)] }), 'failure');
});
test('REVIEWER_SESSIONS as a comma-separated string, trimmed', () =>
  assert.equal(state({ sessions: ` ${OTHER_REVIEWER} , ${REVIEWER} `, comments: [marker('pass', 1)] }), 'success'));
test('REVIEWER_SESSIONS = none allows no session', () => {
  assert.equal(state({ sessions: 'none', comments: [marker('pass', 1)] }), 'failure');
  assert.equal(state({ sessions: ' none ', comments: [marker('pass', 1, { session: 'none' })] }), 'failure');
  assert.equal(state({ sessions: '', comments: [marker('pass', 1)] }), 'failure');
});
test('unknown verdicts do not count', () =>
  assert.equal(state({ comments: [{ ...marker('pass', 1), body: marker('pass', 1).body.replace('verdict=pass', 'verdict=maybe') }] }), 'pending'));

const justified = '## Summary\nx\n\n## Review override\nThe maintainer decided B1 is acceptable: https://claude.ai/project/p/message/cmsg_1\n\n## Docs\n';
test('override: label + linked justification + applied by the maintainer', () =>
  assert.equal(state({ labels: ['review-override'], body: justified, overrideLabeledBy: 'the-maintainer', comments: [marker('fail', 1)] }), 'success'));
test('override applied by someone else fails', () =>
  assert.equal(state({ labels: ['review-override'], body: justified, overrideLabeledBy: 'an-agent' }), 'failure'));
test('override without the section fails', () =>
  assert.equal(state({ labels: ['review-override'], body: '## Summary\nx\n', overrideLabeledBy: 'the-maintainer' }), 'failure'));
test('override section without a link fails', () =>
  assert.equal(state({ labels: ['review-override'], body: '## Review override\nBecause I said so, at length.\n', overrideLabeledBy: 'the-maintainer' }), 'failure'));
test('override section holding only the template comment fails', () =>
  assert.equal(state({ labels: ['review-override'], body: '## Review override\n<!-- link https://x -->\n## Docs\n', overrideLabeledBy: 'the-maintainer' }), 'failure'));

test('escalate in the highest round: failure', () =>
  assert.equal(state({ comments: [marker('pass', 1), marker('escalate', 2)] }), 'failure'));
test('escalate beats a pass in the same round', () =>
  assert.equal(state({ comments: [marker('pass', 3), marker('escalate', 3, { session: OTHER_REVIEWER })] }), 'failure'));
test('recuse markers do not count', () =>
  assert.equal(state({ comments: [marker('recuse', 1)] }), 'pending'));
test('a recuse does not hide another reviewer\'s pass', () =>
  assert.equal(state({ comments: [marker('recuse', 2), marker('pass', 1, { session: OTHER_REVIEWER })] }), 'success'));
