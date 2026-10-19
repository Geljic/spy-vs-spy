#!/usr/bin/env python3
"""Print the game's behaviour scripts as readable pseudo-code, from vcs2json --types JSON.

usage: script_print.py <file.json> [more.json ...] <query>
  query  dialog:DG_MainMenu   the dialog's events and every element's events
         sm:Name              a StateMachine (states, handlers, local functions)
         fn:Name              a global Function
         game                 the .gam Game event scripts
         find:ClassName       every object of that class with its owner label (first 40)
The first JSON is the one queried; the rest (e.g. the .gam) only resolve references and enum names.
JSON files written by vcs2json hold raw Latin-1 bytes; they are read as Latin-1.
Names of statements are ours (notes/script-vm-plan.md, notes/ui.md); unknown classes print as Class{fields}.
"""
import json
import sys

refs = {}     # asset id -> object (from every AssetReference with first=true)
enums = {}    # enumeration id -> (label, {entry id: entry label})


def load(path):
    return json.loads(open(path, 'rb').read().decode('latin-1'))


def index(o):
    if isinstance(o, dict):
        if 'id' in o and 'type' in o and o.get('first') and isinstance(o.get('asset'), dict):
            refs[o['id']] = o['asset']
            a = o['asset']
            if a.get('__class') == 'Enumeration':
                entries = {}
                for ch in items(a.get('children')):
                    e = ch.get('asset') if isinstance(ch, dict) else None
                    if e and e.get('__class') == 'EnumerationEntry':
                        entries[e['id']] = e['label']['buffer']
                enums[o['id']] = (a['label']['buffer'], entries)
        for v in o.values():
            index(v)
    elif isinstance(o, list):
        for v in o:
            index(v)


def lab(r):
    if not isinstance(r, dict):
        return '?'
    if r.get('type', 0) < 0:
        return '-'
    if 'label' in r and isinstance(r['label'], dict):
        return r['label'].get('buffer') or r.get('id', '?')
    return r.get('id', '?')


def enum_entry(enumref, eid):
    if not isinstance(enumref, dict) or enumref.get('type', -1) < 0:
        return str(eid)
    name, entries = enums.get(enumref.get('id'), (lab(enumref), {}))
    return '%s.%s' % (name, entries.get(eid, eid))


def v421(o):
    if not isinstance(o, dict):
        return '?'
    s = enum_entry(o.get('enumeration'), o.get('enumerationId'))
    if o.get('f_0x10', {}).get('type', -1) >= 0:
        s += '[' + expr(o['f_0x10']) + ']'
    return s


def items(x):
    """A list field: {count, list}, a bare list, or an Expression {children}."""
    if isinstance(x, dict) and x.get('__class') == 'Expression':
        return items(x.get('children'))
    if isinstance(x, list):
        return x
    if isinstance(x, dict):
        return x.get('list', []) or []
    return []


def obj(a):
    """AssetFromType {type, asset} -> object or None."""
    if not isinstance(a, dict) or a.get('type', -1) < 0:
        return None
    return a.get('asset', a if '__class' in a else None)


# BinaryOperationExpression ops, from its evaluate function (0x1bb958)
OPS = {0: '&&', 1: '||', 2: '<', 3: '<=', 4: '==', 5: '>=', 6: '>', 7: '!=', 8: '+', 9: '-', 10: '*', 11: '/', 12: 'min', 13: 'max', 14: 'pow', 15: 'fmod', 16: 'angdiff'}


def entity(e):
    if not isinstance(e, dict):
        return '?'
    x = obj((e.get('f_5') or {}).get('object'))
    s = expr_obj(x) if x else 'self'
    v = e.get('v421')
    if v and v.get('enumeration', {}).get('type', -1) >= 0:
        s += '.' + v421(v)
    return s


def expr(a):
    return expr_obj(obj(a)) if a else '-'


def expr_obj(o):
    if o is None:
        return '-'
    c = o.get('__class')
    if c == 'FloatLiteralExpression':
        v = o['value']
        return str(int(v)) if v == int(v) else repr(v)
    if c == 'BinaryOperationExpression':
        ops = items(o.get('operands'))
        return '(' + (' %s ' % OPS.get(o['operation'], 'op%d' % o['operation'])).join(expr(x) for x in ops) + ')'
    if c == 'UnaryOperationExpression':
        return '%s(%s)' % ({0: '!', 1: 'abs', 2: 'floor', 3: 'ceil', 4: 'round', 5: 'sqrt', 6: 'sin', 7: 'cos', 8: 'inc', 9: 'dec', 10: '-'}.get(o['operation'], 'u%d' % o['operation']), expr(o['operand']))
    if c == 'ExecutionContextValueExpression':
        return 'ctx(%s,%s)' % (o['source'], o['index'])
    if c == 'V358':
        return 'ext:' + lab(o['f_1'])
    if c == 'V157':
        return 'var:' + lab(o.get('f_1'))
    if c == 'V158':
        return 'const:' + lab(o.get('f_1'))
    if c in ('V210', 'V212'):
        return '%s.%s' % (entity(o.get('f_0x08')), v421(o.get('f_1')))
    if c == 'V421':
        return v421(o)
    if c == 'AssetElementIndexExpression':
        el = o.get('element', {})
        return enum_entry(el.get('enumeration'), el.get('enumerationId')) if 'enumeration' in el else 'elem' + json.dumps(el)[:60]
    if c == 'V436':
        return 'gs:%s' % json.dumps({k: v for k, v in o.items() if k.startswith('f_')})[:120]
    return short(o)


