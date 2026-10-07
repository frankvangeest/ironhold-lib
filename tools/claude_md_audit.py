#!/usr/bin/env python3
"""Audit for the split of crates/ironhold_core/src/CLAUDE.md (planning/features/core_claude_md_split.md).

Run from the repo root. Exit 0 = no problems, 1 = problems, 2 = tool error.

SCHEME
  Block ID     `b:<first line>` of a map row, in the frozen base file (e.g. `b:92`). Rows that the map splits
               across destinations carry sub-IDs (`b:108.rule`, `b:108.ref`); the Safety check applies to the
               `.rule` part. The sidecar `planning/investigations/core_claude_md_split_blocks.json` holds one
               record per block: id, lines, title, type, safety (Y/N), dest (canonical), also (extra
               placements; optional sub_dest {sub: dest} when a split block's parts live in different places), governs (repo-relative globs a Safety=Y rule must be loaded for), subs, text_sha1
               (hash of the block's text in the frozen base), review (dest label was ambiguous), notes.
  Anchor       `<!-- b:92 -->` written immediately before the block's text in its destination. HTML block
               comments are stripped from loaded context (Phase 0 spike), so anchors cost no tokens.
  Destinations PARENT crates/ironhold_core/src/CLAUDE.md, CAP .../capabilities/CLAUDE.md, SM
               .../runtime/scene_manager/CLAUDE.md, RT .../runtime/CLAUDE.md, SCH .../schema/CLAUDE.md,
               ASSETS assets/CLAUDE.md, TESTS crates/ironhold_core/tests/CLAUDE.md, TOPIC:<name>
               docs/dev/<name>.md, CUT (no anchor, notes must say why), DOC (a docs/ page, not anchored).

MODES
  freeze   Build the sidecar from the section map + the base file. One-time; afterwards edit the sidecar by
           hand (dest, governs, subs). Refuses to overwrite unless --force.
  rebase   After an intentional edit to the monolith that is already committed (R10 fixes, a ported-forward concurrent
           edit): re-point every block's `lines` and `text_sha1` at the current parent by diffing it against the
           file at the sidecar's `base_commit`, then move `base_commit` forward. Block IDs do NOT change (they
           are names; `b:92` stays `b:92` even if the text moves). Lists the blocks whose text changed and any new
           non-blank lines that fall between blocks (assign those by hand in the sidecar `lines`).
  self     (Phase 0) The parent still holds everything. Checks the sidecar covers every non-blank,
           non-heading line of the parent exactly once and that each block's text still matches the frozen
           hash. A hash mismatch means the monolith was edited after the freeze: port that edit forward.
  dest     (Phases 1-3, "destinations-only") The parent is NOT scanned. Every non-CUT block (every sub-ID
           for split blocks) must carry its anchor exactly once, in its expected destination file(s); a
           Safety=Y block must sit in the parent or in a directory file whose subtree covers every governed
           glob; each topic needs docs/dev/<name>.md and a pointer line in its directory file; every
           .claude/rules/*.md needs the exact frontmatter key `paths:` and each glob must match a file;
           destination files may not contain unresolved `above`/`below`/`see above`/`line N`/`four-site`
           (suppress a deliberate hit with `<!-- audit:ok -->` on that line).
  full     (Phase 4) Same as dest, plus the parent is scanned like any other destination.

OPTIONS
  --hints  Also print, per block, the lines of the frozen base carrying a modal verb (must/never/do not/
           don't/NOT/cannot/load-bearing/deliberate/hard invariant/.before(/.after(). A reviewer aid, never a gate.
  --strict Treat blocks whose dest label was ambiguous (review=true) as problems.
  --selftest Run the built-in fixture test.

BLIND SPOT: the Safety check only proves consistency with the author-written governs globs; a wrong glob
passes. The reviewer must read each Safety=Y row's governs list.
"""
import argparse, difflib, glob, hashlib, json, os, re, subprocess, sys, tempfile

