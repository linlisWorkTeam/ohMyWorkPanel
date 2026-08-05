//! A2A control-plane skills for Live / Extend (text only — no PCM).

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const LIVE_SKILLS: &[&str] = &[
    "live.session.start",
    "live.session.stop",
    "live.session.cancel",
    "live.transcribe.result",
    "live.synthesize.request",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aEnvelope {
    pub skill: String,
    pub group_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aDispatchResult {
    pub accepted: bool,
    pub skill: String,
    pub session_id: Option<String>,
    pub message: String,
}

/// Reject audio / PCM smuggling in control-plane payloads.
pub fn payload_contains_forbidden_media(payload: &Value) -> Option<&'static str> {
    fn walk(v: &Value) -> Option<&'static str> {
        match v {
            Value::Object(map) => {
                for (k, child) in map {
                    let key = k.to_ascii_lowercase();
                    if matches!(
                        key.as_str(),
                        "pcm"
                            | "audio"
                            | "audio_base64"
                            | "audiobase64"
                            | "wav"
                            | "raw_audio"
                            | "rawaudio"
                            | "samples"
                            | "audio_bytes"
                            | "audiobytes"
                    ) {
                        return Some("A2A 控制面禁止携带音频/PCM 字段");
                    }
                    if let Some(reason) = walk(child) {
                        return Some(reason);
                    }
                }
                None
            }
            Value::Array(items) => {
                for item in items {
                    if let Some(reason) = walk(item) {
                        return Some(reason);
                    }
                }
                None
            }
            _ => None,
        }
    }
    walk(payload)
}

pub fn validate_live_skill(skill: &str) -> Result<(), String> {
    if LIVE_SKILLS.contains(&skill) {
        Ok(())
    } else {
        Err(format!("未知或不支持的 Live skill：{skill}"))
    }
}

pub fn dispatch_live_skill(envelope: &A2aEnvelope) -> Result<A2aDispatchResult, String> {
    validate_live_skill(&envelope.skill)?;
    if let Some(reason) = payload_contains_forbidden_media(&envelope.payload) {
        return Err(reason.into());
    }
    // MVP: accept + acknowledge. Downstream ChatBot/Agent routing hooks later.
    let msg = match envelope.skill.as_str() {
        "live.session.start" => "Live session start accepted",
        "live.session.stop" => "Live session stop accepted",
        "live.session.cancel" => "Live session cancel accepted (stop LLM/TTS on PanelLive side)",
        "live.transcribe.result" => "Transcript text accepted (no media)",
        "live.synthesize.request" => "TTS text request accepted (PanelLive synthesizes)",
        other => return Err(format!("未处理 skill：{other}")),
    };
    Ok(A2aDispatchResult {
        accepted: true,
        skill: envelope.skill.clone(),
        session_id: envelope.session_id.clone(),
        message: msg.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_pcm_field() {
        let env = A2aEnvelope {
            skill: "live.transcribe.result".into(),
            group_id: "g".into(),
            session_id: Some("s1".into()),
            payload: json!({ "text": "hi", "pcm": "AAAA" }),
            source: None,
            target: None,
        };
        let err = dispatch_live_skill(&env).unwrap_err();
        assert!(err.contains("禁止"));
    }

    #[test]
    fn accepts_text_transcript() {
        let env = A2aEnvelope {
            skill: "live.transcribe.result".into(),
            group_id: "g".into(),
            session_id: Some("s1".into()),
            payload: json!({ "text": "hello", "final": true }),
            source: Some("panellive".into()),
            target: Some("chatbot".into()),
        };
        let ok = dispatch_live_skill(&env).unwrap();
        assert!(ok.accepted);
    }

    #[test]
    fn rejects_unknown_skill() {
        assert!(validate_live_skill("live.audio.upload").is_err());
    }
}
