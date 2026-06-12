#!/usr/bin/env bash
# 录制 LLM e2e fixtures
#
# 工作流:
#   1. 清理旧的 fixtures(可选,默认保留)
#   2. 走 cargo test --features=backends,e2e + E2E_MODE=record
#   3. 真实调用 DeepSeek + 落盘响应到 tests/e2e/common/fixtures/{test}/{model}/{key}.json
#   4. 跑 validate-fixtures.py 做基础校验(JSON 合法 / 必备字段 / 落盘数量)
#
# 用法:
#   ./scripts/record-fixtures.sh                # 用现有 DEEPSEEK_API_KEY
#   ./scripts/record-fixtures.sh --clean        # 录制前先 rm -rf fixtures
#   ./scripts/record-fixtures.sh --test <name>  # 只录某个 test(子集)
#
# 环境要求:
#   - DEEPSEEK_API_KEY(必须,真实调用需要)
#   - DEEPSEEK_BASE_URL(可选,默认 https://api.deepseek.com/v1)
#   - DEEPSEEK_MODEL(可选,默认 deepseek-chat)
#   - jq(可选,validate 时用)
#   - python3(validate-fixtures.py 需要)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURES_DIR="$ROOT_DIR/crates/axon-llm/tests/e2e/common/fixtures"

# ─── 参数解析 ──────────────────────────────────────────

CLEAN=0
TEST_FILTER=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --clean) CLEAN=1; shift ;;
        --test) TEST_FILTER="$2"; shift 2 ;;
        -h|--help)
            grep -E "^#( |!)" "$0" | sed -E 's/^# ?//'
            exit 0
            ;;
        *) echo "未知参数: $1" >&2; exit 1 ;;
    esac
done

# ─── 前置检查 ──────────────────────────────────────────

if [[ -z "${DEEPSEEK_API_KEY:-}" ]]; then
    echo "❌ DEEPSEEK_API_KEY 未设置,无法录制真实响应" >&2
    echo "   export DEEPSEEK_API_KEY=sk-... 后重试" >&2
    exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "❌ python3 未找到,validate-fixtures.py 需要" >&2
    exit 1
fi

# ─── 可选清理 ──────────────────────────────────────────

if [[ "$CLEAN" -eq 1 && -d "$FIXTURES_DIR" ]]; then
    echo "🧹 清理旧 fixtures: $FIXTURES_DIR"
    rm -rf "$FIXTURES_DIR"
fi

mkdir -p "$FIXTURES_DIR"

# ─── 录制 ─────────────────────────────────────────────

echo "▶ 开始录制 fixtures(模式:record,base_url=${DEEPSEEK_BASE_URL:-https://api.deepseek.com/v1})"
echo "  model=${DEEPSEEK_MODEL:-deepseek-chat}"
echo "  fixtures: $FIXTURES_DIR"
echo

export E2E_MODE=record
export RUST_LOG="${RUST_LOG:-info}"

cd "$ROOT_DIR"

# 选跑哪些测试(默认全部 4 个 e2e)
TESTS=(
    "e2e_simple_chat_test"
    "e2e_tool_calling_test"
    "e2e_react_loop_test"
    "e2e_explain_e2e_test"
)

if [[ -n "$TEST_FILTER" ]]; then
    TESTS=("$TEST_FILTER")
fi

for t in "${TESTS[@]}"; do
    echo "─── $t ───"
    cargo test \
        -p axon-llm \
        --features "backends e2e" \
        --test "$t" \
        -- \
        --nocapture --test-threads=1 2>&1 | tail -20 || {
            echo "  ⚠️  $t 录制失败(继续跑下一个)" >&2
        }
    echo
done

# ─── 校验 ─────────────────────────────────────────────

echo "▶ 校验录制的 fixtures"
python3 "$ROOT_DIR/scripts/validate-fixtures.py" "$FIXTURES_DIR" || {
    echo "❌ validate-fixtures.py 校验失败" >&2
    exit 1
}

echo
echo "✅ 录制完成。fixtures 已写入: $FIXTURES_DIR"
echo "   git add fixtures/ && git commit -m 'chore(axon-llm): refresh e2e fixtures'"
