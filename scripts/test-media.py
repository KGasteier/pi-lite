import importlib.util, tempfile, json, os
from pathlib import Path
spec=importlib.util.spec_from_file_location('worker','scripts/media-worker.py')
w=importlib.util.module_from_spec(spec);spec.loader.exec_module(w)
with tempfile.TemporaryDirectory() as d:
    w.CONFIG=Path(d)/'routes.json'
    assert w.public_routes()==[]
    w.CONFIG.write_text(json.dumps({'routes':[{'id':'test','adapter':'openrouter','model':'test/model','modalities':['audio'],'verified':False}]}))
    assert w.public_routes()==[]
    w.CONFIG.write_text(json.dumps({'routes':[{'id':'test','adapter':'openrouter','model':'test/model','modalities':['audio'],'verified':True,'keyEnv':'PI_LITE_TEST_MEDIA_KEY'}]}))
    os.environ['PI_LITE_TEST_MEDIA_KEY']='synthetic-secret'
    assert w.public_routes()[0]['available']
    assert 'synthetic-secret' not in json.dumps(w.public_routes())
    p=Path(d)/'test.mp4';p.write_bytes(b'synthetic')
    try: w.analyze({'paths':[str(p)],'question':'test'})
    except ValueError as e: assert 'Keine' in str(e)
    else: raise AssertionError('Audio-only route must reject video')
    try: w.analyze({'paths':[str(p)],'question':'test','route':'missing'})
    except ValueError: pass
    else: raise AssertionError('Never silently replace explicit route')
    del os.environ['PI_LITE_TEST_MEDIA_KEY']
print('Media routes: opt-in, credentials redaction, capabilities and manual route rejection passed; no network calls')
