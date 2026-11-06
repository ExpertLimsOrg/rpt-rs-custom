//! # rpt-db-mssql — a live Microsoft SQL Server [`RowSource`]
//!
//! The native live-data path over TDS ([`tiberius`]): given a report's decoded database schema and a
//! connection URL (`mssql://` / `sqlserver://`), it generates SQL via [`rpt_query`] with
//! [`Dialect::Mssql`], executes it, and returns a [`RowSource`] the [`rpt_data`] pipeline consumes
//! like the Postgres and SQLite drivers. Record-selection formulas are **not** pushed into `WHERE`
//! (same trade-off as SQLite); the pipeline applies them per row.
//!
//! SQL generation is pure ([`rpt_query`]); this crate only parses the URL, connects (TLS via the
//! `native-tls` feature on [`tiberius`]), runs the string, and re-types cells. Text columns use
//! `CAST(... AS nvarchar(max))` from [`rpt_query`]; binary columns are selected raw.

use rpt_data::{Cell, Column, RowData, RowSource};
use rpt_model::{Database, FieldValueType, Report};
use rpt_query::{build_query_for_report, build_query_full, Dialect, QueryColumn, SqlQuery, Value};
use std::future::Future;
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tiberius::{AuthMethod, Client, Config};
use percent_encoding::percent_decode_str;
use url::Url;

/// A failure on the SQL Server path — the shared [`rpt_data::DbError`] with the driver's error type.
pub type DbError = rpt_data::DbError<MssqlError>;

/// Wraps [`tiberius::error::Error`] so this crate implements `Display`/`Error` for workspace lints.
#[derive(Debug)]
pub struct MssqlError(tiberius::error::Error);

impl std::fmt::Display for MssqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for MssqlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<tiberius::error::Error> for MssqlError {
    fn from(e: tiberius::error::Error) -> Self {
        MssqlError(e)
    }
}

/// Rows materialized from a SQL Server query ([`RowSource`] wrapper over [`RowData`]).
#[derive(Debug, Clone)]
pub struct MssqlSource(RowData);

impl RowSource for MssqlSource {
    fn columns(&self) -> &[Column] {
        self.0.columns()
    }
    fn rows(&self) -> Vec<rpt_data::Row> {
        self.0.rows()
    }
}

/// Parsed `mssql://` / `sqlserver://` URL for [`tiberius`].
struct ParsedUrl {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
    trust_server_certificate: bool,
}

fn parse_mssql_url(raw: &str) -> Result<ParsedUrl, String> {
    let u = Url::parse(raw).map_err(|e| format!("invalid URL: {e}"))?;
    let scheme = u.scheme();
    if !matches!(scheme, "mssql" | "sqlserver") {
        return Err(format!(
            "expected mssql:// or sqlserver:// URL, got scheme {scheme:?}"
        ));
    }
    let host = u
        .host_str()
        .ok_or_else(|| "URL missing host".to_string())?
        .to_string();
    let port = u.port().unwrap_or(1433);
    let database = u.path().trim_start_matches('/').to_string();
    if database.is_empty() {
        return Err(
            "URL path must name the database (e.g. mssql://user:pass@host/dbname)".into(),
        );
    }
    let user = u.username();
    if user.is_empty() {
        return Err("URL must include a username (before @)".into());
    }
    let user = percent_decode_str(user).decode_utf8_lossy().into_owned();
    let password = u
        .password()
        .map(|p| percent_decode_str(p).decode_utf8_lossy().into_owned())
        .unwrap_or_default();
    let mut trust_server_certificate = false;
    for (k, v) in u.query_pairs() {
        if k.eq_ignore_ascii_case("trustservercertificate")
            || k.eq_ignore_ascii_case("trust_server_certificate")
        {
            trust_server_certificate = matches!(v.as_ref(), "true" | "True" | "1" | "yes" | "on");
        }
    }
    Ok(ParsedUrl {
        host,
        port,
        database,
        user,
        password,
        trust_server_certificate,
    })
}

/// Map common boolean text to ODBC-style `1`/`0` for declared boolean columns.
fn bool_text_for_cell(s: &str) -> String {
    match s.trim() {
        "true" | "True" | "TRUE" => "1".to_string(),
        "false" | "False" | "FALSE" => "0".to_string(),
        other => other.to_string(),
    }
}

