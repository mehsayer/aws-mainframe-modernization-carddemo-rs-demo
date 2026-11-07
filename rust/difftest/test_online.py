"""Unit tests of online.py's report helpers (no cargo, no GnuCOBOL):  python3 -m unittest rust/difftest/test_online.py"""
import os, sys, unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from online import ended_early  # noqa: E402

CRASH = """    Finished `release` profile [optimized] target(s) in 0.1s
     Running tests/transactions.rs (target/release/deps/transactions-5fbd8e586f70c9fd)

running 3 tests
test transaction_list_view_add ... error: test failed, to rerun pass `-p online --test transactions`

Caused by:
  process didn't exit successfully: `target/release/deps/transactions-5fbd --test-threads=1` (exit status: 1)"""

BUILD = """   Compiling online v0.1.0 (rust/crates/online)
error[E0425]: cannot find function `nosuch` in this scope
  --> crates/online/tests/signon.rs:5:5
error: could not compile `online` (test "signon") due to 1 previous error"""


class EndedEarly(unittest.TestCase):
    def test_crash_with_libcob_message_is_not_a_build_error(self):
        # cargo's "error: test failed" line must not make a crashed run look like a build failure
        s = ended_early("transactions / cobol", CRASH, 101, "PROCESS ENTER KEY\nlibcob: error: module 'CSUTLDTC' not found\n",
                        "transactions/cobol-sysout.log")
        self.assertIn("the test run ended early (exit code 101)", s)
        self.assertIn("libcob: error: module 'CSUTLDTC' not found", s)
        self.assertNotIn("PROCESS ENTER KEY", s)

    def test_crash_without_libcob_message_shows_no_sysout(self):
        # a Rust abort or a kill: earlier DISPLAY output in the sysout file explains nothing
        s = ended_early("manual_suite / cobol", CRASH, 134, "PROCESS ENTER KEY\n", "manual_suite/cobol-sysout.log")
        self.assertIn("the test run ended early", s)
        self.assertNotIn("sysout", s)
        self.assertNotIn("PROCESS ENTER KEY", s)

    def test_build_error(self):
        s = ended_early("signon / rust", BUILD, 101, "", "signon/rust-sysout.log")
        self.assertIn("the tests did not build", s)
        self.assertIn("error[E0425]: cannot find function `nosuch`", s)
        self.assertIn("  --> crates/online/tests/signon.rs:5:5", s)


if __name__ == "__main__":
    unittest.main()
