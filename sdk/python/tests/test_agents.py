from __future__ import annotations

import httpx


def test_agents_submit_success(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/v1/agents/tasks"
        payload = request.read().decode("utf-8")
        assert '"workflow_kind":"research_brief"' in payload
        assert '"question":"What changed?"' in payload
        return httpx.Response(
            201,
            json={
                "ok": True,
                "execution": {
                    "execution_id": "exec-1",
                    "workflow_kind": "research_brief",
                    "status": "completed",
                    "created_at_ms": 1,
                    "updated_at_ms": 2,
                    "source_count": 1,
                },
                "provenance": {
                    "execution_id": "exec-1",
                    "root_receipt_id": "rcpt-1",
                    "nodes": [],
                    "edges": [],
                    "incomplete": False,
                },
                "receipts": [],
            },
        )

    client = make_client(handler)
    response = client.agents.submit(
        question="What changed?",
        sources=[{"id": "s1", "content": "A market shifted."}],
        policy={"trust_tier": "verified_mesh", "allowed_supply_tiers": ["private"]},
    )

    assert response.execution.execution_id == "exec-1"
    assert response.execution.status == "completed"


def test_agents_status_and_receipts(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == "/v1/executions/exec-2":
            return httpx.Response(
                200,
                json={
                    "ok": True,
                    "execution": {
                        "execution_id": "exec-2",
                        "workflow_kind": "research_brief",
                        "status": "running",
                        "created_at_ms": 10,
                        "updated_at_ms": 20,
                        "source_count": 2,
                    },
                },
            )
        if request.url.path == "/v1/executions/exec-2/receipts":
            return httpx.Response(
                200,
                json={
                    "ok": True,
                    "receipts": [
                        {
                            "receipt_id": "rcpt-2",
                            "execution_id": "exec-2",
                            "step_id": "planner",
                            "attempt_id": "planner-1",
                            "event_kind": "planned",
                            "timestamp_ms": 10,
                            "workflow_kind": "research_brief",
                        }
                    ],
                },
            )
        raise AssertionError(f"Unexpected path {request.url.path}")

    client = make_client(handler)
    summary = client.agents.status("exec-2")
    receipts = client.agents.receipts("exec-2")

    assert summary is not None
    assert summary.status == "running"
    assert receipts[0].receipt_id == "rcpt-2"


def test_agents_provenance_and_capabilities(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == "/v1/executions/exec-3/provenance":
            return httpx.Response(
                200,
                json={
                    "ok": True,
                    "provenance": {
                        "execution_id": "exec-3",
                        "root_receipt_id": "rcpt-root",
                        "nodes": [
                            {
                                "receipt_id": "rcpt-root",
                                "step_id": "workflow",
                                "attempt_id": "workflow-1",
                                "event_kind": "planned",
                                "timestamp_ms": 1,
                            }
                        ],
                        "edges": [],
                        "incomplete": True,
                    },
                },
            )
        if request.url.path == "/v1/capabilities":
            return httpx.Response(
                200,
                json={
                    "ok": True,
                    "count": 1,
                    "capabilities": [
                        {
                            "candidate_id": "node-1",
                            "display_name": "node-1",
                            "supply_tier": "personal",
                            "trust_tier": "local",
                            "tags": ["planning"],
                        }
                    ],
                },
            )
        raise AssertionError(f"Unexpected path {request.url.path}")

    client = make_client(handler)
    provenance = client.agents.provenance("exec-3")
    capabilities = client.agents.capabilities()

    assert provenance.execution_id == "exec-3"
    assert provenance.incomplete is True
    assert capabilities[0].candidate_id == "node-1"
