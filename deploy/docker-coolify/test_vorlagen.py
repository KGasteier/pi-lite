#!/usr/bin/env python3
"""Offline-Vorlagenregression: kein Dockerdaemon, kein Netzwerk, keine Rootwirkung."""
import pathlib, subprocess, tempfile, unittest, os
BASE=pathlib.Path(__file__).resolve().parent
class Vorlagen(unittest.TestCase):
    def test_bash_syntax(self):
        for p in BASE.glob('*.sh'): subprocess.run(['bash','-n',str(p)],check=True)
        subprocess.run(['sh','-n',str(BASE/'image/entrypoint.sh')],check=True)
    def test_zehn(self):
        s=subprocess.check_output(['bash',str(BASE/'02-compose-erzeugen.sh'),'10'],text=True)
        for i in range(1,11):
            self.assertIn(f'127.0.0.1:{18800+i}:8787',s)
            self.assertIn(f'/slot{i:02}/home',s)
        self.assertEqual(s.count('cap_drop: [ALL]'),10)
        self.assertNotIn('privileged:',s)
        self.assertNotIn('docker.sock',s)
        self.assertNotIn('network_mode:',s)
    def test_anzahl(self):
        for n in ['0','11','x','1;id']:
            self.assertNotEqual(subprocess.run(['bash',str(BASE/'02-compose-erzeugen.sh'),n],capture_output=True).returncode,0)
    def test_plan(self):
        r=subprocess.run(['bash',str(BASE/'03-pilot-ablagen.sh')],capture_output=True,text=True,check=True)
        self.assertIn('PLAN:',r.stdout)
    def test_apply_braucht_freigabe(self):
        env=os.environ.copy();env.pop('COCOPI_CONTAINER_STORAGE_FREIGABE',None)
        self.assertNotEqual(subprocess.run(['bash',str(BASE/'03-pilot-ablagen.sh'),'--ausfuehren'],env=env,capture_output=True).returncode,0)
    def test_compose_parser(self):
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d)/'compose.yaml'
            p.write_bytes(subprocess.check_output(['bash',str(BASE/'02-compose-erzeugen.sh'),'10']))
            env=os.environ|{'PILOT_IMAGE':'example.invalid/pilot@sha256:'+'a'*64}
            subprocess.run(['docker','compose','-f',str(p),'config','--quiet'],env=env,check=True)
if __name__=='__main__': unittest.main()
