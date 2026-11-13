#!/usr/bin/env python3
"""Chrome tests for custom editor using a mocked OpenIDEA bridge, no real helper."""
import json
from pathlib import Path
from playwright.sync_api import sync_playwright
root = Path(__file__).resolve().parents[3]
uri = (root / 'openidea-collab/web/collab-editor/index.html').as_uri()
with sync_playwright() as p:
    browser = p.chromium.launch(executable_path='/usr/bin/google-chrome', headless=True, args=['--no-sandbox'])
    page = browser.new_page(viewport={'width': 900,'height':600})
    page.add_init_script('''(() => {
      let version=1, text='a😀', callback; window.sent=[]; window.edits=[];
      window.openidea={postMessage(raw){window.sent.push(JSON.parse(raw))},onMessage(fn){window.deliver=fn},
        document:{get:async()=>({version,text,readOnly:false}),onChange(fn){callback=fn},edit:async(v, edits)=>{
          if(window.forceStale) throw Error('stale'); if(v!==version) throw Error('stale');
          window.edits.push(edits[0]); text=text.slice(0,edits[0].from)+edits[0].insert+text.slice(edits[0].to); return {version:++version};}}};
    })();''')
    page.goto(uri); page.wait_for_function("document.querySelector('#status').textContent.includes('Informe')")
    page.fill('#endpoint','http://localhost:4444'); page.select_option('#mode','join'); page.fill('#invite-input', json.dumps({'version':1,'room':'room','token':'test-token'})); page.check('#recovery'); page.click('#start')
    session = page.evaluate("window.sent.find(x=>x.op==='pty-spawn').sessionId")
    def deliver(payload, sequence):
        page.evaluate('(raw) => window.deliver(raw)', json.dumps({'op':'pty-data','sessionId':session,'sequence':sequence,'data':json.dumps(payload)+'\n'}))
    deliver({'type':'ready','protocol':1},1)
    assert page.locator('#invite-input').input_value() == ''
    assert page.evaluate("JSON.parse(window.sent.find(x=>x.op==='pty-input').data).recovery") is True
    deliver({'type':'joined','id':'attach','room_id':'room','text':'a😀','pending':0,'recovery_path':'/private/copy.txt'},2)
    assert '/private/copy.txt' in page.locator('#recovery-location').inner_text()
    assert page.locator('#connect').is_hidden()
    page.locator('#editor').fill('a😃')
    page.wait_for_function('window.edits.length === 1')
    assert page.evaluate('window.edits[0]') == {'from':1,'to':3,'insert':'😃'}
    sent = page.evaluate("window.sent.filter(x=>x.op==='pty-input').map(x=>JSON.parse(x.data))")
    assert sent[-1]['type'] == 'edit' and sent[-1]['from'] == 1 and sent[-1]['to'] == 3
    deliver({'type':'edited','id':sent[-1]['id'],'text':'a😃','pending':0},3)
    deliver({'type':'connection_error','message':'update rejected: HTTP 507','pending':1},4)
    assert 'Servidor sem espaço' in page.locator('#status').inner_text()
    page.click('#leave-session', timeout=1000)
    sent = page.evaluate("window.sent.filter(x=>x.op==='pty-input').map(x=>JSON.parse(x.data))")
    assert sent[-1]['type'] == 'disconnect'
    deliver({'type':'disconnect_blocked','id':sent[-1]['id'],'pending':1},5)
    assert 'alterações pendentes' in page.locator('#status').inner_text().lower()
    assert page.locator('#editor').get_attribute('readonly') is None
    page.evaluate('window.forceStale=true')
    page.locator('#editor').fill('a😃 keep')
    page.wait_for_function("!document.querySelector('#error').hidden")
    assert page.locator('#editor').input_value() == 'a😃 keep'
    assert page.locator('#editor').get_attribute('readonly') is not None
    print('PASS: ponte simulada: UTF-16, credencial limpa, erro stale preserva texto')
    browser.close()
