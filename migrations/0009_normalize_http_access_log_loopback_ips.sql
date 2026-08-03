UPDATE http_access_logs
SET client_ip = substr(client_ip, length('::ffff:') + 1)
WHERE lower(client_ip) LIKE '::ffff:127.%';
