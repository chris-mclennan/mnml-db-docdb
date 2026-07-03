# mnml-db-docdb

Amazon DocumentDB / MongoDB viewer for [mnml](https://mnml.sh) —
terminal TUI with multiple saved cluster connections. Same shape as
the rest of the `mnml-db-*` family but the "query" is a MongoDB find
filter (JSON), and results render as `_id` + JSON-stringified document
rows.

DocumentDB is MongoDB-wire-compatible (3.6 / 4.0 / 5.0 surfaces), so
the same binary points at either without code changes. The official
`mongodb` Rust driver handles both transparently.

## Install

```sh
cargo install --git https://github.com/chris-mclennan/mnml-db-docdb mnml-db-docdb
mnml-db-docdb --install
```

## Setup

1. **Run once** to scaffold config:
   ```sh
   mnml-db-docdb
   ```
   Writes `~/.config/mnml-db-docdb.toml`. `chmod 600` it.

2. **Edit `[[connections]]`**:
   ```toml
   [[connections]]
   name = "local"
   uri  = "mongodb://localhost:27017"
   default_db = "test"

   [[connections]]
   name = "docdb-prod"
   uri  = "mongodb://api:${DOCDB_PASS}@docdb-cluster.cluster-xyz.us-east-1.docdb.amazonaws.com:27017/?tls=true&tlsCAFile=/etc/ssl/global-bundle.pem&replicaSet=rs0&readPreference=secondaryPreferred&retryWrites=false"
   default_db = "api"
   ```

   DocumentDB requires `retryWrites=false` and a CA bundle; the
   example above is the canonical DocumentDB connection string.
   MongoDB Atlas + self-hosted MongoDB don't need those.

3. **Re-run** — TUI launches; type a query, `Ctrl+Enter` to run.

## Query syntax

Three input shapes are accepted:

```
{ "name": "alice" }                                  # filter only — uses default_db + "default" collection
users.find({ "active": true })                       # explicit collection on default_db
analytics.events.find({ "type": "click" })           # explicit <db>.<collection>
```

Only `find` is supported in v0.1. Sort / projection / aggregations
are v0.2 — for now the find result is rendered as `_id` + a
JSON-stringified body column.

## Keys

| Chord                | Action                                            |
|----------------------|---------------------------------------------------|
| `Ctrl+Enter` / `F5`  | Run the current query                             |
| `Alt+1`-`Alt+9`      | Switch to that connection                         |
| `Ctrl+U`             | Clear the query buffer                            |
| `Ctrl+↑/↓` / `Ctrl+P/N` | Move selection in the results table             |
| `R` (uppercase)      | Double `row_limit` for the next run               |
| `q` / `Esc` / `Ctrl+C` | Quit                                            |

## Useful DocumentDB queries

```js
// List collections in the default db.
{ "listCollections": 1 }   // ← not supported in v0.1 (find-only); use mongosh

// Recent events from a time-series-ish collection.
events.find({ "ts": { "$gt": { "$date": "2026-06-01T00:00:00Z" } } })

// Lookup by ObjectId — wrap in `$oid` JSON extended-form.
users.find({ "_id": { "$oid": "654d2f8a4e7e1b9f6a8c3d2e" } })
```

## License

MIT.
