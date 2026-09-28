//! Session id identity across deletion, restarts, and concurrent stores
//! (GSL-PT-20260927-D01).

use gosling::config::GoslingMode;
use gosling::session::import_formats::SessionImportTransport;
use gosling::session::{SessionImportOutcome, SessionManager, SessionType};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::collections::HashSet;
use std::path::Path;
use tempfile::TempDir;

async fn create_user_session(manager: &SessionManager, dir: &Path, name: &str) -> String {
    manager
        .create_session(
            dir.to_path_buf(),
            name.to_string(),
            SessionType::User,
            GoslingMode::Auto,
        )
        .await
        .unwrap()
        .id
}

fn sequence(id: &str) -> u64 {
    id.rsplit_once('_').unwrap().1.parse().unwrap()
}

async fn raw_pool(data_dir: &Path) -> Pool<Sqlite> {
    SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(data_dir.join("sessions").join("sessions.db"))
                .foreign_keys(true)
                .busy_timeout(std::time::Duration::from_secs(30)),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn a_deleted_session_id_is_never_handed_out_again() {
    let temp = TempDir::new().unwrap();
    let manager = SessionManager::new(temp.path().to_path_buf());

    let deleted = create_user_session(&manager, temp.path(), "project-A").await;
    manager.delete_session(&deleted).await.unwrap();
    let unrelated = create_user_session(&manager, temp.path(), "project-B").await;

    assert_ne!(unrelated, deleted);
    assert!(sequence(&unrelated) > sequence(&deleted));
    let stale_lookup = manager.get_session(&deleted, false).await.unwrap_err();
    assert!(stale_lookup.to_string().contains("Session not found"));
    assert!(manager.delete_session(&deleted).await.is_err());
    assert_eq!(
        manager.get_session(&unrelated, false).await.unwrap().name,
        "project-B"
    );
}

#[tokio::test]
async fn a_deleted_session_id_stays_retired_across_restarts() {
    let temp = TempDir::new().unwrap();
    let first_process = SessionManager::new(temp.path().to_path_buf());
    let kept = create_user_session(&first_process, temp.path(), "kept").await;
    let newest = create_user_session(&first_process, temp.path(), "newest").await;
    first_process.delete_session(&newest).await.unwrap();
    drop(first_process);

    let second_process = SessionManager::new(temp.path().to_path_buf());
    let next = create_user_session(&second_process, temp.path(), "next").await;

    assert_ne!(next, newest);
    assert!(sequence(&next) > sequence(&newest));
    assert!(second_process.get_session(&kept, false).await.is_ok());
}

#[tokio::test]
async fn ids_of_deleted_copies_imports_and_subagents_are_retired() {
    let temp = TempDir::new().unwrap();
    let manager = SessionManager::new(temp.path().to_path_buf());
    let mut issued = Vec::new();

    let source = create_user_session(&manager, temp.path(), "source").await;
    issued.push(source.clone());

    let copy = manager
        .copy_session(&source, "copy".to_string())
        .await
        .unwrap()
        .id;
    issued.push(copy.clone());
    manager.delete_session(&copy).await.unwrap();

    let fork = manager
        .fork_session(&source, "fork".to_string(), None)
        .await
        .unwrap()
        .id;
    issued.push(fork.clone());
    manager.delete_session(&fork).await.unwrap();

    let export = manager.export_session(&source).await.unwrap();
    let SessionImportOutcome::Imported(imported) = manager
        .import_session(
            &export,
            None,
            temp.path().to_path_buf(),
            SessionImportTransport::Json,
        )
        .await
        .unwrap()
    else {
        panic!("a first import of an export must create a session");
    };
    issued.push(imported.id.clone());
    manager.delete_session(&imported.id).await.unwrap();

    let subagent = manager
        .create_session(
            temp.path().to_path_buf(),
            "subagent".to_string(),
            SessionType::SubAgent,
            GoslingMode::Auto,
        )
        .await
        .unwrap()
        .id;
    issued.push(subagent.clone());
    manager.delete_session(&subagent).await.unwrap();

    issued.push(create_user_session(&manager, temp.path(), "after").await);

    let unique: HashSet<&String> = issued.iter().collect();
    assert_eq!(unique.len(), issued.len(), "an id was reissued: {issued:?}");
}

#[tokio::test]
async fn an_id_written_and_deleted_by_an_older_build_is_not_reissued() {
    let temp = TempDir::new().unwrap();
    let manager = SessionManager::new(temp.path().to_path_buf());
    let current = create_user_session(&manager, temp.path(), "current").await;

    // An older build shares the database but allocates from the highest
    // surviving id and knows nothing of the high-water mark.
    let day = current.split_once('_').unwrap().0;
    let older_build_id = format!("{day}_{}", sequence(&current) + 1);
    let pool = raw_pool(temp.path()).await;
    sqlx::query(
        "INSERT INTO sessions (id, name, working_dir, extension_data, gosling_mode) VALUES (?, 'older', '/tmp', '{}', 'auto')",
    )
    .bind(&older_build_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&older_build_id)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let next = create_user_session(&manager, temp.path(), "next").await;
    assert_ne!(next, older_build_id);
    assert!(sequence(&next) > sequence(&older_build_id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_stores_never_allocate_the_same_id() {
    const STORES: usize = 6;
    const SESSIONS_PER_STORE: usize = 12;

    let temp = TempDir::new().unwrap();
    // One store per simulated process: separate pools and write gates, so
    // only SQLite's own locking orders their allocations.
    let stores: Vec<SessionManager> = (0..STORES)
        .map(|_| SessionManager::new(temp.path().to_path_buf()))
        .collect();

    let tasks = stores.into_iter().enumerate().map(|(store_index, store)| {
        let dir = temp.path().to_path_buf();
        tokio::spawn(async move {
            let mut ids = Vec::new();
            for round in 0..SESSIONS_PER_STORE {
                let id = create_user_session(&store, &dir, &format!("{store_index}-{round}")).await;
                if round % 2 == 0 {
                    store.delete_session(&id).await.unwrap();
                }
                ids.push(id);
            }
            ids
        })
    });
    let issued: Vec<String> = futures::future::join_all(tasks)
        .await
        .into_iter()
        .flat_map(|ids| ids.unwrap())
        .collect();

    let unique: HashSet<&String> = issued.iter().collect();
    assert_eq!(issued.len(), STORES * SESSIONS_PER_STORE);
    assert_eq!(unique.len(), issued.len(), "an id was issued twice");
}
