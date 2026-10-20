/// One UTC day's counters for a monitor. Maintenance is excluded from the
/// uptime ratio: a window where the only samples are maintenance has no ratio.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DailyCount {
    pub day: i64,
    pub success: i64,
    pub degraded: i64,
    pub failure: i64,
    pub maintenance: i64,
    pub latency_sum: i64,
    pub latency_count: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DayStatus {
    Up,
    Degraded,
    Down,
    Maintenance,
    Empty,
}

impl DayStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Degraded => "degraded",
            Self::Down => "down",
            Self::Maintenance => "maintenance",
            Self::Empty => "empty",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DayBucket {
    pub day: i64,
    pub status: DayStatus,
    pub uptime_ratio: Option<f64>,
    pub total: i64,
    pub avg_latency_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowStats {
    pub uptime_ratio: Option<f64>,
    pub avg_latency_ms: Option<i64>,
    pub days: Vec<DayBucket>,
}

pub fn summarize(stats: &[DailyCount], today: i64, window_days: i64) -> WindowStats {
    let window_days = window_days.max(1);
    let start = today - (window_days - 1);
    let mut merged: Vec<DailyCount> = Vec::new();
    for stat in stats {
        if !(start..=today).contains(&stat.day) {
            continue;
        }
        if let Some(existing) = merged.iter_mut().find(|item| item.day == stat.day) {
            existing.success += stat.success;
            existing.degraded += stat.degraded;
            existing.failure += stat.failure;
            existing.maintenance += stat.maintenance;
            existing.latency_sum += stat.latency_sum;
            existing.latency_count += stat.latency_count;
        } else {
            merged.push(stat.clone());
        }
    }

    let mut success = 0;
    let mut degraded = 0;
    let mut failure = 0;
    let mut latency_sum = 0;
    let mut latency_count = 0;
    let mut days = Vec::with_capacity(window_days as usize);
    for day in start..=today {
        let count = merged
            .iter()
            .find(|item| item.day == day)
            .cloned()
            .unwrap_or(DailyCount {
                day,
                ..DailyCount::default()
            });
        success += count.success;
        degraded += count.degraded;
        failure += count.failure;
        latency_sum += count.latency_sum;
        latency_count += count.latency_count;
        days.push(bucket(count));
    }

    WindowStats {
        uptime_ratio: ratio(success, degraded, failure),
        avg_latency_ms: average(latency_sum, latency_count),
        days,
    }
}

fn bucket(count: DailyCount) -> DayBucket {
    let status = if count.failure > 0 {
        DayStatus::Down
    } else if count.degraded > 0 {
        DayStatus::Degraded
    } else if count.success > 0 {
        DayStatus::Up
    } else if count.maintenance > 0 {
        DayStatus::Maintenance
    } else {
        DayStatus::Empty
    };
    DayBucket {
        day: count.day,
        status,
        uptime_ratio: ratio(count.success, count.degraded, count.failure),
        total: count.success + count.degraded + count.failure + count.maintenance,
        avg_latency_ms: average(count.latency_sum, count.latency_count),
    }
}

fn ratio(success: i64, degraded: i64, failure: i64) -> Option<f64> {
    let counted = success + degraded + failure;
    if counted == 0 {
        None
    } else {
        Some((success + degraded) as f64 / counted as f64)
    }
}

fn average(sum: i64, count: i64) -> Option<i64> {
    if count <= 0 { None } else { Some(sum / count) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_gaps_and_excludes_maintenance_from_the_ratio() {
        let stats = vec![
            DailyCount {
                day: 10,
                success: 9,
                degraded: 1,
                failure: 0,
                maintenance: 4,
                latency_sum: 1000,
                latency_count: 10,
            },
            DailyCount {
                day: 11,
                success: 0,
                degraded: 0,
                failure: 2,
                maintenance: 0,
                latency_sum: 40,
                latency_count: 2,
            },
        ];
        let window = summarize(&stats, 12, 3);
        assert_eq!(window.days.len(), 3);
        assert_eq!(window.days[0].status, DayStatus::Degraded);
        assert_eq!(window.days[1].status, DayStatus::Down);
        assert_eq!(window.days[2].status, DayStatus::Empty);
        // 10 reachable samples, 2 failures. Maintenance is ignored.
        assert!((window.uptime_ratio.unwrap() - (10.0 / 12.0)).abs() < 1e-9);
        assert_eq!(window.avg_latency_ms, Some(1040 / 12));
    }

    #[test]
    fn maintenance_only_day_has_no_ratio() {
        let stats = vec![DailyCount {
            day: 1,
            maintenance: 8,
            ..DailyCount::default()
        }];
        let window = summarize(&stats, 1, 1);
        assert_eq!(window.days[0].status, DayStatus::Maintenance);
        assert_eq!(window.uptime_ratio, None);
    }
}