def short(o, n=160):
    c = o.get('__class', '?')
    parts = []
    for k, v in o.items():
        if k in ('__class', '__id', 't1'):
            continue
        if isinstance(v, dict) and 'label' in v and isinstance(v['label'], dict):
            parts.append('%s=%s' % (k, lab(v)))
        elif isinstance(v, dict) and v.get('__class') == 'V421':
            parts.append('%s=%s' % (k, v421(v)))
        elif isinstance(v, dict) and 'v421' in v:
            parts.append('%s=%s' % (k, v421(v['v421'])))
        elif isinstance(v, dict) and 'f_5' in v:
            parts.append('%s=%s' % (k, entity(v)))
        elif isinstance(v, dict) and v.get('type') is not None and 'asset' in v and isinstance(v['asset'], dict):
            parts.append('%s=%s' % (k, expr(v)))
        elif isinstance(v, (int, float, bool, str)):
            parts.append('%s=%s' % (k, v))
        elif (isinstance(v, dict) and 'list' in v) or isinstance(v, list):
            parts.append('%s=[%s]' % (k, ', '.join(expr(x) if isinstance(x, dict) and 'type' in x else '?' for x in items(v))))
    s = '%s{%s}' % (c, ', '.join(parts))
    return s if len(s) <= n else s[:n] + '…}'


def stmts(lst, d):
    for a in items(lst):
        stmt(obj(a), d)


def stmt(o, d):
    pad = '  ' * d
    if o is None:
        return
    c = o.get('__class')
    if c in ('Script',):
        stmts(o['code'].get('children'), d)
    elif c == 'Expression':
        stmts(o.get('children'), d)
    elif c == 'IfStatement':
        print(pad + 'if %s:' % expr(o['condition']))
        stmts(o.get('trueExpr'), d + 1)
        if items(o.get('falseExpr')):
            print(pad + 'else:')
            stmts(o['falseExpr'], d + 1)
    elif c == 'AssignValueStatement':
        op = {0: '=', 1: '+=', 2: '-=', 3: '*=', 4: '/='}.get(o['operation'], 'op%d=' % o['operation'])
        print(pad + '%s %s %s' % (expr(o['destination']), op, expr(o['source'])))
    elif c == 'CallFunctionStatement':
        args = ', '.join(expr(x) for x in items(o.get('arguments')))
        print(pad + 'call %s(%s)%s' % (lab(o['function']), args, (' -> ' + expr(o['f_0x14'])) if obj(o.get('f_0x14')) else ''))
    elif c == 'ReturnStatement':
        print(pad + 'return %s' % expr(o.get('value')))
    elif c == 'SwitchStatement':
        print(pad + 'switch %s:' % expr(o['target']))
        for a in items(o.get('cases')):
            cs = obj(a)
            if cs:
                print(pad + '  case %s:' % ', '.join(expr(x) for x in items(cs.get('matchValues'))))
                stmts(cs.get('body'), d + 2)
        if items(o.get('defaultBody')):
            print(pad + '  default:')
            stmts(o['defaultBody'], d + 2)
    elif c == 'WhileStatement':
        print(pad + 'while %s:' % expr(o['condition']))
        stmts(o.get('body'), d + 1)
    elif c == 'V138':
        sel = o.get('f_0x40') or {}
        fb = (sel.get('fallback') or {}).get('asset') or {}
        tgt = lab(fb) if fb else expr((sel.get('expression') or {}).get('object'))
        fl = o.get('f_0x54', 0)
        kind = 'close' if fl & 0x80 else ('replace' if fl & 1 else 'show')
        print(pad + 'dialog.%s %s flags=0x%x dur=%s slide=%s' % (kind, tgt or '(current)', fl, expr(o.get('f_0x5c')), o.get('f_0x64')))
    elif c == 'V194':
        print(pad + 'goto state #%d %s' % (o.get('f_0x0c', 0) & 0xffff, short(o, 200)))
    elif c == 'V201':
        args = ', '.join(expr(x) for x in items(o.get('f_0x10')))
        print(pad + 'call local #%d(%s)' % (o.get('f_0x0c', 0) & 0xffff, args))
    elif c == 'SetUIFocusStatement':
        print(pad + 'focus %s' % short(o.get('target') or {}, 200))
    elif c == 'ChangeMapStatement':
        print(pad + 'CHANGE MAP ' + short(o, 300))
    elif c == 'V192':
        args = ', '.join(expr(x) for x in items(o.get('f_0x4c')))
        tgt = o.get('f_2', {})
        msg = v421(tgt['v421']) if 'v421' in tgt else lab(tgt)
        print(pad + 'send %s -> %s(%s)' % (msg, entity(o.get('f_1')), args))
    else:
        print(pad + short(o, 400))


