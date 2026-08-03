use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn loopback_ip_migration_normalizes_only_ipv4_mapped_loopback_rows() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 8).await;
    for (request_id, client_ip) in [
        ("mapped-loopback", "::ffff:127.0.0.1"),
        ("ipv4-loopback", "127.0.0.2"),
        ("ipv6-loopback", "::1"),
        ("mapped-external", "::ffff:203.0.113.8"),
        ("ipv4-external", "203.0.113.9"),
    ] {
        insert_log(&mut connection, request_id, client_ip).await;
    }

    migrate_through(&mut connection, 9).await;

    assert_eq!(
        migration_versions(&mut connection).await,
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT request_id, client_ip FROM http_access_logs ORDER BY request_id")
            .fetch_all(&mut connection)
            .await
            .expect("normalized client IP rows");
    assert_eq!(
        rows,
        [
            ("ipv4-external".into(), "203.0.113.9".into()),
            ("ipv4-loopback".into(), "127.0.0.2".into()),
            ("ipv6-loopback".into(), "::1".into()),
            ("mapped-external".into(), "::ffff:203.0.113.8".into()),
            ("mapped-loopback".into(), "127.0.0.1".into()),
        ]
    );
}

async fn insert_log(connection: &mut SqliteConnection, request_id: &str, client_ip: &str) {
    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, client_ip, method, path, http_version, \
          status_code, duration_ms, response_bytes, outcome) \
         VALUES (?, 1, 1, ?, 'GET', '/api/health', 'HTTP/1.1', 200, 1, 1, 'completed')",
    )
    .bind(request_id)
    .bind(client_ip)
    .execute(connection)
    .await
    .expect("representative HTTP access log");
}
