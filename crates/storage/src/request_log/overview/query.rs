use sqlx::{FromRow, SqlitePool};

use crate::{error::StorageError, request_log::usage_window::unix_now_ms};

use super::model::{
    REQUEST_LOG_OVERVIEW_MODEL_LIMIT, RequestLogOverview, RequestLogOverviewModel,
    RequestLogOverviewRange, RequestLogOverviewTotals, empty_buckets,
};

pub(crate) async fn load_request_log_overview(
    pool: &SqlitePool,
    range: RequestLogOverviewRange,
) -> Result<RequestLogOverview, StorageError> {
    load_request_log_overview_at(pool, range, unix_now_ms()?).await
}

pub(super) async fn load_request_log_overview_at(
    pool: &SqlitePool,
    range: RequestLogOverviewRange,
    now_ms: u64,
) -> Result<RequestLogOverview, StorageError> {
    let mut overview = RequestLogOverview::empty(range, now_ms);
    let start = to_i64(overview.range_started_at_ms)?;
    let end = to_i64(overview.range_ended_at_ms)?;
    let width = to_i64(range.bucket_width_ms())?;
    let mut transaction = pool.begin().await?;

    let retained = sqlx::query_as::<_, AggregateRow>(aggregate_query(false))
        .fetch_one(&mut *transaction)
        .await?;
    let selected = sqlx::query_as::<_, AggregateRow>(aggregate_query(true))
        .bind(start)
        .bind(end)
        .fetch_one(&mut *transaction)
        .await?;
    let bucket_rows = sqlx::query_as::<_, BucketRow>(
        "SELECT ((started_at_ms - ?) / ?) AS bucket_index, COUNT(*) AS request_count, \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
         AS successful_request_count FROM request_logs \
         WHERE started_at_ms >= ? AND started_at_ms < ? \
         GROUP BY bucket_index ORDER BY bucket_index",
    )
    .bind(start)
    .bind(width)
    .bind(start)
    .bind(end)
    .fetch_all(&mut *transaction)
    .await?;
    let model_rows = sqlx::query_as::<_, ModelRow>(
        "SELECT public_model, COUNT(*) AS request_count, \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
         AS successful_request_count, \
         SUM(CASE WHEN input_tokens IS NOT NULL OR output_tokens IS NOT NULL THEN 1 ELSE 0 END) \
         AS token_usage_request_count, COALESCE(SUM(input_tokens), 0) AS input_tokens, \
         COALESCE(SUM(output_tokens), 0) AS output_tokens FROM request_logs \
         WHERE started_at_ms >= ? AND started_at_ms < ? GROUP BY public_model \
         ORDER BY request_count DESC, public_model COLLATE BINARY ASC LIMIT ?",
    )
    .bind(start)
    .bind(end)
    .bind(i64::try_from(REQUEST_LOG_OVERVIEW_MODEL_LIMIT + 1).expect("model limit fits i64"))
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;

    let (retained_started_at_ms, retained_totals) = parse_aggregate(retained)?;
    let (_, range_totals) = parse_aggregate(selected)?;
    overview.retained_started_at_ms = retained_started_at_ms;
    overview.retained_totals = retained_totals;
    overview.range_totals = range_totals;
    overview.time_buckets = parse_buckets(&overview, bucket_rows)?;
    overview.models = parse_models(&overview.range_totals, model_rows)?;
    Ok(overview)
}

fn aggregate_query(bounded: bool) -> &'static str {
    if bounded {
        "SELECT MIN(started_at_ms) AS oldest_started_at_ms, COUNT(*) AS request_count, \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
         AS successful_request_count, \
         SUM(CASE WHEN input_tokens IS NOT NULL OR output_tokens IS NOT NULL THEN 1 ELSE 0 END) \
         AS token_usage_request_count, COALESCE(SUM(input_tokens), 0) AS input_tokens, \
         COALESCE(SUM(output_tokens), 0) AS output_tokens FROM request_logs \
         WHERE started_at_ms >= ? AND started_at_ms < ?"
    } else {
        "SELECT MIN(started_at_ms) AS oldest_started_at_ms, COUNT(*) AS request_count, \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
         AS successful_request_count, \
         SUM(CASE WHEN input_tokens IS NOT NULL OR output_tokens IS NOT NULL THEN 1 ELSE 0 END) \
         AS token_usage_request_count, COALESCE(SUM(input_tokens), 0) AS input_tokens, \
         COALESCE(SUM(output_tokens), 0) AS output_tokens FROM request_logs"
    }
}

