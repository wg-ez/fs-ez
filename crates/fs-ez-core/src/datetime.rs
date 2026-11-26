//! Formatage et lecture RFC3339 partagés par les adaptateurs.

use time::OffsetDateTime;
use time::UtcOffset;
use time::format_description::well_known::Rfc3339;

/// Décalage local à cet instant, UTC si le système ne peut pas le déterminer.
pub fn local_offset_at(value: OffsetDateTime) -> UtcOffset {
    UtcOffset::local_offset_at(value).unwrap_or(UtcOffset::UTC)
}

/// Même instant converti dans le fuseau local du processus (`TZ` honoré).
pub fn to_local(value: OffsetDateTime) -> OffsetDateTime {
    value.to_offset(local_offset_at(value))
}

/// Instant courant dans le fuseau local du processus (`TZ` honoré).
pub fn now_local() -> OffsetDateTime {
    to_local(OffsetDateTime::now_utc())
}

/// Formatage RFC3339, repli sur `Display` si la date sort du calendrier supporté.
pub fn format_rfc3339(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_else(|_| value.to_string())
}

/// Variante SQLite : normalise en UTC pour garder un ordre lexicographique stable.
pub fn format_rfc3339_utc(value: OffsetDateTime) -> String {
    format_rfc3339(value.to_offset(UtcOffset::UTC))
}

pub fn parse_rfc3339(raw: &str) -> Result<OffsetDateTime, time::error::Parse> {
    OffsetDateTime::parse(raw, &Rfc3339)
}

#[cfg(test)]
mod tests;
