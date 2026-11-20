#!/usr/bin/env python3
"""Print a successful GitHub Actions run ID for this exact workflow, tag and SHA."""
import argparse
import json
import os
import re
import subprocess
import time

FIELDS = 'databaseId,workflowName,headBranch,headSha,status,conclusion'


def wait(workflow, tag, sha, repo, run_id=None, minutes=20):
    if run_id and not re.fullmatch('[0-9]+', run_id):
        raise SystemExit('Expected a numeric workflow run ID')
    args = (['view', run_id] if run_id else
            ['list', '--workflow', workflow + '.yml', '--branch', tag, '--commit', sha, '--limit', '1'])
    deadline = time.monotonic() + minutes * 60
    while time.monotonic() < deadline:
        data = json.loads(subprocess.check_output(['gh', 'run', *args, '-R', repo, '--json', FIELDS]))
        row = data if run_id else next(iter(data), None)
        if row:
            if (row['workflowName'], row['headBranch'], row['headSha']) != (workflow, tag, sha):
                raise SystemExit('Workflow run does not match expected workflow, tag and commit')
            if row['status'].lower() == 'completed':
                if row['conclusion'] != 'success':
                    raise SystemExit(f"{workflow} #{row['databaseId']}: {row['conclusion']}")
                return row['databaseId']
        time.sleep(15)
    raise SystemExit(f'Timed out waiting for {workflow} on {tag} @ {sha}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('workflow'); parser.add_argument('--run-id'); parser.add_argument('--minutes', type=int, default=20)
    a = parser.parse_args()
    print(wait(a.workflow, os.environ['GITHUB_REF_NAME'], os.environ['GITHUB_SHA'], os.environ['GITHUB_REPOSITORY'], a.run_id, a.minutes))
