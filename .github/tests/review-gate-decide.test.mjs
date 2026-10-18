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
const ROLE = 'reviewer-0';
const OTHER_ROLE = 'reviewer-1';
// The role a test session posts as, unless a test names another.
const roleOf = (session) => (session === OTHER_REVIEWER ? OTHER_ROLE : ROLE);
const BUILDER = 'session_BUILDER1';
const T0 = '2026-01-01T00:00:00Z';

let nextId = 1;
const markerLine = (verdict, round, o = {}) =>
  `<!-- review v1 verdict=${verdict} sha=${o.sha ?? H} round=${round} reviewer=${o.role ?? roleOf(o.session ?? REVIEWER)} reviewer-session=${o.session ?? REVIEWER} -->`;
const marker = (verdict, round, o = {}) => ({
  id: o.id ?? nextId++,
  user: { login: o.login ?? LOGIN },
  created_at: T0,
  updated_at: o.edited ? '2026-01-01T00:05:00Z' : T0,
  body: `Round ${round}\n\nB1: …\n\n${o.extra ?? ''}${markerLine(verdict, round, o)}\n`,
});
const commit = (o = {}) => ({
  sha: o.sha ?? 'c'.repeat(40),
  parents: o.merge ? [{}, {}] : [{}],
  author: { login: o.author ?? 'builder' },
  commit: {
    message: o.message ?? `feat: x\n\nClaude-Session: https://claude.ai/code/${BUILDER}\n`,
    verification: { verified: o.verified ?? false },
  },
});
const run = (o = {}) => decide({
  base: 'base' in o ? o.base : 'main',
  defaultBranch: 'defaultBranch' in o ? o.defaultBranch : 'main',
  head: H,
  comments: o.comments ?? [],
  commits: o.commits ?? [commit()],
  onMain: o.onMain ?? [],
  pushed: o.pushed ?? false,
  labels: o.labels ?? [],
  body: o.body ?? '',
  overrideLabeledBy: o.overrideLabeledBy ?? null,
  maintainer: o.maintainer ?? 'the-maintainer',
  reviewerLogin: o.reviewerLogin ?? LOGIN,
  roles: o.roles ?? [ROLE, OTHER_ROLE, 'reviewer-2'],
});
const state = (o) => run(o)[0];

test('no markers: pending', () => assert.equal(state({}), 'pending'));
test('pass from the configured reviewer: success', () => assert.equal(state({ comments: [marker('pass', 1)] }), 'success'));
test('fail: failure', () => assert.equal(state({ comments: [marker('fail', 1)] }), 'failure'));
test('marker from another login is ignored', () =>
  assert.equal(state({ comments: [marker('pass', 1, { login: 'someone' })] }), 'pending'));
test('marker from an unlisted role is ignored', () =>
  assert.equal(state({ comments: [marker('pass', 1, { role: 'reviewer-9' })] }), 'pending'));
test('any session of a listed role counts: a fresh session per fire', () =>
  assert.equal(state({ comments: [marker('pass', 1, { session: 'session_FRESHFIRE7' })] }), 'success'));
test('a marker without the reviewer field does not count', () =>
  assert.equal(state({ comments: [{ ...marker('pass', 1), body: marker('pass', 1).body.replace(/ reviewer=\S+/, '') }] }), 'pending'));
test('an edited marker comment counts as a fail', () => {
  const [st, why] = run({ comments: [marker('pass', 1, { edited: true, id: 77 })] });
  assert.equal(st, 'failure');
  assert.match(why, /comment 77 \(round 1\) was edited; the reviewer posts a new round/);
});
test('edited pass does not mask an unedited fail', () =>
  assert.equal(state({ comments: [marker('fail', 1), marker('pass', 2, { edited: true })] }), 'failure'));
test('an edited higher-round fail does not bring back a lower-round pass on the same head', () => {
  assert.equal(state({ comments: [marker('pass', 1), marker('fail', 2, { edited: true })] }), 'failure');
  // Edited to name another SHA or an unlisted role: still a fail in round order.
  assert.equal(state({ comments: [marker('pass', 1), marker('fail', 2, { edited: true, sha: OLD })] }), 'failure');
  assert.equal(state({ comments: [marker('pass', 1), marker('fail', 2, { edited: true, role: 'reviewer-9' })] }), 'failure');
});
test('a later round outranks an edited comment', () =>
  assert.equal(state({ comments: [marker('fail', 2, { edited: true }), marker('pass', 3)] }), 'success'));
test('an edited comment by another login is ignored', () =>
  assert.equal(state({ comments: [marker('pass', 1), marker('fail', 2, { edited: true, login: 'someone' })] }), 'success'));
