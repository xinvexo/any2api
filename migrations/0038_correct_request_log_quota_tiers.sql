-- Versions before this migration selected the Codex cost tier from the request
-- preference. Reprice only rows whose stored cost exactly matches the old Fast
-- calculation for the same model and versioned rate card. Rates must divide into
-- exact integer nano-Credits per token, so this repair never relies on floating
-- point arithmetic. Aliases and unavailable or non-matching historical cards stay intact.
WITH built_in_rate_card(
    rate_card_id,
    model,
    standard_input,
    standard_cached_input,
    standard_output,
    fast_input,
    fast_cached_input,
    fast_output
) AS (
    VALUES
        ('openai_codex_credits_2026_08_11', 'gpt-5.6-sol',   125000, 12500, 750000, 312500, 31250, 1875000),
        ('openai_codex_credits_2026_08_11', 'gpt-5.5',       125000, 12500, 750000, 312500, 31250, 1875000),
        ('openai_codex_credits_2026_08_11', 'gpt-5.6-terra',  50000,  5000, 300000, 125000, 12500,  750000),
        ('openai_codex_credits_2026_08_11', 'gpt-5.6-luna',    5000,   500,  30000,  12500,  1250,   75000),
        ('openai_codex_credits_2026_08_11', 'gpt-5.4',        62500,  6250, 375000, 125000, 12500,  750000),
        ('openai_codex_credits_2026_08_11', 'gpt-5.4-mini',   18750,  1875, 113000,  37500,  3750,  226000)
),
configured_rate_card(
    rate_card_id,
    model,
    standard_input,
    standard_cached_input,
    standard_output,
    fast_input,
    fast_cached_input,
    fast_output
) AS (
    SELECT
        json_extract(settings.value_json, '$.id'),
        models.key,
        CAST(json_extract(models.value, '$.standard.input_nanos_per_million') AS INTEGER) / 1000000,
        CAST(json_extract(models.value, '$.standard.cached_input_nanos_per_million') AS INTEGER) / 1000000,
        CAST(json_extract(models.value, '$.standard.output_nanos_per_million') AS INTEGER) / 1000000,
        CAST(json_extract(models.value, '$.fast.input_nanos_per_million') AS INTEGER) / 1000000,
        CAST(json_extract(models.value, '$.fast.cached_input_nanos_per_million') AS INTEGER) / 1000000,
        CAST(json_extract(models.value, '$.fast.output_nanos_per_million') AS INTEGER) / 1000000
    FROM setting_overrides AS settings,
         json_each(json_extract(settings.value_json, '$.models')) AS models
    WHERE settings.key = 'oauth.codex.rate_card'
      AND json_type(settings.value_json, '$.id') = 'text'
      AND json_type(models.value, '$.standard.input_nanos_per_million') = 'integer'
      AND json_type(models.value, '$.standard.cached_input_nanos_per_million') = 'integer'
      AND json_type(models.value, '$.standard.output_nanos_per_million') = 'integer'
      AND json_type(models.value, '$.fast.input_nanos_per_million') = 'integer'
      AND json_type(models.value, '$.fast.cached_input_nanos_per_million') = 'integer'
      AND json_type(models.value, '$.fast.output_nanos_per_million') = 'integer'
      AND CAST(json_extract(models.value, '$.standard.input_nanos_per_million') AS INTEGER) % 1000000 = 0
      AND CAST(json_extract(models.value, '$.standard.cached_input_nanos_per_million') AS INTEGER) % 1000000 = 0
      AND CAST(json_extract(models.value, '$.standard.output_nanos_per_million') AS INTEGER) % 1000000 = 0
      AND CAST(json_extract(models.value, '$.fast.input_nanos_per_million') AS INTEGER) % 1000000 = 0
      AND CAST(json_extract(models.value, '$.fast.cached_input_nanos_per_million') AS INTEGER) % 1000000 = 0
      AND CAST(json_extract(models.value, '$.fast.output_nanos_per_million') AS INTEGER) % 1000000 = 0
),
rate_card AS (
    SELECT built_in_rate_card.*
    FROM built_in_rate_card
    WHERE NOT EXISTS (
        SELECT 1
        FROM configured_rate_card
        WHERE configured_rate_card.rate_card_id = built_in_rate_card.rate_card_id
          AND configured_rate_card.model = built_in_rate_card.model
    )
    UNION ALL
    SELECT configured_rate_card.*
    FROM configured_rate_card
),
candidate_costs(request_id, standard_cost, fast_cost) AS (
    SELECT
        request_logs.request_id,
        (request_logs.input_tokens - MIN(
            COALESCE(request_logs.cache_read_tokens, 0),
            request_logs.input_tokens
        )) * rate_card.standard_input
        + MIN(
            COALESCE(request_logs.cache_read_tokens, 0),
            request_logs.input_tokens
        ) * rate_card.standard_cached_input
        + request_logs.output_tokens * rate_card.standard_output,
        (request_logs.input_tokens - MIN(
            COALESCE(request_logs.cache_read_tokens, 0),
            request_logs.input_tokens
        )) * rate_card.fast_input
        + MIN(
            COALESCE(request_logs.cache_read_tokens, 0),
            request_logs.input_tokens
        ) * rate_card.fast_cached_input
        + request_logs.output_tokens * rate_card.fast_output
    FROM request_logs
    JOIN rate_card
      ON rate_card.rate_card_id = request_logs.quota_cost_rate_card
     AND rate_card.model = request_logs.public_model
    WHERE request_logs.quota_cost_unit = 'codex_credits'
      AND request_logs.quota_service_tier = 'fast'
      AND (
          request_logs.effective_speed_tier IS NULL
          OR request_logs.effective_speed_tier <> 'fast'
      )
      AND request_logs.input_tokens IS NOT NULL
      AND request_logs.output_tokens IS NOT NULL
)
UPDATE request_logs
SET quota_cost_nanos = (
        SELECT candidate_costs.standard_cost
        FROM candidate_costs
        WHERE candidate_costs.request_id = request_logs.request_id
    ),
    quota_service_tier = 'standard'
WHERE EXISTS (
      SELECT 1
      FROM candidate_costs
      WHERE candidate_costs.request_id = request_logs.request_id
        AND typeof(candidate_costs.standard_cost) = 'integer'
        AND typeof(candidate_costs.fast_cost) = 'integer'
        AND request_logs.quota_cost_nanos = candidate_costs.fast_cost
  );