PARENT = 'crates/ironhold_core/src/CLAUDE.md'
MAP = 'planning/investigations/core_claude_md_split_map.md'
SIDECAR = 'planning/investigations/core_claude_md_split_blocks.json'
DEST_FILE = {
    'PARENT': PARENT,
    'CAP': 'crates/ironhold_core/src/capabilities/CLAUDE.md',
    'SM': 'crates/ironhold_core/src/runtime/scene_manager/CLAUDE.md',
    'RT': 'crates/ironhold_core/src/runtime/CLAUDE.md',
    'SCH': 'crates/ironhold_core/src/schema/CLAUDE.md',
    'ASSETS': 'assets/CLAUDE.md',
    'TESTS': 'crates/ironhold_core/tests/CLAUDE.md',
}
# Provisional: which directory file carries the pointer line for each topic (Phase 1 confirms).
TOPIC_POINTER_IN = {
    'player-ground-detection': 'CAP', 'split-screen-cameras-and-widgets': 'CAP', 'action-bar-input-routing': 'CAP',
    'gamepad-routing': 'RT', 'player-spawn-sites': 'SM', 'animation-pipeline': 'CAP', 'lootable-corpse': 'SM',
}
# Rows the map splits across destinations (map section 7); sub-IDs `.rule` / `.ref`.
SPLIT_STARTS = [108, 284, 286, 452, 575, 856, 1121, 1530, 1606, 1657, 1668, 1713, 1731]
# Re-homings from map sections 6/7 that the row table still labels the old way (first line -> dest).
DEST_OVERRIDES = {622: 'TOPIC:animation-pipeline', 1463: 'TOPIC:split-screen-cameras-and-widgets',
                  383: 'TOPIC:split-screen-cameras-and-widgets', 1063: 'CUT', 711: 'ASSETS'}
SRC = 'crates/ironhold_core/src/'
# Map section 7 governs lists (repo-relative). Safety=Y rows without an entry default to the whole src tree.
GOVERNS = {
    '1530.rule': [SRC + 'capabilities/action_bar.rs', SRC + 'capabilities/camera.rs', SRC + 'capabilities/interactable.rs',
                  SRC + 'capabilities/targeting.rs', SRC + 'runtime/input.rs', SRC + 'capabilities/player.rs'],
    '1606.rule': [SRC + 'runtime/input.rs', SRC + 'runtime/scene_manager/action_executor.rs'],
    '670': [SRC + 'capabilities/animation.rs', SRC + 'capabilities/animation_resolver.rs'],
    '1224': [SRC + 'capabilities/player.rs', SRC + 'runtime/scene_manager/entity_spawner.rs'],
    '247': [SRC + 'capabilities/action_bar.rs', SRC + 'runtime/scene_manager/*.rs'],
    '711': ['assets/**/*.wgsl'],
}
BAD_PHRASE = re.compile(r'\b(see above|see below|above|below)\b|\bline \d+|four-site', re.I)
MODAL = re.compile(r"\b(must|never|do not|don't|cannot)\b|\bNOT\b|load-bearing|deliberate|hard invariant|\.before\(|\.after\(", re.I)
ANCHOR = re.compile(r'<!--\s*(b:\d+(?:\.[a-z]+)?)\s*-->')


def norm(text):
    return text.replace('\r\n', '\n')


def read(root, rel):
    with open(os.path.join(root, rel), encoding='utf-8', newline='') as f:
        return norm(f.read())


def sha1(s):
    return hashlib.sha1(s.encode('utf-8')).hexdigest()


def parse_ranges(s):
    out = []
    for part in re.split(r'[,;]\s*', s):
        m = re.fullmatch(r'(\d+)(?:-(\d+))?', part.strip())
        if not m:
            raise ValueError(f'bad line range {part!r}')
        out.append([int(m[1]), int(m[2] or m[1])])
    return out


def canon_dest(label, prev):
    l = label.strip()
    if re.match(r'^same (topic|TOPIC)', l):
        return prev, True
    review = bool(re.search(r'\bor\b|\+|;|\(|\?', l))
    for key in ('PARENT', 'CAP', 'SM', 'RT', 'SCH'):
        if re.match(key + r'\b', l):
            return key, review
    m = re.match(r'TOPIC:([\w-]+)', l)
    if m:
        return 'TOPIC:' + m[1], review
    if l.startswith('TOPIC'):
        return 'TOPIC:?', True
    if l.startswith('CUT'):
        return 'CUT', review
    if l.startswith('DOC'):
        return 'DOC', review
    if l.startswith('tests/'):
        return 'TESTS', True
    return '?', True


