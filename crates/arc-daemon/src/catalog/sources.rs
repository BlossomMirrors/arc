use super::db::{exec, read, row, rows, write};
use super::model::{provider_source, provider_str, NewApp};
use super::state::{fingerprint, meta_int, set_meta};
use super::tables::{AppCategory, AppDesc, AppText, Apps, Search, Sources, Q};
use libarc::{Package, Provider};
use rusqlite::Transaction;
use sea_query::{DynIden, Expr, ExprTrait, IntoIden, OnConflict, Query, SelectStatement};
use std::collections::{HashMap, HashSet};

pub fn stored_sources() -> HashMap<String, (i64, u64)> {
    let select = Query::select()
        .columns([Sources::Path, Sources::Mtime, Sources::Size])
        .from(Sources::Table)
        .to_owned();
    read(|conn| {
        let list = rows(conn, &select, |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get(1)?, r.get::<_, i64>(2)? as u64),
            ))
        })?;
        Ok(list.into_iter().collect())
    })
}

pub struct SourceStamp<'a> {
    pub path: &'a str,
    pub remote: Option<&'a str>,
    pub rank: i64,
    pub mtime: i64,
    pub size: u64,
}

pub struct SourceWriter<'t> {
    tx: &'t Transaction<'t>,
    source: String,
    srank: i64,
    ord: i64,
    ids: Vec<String>,
}

impl SourceWriter<'_> {
    pub fn add(&mut self, app: &NewApp) -> rusqlite::Result<()> {
        let provider = app.provider.as_ref().map(provider_str).unwrap_or("Flatpak");
        exec(
            self.tx,
            Query::insert()
                .into_table(Apps::Table)
                .columns([
                    Apps::Id,
                    Apps::Cid,
                    Apps::IdLc,
                    Apps::Provider,
                    Apps::Source,
                    Apps::Prio,
                    Apps::Srank,
                    Apps::Ord,
                    Apps::Listable,
                    Apps::Remote,
                    Apps::IconUrl,
                    Apps::Version,
                    Apps::IsRuntime,
                    Apps::License,
                    Apps::EulaUrl,
                    Apps::HomepageUrl,
                    Apps::ContentRating,
                    Apps::Verified,
                    Apps::Categories,
                    Apps::Screenshots,
                    Apps::ProjectUrls,
                ])
                .values_panic([
                    app.id.as_str().into(),
                    app.cid.as_str().into(),
                    app.id.to_lowercase().into(),
                    provider.into(),
                    self.source.as_str().into(),
                    app.prio.into(),
                    self.srank.into(),
                    self.ord.into(),
                    app.listable.into(),
                    app.remote.clone().into(),
                    app.icon_url.clone().into(),
                    app.version.as_str().into(),
                    app.is_runtime.into(),
                    app.license.clone().into(),
                    app.eula_url.clone().into(),
                    app.homepage_url.clone().into(),
                    app.content_rating.as_str().into(),
                    app.verified.into(),
                    to_json(&app.categories, "[]").into(),
                    to_json(&app.screenshots, "[]").into(),
                    to_json(&app.project_urls, "{}").into(),
                ]),
        )?;
        let rid = self.tx.last_insert_rowid();
        self.ord += 1;
        for (locale, t) in &app.texts {
            if t.name.is_some() || t.summary.is_some() || t.developer.is_some() {
                exec(
                    self.tx,
                    Query::insert()
                        .into_table(AppText::Table)
                        .columns([
                            AppText::Rid,
                            AppText::Locale,
                            AppText::Name,
                            AppText::Summary,
                            AppText::Developer,
                            AppText::NameLc,
                            AppText::SummaryLc,
                        ])
                        .values_panic([
                            rid.into(),
                            locale.as_str().into(),
                            t.name.clone().into(),
                            t.summary.clone().into(),
                            t.developer.clone().into(),
                            t.name.as_ref().map(|s| s.to_lowercase()).into(),
                            t.summary.as_ref().map(|s| s.to_lowercase()).into(),
                        ]),
                )?;
            }
            if let Some(d) = &t.description {
                exec(
                    self.tx,
                    Query::insert()
                        .into_table(AppDesc::Table)
                        .columns([AppDesc::Rid, AppDesc::Locale, AppDesc::Description])
                        .values_panic([rid.into(), locale.as_str().into(), d.as_str().into()]),
                )?;
            }
        }
        for c in &app.categories {
            exec(
                self.tx,
                Query::insert()
                    .into_table(AppCategory::Table)
                    .columns([AppCategory::Category, AppCategory::Rid])
                    .values_panic([c.as_str().into(), rid.into()])
                    .on_conflict(OnConflict::new().do_nothing().to_owned()),
            )?;
        }
        self.ids.push(app.id.clone());
        Ok(())
    }
}

