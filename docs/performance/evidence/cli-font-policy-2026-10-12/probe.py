import hashlib
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

binary = Path(sys.argv[1]).resolve()
report_path = Path(sys.argv[2]).resolve()
font = Path('crates/merman-export/tests/fonts/Cousine-Regular.ttf').resolve()
base_args = ['mmdc', '-i', '-', '-o', '-', '--svgId', 'pr193-performance']
lanes = {
    'default': [],
    'deterministic': ['--font-measurement', 'deterministic'],
    'explicit-font': ['--font-file', str(font)],
}
preamble = '---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\nflowchart LR\n'
inputs = {
    'two-node': preamble + 'A[First label]-->B[Second label]\n',
    '25-node': preamble + ''.join(f'N{i}[Node label {i}]-->N{i+1}\n' for i in range(24)),
}
report = {
    'binary': str(binary),
    'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
    'font_sha256': hashlib.sha256(font.read_bytes()).hexdigest(),
    'profile': 'dist; exact cli-release feature recipe',
    'fresh_processes_per_lane': 9,
    'warmups_per_lane': 1,
    'filesystem_caches': 'warm; not flushed',
    'clock': 'perf_counter_ns; includes spawn, input, render and output capture',
    'inputs': {},
}
for name, source in inputs.items():
    results = {lane: {'samples_ms': [], 'svg_sha256': []} for lane in lanes}
    payload = source.encode('utf-8')
    def run(lane):
        start = time.perf_counter_ns()
        result = subprocess.run([str(binary), *base_args, *lanes[lane]], input=payload, capture_output=True, timeout=60)
        elapsed = (time.perf_counter_ns() - start) / 1_000_000
        if result.returncode:
            raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
        if b'<svg' not in result.stdout:
            raise RuntimeError('Expected an SVG payload.')
        return elapsed, hashlib.sha256(result.stdout).hexdigest()
    for lane in lanes:
        run(lane)
    names = list(lanes)
    for repetition in range(9):
        order = names[repetition % len(names):] + names[:repetition % len(names)]
        for lane in order:
            elapsed, output_hash = run(lane)
            results[lane]['samples_ms'].append(elapsed)
            results[lane]['svg_sha256'].append(output_hash)
    for lane, result in results.items():
        if len(set(result['svg_sha256'])) != 1:
            raise RuntimeError(f'Unstable output for {name}/{lane}.')
        result['median_ms'] = statistics.median(result['samples_ms'])
        print(name, lane, round(result['median_ms'], 2), 'ms', flush=True)
    report['inputs'][name] = {'source': source, 'source_sha256': hashlib.sha256(payload).hexdigest(), 'lanes': results}
report_path.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
print('REPORT', report_path, flush=True)