def freeze(root, force):
    out = os.path.join(root, SIDECAR)
    if os.path.exists(out) and not force:
        print(f'{SIDECAR} exists; use --force to rebuild (this discards hand edits)')
        return 2
    base = read(root, PARENT)
    lines = base.split('\n')
    txt = read(root, MAP)
    sec = txt[txt.index('## 2. Rows'):txt.index('## 3. Totals')]
    blocks, prev = [], 'PARENT'
    for l in sec.split('\n'):
        if not l.startswith('|') or l.startswith('|---') or l.startswith('| lines'):
            continue
        c = [x.strip() for x in l.strip().strip('|').split('|')]
        rng = parse_ranges(c[0])
        first = rng[0][0]
        dest, review = canon_dest(c[4], prev)
        if first in DEST_OVERRIDES:
            dest, review = DEST_OVERRIDES[first], False
        prev = dest
        safety = 'Y' if re.match(r'^Y', c[5]) else 'N'
        subs = ['rule', 'ref'] if first in SPLIT_STARTS else []
        text = '\n'.join('\n'.join(lines[a - 1:b]) for a, b in rng)
        gov = []
        if safety == 'Y':
            gov = GOVERNS.get(f'{first}.rule') or GOVERNS.get(str(first)) or [SRC + '**']
        blocks.append({
            'id': f'b:{first}', 'lines': rng, 'title': c[2], 'type': c[3], 'safety': safety, 'dest': dest, 'also': [],
            'dest_raw': c[4], 'governs': gov, 'subs': subs, 'text_sha1': sha1(text), 'review': review, 'notes': c[6],
        })
    ids = [b['id'] for b in blocks]
    assert len(ids) == len(set(ids)), 'duplicate block ids in the map'
    found = {int(b['id'][2:]) for b in blocks}
    for s in SPLIT_STARTS:
        if s not in found:
            print(f'warning: split start {s} has no map row beginning there')
    for s in DEST_OVERRIDES:
        if s not in found:
            print(f'warning: override {s} has no map row beginning there')
    doc = {
        'base_commit': subprocess.run(['git', 'log', '-1', '--format=%h', '--', PARENT], cwd=root, capture_output=True,
                                      text=True).stdout.strip(),
        'base_lines': len(lines), 'base_chars': len(base), 'base_sha1': sha1(base),
        'topics': {k: {'pointer_in': v} for k, v in TOPIC_POINTER_IN.items()},
        'blocks': blocks,
    }
    with open(out, 'w', encoding='utf-8', newline='\n') as f:
        json.dump(doc, f, indent=1, ensure_ascii=False)
        f.write('\n')
    print(f'wrote {SIDECAR}: {len(blocks)} blocks, {sum(b["review"] for b in blocks)} with an ambiguous dest label')
    return 0


def rebase(root):
    p = os.path.join(root, SIDECAR)
    doc = json.load(open(p, encoding='utf-8'))
    old = norm(subprocess.run(['git', 'show', f'{doc["base_commit"]}:{PARENT}'], cwd=root, capture_output=True,
                              text=True, encoding='utf-8').stdout).split('\n')
    if sha1('\n'.join(old)) != doc['base_sha1']:
        print('the file at base_commit does not match the sidecar base hash; refusing to rebase')
        return 2
    new_text = read(root, PARENT)
    new = new_text.split('\n')
    start, end = {}, {}
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, old, new, autojunk=False).get_opcodes():
        for k, i in enumerate(range(i1, i2)):
            if tag == 'equal':
                start[i + 1] = end[i + 1] = j1 + k + 1
            else:
                start[i + 1] = j1 + 1
                end[i + 1] = max(j1 + 1, j2) if j2 > j1 else j1
    changed = []
    for b in doc['blocks']:
        rng = []
        for a, z in b['lines']:
            na, nz = start[a], end[z]
            if nz < na:
                print(f'warning: {b["id"]} range {a}-{z} collapsed after the edit; fix its lines by hand')
                nz = na
            rng.append([na, nz])
        text = '\n'.join('\n'.join(new[a - 1:z]) for a, z in rng)
        if sha1(text) != b['text_sha1']:
            changed.append(b['id'])
        b['orig_lines'] = b.get('orig_lines', b['lines'])
        b['lines'], b['text_sha1'] = rng, sha1(text)
    head = subprocess.run(['git', 'log', '-1', '--format=%h', '--', PARENT], cwd=root, capture_output=True, text=True).stdout.strip()
    doc.update(base_commit=head, base_lines=len(new), base_chars=len(new_text), base_sha1=sha1(new_text))
    with open(p, 'w', encoding='utf-8', newline='\n') as f:
        json.dump(doc, f, indent=1, ensure_ascii=False)
        f.write('\n')
    print(f'rebased {len(doc["blocks"])} blocks onto {head} ({len(new)} lines); text changed in: {", ".join(changed) or "none"}')
    return 0


