//! Local time zone abstraction for rendering UTC instants to the user.
//!
//! The library stores and compares every instant in UTC. Reading the local UTC
//! offset is not sound from a library: `time::UtcOffset::current_local_offset()`
//! refuses to run once the process has several threads (it would race
//! `localtime_r` with `setenv`). The desktop binary therefore supplies an
//! implementation backed by the system tz database, and services that render
//! local times (e.g. the premium audit report) take it as a dependency.

use time::{OffsetDateTime, UtcOffset};

/// Resolves the user's local UTC offset for a given instant.
pub trait LocalTimeZone: Send + Sync + 'static {
    /// UTC offset in force in the local zone at `instant`.
    ///
    /// Implementations must apply the DST rules of `instant` itself, not the
    /// current offset, so that past events render with the offset they had.
    fn offset_at(&self, instant: OffsetDateTime) -> UtcOffset;

    /// `instant` converted to local wall-clock time.
    fn to_local(&self, instant: OffsetDateTime) -> OffsetDateTime {
        instant.to_offset(self.offset_at(instant))
    }

    /// The current local wall-clock time.
    fn now_local(&self) -> OffsetDateTime {
        self.to_local(OffsetDateTime::now_utc())
    }
}

/// Zone that always renders in UTC: default for headless use and tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct UtcTimeZone;

impl LocalTimeZone for UtcTimeZone {
    fn offset_at(&self, _instant: OffsetDateTime) -> UtcOffset {
        UtcOffset::UTC
    }
}

/// Zone with a fixed offset (no DST), mainly for deterministic tests.
#[derive(Clone, Copy, Debug)]
pub struct FixedOffsetTimeZone(pub UtcOffset);

impl LocalTimeZone for FixedOffsetTimeZone {
    fn offset_at(&self, _instant: OffsetDateTime) -> UtcOffset {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_zone_keeps_the_instant_and_offset() -> Result<(), time::error::ComponentRange> {
        let instant = OffsetDateTime::from_unix_timestamp(1_774_000_000)?;
        let local = UtcTimeZone.to_local(instant);
        assert_eq!(local, instant);
        assert_eq!(local.offset(), UtcOffset::UTC);
        Ok(())
    }

    #[test]
    fn fixed_offset_shifts_wall_clock_but_not_the_instant()
    -> Result<(), time::error::ComponentRange> {
        let instant = OffsetDateTime::from_unix_timestamp(1_774_000_000)?;
        let zone = FixedOffsetTimeZone(UtcOffset::from_hms(1, 0, 0)?);
        let local = zone.to_local(instant);
        assert_eq!(local, instant);
        assert_eq!(local.hour(), instant.hour() + 1);
        Ok(())
    }
}
