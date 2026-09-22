//! §18: "A visual's frame **cannot leave the ratio** of the chosen format" and
//! "Every visual for an event is produced in one action, in every format, true
//! to the brand"

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

async fn format_id(app: &TestApp, key: &str) -> Uuid {
    sqlx::query_as::<_, (Uuid,)>("SELECT id FROM formats WHERE key = $1 AND collective_id IS NULL")
        .bind(key)
        .fetch_one(&app.db)
        .await
        .unwrap()
        .0
}

#[tokio::test]
async fn the_format_catalogue_carries_the_prd_ratios() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    let res = antoine
        .get(&format!("/api/collectives/{cid}/studio/formats"))
        .await;
    let formats = res.expect_ok();
    let trouve = |key: &str| {
        formats
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["key"] == key)
            .unwrap_or_else(|| panic!("format {key} missing"))
            .clone()
    };

    assert_eq!(trouve("ig_square")["ratio"], "1:1");
    assert_eq!(trouve("ig_portrait")["ratio"], "4:5");
    assert_eq!(trouve("ig_story")["ratio"], "9:16");
    assert_eq!(trouve("yt_thumbnail")["ratio"], "16:9");
    assert_eq!(trouve("yt_banner")["width"], 2560);
    assert_eq!(trouve("poster_a3")["kind"], "print");
}

#[tokio::test]
async fn the_catalogue_extends_without_touching_the_code() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    let res = antoine
        .post(
            &format!("/api/collectives/{cid}/studio/formats"),
            json!({
                "key": "affiche_club",
                "platform": "Affiche",
                "label": "Format club",
                "width": 1200,
                "height": 1600
            }),
        )
        .await;
    assert_eq!(res.expect_ok()["ratio"], "3:4");

    let res = antoine
        .get(&format!("/api/collectives/{cid}/studio/formats"))
        .await;
    let custom = res
        .expect_ok()
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == "affiche_club")
        .cloned()
        .unwrap();
    assert_eq!(custom["custom"], true);
}

#[tokio::test]
async fn the_frame_cannot_leave_the_formats_ratio() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let story = format_id(&app, "ig_story").await;

    let templates = antoine
        .get(&format!("/api/collectives/{cid}/studio/templates"))
        .await;
    let tid = templates.expect_ok()[0]["id"].as_str().unwrap().to_string();

    // Try to save a story variant with made-up dimensions.
    antoine
        .put(
            &format!("/api/collectives/{cid}/studio/templates/{tid}/variants/{story}"),
            json!({ "layout": { "version": 1, "width": 999, "height": 111, "blocks": [] } }),
        )
        .await
        .expect_ok();

    let reloaded = antoine
        .get(&format!("/api/collectives/{cid}/studio/templates/{tid}"))
        .await;
    let variant = reloaded.expect_ok()["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["format_key"] == "ig_story")
        .cloned()
        .unwrap();
    assert_eq!(
        variant["layout"]["width"], 1080,
        "le format impose sa largeur"
    );
    assert_eq!(variant["layout"]["height"], 1920, "and its height");
    assert_eq!(variant["ratio"], "9:16");
}

#[tokio::test]
async fn a_variant_starts_from_a_correctable_automatic_adaptation() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let banner = format_id(&app, "yt_banner").await;

    let templates = antoine
        .get(&format!("/api/collectives/{cid}/studio/templates"))
        .await;
    let tid = templates.expect_ok()[0]["id"].as_str().unwrap().to_string();

    let derived = antoine
        .post(
            &format!("/api/collectives/{cid}/studio/templates/{tid}/variants"),
            json!({ "format_id": banner }),
        )
        .await;
    let layout = &derived.expect_ok()["layout"];
    assert_eq!(layout["width"], 2560);
    assert_eq!(layout["height"], 1440);
    // The blocks are there, repositioned — not a blank page.
    assert!(!layout["blocks"].as_array().unwrap().is_empty());
    for bloc in layout["blocks"].as_array().unwrap() {
        let y = bloc["y"].as_f64().unwrap();
        let h = bloc["h"].as_f64().unwrap();
        assert!(y >= 0.0 && y + h <= 1.0001, "bloc hors cadre : y={y} h={h}");
    }

    // Then correct it by hand, without touching the master.
    let mut corrige = layout.clone();
    corrige["blocks"][0]["y"] = json!(0.5);
    antoine
        .put(
            &format!("/api/collectives/{cid}/studio/templates/{tid}/variants/{banner}"),
            json!({ "layout": corrige }),
        )
        .await
        .expect_ok();

    let reloaded = antoine
        .get(&format!("/api/collectives/{cid}/studio/templates/{tid}"))
        .await;
    let reloaded = reloaded.expect_ok();
    let master = reloaded["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["is_master"] == true)
        .unwrap();
    assert_ne!(
        master["layout"]["blocks"][0]["y"],
        json!(0.5),
        "le maitre est intact"
    );
}

#[tokio::test]
async fn editing_a_template_archives_a_version() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let square = format_id(&app, "ig_square").await;

    let templates = antoine
        .get(&format!("/api/collectives/{cid}/studio/templates"))
        .await;
    let tid = templates.expect_ok()[0]["id"].as_str().unwrap().to_string();

    let res = antoine
        .put(
            &format!("/api/collectives/{cid}/studio/templates/{tid}/variants/{square}"),
            json!({ "layout": { "version": 1, "width": 1, "height": 1, "blocks": [] } }),
        )
        .await;
    assert_eq!(res.expect_ok()["version"], 2);

    let (versions,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM template_versions WHERE template_id = $1")
            .bind(Uuid::parse_str(&tid).unwrap())
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(
        versions, 2,
        "un visuel deja produit garde sa version d'origine"
    );
}