def covered_lines(blocks):
    cov = {}
    for b in blocks:
        for a, z in b['lines']:
            for n in range(a, z + 1):
                cov.setdefault(n, []).append(b['id'])
    return cov


def check_self(root, doc, problems):
    base = read(root, PARENT)
    lines = base.split('\n')
    if len(lines) != doc['base_lines'] or sha1(base) != doc['base_sha1']:
        problems.append(f'{PARENT} no longer equals the frozen base ({doc["base_commit"]}, {doc["base_lines"]} lines): '
                        'the monolith was edited after the freeze; the per-block hashes below show where')
    cov = covered_lines(doc['blocks'])
    for n, ids in cov.items():
        if len(ids) > 1:
            problems.append(f'line {n} belongs to several blocks: {ids}')
        if n > len(lines):
            problems.append(f'block {ids[0]} points past the end of the file (line {n})')
    for n, l in enumerate(lines, 1):
        if n not in cov and l.strip() and not l.lstrip().startswith('#') and not re.fullmatch(r'-{3,}', l.strip()):
            problems.append(f'line {n} is in no block: {l[:60]!r}')
    for b in doc['blocks']:
        text = '\n'.join('\n'.join(lines[a - 1:z]) for a, z in b['lines'])
        if sha1(text) != b['text_sha1']:
            problems.append(f'{b["id"]} text changed since the freeze ({b["title"][:50]})')
        if b['safety'] == 'Y' and not b['governs']:
            problems.append(f'{b["id"]} is Safety=Y with no governs list')


def dir_of(dest_key):
    return os.path.dirname(DEST_FILE[dest_key]) if dest_key in DEST_FILE else None


def glob_prefix(g):
    parts = []
    for p in g.split('/'):
        if re.search(r'[*?{\[]', p):
            break
        parts.append(p)
    return '/'.join(parts)


def sub_dest(b, sub):
    return b.get('sub_dest', {}).get(sub, b['dest']) if sub else b['dest']


def expected_files(b, sub=None):
    out = []
    for d in [sub_dest(b, sub)] + (b.get('also', []) if not sub or sub == 'rule' else []):
        if d in DEST_FILE:
            out.append(DEST_FILE[d])
        elif d.startswith('TOPIC:'):
            out.append(f'docs/dev/{d[6:]}.md')
    return out


def gather_anchors(root, include_parent):
    files = set()
    for rel in DEST_FILE.values():
        files.add(rel)
    files.update(os.path.relpath(p, root).replace('\\', '/') for p in glob.glob(os.path.join(root, 'docs/dev/**/*.md'), recursive=True))
    files.update(os.path.relpath(p, root).replace('\\', '/') for p in glob.glob(os.path.join(root, 'crates/ironhold_core/src/**/CLAUDE.md'), recursive=True))
    if not include_parent:
        files.discard(PARENT)
    found = {}
    texts = {}
    for rel in sorted(files):
        if not os.path.exists(os.path.join(root, rel)):
            continue
        t = read(root, rel)
        texts[rel] = t
        for m in ANCHOR.finditer(t):
            found.setdefault(m[1], []).append(rel)
    return found, texts


