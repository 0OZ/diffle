"""Structural comparison of two JSON documents, for comparing the two servers' replies.

Ids and timestamps are renamed by order of appearance, so two servers that mint different ids
for the same thing still compare equal. Arrays of objects are matched by their key (`path`,
`id`, ...) rather than by position, so one missing file does not shift every row after it.
"""
import re

UUID = re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
TIME = re.compile(r'^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d')
KEYS = ('path', 'id', 'sha', 'name')
CLOCK_KEYS = ('createdAt', 'updatedAt', 'resolvedAt')  # epoch milliseconds minted by each server
SAMPLES_PER_FINDING = 2
SAMPLE_WIDTH = 140


def normalize(value, ids):
    if isinstance(value, dict):
        return {k: '<time>' if k in CLOCK_KEYS and isinstance(v, (int, float)) else normalize(v, ids)
                for k, v in value.items()}
    if isinstance(value, list):
        return [normalize(v, ids) for v in value]
    if isinstance(value, str):
        if UUID.match(value):
            return ids.setdefault(value, f'<id{len(ids) + 1}>')
        if TIME.match(value):
            return '<time>'
    return value


def type_of(value):
    return {type(None): 'null', bool: 'bool', int: 'number', float: 'number', str: 'string',
            list: 'array', dict: 'object'}[type(value)]


def short(value):
    text = repr(value)
    return text if len(text) <= SAMPLE_WIDTH else text[:SAMPLE_WIDTH] + '…'


def key_of(items):
    """The field that identifies each element, if every element is an object that has one."""
    if not items or not all(isinstance(x, dict) for x in items):
        return None
    return next((k for k in KEYS if all(k in x for x in items)), None)


class Findings:
    """Differences grouped by (kind, path): a count and a few examples, not ten thousand lines."""

    def __init__(self):
        self.groups = {}

    def add(self, kind, path, node, rust):
        group = self.groups.setdefault((kind, path), {'count': 0, 'samples': []})
        group['count'] += 1
        if len(group['samples']) < SAMPLES_PER_FINDING:
            group['samples'].append((short(node), short(rust)))

    def __bool__(self):
        return bool(self.groups)


def compare(node, rust, findings, path='$'):
    kn, kr = type_of(node), type_of(rust)
    if kn != kr:
        findings.add('type', path, node, rust)
    elif kn == 'object':
        for key in node.keys() - rust.keys():
            findings.add('missing in rust', f'{path}.{key}', node[key], None)
        for key in rust.keys() - node.keys():
            findings.add('extra in rust', f'{path}.{key}', None, rust[key])
        for key in node.keys() & rust.keys():
            compare(node[key], rust[key], findings, f'{path}.{key}')
    elif kn == 'array':
        compare_arrays(node, rust, findings, path + '[]')
    elif node != rust:
        findings.add('value', path, node, rust)


def compare_arrays(node, rust, findings, path):
    key = key_of(node) if key_of(node) == key_of(rust) else None
    if key is not None:
        by_node, by_rust = {x[key]: x for x in node}, {x[key]: x for x in rust}
        for k in by_node.keys() - by_rust.keys():
            findings.add('only in node', path, by_node[k][key], None)
        for k in by_rust.keys() - by_node.keys():
            findings.add('only in rust', path, None, by_rust[k][key])
        for k in by_node.keys() & by_rust.keys():
            compare(by_node[k], by_rust[k], findings, path)
        common = [x[key] for x in node if x[key] in by_rust]
        if common != [x[key] for x in rust if x[key] in by_node]:
            findings.add('order', path, common[:3], [x[key] for x in rust if x[key] in by_node][:3])
        return
    if len(node) != len(rust):
        findings.add('array length', path, len(node), len(rust))
    for a, b in zip(node, rust):
        compare(a, b, findings, path)
