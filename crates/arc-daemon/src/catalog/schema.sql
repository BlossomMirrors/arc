CREATE TABLE sources(
    path TEXT PRIMARY KEY,
    remote TEXT,
    rank INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    size INTEGER NOT NULL
);

CREATE TABLE apps(
    rid INTEGER PRIMARY KEY,
    id TEXT NOT NULL,
    cid TEXT NOT NULL,
    id_lc TEXT NOT NULL,
    provider TEXT NOT NULL,
    source TEXT NOT NULL,
    prio INTEGER NOT NULL,
    srank INTEGER NOT NULL,
    ord INTEGER NOT NULL,
    shadowed INTEGER NOT NULL DEFAULT 0,
    listable INTEGER NOT NULL,
    remote TEXT,
    icon_url TEXT,
    version TEXT NOT NULL DEFAULT '',
    is_runtime INTEGER NOT NULL DEFAULT 0,
    license TEXT,
    eula_url TEXT,
    homepage_url TEXT,
    content_rating TEXT NOT NULL DEFAULT '',
    verified INTEGER NOT NULL DEFAULT 0,
    categories TEXT NOT NULL DEFAULT '[]',
    screenshots TEXT NOT NULL DEFAULT '[]',
    project_urls TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX apps_id ON apps(id);
CREATE INDEX apps_cid ON apps(cid);
CREATE INDEX apps_source ON apps(source);
CREATE INDEX apps_order ON apps(shadowed, listable, prio, srank, ord);

CREATE TABLE app_text(
    rid INTEGER NOT NULL REFERENCES apps(rid) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    name TEXT,
    summary TEXT,
    developer TEXT,
    name_lc TEXT,
    summary_lc TEXT,
    PRIMARY KEY(rid, locale)
) WITHOUT ROWID;

CREATE TABLE app_desc(
    rid INTEGER NOT NULL REFERENCES apps(rid) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    description TEXT NOT NULL,
    PRIMARY KEY(rid, locale)
) WITHOUT ROWID;

CREATE TABLE app_category(
    category TEXT NOT NULL,
    rid INTEGER NOT NULL REFERENCES apps(rid) ON DELETE CASCADE,
    PRIMARY KEY(category, rid)
) WITHOUT ROWID;
CREATE INDEX app_category_rid ON app_category(rid);

CREATE TABLE search(
    locale TEXT NOT NULL,
    rid INTEGER NOT NULL REFERENCES apps(rid) ON DELETE CASCADE,
    prio INTEGER NOT NULL,
    srank INTEGER NOT NULL,
    ord INTEGER NOT NULL,
    id TEXT NOT NULL,
    provider TEXT NOT NULL,
    id_lc TEXT NOT NULL,
    name_lc TEXT,
    summary_lc TEXT,
    PRIMARY KEY(locale, rid)
) WITHOUT ROWID;
CREATE INDEX search_rid ON search(rid);

CREATE TABLE packages(
    kind TEXT NOT NULL,
    id TEXT NOT NULL,
    provider TEXT NOT NULL,
    package TEXT NOT NULL,
    PRIMARY KEY(kind, id)
) WITHOUT ROWID;

-- the forge answer about an app where verified stays null when the forge does not know it
CREATE TABLE forge_verified(
    id TEXT PRIMARY KEY,
    verified INTEGER,
    developer_name TEXT,
    checked_at INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE installed_runtimes(name TEXT PRIMARY KEY, display TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE remote_refs(name TEXT PRIMARY KEY, remote TEXT NOT NULL, branch TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE meta(key TEXT PRIMARY KEY, value) WITHOUT ROWID;