def check_dest(root, doc, include_parent, strict, problems):
    found, texts = gather_anchors(root, include_parent)
    known = set()
    for b in doc['blocks']:
        subs = b['subs'] or [None]
        ids = [f"{b['id']}.{s}" if s else b['id'] for s in subs]
        known.update(ids)
        if strict and b['review']:
            problems.append(f'{b["id"]} dest label is ambiguous ({b["dest_raw"][:50]!r}); resolve it in the sidecar')
        if b['dest'] in ('?', 'TOPIC:?'):
            problems.append(f'{b["id"]} has no resolved destination')
            continue
        if b['dest'] == 'CUT':
            if not b['notes'].strip():
                problems.append(f'{b["id"]} is CUT with no note saying why')
            for i in ids:
                if i in found:
                    problems.append(f'{i} is CUT but carries an anchor in {found[i]}')
            continue
        if b['dest'] == 'DOC':
            continue
        for sub, i in zip(subs, ids):
            d0 = sub_dest(b, sub)
            if d0 == 'PARENT' and not include_parent:
                continue  # stays in the parent; destinations-only mode does not scan it
            exp = expected_files(b, sub)
            where = found.get(i, [])
            has_also = bool(b['also']) and (not sub or sub == 'rule')
            want = len(exp) if has_also else 1
            if len(where) != want or len(set(where)) != len(where):
                problems.append(f'{i} appears {len(where)} times in destinations (expected {want}, once per file {exp}): {where}')
            elif not set(where) <= set(exp):
                problems.append(f'{i} found in {where} but its dest {d0} expects {exp}')
            if b['safety'] == 'Y' and (not sub or sub == 'rule'):
                places = [d0] + (b['also'] if has_also else [])
                if d0.startswith('TOPIC:') and not b['also']:
                    problems.append(f'{i} is Safety=Y but lives only in {d0}; add an `also` directory-file placement for the rule')
                for d in places:
                    if d == 'PARENT' or d.startswith('TOPIC:'):
                        continue  # the parent loads for everything; a topic doc is never the only home
                    dd = dir_of(d)
                    if dd is None:
                        continue
                    for g in b['governs']:
                        pre = glob_prefix(g)
                        if not (pre == dd or pre.startswith(dd + '/')):
                            problems.append(f'{i} is Safety=Y in {d} ({dd}) but governs {g!r}, which that directory does not cover')
    for i in found:
        if i not in known:
            problems.append(f'anchor {i} in {found[i]} matches no block in the sidecar')
    # topics: doc exists + pointer line in the directory file
    topics = {b['dest'][6:] for b in doc['blocks'] if b['dest'].startswith('TOPIC:') and b['dest'] != 'TOPIC:?'}
    for t in sorted(topics):
        docrel = f'docs/dev/{t}.md'
        if docrel not in texts:
            problems.append(f'topic {t}: {docrel} does not exist')
        pin = doc['topics'].get(t, {}).get('pointer_in')
        if not pin:
            problems.append(f'topic {t}: no pointer_in directory file declared in the sidecar')
        elif DEST_FILE.get(pin) in texts and docrel not in texts[DEST_FILE[pin]]:
            problems.append(f'topic {t}: {DEST_FILE[pin]} has no pointer line naming {docrel}')
    # .claude/rules stubs
    for p in sorted(glob.glob(os.path.join(root, '.claude/rules/*.md'))):
        rel = os.path.relpath(p, root).replace('\\', '/')
        t = read(root, rel)
        m = re.match(r'---\n(.*?)\n---\n', t, re.S)
        globs = []
        if m:
            inpaths = False
            for l in m[1].split('\n'):
                if re.match(r'paths:\s*$', l):
                    inpaths = True
                elif re.match(r'paths:\s*\S', l):
                    globs.append(l.split(':', 1)[1].strip().strip('"\''))
                    inpaths = False
                elif inpaths and re.match(r'\s*-\s*', l):
                    globs.append(re.sub(r'^\s*-\s*', '', l).strip().strip('"\''))
                elif l.strip():
                    inpaths = False
        if not globs:
            problems.append(f'{rel}: no frontmatter key exactly `paths:` with a glob (a rule without it loads on every session)')
        for g in globs:
            if not glob.glob(os.path.join(root, g), recursive=True):
                problems.append(f'{rel}: glob {g!r} matches no existing file')
        if re.search(r'^@\S+', t.split('---', 2)[-1], re.M):
            problems.append(f'{rel}: contains an @import line; imports from a scoped rule load eagerly (Phase 0 spike)')
    # unresolved relative phrases in the destination files
    scanned = {rel for rels in found.values() for rel in rels} | {f'docs/dev/{t}.md' for t in topics}
    for rel, t in texts.items():
        if rel not in scanned:
            continue
        for n, l in enumerate(t.split('\n'), 1):
            if BAD_PHRASE.search(l) and 'audit:ok' not in l:
                problems.append(f'{rel}:{n} unresolved relative reference: {l.strip()[:70]!r}')


def hints(root, doc):
    lines = read(root, PARENT).split('\n')
    for b in doc['blocks']:
        hit = [n for a, z in b['lines'] for n in range(a, z + 1) if MODAL.search(lines[n - 1])]
        if hit:
            print(f'{b["id"]:>10} {b["dest"]:<24} {len(hit):>3} modal lines (first at {hit[0]}) {b["title"][:48]}')


