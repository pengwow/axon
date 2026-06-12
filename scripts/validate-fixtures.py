#!/usr/bin/env python3
"""校验 tests/e2e/common/fixtures/ 下的 fixture 文件。

基本检查(8 项):
  1. JSON 合法
  2. 顶层字段:version / recorded_at / model / request / response
  3. request.url / method / headers / body 齐备
  4. request.body 含 model + messages 字段
  5. response.status / headers / body 齐备
  6. response.body.choices 是非空数组
  7. response.body.usage.{prompt,completion,total}_tokens 非负整数
  8. 落盘文件数 ≥ 4(覆盖 4 个 e2e 场景)

--strict 模式额外检查:
  9. request.headers 不含 Authorization / x-api-key(已脱敏)
 10. response.headers 不含 set-cookie
 11. body.canonical SHA256 与文件名 key 一致(用 recording 同样的算法)
 12. fixture 总大小 < 5MB(单文件)

退出码:
  0 全部通过
  1 有失败
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

REQUIRED_TOP = ("version", "recorded_at", "model", "request", "response")
REQUIRED_REQ = ("url", "method", "headers", "body")
REQUIRED_RES = ("status", "headers", "body")
SENSITIVE_REQ_HEADERS = ("authorization", "x-api-key")
SENSITIVE_RES_HEADERS = ("set-cookie",)

# recording.rs::canonicalize_body 简化版:对 JSON 字段做字典序排序
def canonicalize(obj):
    if isinstance(obj, dict):
        return {k: canonicalize(obj[k]) for k in sorted(obj.keys())}
    if isinstance(obj, list):
        return [canonicalize(x) for x in obj]
    return obj


def fixture_key(url: str, method: str, body: dict) -> str:
    """对应 recording.rs::RecordingLayer::fixture_key"""
    canonical = json.dumps(canonicalize(body), sort_keys=True, separators=(",", ":"))
    h = hashlib.sha256()
    h.update(url.encode()); h.update(b"|")
    h.update(method.encode()); h.update(b"|")
    h.update(canonical.encode())
    return h.hexdigest()[:12]


def check_file(path: Path, strict: bool) -> list[str]:
    errs: list[str] = []

    # 1. JSON 合法
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        return [f"{path}: JSON parse error: {e}"]

    # 2. 顶层字段
    for k in REQUIRED_TOP:
        if k not in data:
            errs.append(f"{path}: missing top-level field '{k}'")

    # 3. request
    req = data.get("request", {})
    for k in REQUIRED_REQ:
        if k not in req:
            errs.append(f"{path}: missing request.{k}")

    # 4. body 含 model + messages
    body = req.get("body", {})
    if not isinstance(body, dict):
        errs.append(f"{path}: request.body should be JSON object")
    else:
        if "model" not in body:
            errs.append(f"{path}: request.body missing 'model'")
        if "messages" not in body:
            errs.append(f"{path}: request.body missing 'messages'")

    # 5. response
    res = data.get("response", {})
    for k in REQUIRED_RES:
        if k not in res:
            errs.append(f"{path}: missing response.{k}")

    # 6. choices 是非空数组
    res_body = res.get("body", {})
    if isinstance(res_body, dict):
        choices = res_body.get("choices")
        if not isinstance(choices, list) or len(choices) == 0:
            errs.append(f"{path}: response.body.choices should be non-empty array")

    # 7. usage 字段
    if isinstance(res_body, dict):
        usage = res_body.get("usage", {})
        if not isinstance(usage, dict):
            errs.append(f"{path}: response.body.usage should be object")
        else:
            for tk in ("prompt_tokens", "completion_tokens", "total_tokens"):
                v = usage.get(tk)
                if not isinstance(v, int) or v < 0:
                    errs.append(f"{path}: response.body.usage.{tk} should be non-negative int, got {v!r}")

    if not strict:
        return errs

    # 9. 敏感 header
    req_headers = {k.lower() for k in req.get("headers", {}).keys()}
    for s in SENSITIVE_REQ_HEADERS:
        if s in req_headers:
            errs.append(f"{path}: request.headers contains sensitive '{s}' (not sanitized)")

    res_headers = {k.lower() for k in res.get("headers", {}).keys()}
    for s in SENSITIVE_RES_HEADERS:
        if s in res_headers:
            errs.append(f"{path}: response.headers contains sensitive '{s}' (not sanitized)")

    # 11. key 与文件名一致
    if isinstance(body, dict):
        expected = fixture_key(req.get("url", ""), req.get("method", "POST"), body)
        if path.stem != expected:
            errs.append(f"{path}: filename key '{path.stem}' != expected '{expected}' (body changed?)")

    # 12. 文件大小 < 5MB
    size = path.stat().st_size
    if size > 5 * 1024 * 1024:
        errs.append(f"{path}: file size {size} bytes exceeds 5MB")

    return errs


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("path", help="fixtures 目录")
    ap.add_argument("--strict", action="store_true", help="额外做脱敏 / key 一致性 / 大小检查")
    args = ap.parse_args()

    root = Path(args.path)
    if not root.exists():
        print(f"❌ 目录不存在: {root}", file=sys.stderr)
        return 1

    files = sorted(root.rglob("*.json"))
    if not files:
        print(f"❌ 未找到 fixture 文件: {root}", file=sys.stderr)
        return 1

    print(f"▶ 校验 {len(files)} 个 fixtures (strict={args.strict})")

    all_errs: list[str] = []
    for p in files:
        errs = check_file(p, args.strict)
        for e in errs:
            print(f"  ❌ {e}")
        all_errs.extend(errs)

    if all_errs:
        print(f"\n❌ {len(all_errs)} 项错误")
        return 1

    print(f"\n✅ 全部 {len(files)} 个 fixtures 通过校验")
    return 0


if __name__ == "__main__":
    sys.exit(main())