fn to_json(value: &impl serde::Serialize, empty: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| empty.to_string())
}

fn source_ids(tx: &Transaction, source: &str) -> rusqlite::Result<Vec<String>> {
    rows(
        tx,
        Query::select()
            .column(Apps::Id)
            .from(Apps::Table)
            .and_where(Expr::col(Apps::Source).eq(source)),
        |r| r.get(0),
    )
}

fn clear_source(tx: &Transaction, source: &str) -> rusqlite::Result<usize> {
    exec(
        tx,
        Query::delete()
            .from_table(Apps::Table)
            .and_where(Expr::col(Apps::Source).eq(source)),
    )
}

pub fn replace_source(
    source: &str,
    stamp: Option<SourceStamp>,
    fill: impl FnOnce(&mut SourceWriter) -> rusqlite::Result<()>,
) -> bool {
    write(|tx| {
        let old_ids = source_ids(tx, source)?;
        clear_source(tx, source)?;
        if let Some(s) = &stamp {
            exec(
                tx,
                Query::insert()
                    .into_table(Sources::Table)
                    .columns([
                        Sources::Path,
                        Sources::Remote,
                        Sources::Rank,
                        Sources::Mtime,
                        Sources::Size,
                    ])
                    .values_panic([
                        s.path.into(),
                        s.remote.into(),
                        s.rank.into(),
                        s.mtime.into(),
                        (s.size as i64).into(),
                    ])
                    .on_conflict(
                        OnConflict::column(Sources::Path)
                            .update_columns([
                                Sources::Remote,
                                Sources::Rank,
                                Sources::Mtime,
                                Sources::Size,
                            ])
                            .to_owned(),
                    ),
            )?;
        }
        let srank: i64 = row(
            tx,
            Query::select()
                .column(Sources::Rank)
                .from(Sources::Table)
                .and_where(Expr::col(Sources::Path).eq(source)),
            |r| r.get(0),
        )?
        .unwrap_or(i64::MAX / 2);
        let mut writer = SourceWriter {
            tx,
            source: source.to_string(),
            srank,
            ord: 0,
            ids: Vec::new(),
        };
        fill(&mut writer)?;
        let mut affected = old_ids;
        affected.append(&mut writer.ids);
        reshadow(tx, &affected)
    })
    .is_some()
}

pub fn remove_source(source: &str) -> bool {
    write(|tx| {
        let ids = source_ids(tx, source)?;
        clear_source(tx, source)?;
        exec(
            tx,
            Query::delete()
                .from_table(Sources::Table)
                .and_where(Expr::col(Sources::Path).eq(source)),
        )?;
        reshadow(tx, &ids)
    })
    .is_some()
}

pub fn set_source_ranks(paths_in_order: &[String]) -> bool {
    write(|tx| {
        for (rank, path) in paths_in_order.iter().enumerate() {
            let rank = rank as i64;
            exec(
                tx,
                Query::update()
                    .table(Sources::Table)
                    .value(Sources::Rank, rank)
                    .and_where(Expr::col(Sources::Path).eq(path)),
            )?;
            exec(
                tx,
                Query::update()
                    .table(Apps::Table)
                    .value(Apps::Srank, rank)
                    .and_where(Expr::col(Apps::Source).eq(path)),
            )?;
        }
        reshadow_all(tx)
    })
    .is_some()
}

