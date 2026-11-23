//! Which automatic backups to keep: the newest one of each of the last days, weeks and months.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Grandfather-father-son retention, applied to each destination on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RetentionPolicy {
    /// Days with a backup, newest first, whose latest backup is kept.
    pub daily: u16,
    /// ISO weeks with a backup whose latest backup is kept.
    pub weekly: u16,
    /// Months with a backup whose latest backup is kept.
    pub monthly: u16,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            daily: 7,
            weekly: 4,
            monthly: 6,
        }
    }
}

impl RetentionPolicy {
    pub fn new(daily: u16, weekly: u16, monthly: u16) -> Self {
        Self {
            daily,
            weekly,
            monthly,
        }
    }

    /// Indices (into `created`) of the backups to keep. The newest backup is always kept,
    /// whatever the policy, so a run can never delete the backup it just wrote.
    pub fn keep(&self, created: &[OffsetDateTime]) -> HashSet<usize> {
        let mut order: Vec<usize> = (0..created.len()).collect();
        order.sort_by_key(|index| std::cmp::Reverse(created[*index]));

        let mut keep = HashSet::new();
        let mut days = HashSet::new();
        let mut weeks = HashSet::new();
        let mut months = HashSet::new();
        for (rank, index) in order.into_iter().enumerate() {
            let at = created[index].to_offset(time::UtcOffset::UTC);
            let day = (at.year(), at.ordinal());
            let (iso_year, week, _) = at.to_iso_week_date();
            let month = (at.year(), u8::from(at.month()));
            let mut kept = rank == 0;
            if !days.contains(&day) && days.len() < usize::from(self.daily) {
                days.insert(day);
                kept = true;
            }
            if !weeks.contains(&(iso_year, week)) && weeks.len() < usize::from(self.weekly) {
                weeks.insert((iso_year, week));
                kept = true;
            }
            if !months.contains(&month) && months.len() < usize::from(self.monthly) {
                months.insert(month);
                kept = true;
            }
            if kept {
                keep.insert(index);
            }
        }
        keep
    }
}

#[cfg(test)]
mod tests {
    use super::RetentionPolicy;
    use time::{Date, Duration, Month, OffsetDateTime, Time};

    /// `y-m-d 08:00 UTC`; `time` macros are not enabled for this crate.
    fn day(year: i32, month: u8, day: u8) -> OffsetDateTime {
        let month = Month::try_from(month).unwrap_or(Month::January);
        let date = Date::from_calendar_date(year, month, day).unwrap_or(Date::MIN);
        date.with_time(Time::from_hms(8, 0, 0).unwrap_or(Time::MIDNIGHT))
            .assume_utc()
    }

    fn kept_dates(policy: RetentionPolicy, created: &[OffsetDateTime]) -> Vec<OffsetDateTime> {
        let keep = policy.keep(created);
        let mut dates: Vec<_> = created
            .iter()
            .enumerate()
            .filter(|(index, _)| keep.contains(index))
            .map(|(_, at)| *at)
            .collect();
        dates.sort();
        dates
    }

    #[test]
    fn keeps_the_latest_backup_of_each_recent_day() {
        let base = day(2026, 11, 23);
        // Two backups a day for ten days.
        let created: Vec<_> = (0..20)
            .map(|i| base - Duration::hours(12 * i64::from(i)))
            .collect();
        let kept = kept_dates(RetentionPolicy::new(3, 0, 0), &created);
        assert_eq!(
            kept,
            vec![
                day(2026, 11, 21) + Duration::hours(12),
                day(2026, 11, 22) + Duration::hours(12),
                day(2026, 11, 23),
            ]
        );
    }

    #[test]
    fn weeks_and_months_extend_the_history() {
        let base = day(2026, 11, 23);
        let created: Vec<_> = (0..200).map(|i| base - Duration::days(i)).collect();
        let kept = kept_dates(RetentionPolicy::default(), &created);
        // 7 days, then the latest of 4 weeks and 6 months, overlapping where they coincide.
        assert!(kept.contains(&base));
        assert!(kept.contains(&day(2026, 11, 17)), "7th day");
        assert!(!kept.contains(&day(2026, 11, 16)), "Monday of week 47");
        assert!(kept.contains(&day(2026, 11, 15)), "Sunday, week 46");
        assert!(kept.contains(&day(2026, 10, 31)), "end of October");
        assert!(kept.contains(&day(2026, 6, 30)), "6th month");
        assert!(!kept.contains(&day(2026, 5, 31)), "7th month dropped");
        assert!(kept.len() <= 7 + 4 + 6);
    }

    #[test]
    fn the_newest_backup_survives_an_empty_policy() {
        let created = [day(2026, 11, 20), day(2026, 11, 23)];
        assert_eq!(
            kept_dates(RetentionPolicy::new(0, 0, 0), &created),
            vec![day(2026, 11, 23)]
        );
        assert!(RetentionPolicy::default().keep(&[]).is_empty());
    }
}
