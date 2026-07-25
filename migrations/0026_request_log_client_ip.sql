ALTER TABLE request_logs
ADD COLUMN client_ip TEXT CHECK (
    client_ip IS NULL
    OR (
        typeof(client_ip) = 'text'
        AND client_ip = trim(client_ip)
        AND length(client_ip) BETWEEN 2 AND 45
    )
);
