import json
import unittest
from unittest.mock import patch

from wait_release_run import wait


class RunGate(unittest.TestCase):
    def test_exact_run_finishes_after_dispatch(self):
        row = dict(databaseId=27, workflowName='build', headBranch='v1', headSha='abc')
        for selected in (None, '27'):
            def payload(status, conclusion):
                data = dict(row, status=status, conclusion=conclusion)
                return json.dumps(data if selected else [data])
            with patch('wait_release_run.time.sleep'), patch('subprocess.check_output', side_effect=[payload('in_progress', ''), payload('completed', 'success')]):
                self.assertEqual(wait('build', 'v1', 'abc', 'owner/repo', selected), 27)
            with patch('subprocess.check_output', return_value=payload('completed', 'failure')), self.assertRaises(SystemExit):
                wait('build', 'v1', 'abc', 'owner/repo', selected)
            with patch('subprocess.check_output', return_value=payload('completed', 'success')), self.assertRaises(SystemExit):
                wait('build', 'v1', 'different', 'owner/repo', selected)


if __name__ == '__main__': unittest.main()
