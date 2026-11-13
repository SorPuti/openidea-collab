#!/usr/bin/env python3
import asyncio
import importlib.util
import json
import os
import socket
import struct
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "load", str(Path(__file__).with_name("relay-load.py"))
)
load = importlib.util.module_from_spec(spec)
spec.loader.exec_module(load)
BINARY = os.environ.get("COLLAB_TEST_BINARY", "/usr/local/bin/collab-server")
TOTAL_ROOMS = int(os.environ.get("SOAK_ROOMS", "16"))
if not 1 <= TOTAL_ROOMS <= 16:
    raise SystemExit("SOAK_ROOMS must be between 1 and 16")
SOAK_SECONDS = int(os.environ.get("SOAK_SECONDS", "180"))
GUARD_HEALTH = os.environ.get("COLLAB_GUARD_HEALTH", "http://127.0.0.1:4444/healthz")
if not 1 <= SOAK_SECONDS <= 3600:
    raise SystemExit("SOAK_SECONDS must be between 1 and 3600")


def http(method, url, body=None, token=None):
    headers = {}
    if token:
        headers["Authorization"] = "Bearer " + token
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=10) as resp:
            raw = resp.read()
            return resp.status, (json.loads(raw) if raw else None)
    except urllib.error.HTTPError as e:
        raw = e.read()
        return e.code, (json.loads(raw) if raw else None)


def call(method, path, body=None, token=None):
    return http(method, BASE + path, body, token)


def require(status, result, expected, where):
    if status != expected:
        raise RuntimeError(
            f"{where}: unexpected HTTP {status}, {result.get('error') if isinstance(result, dict) else ''}"
        )
    return result


def memory_available():
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemAvailable:"):
            return int(line.split()[1])
    raise RuntimeError("cannot inspect available memory")


