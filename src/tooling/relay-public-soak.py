import argparse
import asyncio
import json
import pathlib
import struct
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

import websockets

parser = argparse.ArgumentParser(
    description="Run a synthetic WSS soak; this creates and revokes a real relay session."
)
parser.add_argument("--origin", required=True)
parser.add_argument("--admission-file", required=True)
args = parser.parse_args()
base = args.origin.rstrip("/")
if not base.startswith("https://"):
    parser.error("origin must be HTTPS")
admission = pathlib.Path(args.admission_file).read_text().strip()


def request(method, path, data=None, headers=None):
    h = dict(headers or {})
    payload = None
    if data is not None:
        h["Content-Type"] = "application/json"
        payload = json.dumps(data).encode()
    try:
        with urllib.request.urlopen(
            urllib.request.Request(base + path, data=payload, headers=h, method=method),
            timeout=15,
        ) as response:
            raw = response.read()
            return response.status, (json.loads(raw) if raw else None)
    except urllib.error.HTTPError as error:
        raw = error.read()
        return error.code, (json.loads(raw) if raw else None)


def expect(status, value, code, where):
    if status != code:
        raise RuntimeError(f"{where} HTTP {status}")
    return value


async def main():
    status, _ = request("POST", "/v1/sessions", {"protocol": 1})
    assert status == 401
    status, owner = request(
        "POST", "/v1/sessions", {"protocol": 1}, {"X-Collab-Admission": admission}
    )
    expect(status, owner, 201, "create")
    sid = owner["session_id"]
    token = owner["credential"]
    owner_id = owner["member_id"]
    peers = []
    sockets = []
    try:
        for _ in range(2):
            status, invite = request(
                "POST",
                f"/v1/sessions/{sid}/invitations",
                {"role": "editor", "ttl_seconds": 300},
                {"Authorization": "Bearer " + token},
            )
            expect(status, invite, 200, "invite")
            u = urllib.parse.urlsplit(invite["url"])
            status, peer = request(
                "POST",
                "/v1/invitations/redeem",
                {"invite_id": u.path.rsplit("/", 1)[-1], "secret": u.fragment},
            )
            peers.append(expect(status, peer, 200, "redeem"))
        doc = uuid.uuid4()
        status, value = request(
            "POST",
            f"/v1/sessions/{sid}/documents",
            {"document_id": str(doc)},
            {"Authorization": "Bearer " + token},
        )
        expect(status, value, 204, "register")
        for member in [owner, *peers]:
            sockets.append(
                await websockets.connect(
                    "wss://collab.synthosia.com/v1/socket",
                    additional_headers={
                        "Authorization": "Bearer " + member["credential"]
                    },
                    subprotocols=["openidea-collab.v1"],
                    proxy=None,
                    open_timeout=15,
                )
            )
        print(
            json.dumps({"stage": "public_connected", "clients": len(sockets)}),
            flush=True,
        )
        payload = b"S" * 1024
        samples = []
        begin = time.monotonic()
        report = begin + 30
        while time.monotonic() - begin < 90:
            frame = (
                b"OIC1"
                + bytes([2])
                + doc.bytes
                + bytes(16)
                + uuid.uuid4().bytes
                + struct.pack(">HHI", 0, 1, len(payload))
                + payload
            )
            start = time.perf_counter_ns()
            await sockets[0].send(frame)
            msgs = await asyncio.wait_for(
                asyncio.gather(sockets[1].recv(), sockets[2].recv()), 15
            )
            if any(m[16:] != frame for m in msgs):
                raise RuntimeError("integrity mismatch")
            samples.append((time.perf_counter_ns() - start) / 1e6)
            if time.monotonic() > report:
                print(
                    json.dumps(
                        {
                            "stage": "public_progress",
                            "messages": len(samples),
                            "seconds": round(time.monotonic() - begin),
                        }
                    ),
                    flush=True,
                )
                report += 30
            await asyncio.sleep(0.03)
        # Revoke an editor and check its existing socket closes.
        st, v = request(
            "DELETE",
            f"/v1/sessions/{sid}/members/{peers[0]['member_id']}",
            headers={"Authorization": "Bearer " + token},
        )
        expect(st, v, 204, "revoke")
        try:
            await asyncio.wait_for(sockets[1].recv(), 15)
            raise RuntimeError("revoked socket remained open")
        except websockets.exceptions.ConnectionClosed:
            pass
        samples.sort()
        print(
            json.dumps(
                {
                    "stage": "public_result",
                    "status": "passed",
                    "duration_seconds": round(time.monotonic() - begin, 2),
                    "clients": 3,
                    "messages": len(samples),
                    "deliveries": 2 * len(samples),
                    "bytes_per_message": 1024,
                    "p50_ms": round(samples[len(samples) // 2], 2),
                    "p95_ms": round(samples[int(len(samples) * 0.95)], 2),
                    "p99_ms": round(samples[int(len(samples) * 0.99)], 2),
                    "max_ms": round(samples[-1], 2),
                    "revocation": "passed",
                    "admission": "passed",
                }
            ),
            flush=True,
        )
    finally:
        for s in sockets:
            await s.close()
        st, v = request(
            "DELETE",
            f"/v1/sessions/{sid}/members/{owner_id}",
            headers={"Authorization": "Bearer " + token},
        )
        print(json.dumps({"stage": "public_cleanup", "http": st}), flush=True)


asyncio.run(main())
