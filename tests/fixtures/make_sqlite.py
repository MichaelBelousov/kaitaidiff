#!/usr/bin/env python3
"""Create real SQLite databases for tests using the stdlib sqlite3 module.

Usage: make_sqlite.py <out> <rows> [user_version]
Row count affects `database_size_in_pages`; user_version is written verbatim to
the header at offset 60 — both are visible in the 100-byte header our schema
parses, giving deterministic semantic diffs."""
import sqlite3, sys, os

def make(path: str, rows: int, user_version: int = 0):
    if os.path.exists(path):
        os.remove(path)
    con = sqlite3.connect(path)
    con.execute(f"PRAGMA user_version = {user_version}")
    con.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)")
    for i in range(rows):
        con.execute("INSERT INTO t (name) VALUES (?)", (f"row{i:06d}",))
    con.commit()
    con.close()

if __name__ == "__main__":
    out = sys.argv[1]
    rows = int(sys.argv[2])
    user_version = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    make(out, rows, user_version)
