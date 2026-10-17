#!/usr/bin/env bash
# One-time GitHub setup for a repo made from this template. Idempotent.
# Runbook: docs/runbooks/bootstrap.md
#
#   scripts/bootstrap.sh [--check] [--repo <owner>/<repo>] [--template]
#
#   --check     make no writes: compare live settings with the desired ones, print the
#               gh command that fixes each difference, exit 1 if any differ
#   --repo      the repository (default: the one `gh repo view` resolves here)
#   --template  also mark the repository as a template repository
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# GitHub Actions' app id. Required checks are pinned to it so only Actions can satisfy them.
ACTIONS_APP_ID=15368
# Value written to REVIEWER_SESSIONS when it does not exist yet. It matches no session id.
SESSIONS_PLACEHOLDER="none"

check=0
template=0
repo=""

usage() {
  sed -n '5,10s/^# \{0,1\}//p' "${BASH_SOURCE[0]}"
}

die() {
  printf 'bootstrap: %s\n' "$*" >&2
  exit 2
}

while (($#)); do
  case $1 in
    --check) check=1 ;;
    --template) template=1 ;;
    --repo)
      [ $# -ge 2 ] || die "--repo needs <owner>/<repo>"
      repo=$2
      shift
      ;;
    --repo=*) repo=${1#--repo=} ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
  shift
done

# What bootstrap needs, checked before anything else (docs/GETTING-STARTED.md#before).
# Written for the bash 3.2 that macOS ships: no associative arrays, no ${var,,}.
command -v git >/dev/null || die "git is not installed; see docs/GETTING-STARTED.md#before"
command -v gh >/dev/null ||
  die "gh (GitHub's command-line tool) is not installed; see docs/GETTING-STARTED.md#before"
command -v python3 >/dev/null || die "python3 is not installed; bootstrap needs Python 3.11 or later"
python3 -c 'import sys; sys.exit(sys.version_info < (3, 11))' ||
  die "python3 is version $(python3 -c 'import platform; print(platform.python_version())'); bootstrap needs 3.11 or later"
gh auth status >/dev/null 2>&1 || die "gh is not logged in: run 'gh auth login'"
[ -f "$root/repo.toml" ] || die "no repo.toml at $root"

if [ -z "$repo" ]; then
  repo=$(cd "$root" && gh repo view --json nameWithOwner --jq .nameWithOwner) ||
    die "cannot resolve the repository; pass --repo <owner>/<repo>"
fi
[[ $repo == */* ]] || die "--repo must be <owner>/<repo>, got '$repo'"

# JSON and TOML helpers. Subcommands:
#   config <repo.toml>                        -> "check <name>" and "label <key> <name>" lines
#   get <key> (JSON on stdin)                 -> the value; booleans as true/false, null as ""
#   ruleset <name> <include> <methods> <dnec> <checks>  -> desired ruleset JSON
#   ruleset-diff <desired> (live JSON on stdin)          -> one line per difference
#   label <name> (label list JSON on stdin)   -> "<color>\t<description>", nothing if absent
# Read with `read`, not $(cat <<...): bash 3.2 can misparse a heredoc inside $(...).
IFS= read -r -d '' PYLIB <<'PY' || true
import json, sys

def show(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if v is None:
        return ""
    return v if isinstance(v, str) else json.dumps(v, sort_keys=True)

cmd, args = sys.argv[1], sys.argv[2:]
if cmd == "config":
    import tomllib
    with open(args[0], "rb") as f:
        cfg = tomllib.load(f)
    for name in cfg.get("checks", {}).get("required", []):
        print("check", name)
    for key, name in cfg.get("labels", {}).items():
        print("label", key, name)
elif cmd == "get":
    d = json.load(sys.stdin)
    for part in args[0].split("."):
        d = d.get(part) if isinstance(d, dict) else None
    print(show(d))
elif cmd == "ruleset":
    name, include, methods, dnec, checks = args
    print(json.dumps({
        "name": name,
        "target": "branch",
        "enforcement": "active",
        "bypass_actors": [],
        "conditions": {"ref_name": {"include": [include], "exclude": []}},
        "rules": [
            {"type": "deletion"},
            {"type": "non_fast_forward"},
            {"type": "pull_request", "parameters": {
                "required_approving_review_count": 0,
                "dismiss_stale_reviews_on_push": False,
                "require_code_owner_review": False,
                "require_last_push_approval": False,
                "required_review_thread_resolution": False,
                "allowed_merge_methods": methods.split(","),
            }},
            {"type": "required_status_checks", "parameters": {
                "strict_required_status_checks_policy": False,
                "do_not_enforce_on_create": dnec == "1",
                "required_status_checks": [
                    {"context": c, "integration_id": int(ACTIONS_APP_ID)}
                    for c in checks.split(",") if c
                ],
            }},
        ],
    }, separators=(",", ":")))
elif cmd == "ruleset-diff":
    want = json.loads(args[0])
    have = json.load(sys.stdin)
    out = []
    for key in ("target", "enforcement"):
        if have.get(key) != want[key]:
            out.append(f"{key} is {show(have.get(key))!r}, want {want[key]!r}")
    ref = (have.get("conditions") or {}).get("ref_name") or {}
    for key in ("include", "exclude"):
        if sorted(ref.get(key) or []) != sorted(want["conditions"]["ref_name"][key]):
            out.append(f"ref_name.{key} is {ref.get(key) or []}, want {want['conditions']['ref_name'][key]}")
    if have.get("bypass_actors"):
        out.append(f"bypass_actors is {show(have['bypass_actors'])}, want []")
    live = {r["type"]: r.get("parameters") or {} for r in have.get("rules") or []}
    wanted = {r["type"]: r.get("parameters") or {} for r in want["rules"]}
    for t in sorted(set(live) - set(wanted)):
        out.append(f"extra rule {t}")
    for t, params in wanted.items():
        if t not in live:
            out.append(f"missing rule {t}")
            continue
        for k, v in params.items():
            got = live[t].get(k)
            if k == "required_status_checks":
                g = sorted((c.get("context"), c.get("integration_id")) for c in got or [])
                w = sorted((c["context"], c["integration_id"]) for c in v)
                missing = [f"{c}@{i}" for c, i in w if (c, i) not in g]
                extra = [f"{c}@{i}" for c, i in g if (c, i) not in w]
                if missing:
                    out.append(f"required checks missing {', '.join(missing)}")
                if extra:
                    out.append(f"required checks extra {', '.join(extra)}")
            elif k == "allowed_merge_methods":
                if sorted(got or []) != sorted(v):
                    out.append(f"{t}.{k} is {got}, want {v}")
            elif got != v:
                out.append(f"{t}.{k} is {show(got)}, want {show(v)}")
    print("\n".join(out))
elif cmd == "label":
    for lab in json.load(sys.stdin):
        if lab.get("name") == args[0]:
            print(f"{(lab.get('color') or '').lower()}\t{lab.get('description') or ''}")
            break
else:
    sys.exit(f"unknown helper command {cmd}")
PY

py() {
  ACTIONS_APP_ID=$ACTIONS_APP_ID python3 -c "import os; ACTIONS_APP_ID = os.environ['ACTIONS_APP_ID']
$PYLIB" "$@"
}

diffs=0

ok() {
  printf 'ok: %s\n' "$1"
}

# fix <what> <stdin body or ""> <command...>
# --check: print the difference and the exact command. Otherwise: run the command.
fix() {
  local what=$1 body=$2
  shift 2
  if ((check)); then
    diffs=$((diffs + 1))
    local cmd
    cmd=$(printf '%q ' "$@")
    printf 'differs: %s\n  fix: %s' "$what" "${cmd% }"
    if [ -n "$body" ]; then
      printf " <<'JSON'\n%s\nJSON\n" "$body"
    else
      printf '\n'
    fi
  else
    printf 'set: %s\n' "$what"
    if [ -n "$body" ]; then
      "$@" <<<"$body" >/dev/null
    else
      "$@" >/dev/null
    fi
  fi
}

checks=()
label_governance="" label_override="" label_human="" label_hardware=""
while read -r kind a b; do
  case $kind:$a in
    check:*) checks+=("$a") ;;
    label:governance) label_governance=$b ;;
    label:override) label_override=$b ;;
    label:human) label_human=$b ;;
    label:hardware) label_hardware=$b ;;
  esac
done < <(py config "$root/repo.toml")
((${#checks[@]})) || die "repo.toml has no [checks].required"
checks_csv=$(
  IFS=,
  printf '%s' "${checks[*]}"
)

printf '%s %s\n\n' "$( ((check)) && echo "Checking" || echo "Setting up")" "$repo"

me=$(gh api user --jq .login)

# 0. Access and plan. A private repo needs a paid plan for rulesets, and GitHub Code Security
# for the required dependency-review check. Without them nothing can merge, so setup stops
# here instead of failing partway. --check reports what the plan lacks as `skip:`.
echo "== access and plan"
live=$(gh api "repos/$repo")
private=$(py get private <<<"$live")
if [ "$(py get permissions.admin <<<"$live")" = true ]; then
  ok "$me has admin rights on $repo"
elif ((check)); then
  echo "skip: $me has no admin rights on $repo, so some settings are not visible"
else
  die "$me needs admin rights on $repo to change its settings"
fi
rulesets_offered=1
missing=()
if [ "$private" = true ]; then
  owner=${repo%%/*}
  plan=""
  if [ "$(py get owner.type <<<"$live")" = Organization ]; then
    plan=$(gh api "orgs/$owner" --jq '.plan.name // ""' 2>/dev/null || true)
  elif [ "$owner" = "$me" ]; then
    plan=$(gh api user --jq '.plan.name // ""' 2>/dev/null || true)
  fi
  case $plan in
    "") echo "skip: cannot see the plan of $owner; a private repo needs a paid plan for rulesets" ;;
    free)
      rulesets_offered=0
      missing+=("rulesets, which protect main: a paid plan (GitHub Pro for a personal account, GitHub Team for an organization)")
      ;;
    *) ok "private repo on the '$plan' plan, which offers rulesets" ;;
  esac
  cs=$(py get security_and_analysis.code_security.status <<<"$live")
  [ "$cs" = enabled ] || cs=$(py get security_and_analysis.advanced_security.status <<<"$live")
  if [ "$cs" = enabled ]; then
    ok "GitHub Code Security on, for the dependency-review check"
  else
    missing+=("GitHub Code Security, which the required dependency-review check needs")
  fi
else
  ok "public repo: every feature the template uses is free"
fi
if ((${#missing[@]})); then
  for m in "${missing[@]}"; do
    echo "skip: not on this plan: $m"
  done
  if ! ((check)); then
    {
      echo
      echo "bootstrap: $repo is private, and its plan lacks what the template needs:"
      for m in "${missing[@]}"; do
        echo "  - $m"
      done
      echo "Without these the template can't work: either nothing protects main, or no PR can"
      echo "pass its required checks. Make the repo public (Settings > General > Danger Zone >"
      echo "Change visibility), or add them to the plan, then run bootstrap again."
      echo "Nothing was changed. See docs/GETTING-STARTED.md#before."
    } >&2
    exit 2
  fi
fi

# 1. Repository settings.
echo "== repository settings"
want_bool=(allow_squash_merge=true allow_merge_commit=true allow_rebase_merge=false
  allow_auto_merge=true delete_branch_on_merge=true)
((template)) && want_bool+=(is_template=true)
for pair in "${want_bool[@]}"; do
  key=${pair%%=*} val=${pair#*=}
  got=$(py get "$key" <<<"$live")
  if [ "$got" = "$val" ]; then
    ok "$key=$val"
  else
    fix "$key is '$got', want $val" "" gh api -X PATCH "repos/$repo" -F "$key=$val"
  fi
done
title=$(py get squash_merge_commit_title <<<"$live")
message=$(py get squash_merge_commit_message <<<"$live")
if [ "$title" = PR_TITLE ] && [ "$message" = PR_BODY ]; then
  ok "squash commit = PR title + PR body"
else
  # GitHub validates the title and message as a pair, so both are always sent together.
  fix "squash commit is $title + $message, want PR_TITLE + PR_BODY" "" \
    gh api -X PATCH "repos/$repo" -f squash_merge_commit_title=PR_TITLE \
    -f squash_merge_commit_message=PR_BODY
fi

# 2. Rulesets, matched by name among the repository's own rulesets.
echo "== rulesets"
ruleset() {
  local name=$1 include=$2 methods=$3 dnec=$4 want id have problems
  want=$(py ruleset "$name" "$include" "$methods" "$dnec" "$checks_csv")
  id=$(gh api "repos/$repo/rulesets?includes_parents=false" --paginate \
    --jq ".[] | select(.name == \"$name\" and .source_type == \"Repository\") | .id" | head -n1)
  if [ -z "$id" ]; then
    fix "ruleset '$name' does not exist" "$want" \
      gh api -X POST "repos/$repo/rulesets" --input -
    return
  fi
  have=$(gh api "repos/$repo/rulesets/$id")
  problems=$(py ruleset-diff "$want" <<<"$have")
  if [ -z "$problems" ]; then
    ok "ruleset '$name'"
  else
    fix "ruleset '$name': ${problems//$'\n'/; }" "$want" \
      gh api -X PUT "repos/$repo/rulesets/$id" --input -
  fi
}
if ((rulesets_offered)); then
  ruleset main refs/heads/main squash 0
  ruleset 'wave/*' 'refs/heads/wave/*' squash,merge 1
else
  echo "skip: rulesets 'main' and 'wave/*' (not offered for a private repo on this plan)"
fi

# 3. Actions permissions.
echo "== actions"
live=$(gh api "repos/$repo/actions/permissions")
got=$(py get sha_pinning_required <<<"$live")
if [ "$got" = true ]; then
  ok "sha_pinning_required=true"
else
  # PUT requires `enabled`; allowed_actions is left as it is.
  fix "sha_pinning_required is '$got', want true" "" \
    gh api -X PUT "repos/$repo/actions/permissions" -F enabled=true -F sha_pinning_required=true
fi
live=$(gh api "repos/$repo/actions/permissions/workflow")
perm=$(py get default_workflow_permissions <<<"$live")
approve=$(py get can_approve_pull_request_reviews <<<"$live")
if [ "$perm" = read ] && [ "$approve" = false ]; then
  ok "workflow token read-only, cannot approve PRs"
else
  fix "workflow token is '$perm' (approve PRs: $approve), want read (false)" "" \
    gh api -X PUT "repos/$repo/actions/permissions/workflow" \
    -f default_workflow_permissions=read -F can_approve_pull_request_reviews=false
fi
# The fork-PR approval policy applies to public repositories; GitHub may refuse it on others.
if live=$(gh api "repos/$repo/actions/permissions/fork-pr-contributor-approval" 2>/dev/null); then
  got=$(py get approval_policy <<<"$live")
  if [ "$got" = all_external_contributors ]; then
    ok "fork PRs need approval for all outside collaborators"
  else
    fix "fork PR approval is '$got', want all_external_contributors" "" \
      gh api -X PUT "repos/$repo/actions/permissions/fork-pr-contributor-approval" \
      -f approval_policy=all_external_contributors
  fi
else
  echo "skip: fork PR approval policy is not available for $repo"
fi

# 4. Labels, named in repo.toml [labels].
echo "== labels"
label_color() {
  case $1 in
    governance) echo 5319e7 ;;
    override) echo b60205 ;;
    human) echo fbca04 ;;
    hardware) echo 1d76db ;;
    *) echo ededed ;;
  esac
}
label_description() {
  case $1 in
    governance) echo "Governance change examined; justified in the PR description" ;;
    override) echo "Review gate overridden; the PR description links the maintainer's message" ;;
    human) echo "The maintainer decides this point" ;;
    hardware) echo "Needs a run on real hardware" ;;
    *) echo "" ;;
  esac
}
live=$(gh label list -R "$repo" --limit 500 --json name,color,description)
for key in governance override human hardware; do
  case $key in
    governance) name=$label_governance ;;
    override) name=$label_override ;;
    human) name=$label_human ;;
    hardware) name=$label_hardware ;;
  esac
  [ -n "$name" ] || die "repo.toml [labels] has no '$key'"
  color=$(label_color "$key") desc=$(label_description "$key")
  got=$(py label "$name" <<<"$live")
  if [ "$got" = "$color"$'\t'"$desc" ]; then
    ok "label $name"
  else
    fix "label $name is '${got:-missing}'" "" \
      gh label create "$name" -R "$repo" --color "$color" --description "$desc" --force
  fi
done

# 5. Repository variables. REVIEWER_SESSIONS and REVIEWER_LOGIN are created only when missing,
# never overwritten. MAINTAINER_LOGIN always names the login that runs bootstrap.
echo "== variables"
variable() {
  local name=$1 default=$2
  if gh api "repos/$repo/actions/variables/$name" --jq .value >/dev/null 2>&1; then
    ok "variable $name exists"
  else
    fix "variable $name is not set" "" gh variable set "$name" -R "$repo" --body "$default"
  fi
}
# Placeholder until the maintainer lists the reviewer sessions.
variable REVIEWER_SESSIONS "$SESSIONS_PLACEHOLDER"
variable REVIEWER_LOGIN "$me"
# The person the label checks trust. Without it the workflows fall back to the repo's owner,
# which for an organization's repo is the organization: a login no person has.
got=$(gh api "repos/$repo/actions/variables/MAINTAINER_LOGIN" --jq .value 2>/dev/null || true)
if [ "$got" = "$me" ]; then
  ok "variable MAINTAINER_LOGIN=$me"
else
  fix "variable MAINTAINER_LOGIN is '${got:-not set}', want $me" "" \
    gh variable set MAINTAINER_LOGIN -R "$repo" --body "$me"
fi

# 6. Dependabot security alerts and fixes, private vulnerability reporting, secret scanning and
# push protection.
echo "== security"
# GET answers 204 when alerts are on and 404 when they are off.
if gh api "repos/$repo/vulnerability-alerts" --silent 2>/dev/null; then
  ok "vulnerability alerts on"
else
  fix "vulnerability alerts are off" "" gh api -X PUT "repos/$repo/vulnerability-alerts"
fi
got=$(gh api "repos/$repo/automated-security-fixes" --jq .enabled 2>/dev/null || echo false)
if [ "$got" = true ]; then
  ok "automated security fixes on"
else
  fix "automated security fixes are off" "" gh api -X PUT "repos/$repo/automated-security-fixes"
fi
# Private vulnerability reporting: the Security tab's "Report a vulnerability" (SECURITY.md).
# GitHub offers it on public repositories only.
if [ "$private" = true ]; then
  echo "skip: private vulnerability reporting (offered on public repos only)"
else
  got=$(gh api "repos/$repo/private-vulnerability-reporting" --jq .enabled 2>/dev/null || true)
  if [ "$got" = true ]; then
    ok "private vulnerability reporting on"
  else
    fix "private vulnerability reporting is off" "" \
      gh api -X PUT "repos/$repo/private-vulnerability-reporting" ||
      echo "warn: GitHub refused to enable private vulnerability reporting for $repo"
  fi
fi
# Secret scanning and push protection. GitHub shows security_and_analysis only to admins, and
# offers it on private repositories only with GitHub Secret Protection.
live=$(gh api "repos/$repo")
for feature in secret_scanning secret_scanning_push_protection; do
  got=$(py get "security_and_analysis.$feature.status" <<<"$live")
  if [ "$got" = enabled ]; then
    ok "$feature enabled"
  elif [ -z "$got" ]; then
    echo "skip: $feature is not visible for $repo (needs admin, or not offered on this plan)"
  else
    fix "$feature is '$got', want enabled" \
      "{\"security_and_analysis\":{\"$feature\":{\"status\":\"enabled\"}}}" \
      gh api -X PATCH "repos/$repo" --input - ||
      echo "warn: GitHub refused to enable $feature for $repo"
  fi
done

# The same list as docs/runbooks/bootstrap.md#by-hand.
cat <<'EOF'

By hand (bootstrap cannot do these; docs/runbooks/bootstrap.md#by-hand):
  1. Install the Claude GitHub App on this repository, before adding the repository to the
     Claude project: https://github.com/apps/claude
  2. Paste the project instructions (the block in docs/agents/project-instructions.md, with
     PROJECT_NAME and OWNER/REPO replaced) into the Claude project, if you haven't yet.
  3. When the coordinator posts the reviewer session ids, set REVIEWER_SESSIONS to them,
     then tell the coordinator. Do it again each time it posts new ids.
  4. Private repo only: turn on the dependency graph in Settings > Code security.
  5. After intake's design PR merges, paste the project instructions again and run
     bootstrap again: intake may have changed both.
EOF

if ((check)); then
  echo
  if ((diffs)); then
    echo "$diffs setting(s) differ."
    exit 1
  fi
  echo "All settings match."
fi
