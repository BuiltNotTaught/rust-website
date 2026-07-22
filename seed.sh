#!/usr/bin/env bash
# Adds a couple of sample blog posts so /blog is not empty.
# Requires the sqlite3 CLI. Run from the project root: ./seed.sh
set -e
DB="${DATABASE_PATH:-data.db}"

sqlite3 "$DB" <<'SQL'
CREATE TABLE IF NOT EXISTS posts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  title TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  content TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
INSERT OR IGNORE INTO posts (title, slug, content) VALUES
  ('Hello, world', 'hello-world', 'This is a sample post. Edit or delete it, then write your own.'),
  ('A second post', 'second-post', 'Posts are stored in SQLite and listed newest first.');
SQL

echo "seeded $DB"
sqlite3 "$DB" "SELECT id, slug, title FROM posts;"
