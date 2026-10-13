# Port of Eq5BandGen1.getGainConfiguration/generateCommandParams (bundle.js ~12385) with M4 params (~70218)
import math, struct
P = dict(minGain=-6, maxGain=6, bassBoost=[5,5,0,0,0], loud=[-26.683,-10.813,-4.773,-4.813,-5.993],
         headroom=[5,5,4,3,2], q=[1/math.sqrt(2)]*4, freq=[0,120,400,3000,6000])
def cfg(g, bb):
    o = [x + P['bassBoost'][i] for i, x in enumerate(g)] if bb else list(g)
    s = -10*math.log10(sum(10**((x + P['loud'][i])/10) for i, x in enumerate(o)))
    r = max([x + s - P['headroom'][i] for i, x in enumerate(o)] + [0])
    c = round(10*(s - r if r > 0 else s))/10
    sh = [x + c for x in o]; d = [sh[0]] + [sh[i]-sh[i-1] for i in range(1, 5)]
    if math.trunc(100*d[0]) == 425: d[0] = 4.24
    return c, d[0], d[1:]
def params(master, shelf):
    n = [17, 257, 60*master]
    for b in range(4):
        base = 272 + 16*b
        n += [base, 11, base+1, 3*P['freq'][b+1], base+2, 60*shelf[b], base+3, 4096*P['q'][b]]
    return n + [0]
def toint16(v): v = int(v); return ((v + 0x8000) & 0xFFFF) - 0x8000   # JS DataView ToInt16 truncation
def frame(vid, cmd, payload=b''):
    return bytes([0xFF, 0x03, 0x00, len(payload)]) + struct.pack('>HH', vid, cmd) + payload
for gains, bb in (([0,0,0,0,0], False), ([3.5,2,-1.5,1.5,3], False), ([0,0,0,0,0], True)):
    ls, m, sh = cfg(gains, bb)
    p = [toint16(x) for x in params(m, sh)]
    pl = b''.join(struct.pack('>h', x) for x in p)
    y = 0x4000 | (0x1000 if bb else 0)
    print(f"gains={gains} bass={bb} -> levelShift={ls}dB master={m:.3f} shelves={[round(x,3) for x in sh]}")
    print("  INT16[] =", p)
    print("  frame EQ_UserParameterGroup_Set  =", frame(0x0495, 0x1002, pl).hex(' '), f"(payload {len(pl)} bytes)")
    print("  frame Sound_SetLevelShift        =", frame(0x0495, 0x1006, struct.pack('>b', round(10*ls))).hex(' '))
    print("  frame Sound_SetCustomEQKnobValue =", frame(0x0495, 0x1004, struct.pack('>HH', 1, y)).hex(' '))
print("get battery      :", frame(0x0495, 0x0603).hex(' '))
print("set adaptive on  :", frame(0x0495, 0x1A00, bytes([3, 1])).hex(' '))
print("set transparency :", frame(0x0495, 0x1A02, bytes([50])).hex(' '))
print("register ANC ntf :", frame(0x0495, 0x0007, bytes([13])).hex(' '))
print("get fw version   :", frame(0x0495, 0x1201).hex(' '))
