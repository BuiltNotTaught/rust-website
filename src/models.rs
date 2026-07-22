use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A blog post. Rows come from the `posts` table and are passed straight to
/// the templates, so field names here are the names you use in Tera.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub created_at: String,
}