const BULK_RESHADOW: usize = 500;

fn shadowed_value() -> Expr {
    let order = |t: DynIden| {
        Expr::tuple([
            Expr::col((t.clone(), Apps::Prio)),
            Expr::col((t.clone(), Apps::Srank)),
            Expr::col((t.clone(), Apps::Ord)),
            Expr::col((t, Apps::Rid)),
        ])
    };
    Expr::exists(
        Query::select()
            .expr(Expr::val(1))
            .from_as(Apps::Table, Q::B)
            .and_where(Expr::col((Q::B, Apps::Id)).equals((Apps::Table, Apps::Id)))
            .and_where(order(Q::B.into_iden()).lt(order(Apps::Table.into_iden())))
            .to_owned(),
    )
}

fn search_rows() -> SelectStatement {
    Query::select()
        .columns([(Q::C, AppText::Locale)])
        .columns([
            (Q::A, Apps::Rid),
            (Q::A, Apps::Prio),
            (Q::A, Apps::Srank),
            (Q::A, Apps::Ord),
            (Q::A, Apps::Id),
            (Q::A, Apps::Provider),
            (Q::A, Apps::IdLc),
        ])
        .columns([(Q::C, AppText::NameLc), (Q::C, AppText::SummaryLc)])
        .from_as(Apps::Table, Q::A)
        .join_as(
            sea_query::JoinType::InnerJoin,
            AppText::Table,
            Q::C,
            Expr::col((Q::C, AppText::Rid)).equals((Q::A, Apps::Rid)),
        )
        .and_where(Expr::col((Q::A, Apps::Shadowed)).eq(0))
        .and_where(Expr::col((Q::A, Apps::Listable)).eq(1))
        .to_owned()
}

fn fill_search(tx: &Transaction, select: SelectStatement) -> rusqlite::Result<usize> {
    let mut insert = Query::insert();
    insert
        .into_table(Search::Table)
        .columns([
            Search::Locale,
            Search::Rid,
            Search::Prio,
            Search::Srank,
            Search::Ord,
            Search::Id,
            Search::Provider,
            Search::IdLc,
            Search::NameLc,
            Search::SummaryLc,
        ])
        .select_from(select)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    exec(tx, &insert)
}

fn reshadow(tx: &Transaction, ids: &[String]) -> rusqlite::Result<()> {
    if ids.len() > BULK_RESHADOW {
        return reshadow_all(tx);
    }
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            continue;
        }
        exec(
            tx,
            Query::update()
                .table(Apps::Table)
                .value(Apps::Shadowed, shadowed_value())
                .and_where(Expr::col(Apps::Id).eq(id)),
        )?;
        exec(
            tx,
            Query::delete().from_table(Search::Table).and_where(
                Expr::col(Search::Rid).in_subquery(
                    Query::select()
                        .column(Apps::Rid)
                        .from(Apps::Table)
                        .and_where(Expr::col(Apps::Id).eq(id))
                        .to_owned(),
                ),
            ),
        )?;
        fill_search(
            tx,
            search_rows()
                .and_where(Expr::col((Q::A, Apps::Id)).eq(id))
                .to_owned(),
        )?;
    }
    Ok(())
}

fn reshadow_all(tx: &Transaction) -> rusqlite::Result<()> {
    exec(
        tx,
        Query::update()
            .table(Apps::Table)
            .value(Apps::Shadowed, shadowed_value()),
    )?;
    exec(tx, &Query::delete().from_table(Search::Table).to_owned())?;
    fill_search(tx, search_rows())?;
    Ok(())
}

pub fn replace_provider(provider: &Provider, packages: &[Package]) -> bool {
    let source = provider_source(provider);
    let key = format!("fp:{source}");
    let fp = fingerprint(packages);
    if meta_int(&key) == Some(fp) {
        return true;
    }
    replace_source(&source, None, |w| {
        for pkg in packages {
            w.add(&NewApp::from_package(pkg))?;
        }
        set_meta(w.tx, &key, fp)
    })
}
