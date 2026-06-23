import pytest
from tth.database import connect, apply_migrations


@pytest.fixture
def mem_conn():
    conn = connect()
    apply_migrations(conn)
    yield conn
    conn.close()
