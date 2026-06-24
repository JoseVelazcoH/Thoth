"""Entry point for the `tth` command using stdlib argparse.

The `record` subcommand runs on every shell hook and must stay typer/click-free
so import overhead stays near the stdlib baseline (~40ms). Future human-facing
commands (search, stats) can lazy-import a Typer app inside their own branch.
"""

import argparse
import os
import sys
import time

from tth.database import get_connection
from tth.recorder import record, _log_error


def main(argv=None):
    parser = argparse.ArgumentParser(prog="tth")
    sub = parser.add_subparsers(dest="command")

    rec = sub.add_parser("record", help="Record a shell command into Thoth's history database.")
    rec.add_argument("--cmd", required=True, help="The shell command that was executed")
    rec.add_argument("--dir", dest="dir_", default=None, help="Working directory (default: cwd)")
    rec.add_argument("--exit", dest="exit_", type=int, default=0, help="Exit code of the command")
    rec.add_argument("--duration", type=int, default=0, help="Duration in milliseconds")
    rec.add_argument("--timestamp", type=int, default=None, help="Unix timestamp (default: now)")
    rec.add_argument("--tags", default="[]", help="JSON array of tags")

    args = parser.parse_args(argv)

    if args.command == "record":
        conn = None
        try:
            if args.dir_ is None:
                args.dir_ = os.getcwd()
            if args.timestamp is None:
                args.timestamp = int(time.time())
            conn = get_connection()
            record(args.cmd, args.dir_, args.exit_, args.duration, args.timestamp, args.tags, conn)
        except Exception as exc:
            _log_error(exc)
        finally:
            if conn is not None:
                try:
                    conn.close()
                except Exception:
                    pass
        sys.exit(0)
