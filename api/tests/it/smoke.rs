use crate::harness::TestApp;

#[tokio::test]
async fn l_api_repond_et_la_base_est_migree() {
    let app = TestApp::new().await;
    let (status, body) = crate::harness::anonymous_get(&app.base, "/health").await;
    assert_eq!(status, 200);
    assert_eq!(body, "ok");

    let (tables,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(tables > 25, "schema incomplet : {tables} tables");
}

#[tokio::test]
async fn les_donnees_d_amorcage_installent_bonsoir_techno_et_ramas() {
    let app = TestApp::seeded().await;

    let collective = app.collective_id("bonsoir-techno").await;
    let antoine = app.user_id("Antoine").await;
    let client = app.login_as(antoine).await;

    let me = client.get("/api/me").await;
    let me = me.expect_ok();
    assert_eq!(me["display_name"], "Antoine");
    assert_eq!(me["collectives"][0]["role"], "admin");

    let groups = client
        .get(&format!("/api/collectives/{collective}/groups"))
        .await;
    let groups = groups.expect_ok();
    let names: Vec<&str> = groups
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Ramas"), "groupes : {names:?}");
    assert!(names.contains(&"Dante3p"), "groupes : {names:?}");

    // Ramas : duo dont les deux membres sont aussi membres du collectif (§16).
    let ramas = groups
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "Ramas")
        .unwrap();
    assert_eq!(ramas["members"].as_array().unwrap().len(), 2);

    let events = client
        .get(&format!("/api/collectives/{collective}/events"))
        .await;
    let events = events.expect_ok();
    let types: Vec<&str> = events
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["type_key"].as_str().unwrap())
        .collect();
    assert!(types.contains(&"concert"));
    assert!(types.contains(&"residency"));
    assert!(types.contains(&"stream"));
}
