use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;
use url::Url;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use humantime::parse_duration as parse_humantime;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RequestType {
  Success,
  Fail,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Request {
  // keep external key as "type" but use a typed enum internally
  #[serde(rename = "type")]
  pub request_type: RequestType,
  pub stream: Stream,
  pub gifts: Vec<Gift>,
  pub debug: DebugInfo,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Stream {
  // use UUID for user id
  pub user_id: Uuid,
  pub is_private: bool,
  pub settings: u64,
  // use Url for validated urls
  pub shard_url: Url,
  pub public_tariff: Tariff,
  pub private_tariff: PrivateTariff,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Tariff {
  pub id: u64,
  pub price: u64,
  // deserialize human-readable durations like "1h", "1m", "234ms"
  #[serde(with = "humantime_serde")]
  pub duration: Duration,
  pub description: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PrivateTariff {
  pub client_price: u64,
  #[serde(with = "humantime_serde")]
  pub duration: Duration,
  pub description: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Gift {
  pub id: u64,
  pub price: u64,
  pub description: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DebugInfo {
  #[serde(with = "humantime_serde")]
  pub duration: Duration,
  // use RFC3339 timestamps -> OffsetDateTime from `time` crate
  #[serde(with = "time::serde::rfc3339")]
  pub at: OffsetDateTime,
}

/// Convert JSON string into Request and serialize to TOML string.
pub fn json_to_toml(input: &str) -> Result<String, Box<dyn std::error::Error>> {
  let req: Request = serde_json::from_str(input)?;
  let toml_str = toml::to_string_pretty(&req)?;
  Ok(toml_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"
    {
      "type": "success",
      "stream": {
        "user_id": "8d234120-0bda-49b2-b7e0-fbd3912f6cbf",
        "is_private": false,
        "settings": 45345,
        "shard_url": "https://n3.example.com/sapi",
        "public_tariff": {
          "id": 1,
          "price": 100,
          "duration": "1h",
          "description": "test public tariff"
        },
        "private_tariff": {
          "client_price": 250,
          "duration": "1m",
          "description": "test private tariff"
        }
      },
      "gifts": [
        { "id": 1, "price": 2, "description": "Gift 1" },
        { "id": 2, "price": 3, "description": "Gift 2" }
      ],
      "debug": {
        "duration": "234ms",
        "at": "2019-06-28T08:35:46+00:00"
      }
    }
    "#;

    #[test]
    fn test_json_to_toml_roundtrip_contains_keys() {
        let toml = json_to_toml(SAMPLE_JSON).expect("conversion should succeed");
        assert!(toml.contains("type = \"success\""));
        assert!(toml.contains("user_id = \"8d234120-0bda-49b2-b7e0-fbd3912f6cbf\""));
        assert!(toml.contains("settings = 45345"));
        assert!(toml.contains("public_tariff"));
        assert!(toml.contains("private_tariff"));
        assert!(toml.contains("gifts"));
    assert!(toml.contains("duration = \"234ms\""));
    }

    #[test]
    fn test_deserialize_struct_fields() {
        let req: Request = serde_json::from_str(SAMPLE_JSON).expect("deserialize");
    // typed checks
    assert_eq!(req.request_type, RequestType::Success);
    assert_eq!(req.stream.public_tariff.id, 1);
    assert_eq!(req.gifts.len(), 2);

    // user_id parsed as UUID
    let parsed_uuid = Uuid::parse_str("8d234120-0bda-49b2-b7e0-fbd3912f6cbf").unwrap();
    assert_eq!(req.stream.user_id, parsed_uuid);

    // durations parsed correctly
    let public_dur = humantime::parse_duration("1h").unwrap();
    assert_eq!(req.stream.public_tariff.duration, public_dur);

    let debug_dur = humantime::parse_duration("234ms").unwrap();
    assert_eq!(req.debug.duration, debug_dur);

  // timestamp parsed correctly (RFC3339)
  let expected_at = OffsetDateTime::parse("2019-06-28T08:35:46+00:00", &Rfc3339).unwrap();
  assert_eq!(req.debug.at, expected_at);
    }
}