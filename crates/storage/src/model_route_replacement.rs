use any2api_domain::ModelRouteConfiguration;
use sqlx::SqliteConnection;

use crate::{error::StorageError, model_route_rows::insert_model_route};

pub(crate) async fn replace_model_routes(
    connection: &mut SqliteConnection,
    routes: &ModelRouteConfiguration,
) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM model_routes")
        .execute(&mut *connection)
        .await?;
    for route in routes.routes() {
        insert_model_route(&mut *connection, route).await?;
    }
    Ok(())
}