test('a deleted higher round is invisible to the gate: the reviewer\'s sweep catches it', () =>
  // The comment list simply lacks the deleted round 2 fail.
  assert.equal(state({ comments: [marker('pass', 1)] }), 'success'));
test('a pass that quotes an earlier fail marker counts as a fail, not as the quoted one', () => {
  const quoting = marker('pass', 2, { extra: `Round 1 said:\n\n\`\`\`\n${markerLine('fail', 1)}\n\`\`\`\n\n` });
  const [st, why] = run({ comments: [marker('fail', 1), quoting] });
  assert.equal(st, 'failure');
  assert.match(why, /\(round 2\) has more than one marker line/);
});
test('a pass that quotes a marker for an older SHA does not leave the gate pending', () => {
  const quoting = marker('pass', 2, { extra: `${markerLine('fail', 1, { sha: OLD })}\n` });
  assert.equal(state({ comments: [quoting] }), 'failure');
});
test('a marker quoted with > or indented is not a marker line', () => {
  const quoted = marker('pass', 2, { extra: `> ${markerLine('fail', 1)}\n    ${markerLine('fail', 1)}\n` });
  assert.equal(state({ comments: [marker('fail', 1), quoted] }), 'success');
});
test('marker for an older SHA only: pending', () =>
  assert.equal(state({ comments: [marker('pass', 1, { sha: OLD })] }), 'pending'));
test('a reviewer-session given as a URL is still recused by a bare-id trailer', () => {
  const commits = [commit({ message: `fix: y\n\nClaude-Session: ${OTHER_REVIEWER}\n` })];
  assert.equal(state({ commits, comments: [marker('pass', 1, { session: `https://claude.ai/code/${OTHER_REVIEWER}`, role: OTHER_ROLE })] }), 'pending');
});
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
  assert.equal(state({ commits: [commit({ author: 'dependabot[bot]', verified: true, message: 'build(deps): bump x\n' })], comments: [marker('pass', 1)] }), 'success'));
test('an unsigned commit attributed to dependabot still needs a trailer', () =>
  assert.equal(state({ commits: [commit({ author: 'dependabot[bot]', message: 'build(deps): bump x\n' })], comments: [marker('pass', 1)] }), 'failure'));

// A squash commit's message is the PR title and body, so it carries the template's trailer block.
const squash = (sha, n, session = BUILDER) => commit({
  sha,
  message: `feat: item ${n} (#${n})\n\n## Summary\nx\n\nCompletes: M1-${n}\nClaude-Session: https://claude.ai/code/${session}\n`,
});
const MAIN1 = '1'.repeat(40);
const MAIN2 = '2'.repeat(40);
const fromMain = [
  commit({ sha: MAIN1, message: 'feat: landed earlier (#7)\n\nCompletes: M1-7\n' }),
  commit({ sha: MAIN2, message: `fix: by a reviewer session (#8)\n\nClaude-Session: ${REVIEWER}\n` }),
];
test('sync PR: main\'s commits need no trailer and recuse nobody', () => {
  const commits = [...fromMain, commit({ sha: 'd'.repeat(40), merge: true, message: "Merge remote-tracking branch 'origin/main'\n" })];
  assert.equal(state({ commits, onMain: [MAIN1, MAIN2], comments: [marker('pass', 1)] }), 'success');
  assert.equal(state({ commits, comments: [marker('pass', 1)] }), 'failure');
});
test('landing PR: the wave\'s squash commits carry the trailer from the PR template', () => {
  const commits = [squash('e'.repeat(40), 1), squash('f'.repeat(40), 2), ...fromMain,
    commit({ sha: 'd'.repeat(40), merge: true, message: "Merge remote-tracking branch 'origin/main' into sync\n" })];
  assert.equal(state({ commits, onMain: [MAIN1, MAIN2], comments: [marker('pass', 1)] }), 'success');
});
test('landing PR: a squash commit whose body lost the trailer fails', () => {
  const bare = commit({ sha: 'e'.repeat(40), message: 'feat: item 1 (#1)\n\nCompletes: M1-1\n' });
  assert.equal(state({ commits: [bare, ...fromMain], onMain: [MAIN1, MAIN2], comments: [marker('pass', 1)] }), 'failure');
});
test('landing PR: a reviewer that wrote a squash commit in the wave is recused', () => {
  const commits = [squash('e'.repeat(40), 1, REVIEWER), ...fromMain];
  assert.equal(state({ commits, onMain: [MAIN1, MAIN2], comments: [marker('pass', 1)] }), 'pending');
});
test('unset MAINTAINER_LOGIN fails loudly', () => {
  const [st, why] = run({ maintainer: '', comments: [marker('pass', 1)] });
  assert.equal(st, 'failure');
  assert.match(why, /MAINTAINER_LOGIN not set/);
});
test('unconfigured gate fails', () => {
  assert.equal(state({ reviewerLogin: '', comments: [marker('pass', 1)] }), 'failure');
  assert.equal(state({ roles: [], comments: [marker('pass', 1)] }), 'failure');
});
test('REVIEWER_ROLES as a comma-separated string, trimmed', () =>
  assert.equal(state({ roles: ` ${OTHER_ROLE} , ${ROLE} `, comments: [marker('pass', 1)] }), 'success'));