def run(root, mode, strict, show_hints):
    p = os.path.join(root, SIDECAR)
    if not os.path.exists(p):
        print(f'no sidecar; run `freeze` first ({SIDECAR})')
        return 2
    doc = json.load(open(p, encoding='utf-8'))
    problems = []
    if mode == 'self':
        check_self(root, doc, problems)
    else:
        check_dest(root, doc, mode == 'full', strict, problems)
    if show_hints:
        hints(root, doc)
    review = sum(b['review'] for b in doc['blocks'])
    for pr in problems:
        print('PROBLEM:', pr)
    print(f'{mode}: {len(doc["blocks"])} blocks, {review} with an ambiguous dest label, {len(problems)} problems')
    return 1 if problems else 0


def selftest():
    with tempfile.TemporaryDirectory() as root:
        def w(rel, txt):
            os.makedirs(os.path.dirname(os.path.join(root, rel)), exist_ok=True)
            open(os.path.join(root, rel), 'w', encoding='utf-8', newline='\n').write(txt)
        parent = '# Title\n\nrule one must hold\n\nrule two never breaks\n\nplain note\n'
        w(PARENT, parent)
        mp = ('## 2. Rows\n| lines | chars | title | type | dest | safety | notes |\n|---|---|---|---|---|---|---|\n'
              '| 3 | 1 | one | RULE | CAP | Y | n |\n| 5 | 1 | two | RULE | TOPIC:alpha | N | n |\n| 7 | 1 | note | DER | CUT | N | derivable |\n'
              '## 3. Totals\n')
        w(MAP, mp)
        subprocess.run(['git', 'init', '-q'], cwd=root)
        assert freeze(root, False) == 0
        assert run(root, 'self', False, False) == 0, 'self mode should pass on the unchanged file'
        w(PARENT, parent.replace('never breaks', 'sometimes breaks'))
        assert run(root, 'self', False, False) == 1, 'self mode must flag an edited block'
        w(PARENT, parent)
        assert run(root, 'dest', False, False) == 1, 'dest mode must fail with no destinations'
        doc = json.load(open(os.path.join(root, SIDECAR)))
        for b in doc['blocks']:
            if b['id'] == 'b:3':
                b['governs'] = [SRC + 'capabilities/x.rs']
        doc['topics']['alpha'] = {'pointer_in': 'CAP'}
        json.dump(doc, open(os.path.join(root, SIDECAR), 'w'))
        w(DEST_FILE['CAP'], '<!-- b:3 -->\nrule one must hold\n')
        w('docs/dev/alpha.md', '<!-- b:5 -->\nrule two never breaks\n')
        assert run(root, 'dest', False, False) == 1, 'missing topic pointer line must be flagged'
        w(DEST_FILE['CAP'], '<!-- b:3 -->\nrule one must hold\nSee docs/dev/alpha.md\n'.replace('See docs/dev/alpha.md', 'Topic: docs/dev/alpha.md'))
        w('.claude/rules/alpha.md', '---\npath:\n  - "x/**"\n---\nstub\n')
        assert run(root, 'dest', False, False) == 1, 'a mistyped paths key must be flagged'
        w('.claude/rules/alpha.md', '---\npaths:\n  - "crates/ironhold_core/src/capabilities/**"\n---\nstub\n')
        w('crates/ironhold_core/src/capabilities/x.rs', '//\n')
        assert run(root, 'dest', False, False) == 0, 'a complete fixture must pass'
        w(DEST_FILE['CAP'], '<!-- b:3 -->\nrule one must hold, see above\nTopic: docs/dev/alpha.md\n')
        assert run(root, 'dest', False, False) == 1, 'an unresolved "see above" must be flagged'
    print('selftest ok')
    return 0


def main():
    ap = argparse.ArgumentParser(description='Audit the CLAUDE.md split (see the file header).')
    ap.add_argument('mode', choices=['freeze', 'rebase', 'self', 'dest', 'full'], nargs='?')
    ap.add_argument('--force', action='store_true')
    ap.add_argument('--strict', action='store_true')
    ap.add_argument('--hints', action='store_true')
    ap.add_argument('--selftest', action='store_true')
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if not a.mode:
        ap.error('mode required')
    root = os.getcwd()
    try:
        if a.mode == 'freeze':
            return freeze(root, a.force)
        if a.mode == 'rebase':
            return rebase(root)
        return run(root, a.mode, a.strict, a.hints)
    except (OSError, ValueError, KeyError, AssertionError) as e:
        print('tool error:', e)
        return 2


if __name__ == '__main__':
    sys.exit(main())
