#!/bin/bash
# S10 → herdr 切换助手脚本
# 用法: s10-herdr.sh <next-workspace|prev-workspace|next-agent|prev-agent|esc-esc>

set -e

# herdr socket lives in user's XDG_RUNTIME_DIR; sudo drops this.
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/1000}"

CMD="${1:-}"

next_workspace() {
    local total current next id
    total=$(herdr workspace list 2>/dev/null | jq '.result.workspaces | length')
    if [[ -z "$total" || "$total" == "0" ]]; then
        echo "herdr: no workspaces found (is server running?)" >&2
        exit 1
    fi
    current=$(herdr workspace list 2>/dev/null | jq -r '.result.workspaces[] | select(.focused==true) | .number')
    if [[ -z "$current" ]]; then
        echo "herdr: no focused workspace found" >&2
        exit 1
    fi
    next=$((current % total + 1))
    id=$(herdr workspace list 2>/dev/null | jq -r ".result.workspaces[] | select(.number==$next) | .workspace_id")
    herdr workspace focus "$id" 2>/dev/null
}

prev_workspace() {
    local total current prev id
    total=$(herdr workspace list 2>/dev/null | jq '.result.workspaces | length')
    if [[ -z "$total" || "$total" == "0" ]]; then
        echo "herdr: no workspaces found (is server running?)" >&2
        exit 1
    fi
    current=$(herdr workspace list 2>/dev/null | jq -r '.result.workspaces[] | select(.focused==true) | .number')
    if [[ -z "$current" ]]; then
        echo "herdr: no focused workspace found" >&2
        exit 1
    fi
    prev=$((current - 1))
    [[ $prev -lt 1 ]] && prev=$total
    id=$(herdr workspace list 2>/dev/null | jq -r ".result.workspaces[] | select(.number==$prev) | .workspace_id")
    herdr workspace focus "$id" 2>/dev/null
}

next_agent() {
    local agents current next
    agents=$(herdr agent list 2>/dev/null | jq -r '.result.agents[].agent')
    if [[ -z "$agents" ]]; then
        echo "herdr: no agents found (is server running?)" >&2
        exit 1
    fi
    current=$(herdr agent list 2>/dev/null | jq -r '.result.agents[] | select(.focused==true) | .agent')
    if [[ -z "$current" ]]; then
        # fallback: focus first agent
        next=$(echo "$agents" | head -1)
    else
        next=$(echo "$agents" | awk -v cur="$current" '
            NR==1 { first=$0 }
            prev==cur { print $0; found=1; exit }
            { prev=$0 }
            END { if (!found) print first }
        ')
    fi
    herdr agent focus "$next" 2>/dev/null
}

prev_agent() {
    local agents current prev
    agents=$(herdr agent list 2>/dev/null | jq -r '.result.agents[].agent')
    if [[ -z "$agents" ]]; then
        echo "herdr: no agents found (is server running?)" >&2
        exit 1
    fi
    current=$(herdr agent list 2>/dev/null | jq -r '.result.agents[] | select(.focused==true) | .agent')
    if [[ -z "$current" ]]; then
        # fallback: focus last agent
        prev=$(echo "$agents" | tail -1)
    else
        prev=$(echo "$agents" | awk -v cur="$current" '
            { a[NR]=$0; if ($0==cur) idx=NR }
            END {
                if (idx>1) print a[idx-1]
                else print a[length(a)]
            }
        ')
    fi
    herdr agent focus "$prev" 2>/dev/null
}

double_esc() {
    local agent esc
    agent=$(herdr agent list 2>/dev/null | jq -r '.result.agents[] | select(.focused==true) | .agent')
    if [[ -z "$agent" ]]; then
        echo "herdr: no focused agent found" >&2
        exit 1
    fi
    # Generate two ESC characters (0x1b) and send as text
    esc=$(printf '\x1b\x1b')
    herdr agent send "$agent" "$esc" 2>/dev/null
}

case "$CMD" in
    next-workspace) next_workspace ;;
    prev-workspace) prev_workspace ;;
    next-agent)     next_agent ;;
    prev-agent)     prev_agent ;;
    esc-esc)        double_esc ;;
    *) echo "Usage: $0 {next-workspace|prev-workspace|next-agent|prev-agent|esc-esc}"; exit 1 ;;
esac
