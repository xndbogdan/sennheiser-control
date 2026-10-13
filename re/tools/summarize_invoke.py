import json, sys
for path in sys.argv[1:]:
    d = json.load(open(path, encoding='utf-8'))
    print("######", path.split('/')[-1])
    for p in d['properties']:
        if 'Invocation' not in p and 'Result' not in p: continue
        print(f"## {p['name']}")
        for sec in ('Invocation', 'Result'):
            x = p.get(sec) or {}
            for k, v in x.items():
                if isinstance(v, dict) and 'PI_command_name' in v:
                    g = v.get('gaiaV3', {})
                    ps = ', '.join(f"{q.get('type')}:{q.get('name')}{'[]' if q.get('repitive') else ''}" + (f" ({q['description'][:150]})" if q.get('description') not in (None,'','no description') else '') for q in v.get('parameter', []))
                    print(f"   {sec}.{k}: {v['PI_command_name']} vid={g.get('vendor_ID')} cmd={g.get('GAIA_command_ID')} ntf={g.get('GAIA_notification_ID','')} err={g.get('GAIA_error_ID','')} ({ps})")
            for k in ('Params', 'Additional_Values'):
                if x.get(k): print(f"   {sec}.{k}: {json.dumps(x[k])[:300]}")
        if 'Additional_Values' in p: print("   Additional_Values:", p['Additional_Values'])
