#!/bin/bash
# S10 → herdr 切换助手脚本
# 用法: s10-herdr.sh <next-workspace|prev-workspace|next-agent|prev-agent|esc-esc>

set -e

CMD="${1:-}"

next_workspace() {
    local total current next id
    total=$(herdr workspace list 2>/dev/null | jq '.result.workspaces | length')
    current=$(herdr workspace list 2>/dev/null | jq -r '.result.workspaces[] | select(.focused==true) | .number')
    next=$((current % total + 1))
    id=$(herdr workspace list 2>/dev/null | jq -r ".result.workspaces[] | select(.number==$next) | .workspace_id")
    herdr workspace focus "$id" 2>/dev/null
}

prev_workspace() {
    local total current prev id
    total=$(herdr workspace list 2>/dev/null | jq '.result.workspaces | length')
    current=$(herdr workspace list 2>/dev/null | jq -r '.result.workspaces[] | select(.focused==true) | .number')
    prev=$((current - 1))
    [[ $prev -lt 1 ]] && prev=$total
    id=$(herdr workspace list 2>/dev/null | jq -r ".result.workspaces[] | select(.number==$prev) | .workspace_id")
    herdr workspace focus "$id" 2>/dev/null
}

next_agent() {
    local agents current next target
    agents=$(herdr agent list 2>/dev/null | jq -r '.result.agents[].agent')
    current=$(herdr agent list 2>/dev/null | jq -r '.result.agents[] | select(.focused==true) | .agent')
    # 循环到下一个
    next=$(echo "$agents" | awk -v cur="$current" '
        NR==1 { first=$0 }
        prev==cur { print $0; found=1; exit }
        { prev=$0 }
        END { if (!found) print first }
    ')
    herdr agent focus "$next" 2>/dev/null
}

prev_agent() {
    local agents current prev target
    agents=$(herdr agent list 2>/dev/null | jq -r '.result.agents[].agent')
    current=$(herdr agent list 2>/dev/null | jq -r '.result.agents[] | select(.focused==true) | .agent')
    # 循环到上一个
    prev=$(echo "$agents" | awk -v cur="$current" '
        { a[NR]=$0; if ($0==cur) idx=NR }
        END {
            if (idx>1) print a[idx-1]
            else print a[length(a)]
        }
    ')
    herdr agent focus "$prev" 2>/dev/null
}

double_esc() {
    local agent esc
    agent=$(herdr agent list 2>/dev/null | jq -r '.result.agents[] | select(.focused==true) | .agent')
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
