use gosling_providers::api_client::{ApiClient, AuthMethod};
use gosling_providers::base::Provider;
use gosling_providers::openai::{OpenAiProvider, OpenAiProviderBuilder};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider(server: &MockServer, catalog: &str) -> OpenAiProvider {
    OpenAiProviderBuilder::new(
        ApiClient::new_with_tls(server.uri(), AuthMethod::NoAuth, None).unwrap(),
    )
    .base_path("v1/chat/completions".to_string())
    .canonical_provider_id(Some(catalog.to_string()))
    .custom_models(Some(vec!["static-fallback".to_string()]))
    .dynamic_models(Some(true))
    .build()
}

#[tokio::test]
async fn together_catalog_shapes_keep_muse_and_exclude_other_modalities() -> anyhow::Result<()> {
    let promo = "meta-models/Muse-Spark-Open-816B-A42B-promo";
    let spark = "meta-models/Muse-Spark-Open-816B-A42B";
    let glimmer = "meta-models/Muse-Glimmer-30B";
    let llama = "meta-llama/Llama-3.3-70B-Instruct-Turbo";
    let records = json!([
        {"id": promo, "type": "chat", "context_length": 1048576},
        {"id": spark, "type": "chat"},
        {"id": glimmer, "type": "chat"},
        {"id": llama, "type": "chat"},
        {"id": "Muse-image", "type": "image"},
        {"id": "Muse-vector", "type": "embedding"},
        {"id": "Muse-voice", "type": "audio"},
        {"id": "Muse-score", "type": "rerank"}
    ]);
    for payload in [records.clone(), json!({"data": records})] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(payload))
            .expect(2)
            .mount(&server)
            .await;
        let provider = provider(&server, "togetherai");
        let mut expected = vec![
            promo.to_string(),
            spark.to_string(),
            glimmer.to_string(),
            llama.to_string(),
        ];
        expected.sort();
        assert_eq!(provider.fetch_supported_models().await?, expected);
        let mut recommended = provider.fetch_recommended_models(false).await?;
        recommended.sort();
        assert_eq!(recommended, expected);
    }
    Ok(())
}

#[tokio::test]
async fn empty_together_array_is_not_replaced_with_static_models() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    assert!(provider(&server, "togetherai")
        .fetch_supported_models()
        .await?
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn openai_envelope_and_untyped_together_entries_are_preserved() -> anyhow::Result<()> {
    for catalog in ["openai", "togetherai"] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "muse-spark-1.3"}, {"id": "gpt-4o"}]
            })))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            provider(&server, catalog).fetch_supported_models().await?,
            vec!["gpt-4o", "muse-spark-1.3"]
        );
    }
    Ok(())
}
