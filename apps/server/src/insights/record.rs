//! Writes one analytics event and the daily aggregates it belongs to.
use glimwatch_insights::{DataValue, Incoming, VISIT_IDLE, day, visit_id};
use glimwatch_platform::{Database, Id};
use sqlx::{Any, Transaction};

pub async fn record(db: &Database, event: Incoming) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    insert(&mut tx, event).await?;
    tx.commit().await
}

fn bounce(pageviews: i64, events: i64) -> i64 {
    i64::from(pageviews == 1 && events == 0)
}

async fn visit(
    tx: &mut Transaction<'_, Any>,
    site: &str,
    session: &str,
    timestamp: i64,
    pageview: bool,
    custom: bool,
) -> Result<String, sqlx::Error> {
    let current: Option<(String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, started_at, last_at, pageviews, events FROM insight_visits \
         WHERE site_id = $1 AND session_id = $2 ORDER BY last_at DESC LIMIT 1",
    )
    .bind(site)
    .bind(session)
    .fetch_optional(&mut **tx)
    .await?;
    let (pageviews_delta, events_delta) = (i64::from(pageview), i64::from(custom));
    match current {
        Some((id, started_at, last_at, pageviews, events)) if timestamp - last_at <= VISIT_IDLE => {
            if pageviews_delta + events_delta == 0 {
                return Ok(id);
            }
            let next_pageviews = pageviews + pageviews_delta;
            let next_events = events + events_delta;
            let next_last = last_at.max(timestamp);
            sqlx::query(
                "UPDATE insight_visits SET last_at = $1, pageviews = $2, events = $3 WHERE id = $4",
            )
            .bind(next_last)
            .bind(next_pageviews)
            .bind(next_events)
            .bind(&id)
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO insight_daily (site_id, day, bounces, total_time) VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (site_id, day) DO UPDATE SET \
                 bounces = insight_daily.bounces + EXCLUDED.bounces, \
                 total_time = insight_daily.total_time + EXCLUDED.total_time",
            )
            .bind(site)
            .bind(day(started_at))
            .bind(bounce(next_pageviews, next_events) - bounce(pageviews, events))
            .bind(next_last - last_at)
            .execute(&mut **tx)
            .await?;
            Ok(id)
        }
        _ => {
            let id = visit_id(session, timestamp);
            if pageviews_delta + events_delta == 0 {
                return Ok(id);
            }
            sqlx::query(
                "INSERT INTO insight_visits (id, site_id, session_id, started_at, last_at, pageviews, events) \
                 VALUES ($1, $2, $3, $4, $4, $5, $6) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(&id)
            .bind(site)
            .bind(session)
            .bind(timestamp)
            .bind(pageviews_delta)
            .bind(events_delta)
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO insight_daily (site_id, day, visits, bounces) VALUES ($1, $2, 1, $3) \
                 ON CONFLICT (site_id, day) DO UPDATE SET \
                 visits = insight_daily.visits + 1, \
                 bounces = insight_daily.bounces + EXCLUDED.bounces",
            )
            .bind(site)
            .bind(day(timestamp))
            .bind(bounce(pageviews_delta, events_delta))
            .execute(&mut **tx)
            .await?;
            Ok(id)
        }
    }
}

