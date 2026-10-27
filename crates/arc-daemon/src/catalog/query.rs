use super::db::{read, row, rows};
use super::model::{locale_params, map_app, CatalogApp};
use super::search::SearchPage;
use super::tables::{AppCategory, AppDesc, AppText, Apps, ForgeVerified, Packages, Q};
use super::PRIO_PROVIDER;
use rusqlite::Connection;
use sea_query::{
    Cond, DynIden, Expr, ExprTrait, Func, IntoIden, JoinType, Order, Query, SelectStatement,
    UnionType,
};

fn localized(column: AppText, fallback: Option<Expr>) -> Expr {
    let mut parts = vec![
        Expr::col((Q::L1, column)),
        Expr::col((Q::L2, column)),
        Expr::col((Q::C, column)),
    ];
    parts.extend(fallback);
    Func::coalesce(parts).into()
}

pub(super) fn join_texts(select: &mut SelectStatement, l1: &str, l2: &str) {
    for (alias, locale) in [(Q::C, "C"), (Q::L1, l1), (Q::L2, l2)] {
        select.join_as(
            JoinType::LeftJoin,
            AppText::Table,
            alias,
            Cond::all()
                .add(Expr::col((alias, AppText::Rid)).equals((Q::A, Apps::Rid)))
                .add(Expr::col((alias, AppText::Locale)).eq(locale)),
        );
    }
}

fn description(l1: &str, l2: &str) -> Expr {
    let in_locale = |locale: &str| -> Expr {
        Expr::SubQuery(
            None,
            Box::new(
                Query::select()
                    .column(AppDesc::Description)
                    .from(AppDesc::Table)
                    .and_where(Expr::col(AppDesc::Rid).equals((Q::A, Apps::Rid)))
                    .and_where(Expr::col(AppDesc::Locale).eq(locale))
                    .to_owned()
                    .into(),
            ),
        )
    };
    Func::coalesce([in_locale(l1), in_locale(l2), in_locale("C"), Expr::val("")]).into()
}

pub(super) fn app_rows(
    conn: &Connection,
    locales: &[String],
    rids: &[i64],
    with_description: bool,
) -> rusqlite::Result<Vec<CatalogApp>> {
    if rids.is_empty() {
        return Ok(Vec::new());
    }
    let (l1, l2) = locale_params(locales);
    let rids = serde_json::to_string(rids).unwrap_or_else(|_| "[]".into());
    let mut select = Query::select();
    select
        .columns([
            (Q::A, Apps::Id),
            (Q::A, Apps::Provider),
            (Q::A, Apps::Remote),
            (Q::A, Apps::IconUrl),
            (Q::A, Apps::Version),
            (Q::A, Apps::IsRuntime),
            (Q::A, Apps::License),
            (Q::A, Apps::EulaUrl),
            (Q::A, Apps::HomepageUrl),
            (Q::A, Apps::ContentRating),
        ])
        // the forge decides when it knows the app and the remote's own catalogue otherwise
        .expr(Func::coalesce([Expr::col((Q::F, ForgeVerified::Verified)), Expr::col((Q::A, Apps::Verified))]))
        .columns([
            (Q::A, Apps::Categories),
            (Q::A, Apps::Screenshots),
            (Q::A, Apps::ProjectUrls),
        ])
        .expr(localized(AppText::Name, Some(Expr::val(""))))
        .expr(localized(AppText::Summary, Some(Expr::val(""))))
        .expr(if with_description {
            description(&l1, &l2)
        } else {
            Expr::val("")
        })
        .expr(Func::coalesce([
            Func::cust(Q::Nullif).args([Expr::col((Q::F, ForgeVerified::DeveloperName)), Expr::val("")]).into(),
            localized(AppText::Developer, None),
        ]))
        .expr(Expr::exists(
            Query::select()
                .expr(Expr::val(1))
                .from_as(Packages::Table, Q::I)
                .and_where(Expr::col((Q::I, Packages::Kind)).eq("installed"))
                .and_where(Expr::col((Q::I, Packages::Id)).equals((Q::A, Apps::Id)))
                .to_owned(),
        ))
        .from_function(Func::cust(Q::JsonEach).arg(rids), Q::R)
        .join_as(
            JoinType::InnerJoin,
            Apps::Table,
            Q::A,
            Expr::col((Q::A, Apps::Rid)).equals((Q::R, Q::Value)),
        );
    join_texts(&mut select, &l1, &l2);
    select.join_as(
        JoinType::LeftJoin,
        ForgeVerified::Table,
        Q::F,
        Expr::col((Q::F, ForgeVerified::Id)).equals((Q::A, Apps::Id)),
    );
    select.order_by((Q::R, Q::Key), Order::Asc);
    rows(conn, &select, map_app)
}

