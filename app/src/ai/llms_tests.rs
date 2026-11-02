use super::*;

#[test]
fn llm_info_deserializes_without_base_model_name() {
    let raw = r#"{
            "display_name": "gpt-4o",
            "id": "gpt-4o",
            "usage_metadata": {
                "request_multiplier": 1,
                "credit_multiplier": null
            },
            "description": null,
            "disable_reason": null,
            "vision_supported": false,
            "spec": null,
            "provider": "Unknown"
        }"#;

    let info: LLMInfo = serde_json::from_str(raw).expect("should deserialize");
    assert_eq!(info.display_name, "gpt-4o");
    assert_eq!(info.base_model_name, "gpt-4o");
}

#[test]
fn llm_info_ignores_legacy_server_routing_and_billing_fields() {
    for hosts in [serde_json::json!([]), serde_json::json!({"obsolete": {}})] {
        let raw = serde_json::json!({
            "display_name": "Local model",
            "id": "local-model",
            "provider": "Unknown",
            "host_configs": hosts,
            "usage_metadata": {"request_multiplier": 1, "credit_multiplier": 2},
            "discount_percentage": 50
        });
        let info: LLMInfo = serde_json::from_value(raw).expect("legacy metadata remains readable");
        assert_eq!(info.base_model_name, "Local model");
        let saved = serde_json::to_value(info).expect("serialize");
        for removed in ["host_configs", "usage_metadata", "discount_percentage"] {
            assert!(
                saved.get(removed).is_none(),
                "obsolete field persisted: {removed}"
            );
        }
    }
}

#[test]
fn llm_info_round_trip_serializes_and_deserializes() {
    // Legacy server fields are accepted and discarded.
    let wire_json = r#"{
            "display_name": "claude-3",
            "base_model_name": "claude-3",
            "id": "claude-3",
            "usage_metadata": { "request_multiplier": 2, "credit_multiplier": 1.5 },
            "description": "A powerful model",
            "vision_supported": true,
            "provider": "Anthropic",
            "host_configs": [
                { "enabled": true, "model_routing_host": "DirectApi" }
            ]
        }"#;

    // Deserialize from wire format
    let info: LLMInfo = serde_json::from_str(wire_json).expect("should deserialize");

    // Serialize the retained local metadata.
    let serialized = serde_json::to_string(&info).expect("should serialize");

    // Deserialize again.
    let round_tripped: LLMInfo =
        serde_json::from_str(&serialized).expect("should deserialize after round trip");

    assert_eq!(info, round_tripped);
}
