//! NDataObject behaves as a dict, NDataArray as a list, NDataBytes as bytes.
//! Each test is a Python command body of `assert`s; a failure reports the
//! body line and traceback.
#![cfg(feature = "python_runtime")]

use flowlang::datastore::DataStore;
use flowlang::pycmd::{run, PyCall};
use ndata::dataobject::DataObject;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;

static INIT: Once = Once::new();
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn check(body: &str) {
  INIT.call_once(|| {
    let dir = std::env::temp_dir().join(format!("flowlang_ndata_protocols_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("data")).unwrap();
    DataStore::init(dir.join("data").to_str().unwrap());
  });
  let base = std::env::temp_dir().join(format!("flowlang_ndata_protocols_{}", std::process::id())).join("src");
  let lib = format!("proto{}", SEQ.fetch_add(1, Ordering::SeqCst));
  let dir = base.join(&lib).join("ctl");
  std::fs::create_dir_all(&dir).unwrap();
  let mut src = String::from("import json\nimport collections.abc as abc\nimport ndata_py\nfrom ndata_py import NDataObject, NDataArray, NDataBytes\n\ndef execute(args):\n    return cmd()\n\ndef cmd():\n");
  for line in body.lines() { src.push_str(&format!("    {}\n", line)); }
  src.push_str("    return 'ok'\n");
  let file = dir.join("cmd.py");
  std::fs::write(&file, src).unwrap();
  let call = PyCall {
    label: format!("{}:ctl:cmd", lib), base_dir: base, module: format!("{}.ctl.cmd", lib),
    file, func: "cmd".to_string(), returntype: "Any".to_string(), args: DataObject::new(),
  };
  match run(&call) {
    Ok(env) => assert_eq!(env.get_string("a"), "ok"),
    Err(e) => panic!("{}", e),
  }
}

#[test]
fn object_reads_like_a_dict() {
  check(r#"
o = NDataObject()
o.update({'a': 1, 'b': [1, 2]}, c='x')
assert len(o) == 3 and 'a' in o and 'z' not in o and 5 not in o
assert sorted(o) == ['a', 'b', 'c']
assert sorted(o.keys()) == ['a', 'b', 'c']
assert sorted(o.items(), key=lambda kv: kv[0])[0] == ('a', 1)
assert 1 in o.values()
assert o.get('a') == 1 and o.get('z') is None and o.get('z', 7) == 7 and o.get(5, 8) == 8
assert o['b'] == [1, 2]
try:
    o['z']
    assert False
except KeyError as e:
    assert e.args == ('z',)
"#);
}

#[test]
fn object_views_are_live() {
  check(r#"
o = NDataObject()
ks = o.keys()
o['k'] = 1
assert list(ks) == ['k'] and 'k' in ks
"#);
}

#[test]
fn object_writes_like_a_dict() {
  check(r#"
o = NDataObject()
assert o.setdefault('a', 1) == 1 and o.setdefault('a', 2) == 1
assert o.setdefault('n') is None and o['n'] is None
assert o.pop('a') == 1 and 'a' not in o
assert o.pop('a', 'gone') == 'gone'
try:
    o.pop('a')
    assert False
except KeyError:
    pass
o['p'] = {'q': 1}
k, v = o.popitem()
assert k in ('n', 'p')
o.update([('x', 1), ('y', 2)])
assert o['x'] == 1 and o['y'] == 2
del o['x']
assert 'x' not in o
o.clear()
assert len(o) == 0 and not o
try:
    o.update({1: 'a'})
    assert False
except TypeError as e:
    assert 'keys must be str' in str(e)
"#);
}

#[test]
fn object_equality_and_copies() {
  check(r#"
o = NDataObject()
o.update({'a': 1, 'n': {'m': [1, b'x']}})
assert o == {'a': 1, 'n': {'m': [1, b'x']}}
assert {'a': 1, 'n': {'m': [1, b'x']}} == o
assert o != {'a': 2}
assert o != [1] and not (o == 'a')
p = NDataObject()
p.update(o)
assert o == p
c = o.copy()
c['a'] = 2
assert o['a'] == 1
c['n']['m'].append(3)
assert o['n']['m'] == [1, b'x', 3]      # shallow: nested containers shared
d = o.to_dict()
assert type(d) is dict and type(d['n']) is dict and type(d['n']['m']) is list and d['n']['m'][1] == b'x'
try:
    hash(o)
    assert False
except TypeError:
    pass
"#);
}

#[test]
fn abcs_json_and_repr() {
  check(r#"
o = NDataObject()
o['a'] = [1, {'b': None}]
assert isinstance(o, abc.Mapping) and isinstance(o, abc.MutableMapping)
assert isinstance(o['a'], abc.Sequence) and isinstance(o['a'], abc.MutableSequence)
assert not isinstance(o, dict)
assert json.loads(json.dumps(o, default=ndata_py.to_py)) == {'a': [1, {'b': None}]}
assert json.loads(o.to_json()) == {'a': [1, {'b': None}]}
assert repr(o) == "NDataObject({'a': [1, {'b': None}]})"
assert repr(o['a']) == "NDataArray([1, {'b': None}])"
assert ndata_py.to_py({'x': o, 't': (o['a'],)}) == {'x': {'a': [1, {'b': None}]}, 't': ([1, {'b': None}],)}
assert dict(o) == {'a': [1, {'b': None}]}
"#);
}

#[test]
fn array_is_a_list() {
  check(r#"
a = NDataArray()
a.extend([1, 2, 3, 2])
assert a == [1, 2, 3, 2] and [1, 2, 3, 2] == a and a != [1] and a != (1, 2, 3, 2)
assert 3 in a and 9 not in a
assert a.index(2) == 1 and a.index(2, 2) == 3 and a.count(2) == 2
a.remove(2)
assert a == [1, 3, 2]
a.insert(0, 'first')
a.insert(-1, 'before-last')
a.insert(100, 'end')
assert a == ['first', 1, 3, 'before-last', 2, 'end']
assert a[::-1] == ['end', 2, 'before-last', 3, 1, 'first']
assert a[1:4] == [1, 3, 'before-last'] and a[::2] == ['first', 3, 2] and a[-2:] == [2, 'end']
a.reverse()
assert a[0] == 'end' and a[-1] == 'first'
b = a.copy()
b.append('x')
assert len(a) == 6 and len(b) == 7
a += ['y']
assert a[-1] == 'y'
assert list(reversed(NDataArray.from_json('[1,2]'))) == [2, 1]
assert type(a.to_list()) is list
a.clear()
assert len(a) == 0 and not a
try:
    a.index(1)
    assert False
except ValueError:
    pass
"#);
}

#[test]
fn array_insert_keeps_containers() {
  // insert lifts the tail off as handles and puts it back
  check(r#"
a = NDataArray()
a.append({'k': 1})
a.append([2])
a.insert(0, {'new': True})
assert a == [{'new': True}, {'k': 1}, [2]]
a[1]['k'] = 5
assert a[1] == {'k': 5}
"#);
}

#[test]
fn bytes_are_bytes() {
  check(r#"
b = NDataBytes.from_bytes(b'hi')
assert bytes(b) == b'hi' and b == b'hi' and b == bytearray(b'hi') and b != b'ho'
assert b == NDataBytes.from_bytes(b'hi')
"#);
}

#[test]
fn array_extend_with_itself() {
  check(r#"
a = NDataArray()
a.extend([1, {'k': 2}])
a.extend(a)
assert a == [1, {'k': 2}, 1, {'k': 2}]
a += a
assert len(a) == 8
"#);
}

#[test]
fn stubs_match_the_runtime_modules() {
  // src/ndata_py.pyi and src/flowlang.pyi (which flowb ships next to
  // generated commands) declare exactly what the runtime modules provide.
  let dir = env!("CARGO_MANIFEST_DIR");
  check(&format!(r#"
import ast, flowlang
import collections.abc as cabc
problems = []
for modname, mod in (('ndata_py', ndata_py), ('flowlang', flowlang)):
    tree = ast.parse(open('{dir}/src/%s.pyi' % modname).read())
    top = set()
    for node in tree.body:
        if isinstance(node, ast.FunctionDef):
            top.add(node.name)
        elif isinstance(node, ast.AnnAssign):
            top.add(node.target.id)
        elif isinstance(node, ast.ClassDef):
            top.add(node.name)
            cls = getattr(mod, node.name, None)
            if cls is None:
                continue
            declared = {{n.name for n in node.body if isinstance(n, ast.FunctionDef)}}
            declared |= {{n.target.id for n in node.body if isinstance(n, ast.AnnAssign)}}
            inherited = set()
            for base in node.bases:
                b = base.value if isinstance(base, ast.Subscript) else base
                if isinstance(b, ast.Name) and hasattr(cabc, b.id):
                    inherited |= set(dir(getattr(cabc, b.id)))
                elif isinstance(b, ast.Name) and b.id == 'Exception':
                    inherited |= set(dir(Exception))
            for n in declared - {{'__init__', '__hash__'}}:
                if not hasattr(cls, n):
                    problems.append('%s.%s.%s: in the stub, not at runtime' % (modname, node.name, n))
            if cls.__hash__ is not None and '__hash__' in declared:
                problems.append('%s.%s is hashable at runtime' % (modname, node.name))
            for n in dir(cls):
                if not n.startswith('_') and n not in declared and n not in inherited:
                    problems.append('%s.%s.%s: at runtime, not in the stub' % (modname, node.name, n))
    for n in top:
        if not n.startswith('_') and not hasattr(mod, n):
            problems.append('%s.%s: in the stub, not at runtime' % (modname, n))
    for n in dir(mod):
        if not n.startswith('_') and n not in top:
            problems.append('%s.%s: at runtime, not in the stub' % (modname, n))
assert not problems, '\n'.join(sorted(problems))
"#, dir = dir));
}
