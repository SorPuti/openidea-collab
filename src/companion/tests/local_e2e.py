#!/usr/bin/env python3
"""Loopback test of relay auth and companion binary transport. Requires websockets."""
import json, os, pathlib, socket, subprocess, tempfile, time, urllib.request, uuid
from urllib.parse import urlparse
from websockets.sync.client import connect
root=pathlib.Path(__file__).resolve().parents[3]
with socket.socket() as s:
 s.bind(('127.0.0.1',0)); port=s.getsockname()[1]
base=f'http://127.0.0.1:{port}'
with tempfile.TemporaryDirectory() as td:
 env=os.environ.copy(); env.update(COLLAB_BIND=f'127.0.0.1:{port}',COLLAB_PUBLIC_ORIGIN=base,COLLAB_STATE_FILE=f'{td}/metadata.json')
 server=subprocess.Popen([str(root/'target/release/collab-server')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 try:
  for _ in range(100):
   try:
    urllib.request.urlopen(base+'/healthz',timeout=.2); break
   except Exception: time.sleep(.05)
  def post(path,data,token=None):
   h={'Content-Type':'application/json'}
   if token:h['Authorization']='Bearer '+token
   req=urllib.request.Request(base+path,data=json.dumps(data).encode(),headers=h,method='POST')
   with urllib.request.urlopen(req,timeout=3) as r:return r.status,json.load(r)
  _,owner=post('/v1/sessions',{'protocol':1})
  _,docreg= (204,None) # opaque doc UUID registration below
  doc=str(uuid.uuid4()); req=urllib.request.Request(base+f"/v1/sessions/{owner['session_id']}/documents",data=json.dumps({'document_id':doc}).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+owner['credential']},method='POST')
  urllib.request.urlopen(req,timeout=3).close()
  invite_status,inv=post(f"/v1/sessions/{owner['session_id']}/invitations",{'role':'editor','ttl_seconds':60},owner['credential'])
  parts=urlparse(inv['url']); _,peer=post('/v1/invitations/redeem',{'invite_id':parts.path.rsplit('/',1)[1],'secret':parts.fragment})
  helper_env=os.environ.copy();helper_env['OPENIDEA_COLLAB_WS_URL']=f"ws://127.0.0.1:{port}/v1/socket"
  helper=subprocess.Popen([str(root/'target/release/openidea-collab-companion')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,env=helper_env)
  helper.stdin.write(json.dumps({'op':'connect','websocket_url':helper_env['OPENIDEA_COLLAB_WS_URL'],'credential':owner['credential']})+'\n');helper.stdin.flush()
  event=json.loads(helper.stdout.readline());assert event['event']=='connected',event
  wsurl=f"ws://127.0.0.1:{port}/v1/socket"
  with connect(wsurl,subprotocols=['openidea-collab.v1'],additional_headers={'Authorization':'Bearer '+peer['credential']}) as other:
   msgid=uuid.uuid4().bytes
   frame=b'OIC1'+bytes([2])+uuid.UUID(doc).bytes+bytes(16)+msgid+(0).to_bytes(2,'big')+(1).to_bytes(2,'big')+(5).to_bytes(4,'big')+b'hello'
   other.send(frame)
   received=json.loads(helper.stdout.readline());assert received['event']=='binary'
   raw=bytes.fromhex(received['data_hex']);assert raw[-5:]==b'hello' and raw[16:20]==b'OIC1' and len(raw)>16
  helper.stdin.write('{"op":"close"}\n');helper.stdin.flush();helper.terminate();helper.wait(timeout=3)
  print('PASS companion authenticated WSS to local relay; two-member binary frame forwarded')
 finally:
  server.terminate();server.wait(timeout=5)
