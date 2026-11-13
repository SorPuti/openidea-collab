#!/usr/bin/env python3
"""Renderiza o HTML do painel atual num browser isolado com host OpenIDEA simulado."""
from pathlib import Path
from playwright.sync_api import sync_playwright
import tempfile
root = Path(__file__).resolve().parents[3]
template = (root / 'openidea-collab/src/page.html').read_text()
substitutions = {'__SERVER_URL_JSON__':'"http://localhost:4444"','__SERVER_URL__':'http://localhost:4444', '__IS_DEV__':'false', '__VERSION__':'0.2.0', '__BUILD_TIME__':'teste', '__BUILD_MODE__':'RELEASE', '__INIT_NAME__':'', '__INIT_GITHUB__':''}
for key, value in substitutions.items(): template = template.replace(key, value)
with tempfile.TemporaryDirectory(prefix='collab-ui-test-') as temp:
    path = Path(temp) / 'panel.html'
    path.write_text('<!doctype html><html lang="pt-BR"><body>' + template + '</body></html>')
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(executable_path='/usr/bin/google-chrome', headless=True, args=['--no-sandbox'])
        page = browser.new_page(viewport={'width': 360, 'height': 760})
        page.add_init_script('''window.sent=[]; window.openidea={postMessage(raw){window.sent.push(JSON.parse(raw))},onMessage(fn){window.deliver=fn}};''')
        page.goto(path.as_uri())
        assert page.get_by_label('Seu nome').count() == 1
        assert page.get_by_label('Código da sala', exact=True).count() == 1
        page.click('#btn-create')
        assert page.locator('#err').inner_text() == 'Informe seu nome.'
        page.fill('#inp-name','Ana')
        page.click('#btn-create')
        assert page.evaluate("window.sent.some(m => m.type === 'create-room' && m.name === 'Ana')")
        assert page.locator('#loading').is_visible()
        page.evaluate('''window.deliver(JSON.stringify({type:'state',state:{room_id:'ROOM_TEST',sync_active:true,peers:[],server_url:'http://localhost:4444'}}))''')
        assert page.locator('#room-id').inner_text() == 'ROOM_TEST'
        assert page.locator('#app').is_visible()
        assert page.locator('#sync-label').inner_text() == 'Comunicação ativa'
        assert page.get_by_label('Copiar código da sala').count() == 1
        page.screenshot(path=str(root / 'tasks/provas/collab-real/painel-atual.png'),full_page=True)
        page.click('#btn-leave')
        assert page.evaluate("window.sent.some(m => m.type === 'leave-room')")
        print('PASS: painel real em Chrome com ponte simulada: validação, criar, estado e sair')
        browser.close()