async def main():
    global BASE
    with tempfile.TemporaryDirectory(prefix="collab-soak-") as tmp:
        os.chmod(tmp, 0o700)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        BASE = f"http://127.0.0.1:{port}"
        env = os.environ.copy()
        env.update(
            COLLAB_BIND=f"127.0.0.1:{port}",
            COLLAB_PUBLIC_ORIGIN=BASE,
            COLLAB_STATE_FILE=tmp + "/metadata.json",
        )
        log = open(tmp + "/relay.log", "w+", encoding="utf8")  # noqa: ASYNC230, SIM115 -- closed in finally
        proc = subprocess.Popen(  # noqa: ASYNC220 -- test setup/restart, not data plane
            [BINARY], env=env, stdout=subprocess.DEVNULL, stderr=log
        )
        rooms = []
        sockets = []
        minimum_memory = memory_available()
        peak_rss = 0
        cpu0 = None
        cpu_ticks = 0
        monitor_failure = None

        async def monitor():
            nonlocal minimum_memory, peak_rss, cpu0, cpu_ticks, monitor_failure
            while True:
                try:
                    minimum_memory = min(minimum_memory, memory_available())
                    if minimum_memory < 48 * 1024:
                        raise RuntimeError(
                            "stopped: host memory below 48 MiB available"
                        )
                    if http("GET", GUARD_HEALTH)[0] != 204:
                        raise RuntimeError("stopped: production health failed")
                    if proc.poll() is not None:
                        raise RuntimeError("isolated relay stopped unexpectedly")
                    for line in (
                        Path(f"/proc/{proc.pid}/status").read_text().splitlines()
                    ):
                        if line.startswith("VmRSS:"):
                            peak_rss = max(peak_rss, int(line.split()[1]))
                            break
                    st = (
                        Path(f"/proc/{proc.pid}/stat")
                        .read_text()
                        .rsplit(")", 1)[1]
                        .split()
                    )
                    cpu_ticks = int(st[11]) + int(st[12])
                except Exception as e:  # noqa: BLE001 -- any guard failure aborts the test
                    monitor_failure = str(e)
                    return
                await asyncio.sleep(2)

        monitor_task = asyncio.create_task(monitor())
        try:
            for _ in range(100):
                try:
                    if call("GET", "/healthz")[0] == 204:
                        break
                except OSError:
                    pass
                await asyncio.sleep(0.05)
            for i in range(TOTAL_ROOMS):
                while True:
                    status, body = await asyncio.to_thread(
                        call, "POST", "/v1/sessions", {"protocol": 1}
                    )
                    if status == 201:
                        owner = body
                        break
                    if status != 429 or body.get("error") != "creation_rate_limit":
                        require(status, body, 201, "create session")
                    for _ in range(31):
                        if monitor_failure:
                            raise RuntimeError(monitor_failure)
                        await asyncio.sleep(1)
                sid = owner["session_id"]
                token = owner["credential"]
                doc = uuid.uuid4()
                st, res = await asyncio.to_thread(
                    call,
                    "POST",
                    f"/v1/sessions/{sid}/documents",
                    {"document_id": str(doc)},
                    token,
                )
                require(st, res, 204, "register document")
                members = [owner]
                for _ in range(2):
                    st, invite = await asyncio.to_thread(
                        call,
                        "POST",
                        f"/v1/sessions/{sid}/invitations",
                        {"role": "editor", "ttl_seconds": 300},
                        token,
                    )
                    require(st, invite, 200, "invite")
                    u = urllib.parse.urlsplit(invite["url"])
                    st, member = await asyncio.to_thread(
                        call,
                        "POST",
                        "/v1/invitations/redeem",
                        {"invite_id": u.path.rsplit("/", 1)[-1], "secret": u.fragment},
                    )
                    require(st, member, 200, "redeem")
                    members.append(member)
                rooms.append({"owner": owner, "members": members, "doc": doc})
                print(
                    json.dumps(
                        {
                            "stage": "sessions_created",
                            "count": i + 1,
                            "target": TOTAL_ROOMS,
                        }
                    ),
                    flush=True,
                )
            if TOTAL_ROOMS == 16:
                await asyncio.sleep(31)
                st, res = await asyncio.to_thread(
                    call, "POST", "/v1/sessions", {"protocol": 1}
                )
                if st != 429 or res.get("error") != "session_limit":
                    raise RuntimeError("session cap was not enforced")
                print(
                    json.dumps(
                        {"stage": "session_cap", "limit": 16, "seventeenth": "refused"}
                    ),
                    flush=True,
                )
            # Each piece of credential-bearing metadata is persisted; restart is isolated.
            monitor_task.cancel()
            await asyncio.gather(monitor_task, return_exceptions=True)
            proc.terminate()
            await asyncio.to_thread(proc.wait, 10)
            proc = subprocess.Popen(  # noqa: ASYNC220 -- test setup/restart, not data plane
                [BINARY], env=env, stdout=subprocess.DEVNULL, stderr=log
            )
            for _ in range(100):
                try:
                    if call("GET", "/healthz")[0] == 204:
                        break
                except OSError:
                    pass
                await asyncio.sleep(0.05)
            monitor_task = asyncio.create_task(monitor())
            for room in rooms:
                group = []
                for member in room["members"]:
                    r, w = await load.ws_connect(port, member["credential"])
                    group.append((r, w))
                    sockets.append(w)
                room["sockets"] = group
            print(
                json.dumps(
                    {
                        "stage": "restart_and_reconnect",
                        "sessions": len(rooms),
                        "sockets": len(sockets),
                    }
                ),
                flush=True,
            )
            # Validate a 2 MiB application-level transfer through the real framing.
            room = rooms[0]
            group = room["sockets"]
            payload = b"Z" * (2 * 1024 * 1024)
            chunk = 60000
            parts = (len(payload) + chunk - 1) // chunk
            message = uuid.uuid4()
            assembled = [bytearray(), bytearray()]
            for part in range(parts):
                segment = payload[part * chunk : (part + 1) * chunk]
                frame = (
                    b"OIC1"
                    + bytes([2])
                    + room["doc"].bytes
                    + bytes(16)
                    + message.bytes
                    + struct.pack(">HHI", part, parts, len(payload))
                    + segment
                )
                await load.send(group[0][1], frame)
                replies = await asyncio.wait_for(
                    asyncio.gather(*(load.receive(r, w) for r, w in group[1:])), 10
                )
                for j, reply in enumerate(replies):
                    if reply[16:] != frame:
                        raise RuntimeError("chunk corrupted")
                    assembled[j].extend(reply[16 + 61 :])
                await asyncio.sleep(0.26)
            if any(bytes(value) != payload for value in assembled):
                raise RuntimeError("2 MiB transfer did not reassemble")
            print(
                json.dumps(
                    {
                        "stage": "chunked_transfer",
                        "bytes": len(payload),
                        "chunks": parts,
                        "recipients": 2,
                        "integrity": "passed",
                    }
                ),
                flush=True,
            )
            # Sustained controlled rate; every delivery is compared byte-for-byte.
            duration = SOAK_SECONDS
            stop = time.monotonic() + duration
            samples = []
            count = 0

            async def room_work(room):
                nonlocal count
                group = room["sockets"]
                data = b"Q" * 1024
                while time.monotonic() < stop:
                    if monitor_failure:
                        raise RuntimeError(monitor_failure)
                    frame = (
                        b"OIC1"
                        + bytes([2])
                        + room["doc"].bytes
                        + bytes(16)
                        + uuid.uuid4().bytes
                        + struct.pack(">HHI", 0, 1, len(data))
                        + data
                    )
                    start = time.perf_counter_ns()
                    await load.send(group[0][1], frame)
                    replies = await asyncio.wait_for(
                        asyncio.gather(*(load.receive(r, w) for r, w in group[1:])), 10
                    )
                    if any(reply[16:] != frame for reply in replies):
                        raise RuntimeError("delivery corruption")
                    samples.append((time.perf_counter_ns() - start) / 1e6)
                    count += 1
                    await asyncio.sleep(0.075)

            begin = time.monotonic()
            cpu_start = cpu_ticks
            tasks = [asyncio.create_task(room_work(room)) for room in rooms]

            async def report():
                while not all(t.done() for t in tasks):
                    await asyncio.sleep(30)
                    print(
                        json.dumps(
                            {
                                "stage": "soak_progress",
                                "seconds": round(time.monotonic() - begin),
                                "messages": count,
                            }
                        ),
                        flush=True,
                    )

            progress = asyncio.create_task(report())
            try:
                await asyncio.gather(*tasks)
            finally:
                progress.cancel()
            samples.sort()
            elapsed = time.monotonic() - begin
            # A removed member must immediately lose both socket and HTTP access.
            victim = rooms[0]["members"][1]
            owner = rooms[0]["owner"]
            sid = owner["session_id"]
            st, res = await asyncio.to_thread(
                call,
                "DELETE",
                f"/v1/sessions/{sid}/members/{victim['member_id']}",
                None,
                owner["credential"],
            )
            require(st, res, 204, "revoke")
            try:
                _, w = await load.ws_connect(port, victim["credential"])
                w.close()
                raise RuntimeError("revoked credential accepted")
            except RuntimeError as e:
                if str(e) == "revoked credential accepted":
                    raise
            log.flush()
            log.seek(0)
            audit = log.read()
            if (
                any(
                    member["credential"] in audit
                    for room in rooms
                    for member in room["members"]
                )
                or ("Q" * 1024) in audit
                or ("Z" * 1024) in audit
            ):
                raise RuntimeError("sensitive payload or credentials appeared in logs")
            print(
                json.dumps(
                    {
                        "stage": "result",
                        "status": "passed",
                        "sessions": len(rooms),
                        "connections": len(sockets),
                        "seconds": round(elapsed, 2),
                        "messages": count,
                        "deliveries": count * 2,
                        "p50_ms": round(samples[len(samples) // 2], 2),
                        "p95_ms": round(samples[int(len(samples) * 0.95)], 2),
                        "p99_ms": round(samples[int(len(samples) * 0.99)], 2),
                        "max_ms": round(samples[-1], 2),
                        "peak_relay_rss_kib": peak_rss,
                        "host_min_available_kib": minimum_memory,
                        "relay_mean_cpu_pct_one_core": round(
                            100
                            * (cpu_ticks - cpu_start)
                            / os.sysconf("SC_CLK_TCK")
                            / elapsed,
                            2,
                        ),
                        "restart_reconnect": "passed",
                        "transfer_2mib_integrity": "passed",
                        "revocation": "passed",
                        "production_health": "passed",
                        "log_privacy": "passed",
                    }
                ),
                flush=True,
            )
        finally:
            monitor_task.cancel()
            for w in sockets:
                w.close()
            if proc.poll() is None:
                proc.terminate()
                try:
                    await asyncio.to_thread(proc.wait, 5)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    await asyncio.to_thread(proc.wait)
            log.close()


asyncio.run(main())
