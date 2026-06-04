//! mongodb wrapper. The "query" is a JSON document interpreted as
//! a MongoDB find filter. Results are returned as BSON, JSON-
//! serialized into a single `body` column per row + an `_id`
//! column for navigation.
//!
//! Two variants on the input parsing path:
//!   - `{ ... }` ⇒ filter applied via `find(filter, None)`
//!   - `db.collection.find({ ... })` ⇒ same, but pre-parses the
//!     collection name from the JS-shell syntax for familiarity.

use anyhow::{Context, Result, anyhow};
use bson::{Bson, Document};
use futures_util::TryStreamExt;
use mongodb::{Client, options::FindOptions};

pub async fn connect(uri: &str) -> Result<Client> {
    let client = Client::with_uri_str(uri)
        .await
        .context("connecting to MongoDB")?;
    Ok(client)
}

#[derive(Debug, Clone, Default)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub elapsed: std::time::Duration,
    pub server_row_count: usize,
    pub truncated: bool,
}

/// Run a query against a specific database. Default collection +
/// filter come from parsing the input:
///   - Bare JSON `{...}` ⇒ filter on the user-provided `default_db`
///     + a default collection (first one in the db).
///   - `<collection>.find({...})` ⇒ filter on `<collection>` (DB
///     stays as default_db).
///   - `<db>.<collection>.find({...})` ⇒ filter on
///     `<db>.<collection>`.
pub async fn run_query(
    client: &Client,
    default_db: &str,
    input: &str,
    row_limit: u32,
) -> Result<QueryResult> {
    let start = std::time::Instant::now();
    let parsed = parse_query(input)?;
    let db_name = parsed.db.as_deref().unwrap_or(default_db);
    let coll_name = parsed.coll.unwrap_or_else(default_collection_label);
    let filter_doc = parsed.filter;
    let coll = client.database(db_name).collection::<Document>(&coll_name);
    let opts = FindOptions::builder().limit(Some(row_limit as i64)).build();
    let mut cursor = coll
        .find(filter_doc)
        .with_options(opts)
        .await
        .context("running find()")?;
    let mut docs: Vec<Document> = Vec::new();
    while let Some(d) = cursor.try_next().await.context("draining cursor")? {
        docs.push(d);
    }
    let elapsed = start.elapsed();
    let server_row_count = docs.len();
    let take = (row_limit as usize).min(docs.len());
    let truncated = docs.len() > take;
    let rows: Vec<Vec<String>> = docs
        .into_iter()
        .take(take)
        .map(|d| {
            let id = d
                .get("_id")
                .map(bson_to_string)
                .unwrap_or_else(|| "—".to_string());
            let body = serde_json::to_string(&d.to_bson_value()).unwrap_or_default();
            vec![id, body]
        })
        .collect();
    Ok(QueryResult {
        columns: vec!["_id".to_string(), "document".to_string()],
        rows,
        elapsed,
        server_row_count,
        truncated,
    })
}

fn default_collection_label() -> String {
    "default".to_string()
}

fn bson_to_string(v: &Bson) -> String {
    match v {
        Bson::ObjectId(oid) => oid.to_hex(),
        Bson::String(s) => s.clone(),
        Bson::Int32(n) => n.to_string(),
        Bson::Int64(n) => n.to_string(),
        Bson::Double(n) => n.to_string(),
        Bson::Boolean(b) => b.to_string(),
        Bson::Null => "null".to_string(),
        _ => format!("{v:?}"),
    }
}

trait BsonValueExt {
    fn to_bson_value(&self) -> serde_json::Value;
}

impl BsonValueExt for Document {
    fn to_bson_value(&self) -> serde_json::Value {
        let bson = Bson::Document(self.clone());
        serde_json::to_value(bson).unwrap_or_default()
    }
}

#[derive(Debug)]
struct ParsedQuery {
    db: Option<String>,
    coll: Option<String>,
    filter: Document,
}

/// Parse three forms:
///   1) `{...}`                       — filter only
///   2) `<coll>.find({...})`          — collection + filter
///   3) `<db>.<coll>.find({...})`     — db + collection + filter
fn parse_query(input: &str) -> Result<ParsedQuery> {
    let trimmed = input.trim();
    // Bare JSON.
    if trimmed.starts_with('{') {
        let filter = parse_filter(trimmed)?;
        return Ok(ParsedQuery {
            db: None,
            coll: None,
            filter,
        });
    }
    // Shell-style: <id>(.<id>)?.find( ... )
    let Some(open_paren) = trimmed.find('(') else {
        return Err(anyhow!("expected `{{...}}` or `<coll>.find({{...}})`"));
    };
    let prefix = &trimmed[..open_paren];
    // Match `.find` at the end of the prefix.
    let Some(prefix) = prefix.strip_suffix(".find") else {
        return Err(anyhow!(
            "only `.find(...)` is supported in v0.1; got `{prefix}(…)`"
        ));
    };
    let segments: Vec<&str> = prefix.split('.').collect();
    let (db, coll) = match segments.as_slice() {
        [coll] => (None, Some(coll.to_string())),
        [db, coll] => (Some(db.to_string()), Some(coll.to_string())),
        _ => {
            return Err(anyhow!(
                "expected `<coll>.find(...)` or `<db>.<coll>.find(...)`"
            ));
        }
    };
    let Some(close_paren) = trimmed.rfind(')') else {
        return Err(anyhow!("missing closing `)`"));
    };
    let body = trimmed[open_paren + 1..close_paren].trim();
    let body = if body.is_empty() { "{}" } else { body };
    let filter = parse_filter(body)?;
    Ok(ParsedQuery { db, coll, filter })
}

fn parse_filter(s: &str) -> Result<Document> {
    let v: serde_json::Value =
        serde_json::from_str(s).with_context(|| format!("parsing filter JSON: {s}"))?;
    let bson: Bson = bson::to_bson(&v).context("converting JSON to BSON")?;
    match bson {
        Bson::Document(d) => Ok(d),
        _ => Err(anyhow!("filter must be a JSON object")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bare_json_works() {
        let p = parse_query("{ \"name\": \"alice\" }").unwrap();
        assert!(p.db.is_none());
        assert!(p.coll.is_none());
        assert_eq!(p.filter.get_str("name").unwrap(), "alice");
    }

    #[test]
    fn parse_coll_find_works() {
        let p = parse_query("users.find({})").unwrap();
        assert!(p.db.is_none());
        assert_eq!(p.coll.as_deref(), Some("users"));
        assert!(p.filter.is_empty());
    }

    #[test]
    fn parse_db_coll_find_works() {
        let p = parse_query("analytics.events.find({ \"type\": \"click\" })").unwrap();
        assert_eq!(p.db.as_deref(), Some("analytics"));
        assert_eq!(p.coll.as_deref(), Some("events"));
        assert_eq!(p.filter.get_str("type").unwrap(), "click");
    }

    #[test]
    fn parse_rejects_unknown_operator() {
        // `insertOne` isn't supported; only `find`.
        assert!(parse_query("users.insertOne({})").is_err());
    }

    #[test]
    fn parse_rejects_malformed_json() {
        assert!(parse_query("{ not valid json").is_err());
    }
}