test('REVIEWER_ROLES empty or none allows no role', () => {
  assert.equal(state({ roles: 'none', comments: [marker('pass', 1)] }), 'failure');
  assert.equal(state({ roles: ' none ', comments: [marker('pass', 1, { role: 'none' })] }), 'failure');
  assert.equal(state({ roles: '', comments: [marker('pass', 1)] }), 'failure');
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
test('override does not survive a push: the run for the new head ignores it', () => {
  const o = { labels: ['review-override'], body: justified, overrideLabeledBy: 'the-maintainer', pushed: true };
  assert.equal(state({ ...o, comments: [marker('fail', 1)] }), 'failure');
  assert.equal(state({ ...o, comments: [marker('fail', 1, { sha: OLD })] }), 'pending');
  assert.equal(state({ ...o, comments: [marker('pass', 2)] }), 'success');
});
test('override: the trailer block is not the section, and a session URL is not the maintainer\'s link', () => {
  const o = { labels: ['review-override'], overrideLabeledBy: 'the-maintainer', comments: [marker('fail', 1)] };
  const trailers = 'Completes: M1-1\nClaude-Session: https://claude.ai/code/session_BUILDER1\n';
  assert.equal(state({ ...o, body: `## Review override\nB1 is fine.\n\n${trailers}` }), 'failure');
  assert.equal(state({ ...o, body: '## Review override\nB1 is fine, see https://claude.ai/code/session_BUILDER1\n' }), 'failure');
  assert.equal(state({ ...o, body: `## Review override\nB1 is fine: https://claude.ai/project/p/message/cmsg_1\n\n${trailers}` }), 'success');
});
const template = fs.readFileSync(new URL('../pull_request_template.md', import.meta.url), 'utf8');
test('the real PR template, trailers filled in, gives an override no link', () => {
  const body = template
    .replace('Completes: <id>', 'Completes: M1-1')
    .replace('Part-of: <id>\n', '')
    .replace('Claude-Session: <your session URL>', 'Claude-Session: https://claude.ai/code/session_BUILDER1');
  assert.match(body, /^## Review override$/m);
  const [st, why] = run({ labels: ['review-override'], body, overrideLabeledBy: 'the-maintainer', comments: [marker('fail', 1)] });
  assert.equal(st, 'failure');
  assert.match(why, /needs a "## Review override" section/);
  const filled = body.replace(/(## Review override\n<!--[\s\S]*?-->\n)/, '$1Overrides B1: https://claude.ai/project/p/message/cmsg_1\n');
  assert.equal(run({ labels: ['review-override'], body: filled, overrideLabeledBy: 'the-maintainer' })[0], 'success');
});
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

// The base: pull_request_target takes the workflow from it, so only main and wave/* decide.
test('a PR into main or a wave is gated as usual', () => {
  assert.equal(state({ comments: [marker('pass', 1)] }), 'success');
  assert.equal(state({ base: 'wave/spring', comments: [marker('pass', 1)] }), 'success');
});
test('a stacked PR, whose base is another PR\'s branch, fails closed whatever its markers', () => {
  const [st, why] = run({ base: 'claude/feature-x', comments: [marker('pass', 1)] });
  assert.equal(st, 'failure');
  assert.match(why, /^Base claude\/feature-x is not main or wave\/\*: a stacked PR is gated once it targets one$/);
  assert.equal(state({ base: 'wave/', comments: [marker('pass', 1)] }), 'failure');
  assert.equal(state({ base: 'mainline', comments: [marker('pass', 1)] }), 'failure');
  assert.equal(state({ base: 'x/wave/y', comments: [marker('pass', 1)] }), 'failure');
});
test('a missing base or default branch fails closed', () => {
  assert.equal(state({ base: undefined, comments: [marker('pass', 1)] }), 'failure');
  assert.equal(state({ base: '', defaultBranch: '', comments: [marker('pass', 1)] }), 'failure');
  assert.equal(state({ defaultBranch: undefined, comments: [marker('pass', 1)] }), 'failure');
});
test('the default branch need not be called main', () =>
  assert.equal(state({ base: 'trunk', defaultBranch: 'trunk', comments: [marker('pass', 1)] }), 'success'));
