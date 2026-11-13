#!/usr/bin/env python3
"""Two actual helper processes + actual server + browser custom editors with an IDE bridge mock and real OS PTYs.
No user files, network exposure or real OpenIDEA are used. Linux/Chrome required.
"""
import asyncio, json, os, pty, socket, subprocess, tempfile, urllib.request
from pathlib import Path
from playwright.async_api import async_playwright
root = Path(__file__).resolve().parents[3]
server_bin = root / 'collab-server/target/debug/collab-server'
helper_bin = root / 'collab-helper/target/debug/openidea-collab-helper'
html = (root / 'openidea-collab/web/collab-editor/index.html').as_uri()

async def main():
    state_temp = tempfile.TemporaryDirectory(prefix='collab-editor-recovery-')
    sock = socket.socket(); sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]; sock.close()
    env = {**os.environ, 'COLLAB_BIND':'127.0.0.1', 'COLLAB_PORT':str(port), 'COLLAB_API':'realtime'}
    server = subprocess.Popen([str(server_bin)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    helpers = []; readers = []
    try:
        base = f'http://127.0.0.1:{port}'
        for _ in range(100):
            try:
                with urllib.request.urlopen(base + '/health', timeout=.2): break
            except OSError: await asyncio.sleep(.05)
        else: raise RuntimeError('server did not start')
        invite = None
        async with async_playwright() as p:
            browser = await p.chromium.launch(executable_path='/usr/bin/google-chrome', headless=True, args=['--no-sandbox'])
            context = await browser.new_context(viewport={'width': 900, 'height': 600})
            pages = []
            for index in range(2):
                page = await context.new_page()
                holder = {'child':None, 'seq':0, 'master':None, 'ack':asyncio.Event()}
                async def bridge(source, raw, holder=holder, page=page):
                    msg = json.loads(raw)
                    if msg['op'] == 'pty-spawn':
                        master, slave = pty.openpty()
                        os.set_blocking(master, False)
                        child = await asyncio.create_subprocess_exec(str(helper_bin), stdin=slave, stdout=slave, stderr=asyncio.subprocess.DEVNULL, env={**os.environ, 'XDG_STATE_HOME':state_temp.name})
                        os.close(slave); holder['master'] = master; holder['child'] = child; helpers.append(child)
                        queue = asyncio.Queue()
                        loop = asyncio.get_running_loop()
                        def available():
                            try: data = os.read(master, 4096)
                            except (BlockingIOError, OSError): return
                            if data: queue.put_nowait(data)
                        loop.add_reader(master, available)
                        async def pump():
                            try:
                                while True:
                                    data = await queue.get()
                                    holder['seq'] += 1; holder['ack'].clear()
                                    event = json.dumps({'op':'pty-data','sessionId':msg['sessionId'],'data':data.decode(),'sequence':holder['seq']})
                                    await page.evaluate('(raw) => window.__deliver(raw)', event)
                                    await asyncio.wait_for(holder['ack'].wait(), 3)
                            finally:
                                loop.remove_reader(master); os.close(master)
                        readers.append(asyncio.create_task(pump()))
                    elif msg['op'] == 'pty-input':
                        os.write(holder['master'], msg['data'].encode())
                    elif msg['op'] == 'pty-ack':
                        if msg['sequence'] == holder['seq']: holder['ack'].set()
                    elif msg['op'] == 'pty-kill' and holder['child']:
                        holder['child'].terminate()
                await page.expose_binding('__host_send', bridge)
                await page.add_init_script('''(() => {
                  let text='ab', version=1, callback=()=>{};
                  window.__doc=()=>({text,version});
                  window.openidea={postMessage:raw=>window.__host_send(raw),onMessage:fn=>window.__deliver=fn,
                    document:{get:async()=>({text,version,readOnly:false,name:'test.collabtest'}),onChange:fn=>callback=fn,
                      edit:async(v, edits)=>{if(v!==version) throw Error('stale'); for(const e of edits){text=text.slice(0,e.from)+e.insert+text.slice(e.to)}; return {version:++version};}}};
                })();''')
                await page.goto(html)
                await page.wait_for_function("document.querySelector('#status').textContent.includes('Informe')")
                await page.fill('#endpoint',base)
                if index == 1:
                    await page.select_option('#mode','join'); await page.fill('#invite-input',invite)
                else:
                    await page.check('#recovery')
                await page.click('#start')
                await page.wait_for_function("document.querySelector('#status').textContent.includes('Conectado')",timeout=5000)
                if index == 0:
                    await page.click('#create-invite')
                    await page.wait_for_function("document.querySelector('#invite-output').value.length > 0",timeout=5000)
                    invite = await page.locator('#invite-output').input_value()
                pages.append(page)
            await pages[0].click('#clear-invite')
            await asyncio.gather(pages[0].locator('#editor').fill('aXb'), pages[1].locator('#editor').fill('aYb'))
            for page in pages:
                await page.wait_for_function("(() => { const t=document.querySelector('#editor').value; return t.includes('X') && t.includes('Y'); })()",timeout=7000)
            values = [await page.locator('#editor').input_value() for page in pages]
            docs = [await page.evaluate('window.__doc()') for page in pages]
            assert values[0] == values[1] == docs[0]['text'] == docs[1]['text'], (values, docs)
            backups = list((Path(state_temp.name)/'openidea-collab-recovery').glob('*.txt'))
            assert len(backups) == 1 and backups[0].read_text() == values[0]
            assert backups[0].stat().st_mode & 0o777 == 0o600
            await pages[0].screenshot(path=str(root / 'tasks/provas/collab-real/editor-experimental.png'))
            print('PASS: dois editores no Chrome + ponte simulada + dois auxiliares e PTYs reais + servidor real convergiram')
            await browser.close()
    finally:
        for task in readers: task.cancel()
        for helper in helpers:
            if helper.returncode is None:
                helper.terminate()
                try: await asyncio.wait_for(helper.wait(), 3)
                except asyncio.TimeoutError: helper.kill(); await helper.wait()
        server.terminate()
        try: server.wait(timeout=3)
        except subprocess.TimeoutExpired: server.kill(); server.wait()
        state_temp.cleanup()

asyncio.run(main())
