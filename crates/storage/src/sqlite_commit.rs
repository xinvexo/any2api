use sqlx::{Sqlite, Transaction};

pub(crate) const SQLITE_PRIMARY_CODE_MASK: i32 = 0xff;
const SQLITE_IOERR_PRIMARY_CODE: i32 = 10;

pub(crate) enum SqliteCommitError {
    Determinate(sqlx::Error),
    Indeterminate(sqlx::Error),
}

pub(crate) async fn commit(transaction: Transaction<'_, Sqlite>) -> Result<(), SqliteCommitError> {
    transaction.commit().await.map_err(|source| {
        if commit_error_is_indeterminate(&source) {
            SqliteCommitError::Indeterminate(source)
        } else {
            SqliteCommitError::Determinate(source)
        }
    })
}

pub(crate) fn commit_error_is_indeterminate(error: &sqlx::Error) -> bool {
    // SQLite can report IOERR from COMMIT_PHASETWO after the WAL commit is visible. A missing
    // worker acknowledgement is equally unsafe; neither case may be returned as a normal error.
    let sqlx::Error::Database(database) = error else {
        return true;
    };
    database
        .code()
        .and_then(|code| code.parse::<i32>().ok())
        .is_none_or(|code| sqlite_primary_code_is_indeterminate(code & SQLITE_PRIMARY_CODE_MASK))
}

pub(crate) const fn sqlite_primary_code_is_indeterminate(code: i32) -> bool {
    code == SQLITE_IOERR_PRIMARY_CODE
}