async fn insert(tx: &mut Transaction<'_, Any>, event: Incoming) -> Result<(), sqlx::Error> {
    let day = day(event.timestamp);
    let performance = event.event_type == "performance";
    let pageview = event.event_type == "pageview";
    let custom = !pageview && !performance;
    let mut visit = None;
    if let (Some(site), Some(session)) = (&event.site_id, &event.session_id) {
        visit = Some(self::visit(tx, site, session, event.timestamp, pageview, custom).await?);
    }
    let event_id = Id::new().to_string();
    let [utm_source, utm_medium, utm_campaign, utm_content, utm_term] = &event.utm;
    sqlx::query(
        "INSERT INTO insight_events ( \
            id, site_id, link_id, timestamp, event_type, session_id, visit_id, pathname, url_query, \
            title, hostname, utm_source, utm_medium, utm_campaign, utm_content, utm_term, \
            referrer_domain, country, region, city, browser, os, device, screen, language, metadata, \
            lcp, inp, cls, fcp, ttfb \
         ) VALUES ( \
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31 \
         )",
    )
    .bind(&event_id)
    .bind(&event.site_id)
    .bind(&event.link_id)
    .bind(event.timestamp)
    .bind(&event.event_type)
    .bind(&event.session_id)
    .bind(&visit)
    .bind(&event.pathname)
    .bind(&event.url_query)
    .bind(&event.title)
    .bind(&event.hostname)
    .bind(utm_source)
    .bind(utm_medium)
    .bind(utm_campaign)
    .bind(utm_content)
    .bind(utm_term)
    .bind(&event.referrer_domain)
    .bind(&event.country)
    .bind(&event.region)
    .bind(&event.city)
    .bind(&event.browser)
    .bind(&event.os)
    .bind(&event.device)
    .bind(&event.screen)
    .bind(&event.language)
    .bind(&event.metadata)
    .bind(event.vitals.lcp)
    .bind(event.vitals.inp)
    .bind(event.vitals.cls)
    .bind(event.vitals.fcp)
    .bind(event.vitals.ttfb)
    .execute(&mut **tx)
    .await?;
    if let Some(site) = &event.site_id {
        for item in &event.data {
            write_data(
                tx,
                site,
                &event_id,
                &event.event_type,
                event.timestamp,
                item,
            )
            .await?;
        }
    }
    if performance {
        return Ok(());
    }
    if let Some(site) = &event.site_id {
        let visitor = if let Some(session) = &event.session_id {
            new_session(
                tx,
                "INSERT INTO insight_site_sessions (site_id, day, session_id) VALUES ($1, $2, $3) \
                 ON CONFLICT (site_id, day, session_id) DO NOTHING",
                site,
                day,
                session,
            )
            .await?
        } else {
            0
        };
        sqlx::query(
            "INSERT INTO insight_daily (site_id, day, pageviews, visitors, events) VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (site_id, day) DO UPDATE SET \
             pageviews = insight_daily.pageviews + EXCLUDED.pageviews, \
             visitors = insight_daily.visitors + EXCLUDED.visitors, \
             events = insight_daily.events + EXCLUDED.events",
        )
        .bind(site)
        .bind(day)
        .bind(i64::from(pageview))
        .bind(visitor)
        .bind(i64::from(custom))
        .execute(&mut **tx)
        .await?;
        if pageview {
            for (dimension, value) in [
                ("path", event.pathname.as_ref()),
                ("referrer", event.referrer_domain.as_ref()),
                ("country", event.country.as_ref()),
                ("browser", event.browser.as_ref()),
                ("os", event.os.as_ref()),
                ("device", event.device.as_ref()),
            ] {
                if let Some(value) = value {
                    sqlx::query(
                        "INSERT INTO insight_dimension_daily (site_id, day, dimension, value, count) \
                         VALUES ($1, $2, $3, $4, 1) \
                         ON CONFLICT (site_id, day, dimension, value) DO UPDATE SET \
                         count = insight_dimension_daily.count + 1",
                    )
                    .bind(site)
                    .bind(day)
                    .bind(dimension)
                    .bind(value)
                    .execute(&mut **tx)
                    .await?;
                }
            }
        }
    }
    if let Some(link) = &event.link_id {
        let visitor = if let Some(session) = &event.session_id {
            new_session(
                tx,
                "INSERT INTO insight_link_sessions (link_id, day, session_id) VALUES ($1, $2, $3) \
                 ON CONFLICT (link_id, day, session_id) DO NOTHING",
                link,
                day,
                session,
            )
            .await?
        } else {
            0
        };
        sqlx::query(
            "INSERT INTO insight_link_daily (link_id, day, clicks, visitors) VALUES ($1, $2, 1, $3) \
             ON CONFLICT (link_id, day) DO UPDATE SET \
             clicks = insight_link_daily.clicks + 1, \
             visitors = insight_link_daily.visitors + EXCLUDED.visitors",
        )
        .bind(link)
        .bind(day)
        .bind(visitor)
        .execute(&mut **tx)
        .await?;
        for (dimension, value) in [
            ("referrer", event.referrer_domain.as_ref()),
            ("country", event.country.as_ref()),
            ("browser", event.browser.as_ref()),
            ("os", event.os.as_ref()),
            ("device", event.device.as_ref()),
        ] {
            if let Some(value) = value {
                sqlx::query(
                    "INSERT INTO insight_link_dimension_daily (link_id, day, dimension, value, count) \
                     VALUES ($1, $2, $3, $4, 1) \
                     ON CONFLICT (link_id, day, dimension, value) DO UPDATE SET \
                     count = insight_link_dimension_daily.count + 1",
                )
                .bind(link)
                .bind(day)
                .bind(dimension)
                .bind(value)
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    Ok(())
}

async fn new_session(
    tx: &mut Transaction<'_, Any>,
    sql: &str,
    owner: &str,
    day: i64,
    session: &str,
) -> Result<i64, sqlx::Error> {
    let inserted = sqlx::query(sql)
        .bind(owner)
        .bind(day)
        .bind(session)
        .execute(&mut **tx)
        .await?
        .rows_affected();
    Ok(i64::try_from(inserted).unwrap_or(0))
}

async fn write_data(
    tx: &mut Transaction<'_, Any>,
    site: &str,
    event_id: &str,
    name: &str,
    timestamp: i64,
    item: &DataValue,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO insight_event_data ( \
            id, site_id, event_id, event_name, timestamp, key, string_value, number_value, data_type \
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(Id::new().to_string())
    .bind(site)
    .bind(event_id)
    .bind(name)
    .bind(timestamp)
    .bind(&item.key)
    .bind(&item.string_value)
    .bind(item.number_value)
    .bind(item.data_type)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