fn make_config(parsed: &ParsedUrl) -> Config {
    let mut c = Config::new();
    c.host(&parsed.host);
    c.port(parsed.port);
    c.database(&parsed.database);
    c.authentication(AuthMethod::sql_server(&parsed.user, &parsed.password));
    if parsed.trust_server_certificate {
        c.trust_cert();
    }
    c
}

async fn connect_client(
    config: &Config,
) -> Result<Client<tokio_util::compat::Compat<TcpStream>>, MssqlError> {
    let addr = config.get_addr();
    let tcp = TcpStream::connect(addr)
        .await
        .map_err(|e| tiberius::error::Error::Conversion(format!("TCP connect: {e}").into()))?;
    tcp.set_nodelay(true).ok();
    Client::connect(config.clone(), tcp.compat())
        .await
        .map_err(MssqlError::from)
}

async fn server_version_async(cfg: Config) -> Option<String> {
    let mut client = connect_client(&cfg).await.ok()?;
    let stream = client.simple_query("SELECT @@VERSION").await.ok()?;
    let row = stream.into_row().await.ok()??;
    let full = row.try_get::<&str, usize>(0).ok().flatten()?.to_string();
    full.lines().next().map(str::trim).map(String::from)
}

fn block_on<F, T>(rt: &tokio::runtime::Runtime, f: F) -> Result<T, MssqlError>
where
    F: Future<Output = Result<T, MssqlError>>,
{
    rt.block_on(f)
}

/// An open SQL Server handle: [`ping`](Self::ping) then [`run`](Self::run).
///
/// Each operation opens a fresh TCP + TDS session.
pub struct MssqlConn {
    rt: tokio::runtime::Runtime,
    config: Config,
}

impl std::fmt::Debug for MssqlConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MssqlConn").finish_non_exhaustive()
    }
}

impl MssqlConn {
    /// Validate `url` and prepare a handle. The connection opens on [`ping`](Self::ping) /
    /// [`run`](Self::run). Append `?trustservercertificate=true` to accept a self-signed certificate.
    ///
    /// # Errors
    ///
    /// [`DbError::Connect`] if the URL is invalid or the async runtime cannot start.
    pub fn connect(url: &str) -> Result<MssqlConn, DbError> {
        let parsed = parse_mssql_url(url).map_err(|msg| {
            DbError::connect(MssqlError(tiberius::error::Error::Conversion(msg.into())))
        })?;
        let config = make_config(&parsed);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                DbError::connect(MssqlError(tiberius::error::Error::Conversion(
                    e.to_string().into(),
                )))
            })?;
        Ok(MssqlConn { rt, config })
    }

    /// `SELECT 1` probe.
    ///
    /// # Errors
    ///
    /// [`DbError::Query`] on failure.
    pub fn ping(&mut self) -> Result<(), DbError> {
        let cfg = self.config.clone();
        block_on(&self.rt, async move {
            let mut client = connect_client(&cfg).await.map_err(MssqlError::from)?;
            let stream = client.simple_query("SELECT 1").await.map_err(MssqlError::from)?;
            stream
                .into_first_result()
                .await
                .map_err(MssqlError::from)?;
            Ok(())
        })
        .map_err(DbError::query)?;
        Ok(())
    }

    /// First line of `SELECT @@VERSION` for logs, or `None` if unavailable.
    pub fn server_version(&mut self) -> Option<String> {
        let cfg = self.config.clone();
        block_on(&self.rt, async move { Ok(server_version_async(cfg).await) })
            .ok()
            .flatten()
    }

    /// Execute `query` and return rows. Boolean-declared columns read `bit` as `1`/`0` text.
    ///
    /// # Errors
    ///
    /// [`DbError::Query`] on failure.
    pub fn run(&mut self, query: &SqlQuery, database: &Database) -> Result<MssqlSource, DbError> {
        let cfg = self.config.clone();
        let sql = query.sql.clone();
        let columns: Vec<Column> = query.result_columns();
        let bool_mask = bit_column_mask(database, &query.columns);
        let binary: Vec<bool> = columns.iter().map(|c| c.value_type.is_binary()).collect();
        let n = columns.len();

        let all_rows = block_on(&self.rt, async move {
            let mut client = connect_client(&cfg).await.map_err(MssqlError::from)?;
            let stream = client.simple_query(&sql).await.map_err(MssqlError::from)?;
            let rows_vec = stream
                .into_first_result()
                .await
                .map_err(MssqlError::from)?;
            let mut out: Vec<Vec<Option<Cell>>> = Vec::new();
            for row in rows_vec {
                let mut cells = Vec::with_capacity(n);
                for i in 0..n {
                    let is_bin = binary.get(i).copied().unwrap_or(false);
                    let is_bool = bool_mask.get(i).copied().unwrap_or(false);
                    let cell = if is_bin {
                        row.try_get::<&[u8], usize>(i)
                            .map_err(MssqlError::from)?
                            .map(|b| Cell::Bytes(b.to_vec()))
                    } else if is_bool {
                        if let Ok(Some(b)) = row.try_get::<bool, usize>(i) {
                            Some(Cell::Text(if b { "1".into() } else { "0".into() }))
                        } else if let Ok(Some(v)) = row.try_get::<u8, usize>(i) {
                            Some(Cell::Text(if v != 0 {
                                "1".into()
                            } else {
                                "0".into()
                            }))
                        } else if let Ok(Some(s)) = row.try_get::<&str, usize>(i) {
                            Some(Cell::Text(bool_text_for_cell(s)))
                        } else {
                            None
                        }
                    } else if let Ok(Some(s)) = row.try_get::<&str, usize>(i) {
                        Some(Cell::Text(s.to_string()))
                    } else if let Ok(Some(n)) = row.try_get::<i64, usize>(i) {
                        Some(Cell::Text(n.to_string()))
                    } else if let Ok(Some(n)) = row.try_get::<i32, usize>(i) {
                        Some(Cell::Text(n.to_string()))
                    } else if let Ok(Some(n)) = row.try_get::<f64, usize>(i) {
                        Some(Cell::Text(n.to_string()))
                    } else {
                        None
                    };
                    cells.push(cell);
                }
                out.push(cells);
            }
            Ok(out)
        })
        .map_err(|e| DbError::query(e).in_context(None, Some(&query.sql)))?;

        let mut row_iter = all_rows.into_iter();
        let data = RowData::from_cells(columns, move || Ok(row_iter.next()))
            .map_err(DbError::query)?;
        Ok(MssqlSource(data))
    }
}

