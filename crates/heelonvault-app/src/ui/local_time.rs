//! Local wall-clock time for the desktop UI.
//!
//! `time::UtcOffset::current_local_offset()` refuses to run once the process has
//! several threads (reading the zone through `localtime_r` would race `setenv`),
//! and the app always runs a Tokio runtime. GLib resolves the local zone from the
//! system tz database instead, is thread-safe and applies the DST rules of the
//! instant being converted, so every local display goes through this module.
//! Instants are stored and compared in UTC with `time`; only rendering is local.

use gtk4::glib;
#[cfg(feature = "premium")]
use heelonvault_core::services::local_time_zone::LocalTimeZone;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// System local zone resolved through GLib, injected into core/premium services
/// that render local times (they cannot read the offset soundly themselves).
/// Only the premium audit report consumes it today.
#[cfg(feature = "premium")]
#[derive(Clone, Copy, Debug, Default)]
pub struct GlibLocalTimeZone;

#[cfg(feature = "premium")]
impl LocalTimeZone for GlibLocalTimeZone {
    fn offset_at(&self, instant: OffsetDateTime) -> time::UtcOffset {
        let offset = glib::DateTime::from_unix_local(instant.unix_timestamp())
            .ok()
            .and_then(|local| i32::try_from(local.utc_offset().as_seconds()).ok())
            .and_then(|seconds| time::UtcOffset::from_whole_seconds(seconds).ok());
        match offset {
            Some(offset) => offset,
            None => {
                tracing::warn!("local time zone unavailable, rendering in UTC");
                time::UtcOffset::UTC
            }
        }
    }
}

/// Current local time rendered with a `g_date_time_format` pattern
/// (`%H:%M`, `%d/%m/%Y %H:%M`, …).
///
/// Falls back to UTC, suffixed with ` UTC`, if GLib cannot resolve the local
/// zone, so the reader is never shown an unlabelled UTC time.
pub fn now_formatted(pattern: &str) -> String {
    match glib::DateTime::now_local().and_then(|now| now.format(pattern)) {
        Ok(text) => text.to_string(),
        Err(_) => glib::DateTime::now_utc()
            .and_then(|now| now.format(pattern))
            .map(|text| format!("{text} UTC"))
            .unwrap_or_default(),
    }
}

/// Current local time as RFC 3339 with its UTC offset
/// (e.g. `2026-03-17T14:40:06.123456+01:00`), for log records.
pub fn now_rfc3339() -> String {
    match glib::DateTime::now_local().and_then(|now| now.format_iso8601()) {
        Ok(text) => text.to_string(),
        Err(_) => OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default(),
    }
}

/// Current local date as `YYYYMMDD`, for file names (log rotation).
///
/// Falls back to the UTC date (unsuffixed, file names must stay stable) if
/// GLib cannot resolve the local zone.
pub fn today_stamp() -> String {
    match glib::DateTime::now_local().and_then(|now| now.format("%Y%m%d")) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let today = OffsetDateTime::now_utc().date();
            format!(
                "{:04}{:02}{:02}",
                today.year(),
                u8::from(today.month()),
                today.day()
            )
        }
    }
}

/// Parses a timestamp written by the database layer.
///
/// Accepts RFC 3339 (any offset) and the offset-less SQLite forms
/// `YYYY-MM-DD HH:MM:SS` / `YYYY-MM-DDTHH:MM:SS`, which are UTC by construction.
pub fn parse_utc_timestamp(raw: &str) -> Option<OffsetDateTime> {
    let raw = raw.trim();
    if let Ok(instant) = OffsetDateTime::parse(raw, &Rfc3339) {
        return Some(instant);
    }
    let normalized = format!("{}Z", raw.replacen(' ', "T", 1));
    OffsetDateTime::parse(&normalized, &Rfc3339).ok()
}

/// Converts a UTC instant to the local zone, with the DST offset in force at
/// that instant (not the current one).
pub fn to_local(instant: OffsetDateTime) -> Option<glib::DateTime> {
    glib::DateTime::from_unix_local(instant.unix_timestamp()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339_and_sqlite_utc_forms_to_the_same_instant() {
        let expected = OffsetDateTime::from_unix_timestamp(1_774_000_000).ok();
        assert!(expected.is_some());
        assert_eq!(parse_utc_timestamp("2026-03-20T09:46:40Z"), expected);
        assert_eq!(parse_utc_timestamp("2026-03-20T10:46:40+01:00"), expected);
        assert_eq!(parse_utc_timestamp("2026-03-20 09:46:40"), expected);
        assert_eq!(parse_utc_timestamp("2026-03-20T09:46:40"), expected);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_utc_timestamp(""), None);
        assert_eq!(parse_utc_timestamp("not a date"), None);
        assert_eq!(parse_utc_timestamp("2026-13-40 99:99:99"), None);
    }

    #[cfg(feature = "premium")]
    #[test]
    fn glib_zone_agrees_with_glib_local_conversion() -> Result<(), time::error::ComponentRange> {
        let instant = OffsetDateTime::from_unix_timestamp(1_774_000_000)?;
        let local = GlibLocalTimeZone.to_local(instant);
        assert_eq!(local, instant);
        let glib_local = to_local(instant);
        assert_eq!(
            glib_local.map(|value| (value.hour(), value.minute())),
            Some((i32::from(local.hour()), i32::from(local.minute())))
        );
        Ok(())
    }

    #[test]
    fn local_conversion_preserves_the_instant() {
        let instant = OffsetDateTime::from_unix_timestamp(1_774_000_000).ok();
        let local = instant.and_then(to_local);
        assert_eq!(local.map(|value| value.to_unix()), Some(1_774_000_000));
    }
}
