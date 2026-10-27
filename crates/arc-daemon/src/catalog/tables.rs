use sea_query::Iden;

#[derive(Iden, Clone, Copy)]
pub enum Sources {
    Table,
    Path,
    Remote,
    Rank,
    Mtime,
    Size,
}

#[derive(Iden, Clone, Copy)]
pub enum Apps {
    Table,
    Rid,
    Id,
    Cid,
    IdLc,
    Provider,
    Source,
    Prio,
    Srank,
    Ord,
    Shadowed,
    Listable,
    Remote,
    IconUrl,
    Version,
    IsRuntime,
    License,
    EulaUrl,
    HomepageUrl,
    ContentRating,
    Verified,
    Categories,
    Screenshots,
    ProjectUrls,
}

#[derive(Iden, Clone, Copy)]
pub enum AppText {
    Table,
    Rid,
    Locale,
    Name,
    Summary,
    Developer,
    NameLc,
    SummaryLc,
}

#[derive(Iden, Clone, Copy)]
pub enum AppDesc {
    Table,
    Rid,
    Locale,
    Description,
}

#[derive(Iden, Clone, Copy)]
pub enum AppCategory {
    Table,
    Category,
    Rid,
}

#[derive(Iden, Clone, Copy)]
pub enum Search {
    Table,
    Locale,
    Rid,
    Prio,
    Srank,
    Ord,
    Id,
    Provider,
    IdLc,
    NameLc,
    SummaryLc,
}

#[derive(Iden, Clone, Copy)]
pub enum Packages {
    Table,
    Kind,
    Id,
    Provider,
    Package,
}

#[derive(Iden, Clone, Copy)]
pub enum InstalledRuntimes {
    Table,
    Name,
    Display,
}

#[derive(Iden, Clone, Copy)]
pub enum RemoteRefs {
    Table,
    Name,
    Remote,
    Branch,
}

#[derive(Iden, Clone, Copy)]
pub enum ForgeVerified {
    Table,
    Id,
    Verified,
    DeveloperName,
    CheckedAt,
}

#[derive(Iden, Clone, Copy)]
pub enum Meta {
    Table,
    Key,
    Value,
}

#[derive(Iden, Clone, Copy)]
pub enum Q {
    A,
    B,
    C,
    F,
    I,
    K,
    L1,
    L2,
    P,
    R,
    S,
    Ranked,
    Matched,
    Filtered,
    Stats,
    Page,
    Members,
    Score,
    NameSort,
    Total,
    Providers,
    Pkey,
    Key,
    Value,
    JsonEach,
    ArcScore,
    TotalChanges,
    GroupConcat,
    Nullif,
}
