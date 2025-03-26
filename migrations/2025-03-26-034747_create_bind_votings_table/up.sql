CREATE TABLE bind_votings (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    voter_steam_id TEXT NOT NULL,
    voted_bind_id TEXT NOT NULL,
    vote TEXT NOT NULL
);
-- Your SQL goes here