#[tokio::test]
async fn the_brand_is_data_and_the_group_overrides_the_collective() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let ramas = app.group_id("ramas").await;
    let antoine = app.login_named("Antoine").await;

    let brand = antoine
        .get(&format!("/api/collectives/{cid}/studio/brand"))
        .await;
    let brand = brand.expect_ok();
    let accent = brand["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "accent")
        .unwrap();
    assert_eq!(accent["kind"], "color");

    // Entering the real values requires no code change.
    antoine
        .put(
            &format!("/api/collectives/{cid}/studio/brand/tokens"),
            json!({
                "tokens": [
                    { "kind": "color", "key": "accent", "label": "Accent", "value": { "hex": "#FF2D55" } }
                ]
            }),
        )
        .await
        .expect_ok();

    let brand = antoine
        .get(&format!("/api/collectives/{cid}/studio/brand"))
        .await;
    let accent = brand.expect_ok()["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "accent")
        .cloned()
        .unwrap();
    assert_eq!(accent["value"]["hex"], "#FF2D55");

    // The group overrides partially, and sees what it inherits.
    let romain = app.login_named("Romain").await;
    romain
        .put(
            &format!("/api/collectives/{cid}/studio/brand/tokens"),
            json!({
                "group_id": ramas,
                "tokens": [
                    { "kind": "color", "key": "accent", "label": "Accent Ramas", "value": { "hex": "#00E0B0" } }
                ]
            }),
        )
        .await
        .expect_ok();

    let brand = romain
        .get(&format!(
            "/api/collectives/{cid}/studio/brand?group_id={ramas}"
        ))
        .await;
    let brand = brand.expect_ok();
    let accent = brand["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "accent")
        .unwrap();
    assert_eq!(accent["value"]["hex"], "#00E0B0");
    let inherited = brand["inherited"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "accent")
        .unwrap();
    assert_eq!(
        inherited["value"]["hex"], "#FF2D55",
        "la charte du collectif reste visible"
    );
}

#[tokio::test]
async fn media_is_uploaded_and_comes_back_through_a_signed_url() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    // A minimal PNG, but a real one: it really does go into MinIO.
    let png: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89,
    ];
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(png.clone())
                .file_name("photo.png")
                .mime_str("image/png")
                .unwrap(),
        )
        .text("tags", "live,lyon");

    let resp = reqwest::Client::new()
        .post(format!("{}/api/collectives/{cid}/studio/assets", app.base))
        .header("Authorization", format!("Bearer {}", antoine.token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let asset_id = body["id"].as_str().unwrap().to_string();

    let list = antoine
        .get(&format!("/api/collectives/{cid}/studio/assets?tag=live"))
        .await;
    let list = list.expect_ok();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["filename"], "photo.png");

    // The signed URL really does bring back the bytes that were uploaded.
    let url = antoine
        .get(&format!(
            "/api/collectives/{cid}/studio/assets/{asset_id}/url"
        ))
        .await;
    let url = url.expect_ok()["url"].as_str().unwrap().to_string();
    let fetched = reqwest::get(&url).await.unwrap();
    assert_eq!(fetched.status(), 200, "URL signee : {url}");
    assert_eq!(fetched.bytes().await.unwrap().to_vec(), png);
}

#[tokio::test]
async fn the_automatic_fields_fill_in_from_the_event() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    let (event_id,): (Uuid,) = sqlx::query_as(
        "SELECT e.id FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.collective_id = $1 AND t.key = 'concert' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    let fields = antoine
        .get(&format!(
            "/api/collectives/{cid}/studio/preview-fields/{event_id}"
        ))
        .await;
    let fields = fields.expect_ok();
    assert_eq!(fields["venue"]["name"], "Le Sonic");
    assert_eq!(fields["venue"]["city"], "Lyon");
    assert_eq!(fields["collective"]["name"], "Bonsoir Techno");
    assert!(fields["event"]["title"].as_str().unwrap().contains("Ramas"));

    // Set times are publishable on this event: they come out.
    let line_up = fields["line_up"].as_array().unwrap();
    assert!(line_up.iter().any(|l| l["slot"].is_string()), "{line_up:?}");
}

#[tokio::test]
async fn a_non_publishable_set_time_never_shows_up_in_the_visuals() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    let (event_id,): (Uuid,) = sqlx::query_as(
        "SELECT e.id FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.collective_id = $1 AND t.key = 'concert' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    // The times exist, but are not publishable.
    antoine
        .patch(
            &format!("/api/collectives/{cid}/events/{event_id}"),
            json!({ "set_times_public": false }),
        )
        .await
        .expect_ok();

    let fields = antoine
        .get(&format!(
            "/api/collectives/{cid}/studio/preview-fields/{event_id}"
        ))
        .await;
    let fields = fields.expect_ok();
    let line_up = fields["line_up"].as_array().unwrap();
    assert!(!line_up.is_empty(), "the line-up order is still shown");
    assert!(
        line_up.iter().all(|l| l["slot"].is_null()),
        "aucun visuel ne sort avec un horaire provisoire : {line_up:?}"
    );
}
