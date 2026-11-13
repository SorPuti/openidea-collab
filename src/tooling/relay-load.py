#!/usr/bin/env python3
"""Isolated loopback relay load test; creates only synthetic state in a temporary directory."""
import argparse, asyncio, base64, hashlib, json, os, secrets, socket, struct, subprocess, tempfile, time, uuid
from pathlib import Path

async def ws_connect(port, token):
    reader, writer = await asyncio.open_connection('127.0.0.1', port)
    key = base64.b64encode(secrets.token_bytes(16)).decode()
    request = f'GET /v1/socket HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: openidea-collab.v1\r\nAuthorization: Bearer {token}\r\n\r\n'
    writer.write(request.encode())
    await writer.drain()
    response = await asyncio.wait_for(reader.readuntil(b'\r\n\r\n'), 5)
    if b' 101 ' not in response.split(b'\r\n', 1)[0]:
        raise RuntimeError('WebSocket handshake rejected')
    return (reader, writer)

async def send(writer, payload, opcode=2):
    size = len(payload)
    header = bytes([128 | opcode])
    header += bytes([128 | size]) if size < 126 else bytes([128 | 126]) + struct.pack('!H', size) if size < 65536 else bytes([128 | 127]) + struct.pack('!Q', size)
    mask = secrets.token_bytes(4)
    data = bytes((b ^ mask[i % 4] for i, b in enumerate(payload)))
    writer.write(header + mask + data)
    await writer.drain()

async def receive(reader, writer):
    while True:
        h = await reader.readexactly(2)
        opcode = h[0] & 15
        n = h[1] & 127
        if n == 126:
            n = struct.unpack('!H', await reader.readexactly(2))[0]
        elif n == 127:
            n = struct.unpack('!Q', await reader.readexactly(8))[0]
        if n > 100000:
            raise RuntimeError('oversized reply')
        data = await reader.readexactly(n)
        if opcode == 9:
            await send(writer, data, 10)
            continue
        if opcode == 2:
            return data
        if opcode == 8:
            raise RuntimeError('socket closed under load')

async def run(args):
    sessions = {}
    rooms = []
    expires = int(time.time()) + 1800
    for _ in range(args.rooms):
        sid = uuid.uuid4()
        doc = uuid.uuid4()
        members = {}
        creds = []
        for role in ('owner', 'editor', 'editor'):
            mid = uuid.uuid4()
            token = base64.urlsafe_b64encode(secrets.token_bytes(32)).rstrip(b'=').decode()
            members[str(mid)] = {'digest': list(hashlib.sha256(token.encode()).digest()), 'role': role}
            creds.append(token)
        sessions[str(sid)] = {'expires_at': expires, 'members': members, 'invitations': {}, 'documents': [str(doc)]}
        rooms.append((doc, creds))
    with tempfile.TemporaryDirectory(prefix='relay-load-') as temp:
        os.chmod(temp, 0o700)
        state = Path(temp) / 'metadata.json'
        state.write_text(json.dumps({'version': 1, 'sessions': sessions}))
        os.chmod(state, 0o600)
        with socket.socket() as reserve:
            reserve.bind(('127.0.0.1', 0))
            port = reserve.getsockname()[1]
        env = os.environ.copy()
        env.update(COLLAB_BIND=f'127.0.0.1:{port}', COLLAB_PUBLIC_ORIGIN=f'http://127.0.0.1:{port}', COLLAB_STATE_FILE=str(state))
        proc = subprocess.Popen([args.binary], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        rss_peak = 0
        cpu_ticks = 0
        writers = []
        samples = []
        start = time.monotonic()

        async def sample():
            nonlocal rss_peak, cpu_ticks
            while proc.poll() is None:
                try:
                    for line in Path(f'/proc/{proc.pid}/status').read_text().splitlines():
                        if line.startswith('VmRSS:'):
                            rss_peak = max(rss_peak, int(line.split()[1]))
                            break
                    fields = Path(f'/proc/{proc.pid}/stat').read_text().rsplit(')', 1)[1].split()
                    cpu_ticks = int(fields[11]) + int(fields[12])
                except FileNotFoundError:
                    break
                await asyncio.sleep(0.05)
        monitor = asyncio.create_task(sample())
        try:
            for _ in range(100):
                try:
                    r, w = await asyncio.open_connection('127.0.0.1', port)
                    w.close()
                    await w.wait_closed()
                    break
                except OSError:
                    await asyncio.sleep(0.05)
            sockets = []
            for doc, creds in rooms:
                group = []
                for token in creds:
                    r, w = await ws_connect(port, token)
                    group.append((r, w))
                    writers.append(w)
                sockets.append((doc, group))
            payload = b'x' * args.payload

            async def session_work(doc, group):
                for _ in range(args.messages):
                    message = uuid.uuid4()
                    frame = b'OIC1' + bytes([2]) + doc.bytes + bytes(16) + message.bytes + struct.pack('>HHI', 0, 1, len(payload)) + payload
                    before = time.perf_counter_ns()
                    await send(group[0][1], frame)
                    replies = await asyncio.wait_for(asyncio.gather(*(receive(r, w) for r, w in group[1:])), 10)
                    if any((item[16:] != frame for item in replies)):
                        raise RuntimeError('incorrect delivery')
                    samples.append((time.perf_counter_ns() - before) / 1000000.0)
                    if args.delay:
                        await asyncio.sleep(args.delay)
            await asyncio.gather(*(session_work(doc, group) for doc, group in sockets))
            samples.sort()
            duration = time.monotonic() - start
            print(json.dumps({'rooms': args.rooms, 'connections': len(writers), 'messages': len(samples), 'deliveries': len(samples) * 2, 'payload_bytes': args.payload, 'delay_seconds': args.delay, 'elapsed_seconds': round(duration, 3), 'relay_peak_rss_kib': rss_peak, 'relay_mean_cpu_percent': round(100 * cpu_ticks / os.sysconf('SC_CLK_TCK') / duration, 2), 'p50_ms': round(samples[len(samples) // 2], 3), 'p95_ms': round(samples[len(samples) * 95 // 100], 3), 'p99_ms': round(samples[len(samples) * 99 // 100], 3), 'max_ms': round(samples[-1], 3)}))
        finally:
            for w in writers:
                w.close()
            proc.terminate()
            try:
                await asyncio.to_thread(proc.wait, 5)
            except subprocess.TimeoutExpired:
                proc.kill()
                await asyncio.to_thread(proc.wait)
            await monitor
if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--rooms', type=int, default=4, choices=range(1, 17))
    parser.add_argument('--messages', type=int, default=100, choices=range(1, 1001))
    parser.add_argument('--payload', type=int, default=1024, choices=range(1, 65476))
    parser.add_argument('--delay', type=float, default=0.0)
    args = parser.parse_args()
    if args.delay < 0:
        parser.error('delay must be non-negative')
    asyncio.run(run(args))
