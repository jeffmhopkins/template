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

command -v gh >/dev/null || die "gh is not installed (https://cli.github.com)"
command -v python3 >/dev/null || die "python3 is not installed"
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
PYLIB=$(
  cat <<'PY'
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
)

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
declare -A labels=()
while read -r kind a b; do
  case $kind in
    check) checks+=("$a") ;;
    label) labels[$a]=$b ;;
  esac
done < <(py config "$root/repo.toml")
((${#checks[@]})) || die "repo.toml has no [checks].required"
checks_csv=$(
  IFS=,
  printf '%s' "${checks[*]}"
)

printf '%s %s\n\n' "$( ((check)) && echo "Checking" || echo "Setting up")" "$repo"

# 1. Repository settings.
echo "== repository settings"
live=$(gh api "repos/$repo")
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
ruleset main refs/heads/main squash 0
ruleset 'wave/*' 'refs/heads/wave/*' squash,merge 1

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
  name=${labels[$key]:-}
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

# 5. Repository variables. Existing values are never overwritten.
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
variable REVIEWER_LOGIN "$(gh api user --jq .login)"

# 6. Dependabot security alerts and fixes.
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

cat <<'EOF'

By hand (bootstrap cannot do these):
  1. Install the Claude GitHub App on this repository: https://github.com/apps/claude
  2. Paste docs/agents/project-instructions.md, with the project name filled in, into the
     Claude project's instructions.
EOF

if ((check)); then
  echo
  if ((diffs)); then
    echo "$diffs setting(s) differ."
    exit 1
  fi
  echo "All settings match."
fi
