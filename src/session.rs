use crate::error::ThothError;
use rusqlite::Connection;

pub const SESSION_GAP_MINUTES: i64 = 30;
const GAP_SECONDS: i64 = SESSION_GAP_MINUTES * 60;

pub fn get_or_create(
    project: &str,
    timestamp: i64,
    conn: &Connection,
) -> Result<String, ThothError> {
    let row: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT session_id, project, ended_at FROM sessions ORDER BY ended_at DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();

    let create_new = match &row {
        None => true,
        Some((_, proj, ended_at)) => (timestamp - ended_at) > GAP_SECONDS || proj != project,
    };

    if create_new {
        let sid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO sessions(session_id, project, started_at, ended_at, command_count) VALUES(?1, ?2, ?3, ?4, 0)",
            rusqlite::params![sid, project, timestamp, timestamp],
        )?;
        Ok(sid)
    } else {
        let sid = row.unwrap().0;
        conn.execute(
            "UPDATE sessions SET ended_at=?1 WHERE session_id=?2",
            rusqlite::params![timestamp, sid],
        )?;
        Ok(sid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_conn() -> Connection {
        let mut c = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut c).unwrap();
        c
    }

    #[test]
    fn first_ever_creates_session() {
        let conn = mem_conn();
        let t0 = 1700000000i64;
        let sid = get_or_create("app", t0, &conn).unwrap();
        assert!(!sid.is_empty());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn reuse_within_gap() {
        let conn = mem_conn();
        let t0 = 1700000000i64;
        let sid1 = get_or_create("app", t0, &conn).unwrap();
        let t1 = t0 + 600;
        let sid2 = get_or_create("app", t1, &conn).unwrap();
        assert_eq!(sid1, sid2);
    }

    #[test]
    fn new_session_on_gap() {
        let conn = mem_conn();
        let t0 = 1700000000i64;
        let sid1 = get_or_create("app", t0, &conn).unwrap();
        let t1 = t0 + 31 * 60;
        let sid2 = get_or_create("app", t1, &conn).unwrap();
        assert_ne!(sid1, sid2);
    }

    #[test]
    fn new_session_on_project_change() {
        let conn = mem_conn();
        let t0 = 1700000000i64;
        let sid1 = get_or_create("app", t0, &conn).unwrap();
        let t1 = t0 + 300;
        let sid2 = get_or_create("other-lib", t1, &conn).unwrap();
        assert_ne!(sid1, sid2);
    }
}