impl MssqlSource {
    /// Connect, build SQL, and fetch.
    pub fn fetch(
        url: &str,
        database: &Database,
        sql_exprs: &[(String, String)],
        selection: Option<&str>,
        params: &[(String, Value)],
        comment: Option<&str>,
    ) -> Result<MssqlSource, DbError> {
        let mut query = build_query_full(database, sql_exprs, selection, params, Dialect::Mssql)
            .map_err(|e| DbError::no_query(e.to_string()))?;
        if let Some(c) = comment {
            query = query.with_comment(c);
        }
        let mut conn = MssqlConn::connect(url)?;
        conn.ping()?;
        conn.run(&query, database)
    }

    /// Like [`fetch`](Self::fetch) with [`build_query_for_report`].
    pub fn fetch_for_report(
        url: &str,
        report: &Report,
        selection: Option<&str>,
        sql_exprs: &[(String, String)],
        params: &[(String, Value)],
    ) -> Result<MssqlSource, DbError> {
        let query = build_query_for_report(report, sql_exprs, selection, params, Dialect::Mssql)
            .map_err(|e| DbError::no_query(e.to_string()))?;
        let mut conn = MssqlConn::connect(url)?;
        conn.ping()?;
        conn.run(&query, &report.database)
    }
}

/// Remap `true`/`false` strings when the report declares a Boolean (Crystal often compares to `"1"`).
fn bit_column_mask(database: &Database, columns: &[QueryColumn]) -> Vec<bool> {
    use std::collections::HashMap;
    let alias_to_table: HashMap<&str, &str> = database
        .tables
        .iter()
        .map(|t| (t.alias.as_str(), t.name.as_str()))
        .collect();
    let _ = alias_to_table;
    columns
        .iter()
        .map(|c| c.expr.is_none() && c.value_type == FieldValueType::Boolean)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_standard_url() {
        let p = parse_mssql_url("mssql://sa:sec%20ret@db.local:1433/mydb").unwrap();
        assert_eq!(p.host, "db.local");
        assert_eq!(p.port, 1433);
        assert_eq!(p.database, "mydb");
        assert_eq!(p.user, "sa");
        assert_eq!(p.password, "sec ret");
        assert!(!p.trust_server_certificate);
    }

    #[test]
    fn parse_trust_cert_query_flag() {
        let p = parse_mssql_url("sqlserver://u:p@h/db?trustServerCertificate=true").unwrap();
        assert!(p.trust_server_certificate);
    }
}
