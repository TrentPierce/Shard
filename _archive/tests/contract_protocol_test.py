import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCHEMAS = ROOT / "docs" / "schemas"


def load_schema(name: str) -> dict:
    return json.loads((SCHEMAS / name).read_text(encoding="utf-8"))


def validate(instance, schema: dict, path: str = "$") -> None:
    schema_type = schema.get("type")
    if schema_type == "object":
        assert isinstance(instance, dict), f"{path} must be an object"
        required = schema.get("required", [])
        for field in required:
            assert field in instance, f"{path}.{field} is required"
        properties = schema.get("properties", {})
        for key, value in instance.items():
            child_schema = properties.get(key)
            if child_schema:
                validate(value, child_schema, f"{path}.{key}")
    elif schema_type == "array":
        assert isinstance(instance, list), f"{path} must be an array"
        min_items = schema.get("minItems")
        if min_items is not None:
            assert len(instance) >= min_items, f"{path} must contain at least {min_items} items"
        item_schema = schema.get("items")
        if item_schema:
            for index, item in enumerate(instance):
                validate(item, item_schema, f"{path}[{index}]")
    elif schema_type == "string":
        assert isinstance(instance, str), f"{path} must be a string"
        min_len = schema.get("minLength")
        if min_len is not None:
            assert len(instance) >= min_len, f"{path} minLength violation"
        max_len = schema.get("maxLength")
        if max_len is not None:
            assert len(instance) <= max_len, f"{path} maxLength violation"
    elif schema_type == "integer":
        assert isinstance(instance, int), f"{path} must be an integer"
        minimum = schema.get("minimum")
        if minimum is not None:
            assert instance >= minimum, f"{path} minimum violation"
    elif schema_type == "number":
        assert isinstance(instance, (int, float)), f"{path} must be a number"
    elif isinstance(schema_type, list):
        assert any(
            (entry == "null" and instance is None)
            or (entry == "integer" and isinstance(instance, int))
            or (entry == "string" and isinstance(instance, str))
            for entry in schema_type
        ), f"{path} type mismatch"

    enum = schema.get("enum")
    if enum:
        assert instance in enum, f"{path} must be one of {enum}"

    pattern = schema.get("pattern")
    if pattern and isinstance(instance, str):
        import re

        assert re.fullmatch(pattern, instance), f"{path} does not match pattern {pattern}"

    if "anyOf" in schema:
        errors = []
        for candidate in schema["anyOf"]:
            try:
                validate(instance, candidate, path)
                return
            except AssertionError as exc:  # pragma: no cover - test utility
                errors.append(str(exc))
        raise AssertionError(f"{path} failed anyOf validation: {errors}")


def test_chat_completions_contract() -> None:
    schema = load_schema("v1.chat-completions.request.schema.json")
    payload = {
        "model": "meta-llama/Llama-3.2-1B",
        "stream": True,
        "max_tokens": 128,
        "messages": [{"role": "user", "content": "hello"}],
    }
    validate(payload, schema)


def test_scout_work_contract() -> None:
    schema = load_schema("v1.scout-work.response.schema.json")
    payload = {
        "work": {
            "request_id": "req-1",
            "prompt_context": "prompt",
            "min_tokens": 4,
            "created_at_ms": 123,
        }
    }
    validate(payload, schema)


def test_scout_draft_contract() -> None:
    schema = load_schema("v1.scout-draft.request.schema.json")
    payload = {
        "work_id": "req-1",
        "scout_id": "scout-1",
        "draft_text": "hello world",
        "draft_tokens": [10, 11],
    }
    validate(payload, schema)


def test_signed_envelope_contract() -> None:
    schema = load_schema("signed-envelope.schema.json")
    payload = {
        "envelope": {
            "payload": {"node_pubkey": "x"},
            "signer_pubkey_hex": "a" * 64,
            "nonce": 1,
            "signature_hex": "b" * 128,
        }
    }
    validate(payload, schema)


def test_release_manifest_contract() -> None:
    schema = load_schema("release-manifest.schema.json")
    payload = {
        "version": "0.6.1",
        "git_tag": "v0.6.1",
        "git_commit": "abcdef1234567",
        "artifacts": [
            {
                "path": "dist/shard-installer.exe",
                "size_bytes": 123,
                "sha256": "a" * 64,
                "signature_path": "dist/shard-installer.exe.sig",
                "signed": True,
            }
        ],
    }
    validate(payload, schema)
