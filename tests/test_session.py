import time
import uuid


from tth.session import get_or_create


def test_first_ever_creates_session(mem_conn):
    t0 = int(time.time())
    sid = get_or_create("my-app", t0, mem_conn)
    assert sid is not None
    uuid.UUID(sid)
    row = mem_conn.execute(
        "SELECT session_id FROM sessions WHERE session_id=?", (sid,)
    ).fetchone()
    assert row is not None


def test_reuse_within_gap_same_project(mem_conn):
    t0 = int(time.time()) - (10 * 60)
    t1 = int(time.time())
    sid0 = get_or_create("my-app", t0, mem_conn)
    sid1 = get_or_create("my-app", t1, mem_conn)
    assert sid0 == sid1


def test_new_session_on_gap_over_30min(mem_conn):
    t0 = int(time.time()) - (31 * 60)
    t1 = int(time.time())
    sid0 = get_or_create("my-app", t0, mem_conn)
    sid1 = get_or_create("my-app", t1, mem_conn)
    assert sid0 != sid1


def test_new_session_on_project_change(mem_conn):
    t0 = int(time.time()) - (5 * 60)
    t1 = int(time.time())
    sid0 = get_or_create("my-app", t0, mem_conn)
    sid1 = get_or_create("other-lib", t1, mem_conn)
    assert sid0 != sid1