fn parse_aggregate(
    row: AggregateRow,
) -> Result<(Option<u64>, RequestLogOverviewTotals), StorageError> {
    let totals = parse_totals(
        row.request_count,
        row.successful_request_count,
        row.token_usage_request_count,
        row.input_tokens,
        row.output_tokens,
    )?;
    Ok((row.oldest_started_at_ms.map(from_i64).transpose()?, totals))
}

fn parse_buckets(
    overview: &RequestLogOverview,
    rows: Vec<BucketRow>,
) -> Result<Vec<super::model::RequestLogOverviewBucket>, StorageError> {
    let mut buckets = empty_buckets(
        overview.range,
        overview.range_started_at_ms,
        overview.range_ended_at_ms,
    );
    for row in rows {
        let index =
            usize::try_from(row.bucket_index).map_err(|_| StorageError::CorruptTelemetry)?;
        let bucket = buckets
            .get_mut(index)
            .ok_or(StorageError::CorruptTelemetry)?;
        bucket.request_count = from_i64(row.request_count)?;
        bucket.successful_request_count = from_i64(row.successful_request_count)?;
        validate_counts(bucket.request_count, bucket.successful_request_count, 0)?;
    }
    Ok(buckets)
}

fn parse_models(
    range_totals: &RequestLogOverviewTotals,
    rows: Vec<ModelRow>,
) -> Result<Vec<RequestLogOverviewModel>, StorageError> {
    let has_other = rows.len() > REQUEST_LOG_OVERVIEW_MODEL_LIMIT;
    let mut included = RequestLogOverviewTotals::default();
    let mut models = Vec::with_capacity(REQUEST_LOG_OVERVIEW_MODEL_LIMIT + usize::from(has_other));
    for row in rows.into_iter().take(REQUEST_LOG_OVERVIEW_MODEL_LIMIT) {
        let totals = parse_totals(
            row.request_count,
            row.successful_request_count,
            row.token_usage_request_count,
            row.input_tokens,
            row.output_tokens,
        )?;
        included.checked_add(&totals)?;
        models.push(RequestLogOverviewModel {
            public_model: row.public_model,
            is_other: false,
            totals,
        });
    }
    if has_other {
        models.push(RequestLogOverviewModel {
            public_model: None,
            is_other: true,
            totals: range_totals.checked_subtract(&included)?,
        });
    }
    Ok(models)
}

fn parse_totals(
    request_count: i64,
    successful_request_count: i64,
    token_usage_request_count: i64,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<RequestLogOverviewTotals, StorageError> {
    let totals = RequestLogOverviewTotals {
        request_count: from_i64(request_count)?,
        successful_request_count: from_i64(successful_request_count)?,
        token_usage_request_count: from_i64(token_usage_request_count)?,
        input_tokens: from_i64(input_tokens)?,
        output_tokens: from_i64(output_tokens)?,
    };
    validate_counts(
        totals.request_count,
        totals.successful_request_count,
        totals.token_usage_request_count,
    )?;
    Ok(totals)
}

fn validate_counts(total: u64, successful: u64, token_usage: u64) -> Result<(), StorageError> {
    if successful > total || token_usage > total {
        return Err(StorageError::CorruptTelemetry);
    }
    Ok(())
}

fn from_i64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

#[derive(FromRow)]
struct AggregateRow {
    oldest_started_at_ms: Option<i64>,
    request_count: i64,
    successful_request_count: i64,
    token_usage_request_count: i64,
    input_tokens: i64,
    output_tokens: i64,
}

#[derive(FromRow)]
struct BucketRow {
    bucket_index: i64,
    request_count: i64,
    successful_request_count: i64,
}

#[derive(FromRow)]
struct ModelRow {
    public_model: Option<String>,
    request_count: i64,
    successful_request_count: i64,
    token_usage_request_count: i64,
    input_tokens: i64,
    output_tokens: i64,
}
