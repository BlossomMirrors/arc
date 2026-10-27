use super::tables::{Meta, Q};
use rusqlite::functions::FunctionFlags;
use rusqlite::types::{Value as DbValue, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, Transaction};
use sea_query::{
    Expr, ExprTrait, Func, OnConflict, Query, QueryStatementWriter, SqliteQueryBuilder, Value,
};
use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::watch;
use tracing::warn;

const SCHEMA_VERSION: i64 = 8;

fn text_arg<'a>(v: ValueRef<'a>) -> &'a str {
    match v {
        ValueRef::Text(b) => std::str::from_utf8(b).unwrap_or(""),
        _ => "",
    }
}

fn open() -> rusqlite::Result<Connection> {
    let path = crate::cache::catalog_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(&path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "cache_size", -1024)?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update_and_check(None, "mmap_size", 0, |_| Ok(()))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.set_prepared_statement_cache_capacity(48);
    conn.create_scalar_function(
        "arc_score",
        5,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            Ok(super::score(
                text_arg(ctx.get_raw(0)),
                text_arg(ctx.get_raw(1)),
                text_arg(ctx.get_raw(2)),
                text_arg(ctx.get_raw(3)),
                ctx.get::<i64>(4).unwrap_or(0) != 0,
            ) as i64)
        },
    )?;
    Ok(conn)
}

fn open_current() -> rusqlite::Result<Connection> {
    let conn = open()?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == SCHEMA_VERSION {
        return Ok(conn);
    }
    drop(conn);
    let path = crate::cache::catalog_path();
    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.clone().into_os_string();
        file.push(suffix);
        let _ = std::fs::remove_file(file);
    }
    let conn = open()?;
    conn.execute_batch(include_str!("schema.sql"))?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(conn)
}

static WRITER: OnceLock<Mutex<Option<Connection>>> = OnceLock::new();

fn writer() -> &'static Mutex<Option<Connection>> {
    WRITER.get_or_init(|| match open_current() {
        Ok(c) => Mutex::new(Some(c)),
        Err(e) => {
            warn!("catalog database could not be opened: {e}");
            Mutex::new(None)
        }
    })
}

pub fn write<T>(f: impl FnOnce(&Transaction) -> rusqlite::Result<T>) -> Option<T> {
    let mut guard = writer().lock().unwrap_or_else(|p| p.into_inner());
    let conn = guard.as_mut()?;
    let tx = match conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate) {
        Ok(tx) => tx,
        Err(e) => {
            warn!("catalog write could not start: {e}");
            return None;
        }
    };
    let total_changes = |tx: &Transaction| {
        row(tx, Query::select().expr(Func::cust(Q::TotalChanges)), |r| {
            r.get::<_, i64>(0)
        })
        .map(|n| n.unwrap_or(0))
    };
    let result = total_changes(&tx).and_then(|before| {
        let v = f(&tx)?;
        let changed = total_changes(&tx)? != before;
        if changed {
            exec(
                &tx,
                Query::insert()
                    .into_table(Meta::Table)
                    .columns([Meta::Key, Meta::Value])
                    .values_panic(["generation".into(), 1.into()])
                    .on_conflict(
                        OnConflict::column(Meta::Key)
                            .value(Meta::Value, Expr::col(Meta::Value).add(1))
                            .to_owned(),
                    ),
            )?;
        }
        tx.commit()?;
        Ok((v, changed))
    });
    drop(guard);
    match result {
        Ok((v, changed)) => {
            if changed {
                bump_generation();
            }
            Some(v)
        }
        Err(e) => {
            warn!("catalog write failed: {e}");
            None
        }
    }
}

pub fn write_untracked(f: impl FnOnce(&Connection) -> rusqlite::Result<()>) {
    let guard = writer().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(conn) = guard.as_ref() {
        if let Err(e) = f(conn) {
            warn!("catalog write failed: {e}");
        }
    }
}

thread_local! {
    static READER: RefCell<Option<Connection>> = const { RefCell::new(None) };
}

pub fn read<T: Default>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> T {
    let _ = writer();
    READER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            match open() {
                Ok(c) => *slot = Some(c),
                Err(e) => {
                    warn!("catalog reader could not be opened: {e}");
                    return T::default();
                }
            }
        }
        match f(slot.as_ref().unwrap()) {
            Ok(v) => v,
            Err(e) => {
                warn!("catalog read failed: {e}");
                T::default()
            }
        }
    })
}

static GENERATION: OnceLock<watch::Sender<u64>> = OnceLock::new();

fn generation_sender() -> &'static watch::Sender<u64> {
    GENERATION.get_or_init(|| watch::channel(0).0)
}

pub fn bump_generation() {
    generation_sender().send_modify(|g| *g += 1);
}

pub fn subscribe() -> watch::Receiver<u64> {
    generation_sender().subscribe()
}

fn bind(values: sea_query::Values) -> Vec<DbValue> {
    values
        .0
        .into_iter()
        .map(|v| {
            match v {
                Value::Bool(b) => b.map(|b| DbValue::Integer(b as i64)),
                Value::TinyInt(n) => n.map(|n| DbValue::Integer(n as i64)),
                Value::SmallInt(n) => n.map(|n| DbValue::Integer(n as i64)),
                Value::Int(n) => n.map(|n| DbValue::Integer(n as i64)),
                Value::BigInt(n) => n.map(DbValue::Integer),
                Value::TinyUnsigned(n) => n.map(|n| DbValue::Integer(n as i64)),
                Value::SmallUnsigned(n) => n.map(|n| DbValue::Integer(n as i64)),
                Value::Unsigned(n) => n.map(|n| DbValue::Integer(n as i64)),
                Value::BigUnsigned(n) => n.map(|n| DbValue::Integer(n as i64)),
                Value::Float(n) => n.map(|n| DbValue::Real(n as f64)),
                Value::Double(n) => n.map(DbValue::Real),
                Value::String(s) => s.map(DbValue::Text),
                Value::Char(c) => c.map(|c| DbValue::Text(c.to_string())),
                Value::Bytes(b) => b.map(DbValue::Blob),
                _ => None,
            }
            .unwrap_or(DbValue::Null)
        })
        .collect()
}

pub fn rows<T>(
    conn: &Connection,
    query: &impl QueryStatementWriter,
    map: impl FnMut(&Row) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    let (sql, values) = query.build(SqliteQueryBuilder);
    conn.prepare_cached(&sql)?
        .query_map(rusqlite::params_from_iter(bind(values)), map)?
        .collect()
}

pub fn row<T>(
    conn: &Connection,
    query: &impl QueryStatementWriter,
    map: impl FnOnce(&Row) -> rusqlite::Result<T>,
) -> rusqlite::Result<Option<T>> {
    let (sql, values) = query.build(SqliteQueryBuilder);
    conn.prepare_cached(&sql)?
        .query_row(rusqlite::params_from_iter(bind(values)), map)
        .optional()
}

pub fn exec(conn: &Connection, query: &impl QueryStatementWriter) -> rusqlite::Result<usize> {
    let (sql, values) = query.build(SqliteQueryBuilder);
    conn.prepare_cached(&sql)?
        .execute(rusqlite::params_from_iter(bind(values)))
}
