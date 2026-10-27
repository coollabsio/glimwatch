//! Demo site, page views, and short links for `haruvo seed`.
use haruvo_insights::{Incoming, Vitals, now_secs, session_id};
use haruvo_platform::Id;

use super::record;
use crate::error::AppError;
use crate::state::AppState;

pub async fn seed_demo(state: &AppState) -> Result<(), AppError> {
    let now = now_secs();
    let site_id = Id::new().to_string();
    let public_id = "pub_coollabs_demo".to_owned();
    sqlx::query(
        "INSERT INTO insight_sites (id, public_id, name, domain, color, public_slug, public_enabled, public_threshold, public_sections, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, 1, 1, $7, $8)",
    )
    .bind(&site_id)
    .bind(&public_id)
    .bind("coolLabs website")
    .bind("coolify.io")
    .bind("#4f46e5")
    .bind("coollabs")
    .bind(r#"["pageviews","visitors","path","referrer","country","browser","os","device"]"#)
    .bind(now)
    .execute(state.db.pool())
    .await?;

    let domain_id = Id::new().to_string();
    sqlx::query(
        "INSERT INTO insight_domains (id, site_id, hostname, note, enabled, created_at) VALUES ($1, $2, $3, $4, 1, $5)",
    )
    .bind(&domain_id)
    .bind(&site_id)
    .bind("go.haruvo.test")
    .bind("Demo short domain. On this server, open /go/{slug}.")
    .bind(now)
    .execute(state.db.pool())
    .await?;

    let coolify = insert_link(
        state,
        &domain_id,
        "coolify",
        "https://coolify.io",
        "temporary",
        now,
    )
    .await?;
    let docs = insert_link(
        state,
        &domain_id,
        "docs",
        "https://coolify.io/docs",
        "temporary",
        now,
    )
    .await?;
    sqlx::query("UPDATE insight_links SET public_enabled = 1, public_threshold = 1, public_slug = $1 WHERE id = $2")
        .bind("coolify-link")
        .bind(&coolify)
        .execute(state.db.pool())
        .await?;

    let paths = ["/docs", "/", "/docs/installation", "/blog/self-hosting"];
    let referrers = [
        None,
        Some("github.com"),
        Some("news.ycombinator.com"),
        Some("google.com"),
    ];
    let countries = ["US", "DE", "NL", "JP"];
    let browsers = [
        ("Chrome", "macOS", "Desktop"),
        ("Firefox", "Linux", "Desktop"),
        ("Safari", "iOS", "Mobile"),
    ];
    let visitors = [
        "203.0.113.10",
        "203.0.113.11",
        "203.0.113.12",
        "203.0.113.13",
        "198.51.100.8",
    ];
    let agents = [
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/537.36 Chrome/128.0.0.0 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 Version/17.5 Mobile/15E148 Safari/604.1",
    ];

    for day in 0..14 {
        let base = now - (13 - day) * 86_400;
        for (index, path) in paths.iter().enumerate() {
            let visitor = index % visitors.len();
            let browser = browsers[visitor % browsers.len()];
            let timestamp = base - 4 * 3_600 + (index as i64) * 1_800;
            let session = session_id(
                &state.insights_secret,
                &site_id,
                visitors[visitor],
                agents[visitor % agents.len()],
                timestamp,
            );
            record::record(
                &state.db,
                Incoming {
                    site_id: Some(site_id.clone()),
                    timestamp,
                    event_type: "pageview".into(),
                    session_id: Some(session.clone()),
                    pathname: Some((*path).to_owned()),
                    title: Some(format!("coolLabs {path}")),
                    hostname: Some("coolify.io".into()),
                    utm: [Some("github".into()), None, None, None, None],
                    referrer_domain: referrers[index].map(str::to_owned),
                    country: Some(countries[index].into()),
                    browser: Some(browser.0.into()),
                    os: Some(browser.1.into()),
                    device: Some(browser.2.into()),
                    screen: Some("1440x900".into()),
                    language: Some("en-US".into()),
                    ..Incoming::default()
                },
            )
            .await?;
            if index == 0 {
                record::record(
                    &state.db,
                    Incoming {
                        site_id: Some(site_id.clone()),
                        timestamp: timestamp + 2,
                        event_type: "signup".into(),
                        session_id: Some(session),
                        pathname: Some((*path).to_owned()),
                        country: Some(countries[index].into()),
                        browser: Some(browser.0.into()),
                        os: Some(browser.1.into()),
                        device: Some(browser.2.into()),
                        metadata: Some(r#"{"plan":"hobby"}"#.into()),
                        data: vec![haruvo_insights::DataValue {
                            key: "plan".into(),
                            string_value: "hobby".into(),
                            number_value: None,
                            data_type: haruvo_insights::TYPE_STRING,
                        }],
                        ..Incoming::default()
                    },
                )
                .await?;
                record::record(
                    &state.db,
                    Incoming {
                        site_id: Some(site_id.clone()),
                        timestamp: timestamp + 3,
                        event_type: "performance".into(),
                        session_id: Some(session_id(
                            &state.insights_secret,
                            &site_id,
                            visitors[visitor],
                            agents[visitor % agents.len()],
                            timestamp,
                        )),
                        pathname: Some((*path).to_owned()),
                        device: Some(browser.2.into()),
                        vitals: Vitals {
                            lcp: Some(1800.0 + (day as f64) * 20.0),
                            inp: Some(120.0),
                            cls: Some(0.04),
                            fcp: Some(900.0),
                            ttfb: Some(180.0),
                        },
                        ..Incoming::default()
                    },
                )
                .await?;
            }
        }
        let click_session = session_id(
            &state.insights_secret,
            &coolify,
            visitors[day as usize % visitors.len()],
            agents[0],
            base,
        );
        record::record(
            &state.db,
            Incoming {
                link_id: Some(coolify.clone()),
                timestamp: base - 3_600,
                event_type: "click".into(),
                session_id: Some(click_session),
                referrer_domain: Some("github.com".into()),
                country: Some(countries[day as usize % countries.len()].into()),
                browser: Some("Chrome".into()),
                os: Some("macOS".into()),
                device: Some("Desktop".into()),
                ..Incoming::default()
            },
        )
        .await?;
        if day % 2 == 0 {
            record::record(
                &state.db,
                Incoming {
                    link_id: Some(docs.clone()),
                    timestamp: base - 1_800,
                    event_type: "click".into(),
                    session_id: Some(session_id(
                        &state.insights_secret,
                        &docs,
                        visitors[1],
                        agents[1],
                        base,
                    )),
                    referrer_domain: Some("google.com".into()),
                    country: Some("DE".into()),
                    browser: Some("Firefox".into()),
                    os: Some("Linux".into()),
                    device: Some("Desktop".into()),
                    ..Incoming::default()
                },
            )
            .await?;
        }
    }
    Ok(())
}

async fn insert_link(
    state: &AppState,
    domain_id: &str,
    slug: &str,
    destination: &str,
    redirect_type: &str,
    now: i64,
) -> Result<String, AppError> {
    let id = Id::new().to_string();
    sqlx::query(
        "INSERT INTO insight_links (id, domain_id, slug, destination, redirect_type, enabled, public_slug, public_sections, created_at) \
         VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8)",
    )
    .bind(&id)
    .bind(domain_id)
    .bind(slug)
    .bind(destination)
    .bind(redirect_type)
    .bind(format!("{slug}-public"))
    .bind(r#"["clicks","visitors","referrer","country","browser","os","device"]"#)
    .bind(now)
    .execute(state.db.pool())
    .await?;
    Ok(id)
}
