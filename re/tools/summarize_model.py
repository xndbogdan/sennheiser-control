import json, sys
d = json.load(open(sys.argv[1], encoding='utf-8'))

def params(ps):
    out = []
    for p in ps or []:
        s = f"{p.get('type')}:{p.get('name')}"
        if p.get('repitive'): s += '[]'
        if 'enum' in p: s += '{' + ','.join(f"{e.get('value', e.get('id'))}={e.get('name')}" for e in p['enum'][:12]) + '}'
        for k in ('min', 'max', 'unit', 'length', 'size'):
            if k in p: s += f" {k}={p[k]}"
        out.append(s)
    return ', '.join(out)

def gv(x):
    g = x.get('gaiaV3') or {}
    return f"vid={g.get('vendor_ID')} feat={g.get('GAIA_feature_ID','')} cmd={g.get('GAIA_command_ID')} ntf={g.get('GAIA_notification_ID','')}".strip()

for p in d['properties']:
    print(f"## {p['name']}  value={json.dumps(p.get('Value'))}")
    for op in ('Get', 'Set', 'Event', 'Notification'):
        if op not in p: continue
        for sub, x in p[op].items():
            if not isinstance(x, dict): continue
            print(f"   {op}.{sub}: {x.get('PI_command_name')} [{gv(x)}] ({params(x.get('parameter'))})")
            t = x.get('tunnel')
            if t: print(f"      tunnel {t.get('PI_command_name')} [{gv(t)}] ({params(t.get('parameter'))})")
            other = {k: v for k, v in x.items() if k not in ('PI_command_name', 'gaiaV3', 'parameter', 'tunnel')}
            if other: print(f"      extra: {json.dumps(other)[:300]}")
