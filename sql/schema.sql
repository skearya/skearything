CREATE TABLE IF NOT EXISTS servers(
    server_id INTEGER PRIMARY KEY,
    daily_log_channel INTEGER,
    vc_logs_channel INTEGER
);

CREATE TABLE IF NOT EXISTS days(
    day TEXT NOT NULL,
    server_id INTEGER NOT NULL,
    messages_sent INTEGER,
    unique_chatters INTEGER,
    vc_seconds_elapsed REAL DEFAULT 0,
    FOREIGN KEY (server_id) REFERENCES servers(server_id),
    PRIMARY KEY (day, server_id)
);