fn rids_for(id: &str) -> SelectStatement {
    Query::select()
        .column(Apps::Rid)
        .from(Apps::Table)
        .and_where(Expr::col(Apps::Id).eq(id))
        .union(
            UnionType::All,
            Query::select()
                .column(Apps::Rid)
                .from(Apps::Table)
                .and_where(Expr::col(Apps::Cid).eq(id))
                .to_owned(),
        )
        .to_owned()
}

fn visible() -> Expr {
    Expr::col(Apps::Shadowed).add(0).eq(0)
}

pub fn get(id: &str, locales: &[String]) -> Option<CatalogApp> {
    let id = id.strip_suffix(".desktop").unwrap_or(id);
    read(|conn| {
        let find = Query::select()
            .column(Apps::Rid)
            .from(Apps::Table)
            .and_where(Expr::col(Apps::Rid).in_subquery(rids_for(id)))
            .and_where(visible())
            .order_by_expr(Expr::col(Apps::Id).eq(id), Order::Desc)
            .order_by_columns([
                (Apps::Prio, Order::Asc),
                (Apps::Srank, Order::Asc),
                (Apps::Ord, Order::Asc),
            ])
            .limit(1)
            .to_owned();
        let Some(rid) = row(conn, &find, |r| r.get::<_, i64>(0))? else {
            return Ok(None);
        };
        Ok(app_rows(conn, locales, &[rid], true)?.into_iter().next())
    })
}