def print_events(owner, events, d):
    for a in items(events):
        ev = obj(a)
        if not ev:
            continue
        if ev.get('__class') == 'OnEnumerationEvent':
            print('  ' * d + 'on %s [%s]:' % (enum_entry(ev['enumeration'], ev['enumerationId']), owner))
            if ev.get('f_0x0c') == 1:   # handler is a Function (f_0x10), not an inline script
                fn = ev.get('f_0x10', {})
                f = fn.get('asset') if fn.get('first') else refs.get(fn.get('id'))
                print('  ' * (d + 1) + '(function %s)' % lab(fn))
                if f:
                    stmt(f.get('script'), d + 1)
            else:
                s = obj(ev.get('script')) if 'type' in ev.get('script', {}) else ev.get('script')
                stmt(s, d + 1)
        else:
            print('  ' * d + short(ev))


def find_named(o, cls, name):
    if isinstance(o, dict):
        if o.get('__class') == cls and isinstance(o.get('label'), dict) and o['label'].get('buffer') == name:
            return o
        for v in o.values():
            r = find_named(v, cls, name)
            if r:
                return r
    elif isinstance(o, list):
        for v in o:
            r = find_named(v, cls, name)
            if r:
                return r


def find_sm(o, name):
    if isinstance(o, dict):
        if o.get('__class') == 'StateMachine' and o.get('label', {}).get('buffer') == name and 'f_0x4c' in o:
            return o
        for v in o.values():
            r = find_sm(v, name)
            if r:
                return r
    elif isinstance(o, list):
        for v in o:
            r = find_sm(v, name)
            if r:
                return r


def ui_events(e, d):
    if not isinstance(e, dict):
        return
    if e.get('f_0x60', {}).get('count'):
        print_events(e['label']['buffer'], e['f_0x60'], d)
    for cell in e.get('cells', []) or []:
        for a in [cell.get('firstElement')] + (cell.get('list') or []):
            x = obj(a)
            if x:
                ui_events(x, d)


def main():
    files, query = sys.argv[1:-1], sys.argv[-1]
    docs = [load(f) for f in files]
    for doc in docs:
        index(doc)
    root = docs[0]
    kind, _, name = query.partition(':')
    if kind == 'dialog':
        dg = find_named(root, 'Dialog', name)
        print_events(name, dg.get('events'), 0)
        ui_events(dg['grid'], 0)
    elif kind == 'fn':
        f = find_named(root, 'Function', name)
        stmt(f['script'], 1)
    elif kind == 'sm':
        sm = find_sm(root, name)
        print('StateMachine %s' % name)
        for a in items(sm.get('f_0x5c')):          # machine-level handlers (any state)
            h = obj(a)
            if h:
                print('  on %s (any state):' % enum_entry(h.get('f_0x04'), h.get('f_0x08')))
                stmt(obj(h.get('f_0x18')) if 'type' in (h.get('f_0x18') or {}) else h.get('f_0x18'), 2)
        for i, a in enumerate(items(sm.get('f_0x4c'))):   # states
            st = obj(a)
            if st:
                print('  state %d %s:' % (i, st['label']['buffer']))
                for b in items(st.get('f_0x28')):
                    h = obj(b)
                    if h:
                        print('    on %s:' % enum_entry(h.get('f_0x04'), h.get('f_0x08')))
                        stmt(obj(h.get('f_0x18')) if 'type' in (h.get('f_0x18') or {}) else h.get('f_0x18'), 3)
        for i, a in enumerate(items(sm.get('f_0x54'))):   # local functions
            fn = obj(a)
            if fn:
                print('  function %d %s:' % (i, fn['label']['buffer']))
                stmt(obj(fn.get('f_0x1c')) if 'type' in (fn.get('f_0x1c') or {}) else fn.get('f_0x1c'), 2)
    elif kind == 'find':
        n = 0
        def walk(o, owner):
            nonlocal n
            if isinstance(o, dict):
                if isinstance(o.get('label'), dict) and o.get('__class'):
                    owner = '%s %s' % (o['__class'], o['label']['buffer'])
                if o.get('__class') == name and n < 40:
                    n += 1
                    print('[%s] %s' % (owner, short(o, 300)))
                for v in o.values():
                    walk(v, owner)
            elif isinstance(o, list):
                for v in o:
                    walk(v, owner)
        walk(root, '')
    elif kind == 'game':
        g = root['content']['asset']
        for k, v in g.items():
            if isinstance(v, dict) and v.get('__class') == 'Script':
                print('%s:' % k)
                stmt(v, 1)


if __name__ == '__main__':
    main()
