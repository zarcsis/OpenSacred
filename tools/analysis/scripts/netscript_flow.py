#!/usr/bin/env python3
"""NetScript control flow (bin/**/{FunkCode,StartCode,QuestCode}.bin): a census of the forms that the record walker's
control flow takes in every function of the 20 trees, by the walker as read off the client (docs/re/netscript.md,
"Control flow", #722).

The walker `FUN_00475680` runs one record a call; the function runner `FUN_0046BA90` calls it until the function's end
(the function ran to its end) or a record that answers 0 (it did not). The forms counted here follow from it:
  IF (58)       true: the next record runs. False: the walker reads on, record by record, to the first ELSEIF (66) whose
                condition holds or the first ELSE (59), and the record after it runs; reaching a record of type 0 or
                above 140, or the function's end, it logs "IF mit offenem Ende" and answers 0.
  ELSEIF (66)   reached in the flow (its branch ran): the walker reads on to the first ELSE and skips the one record
                after it; without an ELSE it logs "ELSEIF mit offenem Ende" and answers 0.
  ELSE (59)     reached in the flow: it skips the one record after it.
  41 / 42       41 (a selector 1-5 of the flags of the creature's quest) sets a global skip unless the flag holds;
                while the skip is set every record is passed over but 42, which clears it.
  guards        10-13 (QuestDone, QuestNotDone, QuestInProgress, QuestNotInProgress), 24 (GroupIsDead), 44 (DelDlg) and
                14 (DeleteFunktionBlock, which may also end the function as done) answer 0 on a false result; 43
                (ExitDlg) and 0 answer 0; 111 (ToBegin) runs the function again from its start.
The scan of a false IF does not count IFs: an IF inside a branch shares the first ELSEIF or ELSE that follows it.

Example
  python3 -I tools/analysis/scripts/netscript_flow.py --game-dir ./OriginalGame

Expected output (2.28): the counts below, the same on every run; "IF chains" by their ending, the ELSE bodies by the
type of their one record, the IFs met inside a branch, the 41 blocks, the guards. Dependencies: standard library.
"""
import argparse
import collections
import glob
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sacred_pe import DEFAULT_GAME_DIR
from netscript_dump import read_vectoren, records

IF, ELSE, ELSEIF, BLOCK, BLOCK_END = 58, 59, 66, 41, 42
GUARDS = {10: 'QuestDone', 11: 'QuestNotDone', 12: 'QuestInProgress', 13: 'QuestNotInProgress', 14: 'DeleteFunktionBlock',
          24: 'GroupIsDead', 43: 'ExitDlg', 44: 'DelDlg', 111: 'ToBegin'}


def bodies(tree_dir):
    """(name, record types) of each function of FunkCode.bin by Vectoren.bin, and the start and quest scripts."""
    funk = os.path.join(tree_dir, 'FunkCode.bin')
    vec = os.path.join(tree_dir, 'Vectoren.bin')
    if os.path.exists(funk) and os.path.exists(vec):
        data = open(funk, 'rb').read()
        functions, _, _ = read_vectoren(open(vec, 'rb').read())
        for name, offset, length, *_ in functions:
            types = [kind for _, kind, _ in records(data[offset:offset + length])]
            yield name.split(b'\0')[0].decode('latin-1'), types
    for script in ('StartCode.bin', 'QuestCode.bin'):
        path = os.path.join(tree_dir, script)
        if os.path.exists(path):
            yield script, [kind for _, kind, _ in records(open(path, 'rb').read())]


def census(types, c):
    """Counts the forms of one function's records into the counter `c`."""
    n = len(types)
    for i, kind in enumerate(types):
        if kind in GUARDS:
            c[f'guard {kind} {GUARDS[kind]}'] += 1
        if kind == IF:
            # The chain: the branches up to the ELSE that ends it, if any.
            elseifs, inner, j = 0, 0, i + 1
            end = None
            while j < n:
                t = types[j]
                if t == 0 or t > 140:
                    end = 'type 0'
                    break
                if t == ELSEIF:
                    elseifs += 1
                elif t == ELSE:
                    end = 'ELSE'
                    break
                elif t == IF:
                    inner += 1
                j += 1
            end = end or 'the end'
            c[f'IF chains ending at {end}, {"with" if elseifs else "without"} ELSEIF'] += 1
            if inner:
                c[f'IF chains with an IF inside a branch ({end})'] += 1
            if end == 'ELSE':
                body = types[j + 1] if j + 1 < n else None
                c[f'ELSE bodies of type {body}' if body in (IF, ELSE, ELSEIF, BLOCK, BLOCK_END, None, 0)
                  else 'ELSE bodies of another type'] += 1
        if kind == ELSE:
            # An ELSE that no IF before it in the function leads to.
            if IF not in types[:i]:
                c['ELSE with no IF before it'] += 1
        if kind == BLOCK:
            close = next((j for j in range(i + 1, n) if types[j] == BLOCK_END), None)
            c['41 blocks closed by a 42' if close is not None else '41 blocks open to the end'] += 1
            if close is not None:
                inside = types[i + 1:close]
                if BLOCK in inside:
                    c['41 blocks with another 41 inside'] += 1
                if inside.count(IF) and inside.count(IF) > inside.count(ELSE):
                    c['41 blocks with an IF chain open at their 42'] += 1
        if kind == BLOCK_END and BLOCK not in types[:i]:
            c['42 with no 41 before it'] += 1


def main():
    ap = argparse.ArgumentParser(description='Census of the NetScript control flow over bin/**.')
    ap.add_argument('--game-dir', default=DEFAULT_GAME_DIR)
    args = ap.parse_args()
    trees = sorted({os.path.dirname(p) for p in glob.glob(os.path.join(args.game_dir, 'bin', '**', 'FunkCode.bin'),
                                                            recursive=True)})
    c = collections.Counter()
    functions = 0
    for tree in trees:
        for _, types in bodies(tree):
            functions += 1
            census(types, c)
    print(f'{len(trees)} trees, {functions} functions and scripts')
    for key in sorted(c):
        print(f'  {key}: {c[key]}')


if __name__ == '__main__':
    main()