pub fn by_category(
    category: &str,
    locales: &[String],
    priority: &[String],
    offset: u64,
    limit: u64,
) -> SearchPage {
    let priority = serde_json::to_string(priority).unwrap_or_else(|_| "[]".into());
    let members = Query::select()
        .expr_as(Expr::col((Q::A, Apps::Rid)), Apps::Rid)
        .expr_as(Expr::col((Q::P, Q::Key)), Q::Pkey)
        .expr_as(Expr::col((Q::A, Apps::Prio)), Apps::Prio)
        .expr_as(Expr::col((Q::A, Apps::Srank)), Apps::Srank)
        .expr_as(Expr::col((Q::A, Apps::Ord)), Apps::Ord)
        .from_as(AppCategory::Table, Q::K)
        .join_as(
            JoinType::InnerJoin,
            Apps::Table,
            Q::A,
            Expr::col((Q::A, Apps::Rid)).equals((Q::K, AppCategory::Rid)),
        )
        .join_as(
            JoinType::LeftJoin,
            sea_query::TableRef::FunctionCall(
                Func::cust(Q::JsonEach).arg(priority),
                Q::P.into_iden(),
            ),
            Q::P,
            Expr::col((Q::P, Q::Value)).equals((Q::A, Apps::Id)),
        )
        .and_where(Expr::col((Q::K, AppCategory::Category)).eq(category.to_lowercase()))
        .and_where(Expr::col((Q::A, Apps::Shadowed)).eq(0))
        .and_where(Expr::col((Q::A, Apps::Listable)).eq(1))
        .to_owned();
    let order = |select: &mut SelectStatement, table: Option<Q>| {
        let col = |c: DynIden| match table {
            Some(t) => Expr::col((t, c)),
            None => Expr::col(c),
        };
        select.order_by_expr(col(Q::Pkey.into_iden()).is_null(), Order::Asc);
        for c in [
            Q::Pkey.into_iden(),
            Apps::Prio.into_iden(),
            Apps::Srank.into_iden(),
            Apps::Ord.into_iden(),
        ] {
            select.order_by_expr(col(c), Order::Asc);
        }
    };
    let mut page = Query::select();
    page.column(sea_query::Asterisk)
        .from(Q::Members)
        .limit(limit)
        .offset(offset);
    order(&mut page, None);
    let stats = Query::select()
        .expr_as(Func::count(Expr::col(sea_query::Asterisk)), Q::Total)
        .from(Q::Members)
        .to_owned();
    let mut outer = Query::select();
    outer
        .column((Q::S, Q::Total))
        .column((Q::P, Apps::Rid))
        .from_as(Q::Stats, Q::S)
        .join_as(JoinType::LeftJoin, Q::Page, Q::P, Expr::val(true));
    order(&mut outer, Some(Q::P));
    let query = outer.with(
        sea_query::WithClause::new()
            .cte(
                sea_query::CommonTableExpression::new()
                    .query(members)
                    .table_name(Q::Members)
                    .materialized(true)
                    .to_owned(),
            )
            .cte(
                sea_query::CommonTableExpression::new()
                    .query(stats)
                    .table_name(Q::Stats)
                    .to_owned(),
            )
            .cte(
                sea_query::CommonTableExpression::new()
                    .query(page)
                    .table_name(Q::Page)
                    .to_owned(),
            )
            .to_owned(),
    );
    read(|conn| {
        let mut total = 0u64;
        let rids: Vec<i64> = rows(conn, &query, |r| {
            total = r.get::<_, i64>(0)? as u64;
            r.get::<_, Option<i64>>(1)
        })?
        .into_iter()
        .flatten()
        .collect();
        Ok(SearchPage {
            total,
            items: app_rows(conn, locales, &rids, false)?,
            providers: Vec::new(),
        })
    })
}

pub fn appstream_slice(locales: &[String], limit: u64, newest_first: bool) -> Vec<CatalogApp> {
    let order = if newest_first {
        Order::Desc
    } else {
        Order::Asc
    };
    let select = Query::select()
        .column(Apps::Rid)
        .from(Apps::Table)
        .and_where(Expr::col(Apps::Shadowed).eq(0))
        .and_where(Expr::col(Apps::Listable).eq(1))
        .and_where(Expr::col(Apps::Prio).lt(PRIO_PROVIDER))
        .order_by(Apps::Srank, order.clone())
        .order_by(Apps::Ord, order)
        .limit(limit)
        .to_owned();
    read(|conn| {
        let rids = rows(conn, &select, |r| r.get::<_, i64>(0))?;
        app_rows(conn, locales, &rids, true)
    })
}

pub fn remote_of(id: &str) -> Option<String> {
    let select = Query::select()
        .column(Apps::Remote)
        .from(Apps::Table)
        .and_where(Expr::col(Apps::Rid).in_subquery(rids_for(id)))
        .and_where(visible())
        .and_where(Expr::col(Apps::Remote).is_not_null())
        .order_by_columns([
            (Apps::Prio, Order::Asc),
            (Apps::Srank, Order::Asc),
            (Apps::Ord, Order::Asc),
        ])
        .limit(1)
        .to_owned();
    read(|conn| row(conn, &select, |r| r.get(0)))
}

pub fn name_of(id: &str) -> Option<String> {
    get(id, &crate::appstream_db::system_locales())
        .map(|a| a.name)
        .filter(|n| !n.is_empty())
}
