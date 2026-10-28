#!/usr/bin/env python3
"""Offline reproduction with temporary fixtures; no agent/CLI/server launch."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
BUILD_FILES = ['Cargo.toml', 'Cargo.lock', 'src/main.rs', 'src/upstream.rs']

def command(args, root=ROOT):
    return subprocess.run(args, cwd=root, capture_output=True, text=True, check=False)

def tests(root=ROOT):
    p = command(['cargo', 'test', '--offline', '--locked'], root)
    combined = p.stdout + p.stderr
    m = re.search(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', combined)
    if not m:
        raise RuntimeError('No completed test suite:\n' + combined)
    result = {'passed': int(m[2]), 'failed': int(m[3]), 'ignored': int(m[4]), 'exit_code': p.returncode}
    result['failed_tests'] = sorted(set(re.findall(r'^test (\S+) \.\.\. FAILED$', combined, re.M)))
    return result

def main():
    provenance = json.loads((ROOT/'source.json').read_text())
    upstream = (ROOT/'src/upstream.rs').read_bytes().split(b'\n\n', 1)[1]
    assert hashlib.sha256(upstream).hexdigest() == provenance['function_sha256']
    normal = tests()
    assert normal == {'passed':11, 'failed':0, 'ignored':0, 'exit_code':0, 'failed_tests':[]}, normal
    p = command(['cargo', 'run', '--offline', '--locked', '--quiet'])
    if p.returncode:
        raise RuntimeError(p.stderr)
    run = json.loads(p.stdout)
    assert len(run['rows']) == 33
    faults = {}
    substitutions = {
        'status_registers': ('"status_only" => None,', '"status_only" => Some(upstream::register_mcp_server_in_config(path, COMMAND)),'),
        'explicit_disabled': ('"explicit_registration" => Some(upstream::register_mcp_server_in_config(path, COMMAND)),', '"explicit_registration" => None,'),
    }
    for name, (before, after) in substitutions.items():
        with tempfile.TemporaryDirectory(prefix='config-probe-fault-') as tmp:
            dest = Path(tmp)
            for rel in BUILD_FILES:
                (dest/rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT/rel, dest/rel)
            source = (dest/'src/main.rs').read_text()
            assert source.count(before) == 1
            (dest/'src/main.rs').write_text(source.replace(before, after))
            fault = tests(dest)
            assert fault['exit_code'] != 0 and fault['failed'] > 0 and fault['ignored'] == 0, fault
            expected = 'tests::status_preserves_every_raw_fixture' if name == 'status_registers' else 'tests::explicit_registration_is_functional'
            assert expected in fault['failed_tests'], fault
            faults[name] = fault
    checks = {'normal':normal, 'functional_faults':faults, 'source_body_sha256':hashlib.sha256(upstream).hexdigest(),
        'build_inputs_sha256':{rel:hashlib.sha256((ROOT/rel).read_bytes()).hexdigest() for rel in BUILD_FILES},
        'commands':['cargo test --offline --locked', 'cargo run --offline --locked --quiet'],
        'scope':'assertions on actual temporary files; no model or real agent/config'}
    out=ROOT/'artifacts';out.mkdir(exist_ok=True)
    (out/'run.json').write_text(json.dumps(run,indent=2,sort_keys=True)+'\n')
    (out/'checks.json').write_text(json.dumps(checks,indent=2,sort_keys=True)+'\n')
    evidence = '# Exact component evidence\n\n11 selected fixtures × 3 methods = 33 rows, plus two removal traces. These are designed cases, not sampled attack attempts. No model, coding CLI, Asyar application or MCP server ran.\n\n'
    evidence += '| Fixture | Registration during check: file changed | Status only: file changed | Explicit registration: file changed |\n|---|---|---|---|\n'
    for name in dict.fromkeys(r['case'] for r in run['rows']):
        rr = [r for r in run['rows'] if r['case']==name]
        evidence += '| ' + name + ' | ' + ' | '.join(str(r['file_changed']).lower() for r in rr) + ' |\n'
    evidence += '\nChanged means file existence or exact bytes changed, independently read after the action. A Boolean returned by the helper is not the oracle. Missing files and missing parent directories remain absent under Status only.\n\n'
    main_rows=[r for r in run['rows'] if r['case']=='removed_registration']
    evidence += '## Main paired trace\n\nIdentical initial bytes in each branch:\n\n```json\n' + bytes.fromhex(main_rows[0]['before']['bytes_hex']).decode() + '```\n\n'
    for r in main_rows[:2]:
        evidence += f"**{r['method']}**, final bytes:\n\n```json\n"+bytes.fromhex(r['after']['bytes_hex']).decode()+'\n```\n\n'
    evidence += 'Both paths observe the file. Registration during check additionally calls the unchanged upstream helper. Status only performs reads and reports the missing registration. Explicit registration is a separate harness action calling that same helper; it is a positive control, not a tested consent UI.\n\n'
    evidence += '## Controls and limits\n\nThe already-matching fixture stays byte-identical in all three methods. Existing unrelated valid keys remain in the registration result. After explicit registration and removal, registration during check reinstates the entry; status-only inspection preserves the removed state.\n\nThe original helper accepts extra arguments as long as one is mcp-server, so extra_arguments receives no write although the strict observer reports different_registration. Matching describes only the command and argument fields, not permission or readiness. The helper replaces malformed/non-object/non-UTF8 documents with new configuration; explicit registration retains that behavior. Status only preserves the bytes and reports invalid data.\n\n'
    evidence += f"Normal suite: {normal['passed']} passed, zero failed/ignored. Functional faults: " + ', '.join(f"{k}: {v['failed']} assertion failures" for k,v in faults.items()) + '. No compilation error is counted as a detected fault.\n'
    (out/'evidence.md').write_text(evidence)
    print(json.dumps({'fixtures':11,'rows':33,'tests':normal,'faults':faults},indent=2))

if __name__ == '__main__':
    main()
