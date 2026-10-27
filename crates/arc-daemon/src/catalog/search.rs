use super::db::{read, rows};
use super::model::{locale_params, CatalogApp};
use super::query::{app_rows, join_texts};
use super::tables::{AppText, Apps, Packages, Search, Q};
use sea_query::{
    Asterisk, CommonTableExpression, Expr, ExprTrait, Func, JoinType, Order, Query,
    SelectStatement, WithClause, WithQuery,
};

#[derive(Default)]
pub struct SearchPage {
    pub total: u64,
    pub items: Vec<CatalogApp>,
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub provider: String,
    pub installed: i32,
    pub by_name: bool,
}

pub fn score(name: &str, id: &str, summary: &str, q: &str, fuzzy: bool) -> u32 {
    let field = |f: &str| {
        let s = libarc::score_field(f, q);
        if !fuzzy && s == 30 {
            0
        } else {
            s
        }
    };
    let summary_penalty = if fuzzy { 30 } else { 20 };
    Ord::max(
        Ord::max(field(name), field(id).saturating_sub(10)),
        field(summary).saturating_sub(summary_penalty),
    )
}

pub fn search(
    q: &str,
    locales: &[String],
    fuzzy: bool,
    filter: &SearchFilter,
    offset: u64,
    limit: u64,
) -> SearchPage {
    let (l1, l2) = locale_params(locales);
    let query = search_query(&q.to_lowercase(), &l1, &l2, fuzzy, filter, offset, limit);
    read(|conn| {
        let mut total = 0u64;
        let mut providers = String::new();
        let rids: Vec<i64> = rows(conn, &query, |r| {
            total = r.get::<_, i64>(0)? as u64;
            providers = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            r.get::<_, Option<i64>>(2)
        })?
        .into_iter()
        .flatten()
        .collect();
        Ok(SearchPage {
            total,
            items: app_rows(conn, locales, &rids, false)?,
            providers: providers
                .split(',')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect(),
        })
    })
}

fn search_query(
    q: &str,
    l1: &str,
    l2: &str,
    fuzzy: bool,
    filter: &SearchFilter,
    offset: u64,
    limit: u64,
) -> WithQuery {
    let score = Func::cust(Q::ArcScore).args([
        Expr::col(Search::NameLc),
        Expr::col(Search::IdLc),
        Expr::col(Search::SummaryLc),
        Expr::val(q),
        Expr::val(fuzzy),
    ]);
    let mut ranked = Query::select();
    ranked
        .column(Search::Rid)
        .expr_as(Func::min(Expr::col(Search::Provider)), Search::Provider)
        .expr_as(Func::min(Expr::col(Search::Prio)), Search::Prio)
        .expr_as(Func::min(Expr::col(Search::Srank)), Search::Srank)
        .expr_as(Func::min(Expr::col(Search::Ord)), Search::Ord)
        .expr_as(Func::max(score), Q::Score)
        .from(Search::Table)
        .and_where(Expr::col(Search::Locale).is_in(["C", l1, l2]))
        .group_by_col(Search::Rid)
        .and_having(Expr::col(Q::Score).gt(0));

    if filter.installed != 0 {
        ranked.expr_as(Func::min(Expr::col(Search::Id)), Search::Id);
    }

    let mut matched = Query::select();
    matched.column(Asterisk).from(Q::Ranked);
    if filter.installed != 0 {
        let installed = Query::select()
            .expr(Expr::val(1))
            .from(Packages::Table)
            .and_where(Expr::col(Packages::Kind).eq("installed"))
            .and_where(Expr::col((Packages::Table, Packages::Id)).equals((Q::Ranked, Search::Id)))
            .to_owned();
        matched.and_where(if filter.installed == 1 {
            Expr::exists(installed)
        } else {
            Expr::not_exists(installed)
        });
    }

    let mut filtered = Query::select();
    filtered.column(Asterisk).from(Q::Matched);
    if !filter.provider.is_empty() {
        filtered.and_where(Expr::col(Search::Provider).eq(filter.provider.as_str()));
    }

    let stats = Query::select()
        .expr_as(
            Expr::SubQuery(
                None,
                Box::new(
                    Query::select()
                        .expr(Func::count(Expr::col(Asterisk)))
                        .from(Q::Filtered)
                        .to_owned()
                        .into(),
                ),
            ),
            Q::Total,
        )
        .expr_as(
            Expr::SubQuery(
                None,
                Box::new(
                    Query::select()
                        .expr(Func::cust(Q::GroupConcat).arg(Expr::col(Search::Provider)))
                        .from_subquery(
                            Query::select()
                                .distinct()
                                .column(Search::Provider)
                                .from(Q::Matched)
                                .order_by(Search::Provider, Order::Asc)
                                .to_owned(),
                            Q::P,
                        )
                        .to_owned()
                        .into(),
                ),
            ),
            Q::Providers,
        )
        .to_owned();

    let mut page = Query::select();
    page.columns([Search::Rid, Search::Prio, Search::Srank, Search::Ord])
        .column(Q::Score)
        .from(Q::Filtered);
    if filter.by_name {
        let mut name = Query::select();
        name.expr(Func::coalesce([
            Expr::col((Q::L1, AppText::NameLc)),
            Expr::col((Q::L2, AppText::NameLc)),
            Expr::col((Q::C, AppText::NameLc)),
            Expr::col((Q::A, Apps::IdLc)),
        ]))
        .from_as(Apps::Table, Q::A)
        .and_where(Expr::col((Q::A, Apps::Rid)).equals((Q::Filtered, Search::Rid)));
        join_texts(&mut name, l1, l2);
        page.expr_as(Expr::SubQuery(None, Box::new(name.into())), Q::NameSort);
        page.order_by(Q::NameSort, Order::Asc);
    }
    page.order_by(Q::Score, Order::Desc)
        .order_by(Search::Prio, Order::Asc)
        .order_by(Search::Srank, Order::Asc)
        .order_by(Search::Ord, Order::Asc)
        .limit(limit)
        .offset(offset);

    let mut outer = Query::select();
    outer
        .column((Q::S, Q::Total))
        .column((Q::S, Q::Providers))
        .column((Q::P, Search::Rid))
        .from_as(Q::Stats, Q::S)
        .join_as(JoinType::LeftJoin, Q::Page, Q::P, Expr::val(true));
    if filter.by_name {
        outer.order_by((Q::P, Q::NameSort), Order::Asc);
    }
    outer
        .order_by((Q::P, Q::Score), Order::Desc)
        .order_by((Q::P, Search::Prio), Order::Asc)
        .order_by((Q::P, Search::Srank), Order::Asc)
        .order_by((Q::P, Search::Ord), Order::Asc);

    let cte = |query: SelectStatement, name: Q, materialized: bool| {
        CommonTableExpression::new()
            .query(query)
            .table_name(name)
            .materialized(materialized)
            .to_owned()
    };
    outer.with(
        WithClause::new()
            .cte(cte(ranked.to_owned(), Q::Ranked, true))
            .cte(cte(matched, Q::Matched, false))
            .cte(cte(filtered, Q::Filtered, false))
            .cte(
                CommonTableExpression::new()
                    .query(stats)
                    .table_name(Q::Stats)
                    .to_owned(),
            )
            .cte(
                CommonTableExpression::new()
                    .query(page)
                    .table_name(Q::Page)
                    .to_owned(),
            )
            .to_owned(),
    )
}
