use super::db::{exec, read, row, rows, write, write_untracked};
use super::model::provider_str;
use super::tables::{Apps, ForgeVerified, InstalledRuntimes, Meta, Packages, RemoteRefs, Q};
use libarc::{Package, Provider};
use rusqlite::{Connection, Transaction};
use sea_query::{Cond, Expr, ExprTrait, Query};
use std::time::Duration;

const INSTALLED: &str = "installed";
const UPDATES: &str = "updates";

pub(super) fn fingerprint(value: &(impl serde::Serialize + ?Sized)) -> i64 {
    let json = serde_json::to_vec(value).unwrap_or_default();
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in json {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash as i64
}

pub(super) fn meta_int(key: &str) -> Option<i64> {
    read(|conn| {
        row(
            conn,
            Query::select()
                .column(Meta::Value)
                .from(Meta::Table)
                .and_where(Expr::col(Meta::Key).eq(key)),
            |r| r.get(0),
        )
    })
}

pub(super) fn set_meta(conn: &Connection, key: &str, value: i64) -> rusqlite::Result<()> {
    exec(
        conn,
        Query::insert()
            .replace()
            .into_table(Meta::Table)
            .columns([Meta::Key, Meta::Value])
            .values_panic([key.into(), value.into()]),
    )
    .map(|_| ())
}

fn replace_if_changed(
    key: &str,
    value: &impl serde::Serialize,
    fill: impl FnOnce(&Transaction) -> rusqlite::Result<()>,
) -> bool {
    let fp = fingerprint(value);
    if meta_int(key) == Some(fp) {
        return true;
    }
    write(|tx| {
        set_meta(tx, key, fp)?;
        fill(tx)
    })
    .is_some()
}

fn replace_packages(kind: &str, provider: &Provider, packages: &[Package]) -> bool {
    let provider = provider_str(provider);
    replace_if_changed(&format!("fp:{kind}:{provider}"), &packages, |tx| {
        exec(
            tx,
            Query::delete()
                .from_table(Packages::Table)
                .and_where(Expr::col(Packages::Kind).eq(kind))
                .and_where(Expr::col(Packages::Provider).eq(provider)),
        )?;
        for pkg in packages {
            exec(
                tx,
                Query::insert()
                    .replace()
                    .into_table(Packages::Table)
                    .columns([
                        Packages::Kind,
                        Packages::Id,
                        Packages::Provider,
                        Packages::Package,
                    ])
                    .values_panic([
                        kind.into(),
                        pkg.id.as_str().into(),
                        provider.into(),
                        serde_json::to_string(pkg).unwrap_or_default().into(),
                    ]),
            )?;
        }
        Ok(())
    })
}

fn list_packages(kind: &str) -> Vec<Package> {
    let select = Query::select()
        .column(Packages::Package)
        .from(Packages::Table)
        .and_where(Expr::col(Packages::Kind).eq(kind))
        .order_by(Packages::Id, sea_query::Order::Asc)
        .to_owned();
    read(|conn| {
        let json = rows(conn, &select, |r| r.get::<_, String>(0))?;
        Ok(json
            .iter()
            .filter_map(|j| serde_json::from_str::<Package>(j).ok())
            .collect())
    })
}

pub fn replace_installed(provider: &Provider, packages: &[Package]) -> bool {
    replace_packages(INSTALLED, provider, packages)
}

pub fn list_installed() -> Vec<Package> {
    list_packages(INSTALLED)
}

pub fn is_installed(id: &str) -> bool {
    let select = Query::select()
        .expr(Expr::exists(
            Query::select()
                .expr(Expr::val(1))
                .from(Packages::Table)
                .and_where(Expr::col(Packages::Kind).eq(INSTALLED))
                .and_where(Expr::col(Packages::Id).eq(id))
                .to_owned(),
        ))
        .to_owned();
    read(|conn| row(conn, &select, |r| r.get(0)).map(|v| v.unwrap_or(false)))
}

pub fn replace_updates(provider: &Provider, packages: &[Package]) -> bool {
    replace_packages(UPDATES, provider, packages)
}

pub fn list_updates() -> Vec<Package> {
    list_packages(UPDATES)
}

pub fn remove_update(id: &str) -> bool {
    write(|tx| {
        exec(
            tx,
            Query::delete()
                .from_table(Packages::Table)
                .and_where(Expr::col(Packages::Kind).eq(UPDATES))
                .and_where(Expr::col(Packages::Id).eq(id)),
        )
        .map(|_| ())
    })
    .is_some()
}

pub fn installed_remote_icons() -> Vec<String> {
    let select = Query::select()
        .distinct()
        .column((Q::A, Apps::IconUrl))
        .from_as(Packages::Table, Q::I)
        .join_as(
            sea_query::JoinType::InnerJoin,
            Apps::Table,
            Q::A,
            Expr::col((Q::A, Apps::Id)).equals((Q::I, Packages::Id)),
        )
        .and_where(Expr::col((Q::I, Packages::Kind)).eq(INSTALLED))
        .and_where(Expr::col((Q::A, Apps::Shadowed)).eq(0))
        .cond_where(
            Cond::any()
                .add(Expr::col((Q::A, Apps::IconUrl)).like("http://%"))
                .add(Expr::col((Q::A, Apps::IconUrl)).like("https://%")),
        )
        .to_owned();
    read(|conn| rows(conn, &select, |r| r.get(0)))
}

pub fn replace_installed_runtimes(runtimes: &[(String, String)]) -> bool {
    replace_if_changed("fp:installed_runtimes", &runtimes, |tx| {
        exec(
            tx,
            &Query::delete()
                .from_table(InstalledRuntimes::Table)
                .to_owned(),
        )?;
        for (name, display) in runtimes {
            exec(
                tx,
                Query::insert()
                    .replace()
                    .into_table(InstalledRuntimes::Table)
                    .columns([InstalledRuntimes::Name, InstalledRuntimes::Display])
                    .values_panic([name.as_str().into(), display.as_str().into()]),
            )?;
        }
        Ok(())
    })
}

pub fn replace_remote_refs(refs: &[(String, String, String)]) -> bool {
    replace_if_changed("fp:remote_refs", &refs, |tx| {
        exec(
            tx,
            &Query::delete().from_table(RemoteRefs::Table).to_owned(),
        )?;
        for (name, remote, branch) in refs {
            exec(
                tx,
                Query::insert()
                    .into_table(RemoteRefs::Table)
                    .columns([RemoteRefs::Name, RemoteRefs::Remote, RemoteRefs::Branch])
                    .values_panic([
                        name.as_str().into(),
                        remote.as_str().into(),
                        branch.as_str().into(),
                    ])
                    .on_conflict(sea_query::OnConflict::new().do_nothing().to_owned()),
            )?;
        }
        Ok(())
    })
}

fn prefix_range(prefix: &str) -> (String, String) {
    let mut upper = prefix.to_string();
    upper.push(char::MAX);
    (prefix.to_string(), upper)
}

pub fn installed_runtimes_with_prefix(prefix: &str) -> Vec<(String, String)> {
    let (low, high) = prefix_range(prefix);
    let select = Query::select()
        .columns([InstalledRuntimes::Name, InstalledRuntimes::Display])
        .from(InstalledRuntimes::Table)
        .and_where(Expr::col(InstalledRuntimes::Name).gte(low))
        .and_where(Expr::col(InstalledRuntimes::Name).lt(high))
        .to_owned();
    read(|conn| rows(conn, &select, |r| Ok((r.get(0)?, r.get(1)?))))
}

pub fn remote_refs_with_prefix(prefix: &str) -> Vec<(String, String, String)> {
    let (low, high) = prefix_range(prefix);
    let select = Query::select()
        .columns([RemoteRefs::Name, RemoteRefs::Remote, RemoteRefs::Branch])
        .from(RemoteRefs::Table)
        .and_where(Expr::col(RemoteRefs::Name).gte(low))
        .and_where(Expr::col(RemoteRefs::Name).lt(high))
        .to_owned();
    read(|conn| rows(conn, &select, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const UPDATES_CHECKED: &str = "updates_checked";

pub fn updates_checked_age() -> Option<Duration> {
    meta_int(UPDATES_CHECKED)
        .map(|t| Duration::from_secs(Ord::max(now_secs().saturating_sub(t), 0) as u64))
}

pub fn mark_updates_checked() {
    write_untracked(|conn| set_meta(conn, UPDATES_CHECKED, now_secs()));
}

#[derive(Clone, Default)]
pub struct ForgeVerification {
    pub verified: bool,
    pub developer_name: Option<String>,
}

pub fn forge_checked_age(id: &str) -> Option<Duration> {
    let select = Query::select()
        .column(ForgeVerified::CheckedAt)
        .from(ForgeVerified::Table)
        .and_where(Expr::col(ForgeVerified::Id).eq(id))
        .to_owned();
    let checked_at: Option<i64> = read(|conn| row(conn, &select, |r| r.get(0)));
    checked_at.map(|t| Duration::from_secs(Ord::max(now_secs().saturating_sub(t), 0) as u64))
}

pub fn store_forge_verification(answers: &[(String, Option<ForgeVerification>)]) -> bool {
    write(|tx| {
        for (id, answer) in answers {
            exec(
                tx,
                Query::insert()
                    .replace()
                    .into_table(ForgeVerified::Table)
                    .columns([
                        ForgeVerified::Id,
                        ForgeVerified::Verified,
                        ForgeVerified::DeveloperName,
                        ForgeVerified::CheckedAt,
                    ])
                    .values_panic([
                        id.as_str().into(),
                        answer.as_ref().map(|a| a.verified).into(),
                        answer
                            .as_ref()
                            .and_then(|a| a.developer_name.clone())
                            .into(),
                        now_secs().into(),
                    ]),
            )?;
        }
        Ok(())
    })
    .is_some()
}
