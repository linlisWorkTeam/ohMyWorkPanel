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

/// Events entry payload: only text-ish keys (A1: not written into group chat).
pub fn validate_events_payload(payload: &Value) -> Result<(), String> {
    if let Some(reason) = payload_contains_forbidden_media(payload) {
        return Err(reason.into());
    }
    let obj = payload
        .as_object()
        .ok_or_else(|| "payload 必须是对象".to_string())?;
    const ALLOWED: &[&str] = &["text", "isFinal", "final", "lang", "sessionId", "chunks"];
    for key in obj.keys() {
        if !ALLOWED.iter().any(|a| a.eq_ignore_ascii_case(key)) {
            return Err(format!("payload 含非白名单字段：{key}"));
        }
    }
    let raw = serde_json::to_vec(payload).unwrap_or_default();
    if raw.len() > 8 * 1024 {
        return Err("payload 超过 8KB".into());
    }
    Ok(())
}

/// Dispatch control-plane skill; may call PanelLive upstream (server-side 127.0.0.1).
/// When `sched` is provided, updates in-memory Live session marks for the group.
pub fn dispatch_live_skill(
    envelope: &A2aEnvelope,
    sched: Option<&crate::scheduler::SchedulerState>,
) -> Result<A2aDispatchResult, String> {
    validate_live_skill(&envelope.skill)?;
    if let Some(reason) = payload_contains_forbidden_media(&envelope.payload) {
        return Err(reason.into());
    }

    let root = crate::extensions::panellive_root();
    // Only control-plane skills that call the PanelLive upstream need the manifest;
    // transcript acks are WS-only and must not hard-fail when the Live host repo is absent.
    let upstream_port = || -> Result<u16, String> {
        let manifest = crate::extensions::load_panellive_manifest(&root)?;
        Ok(crate::extensions::panellive_upstream_port(&manifest))
    };
    let host = "127.0.0.1";

    let (msg, session_id) = match envelope.skill.as_str() {
        "live.session.start" => {
            let port = upstream_port()?;
            let (code, body) = crate::extensions::http_post_json_local(host, port, "/v1/session/start", "{}")?;
            if code != 200 {
                return Err(format!("PanelLive session/start HTTP {code}: {body}"));
            }
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let sid = v
                .get("sessionId")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .or_else(|| envelope.session_id.clone());
            if let Some(s) = sched {
                s.mark_live_started(&envelope.group_id);
            }
            ("Live session started via PanelLive".into(), sid)
        }
        "live.session.stop" | "live.session.cancel" => {
            // A2: PanelLive has cancel only — stop maps to cancel.
            let port = upstream_port()?;
            let sid = envelope
                .session_id
                .clone()
                .ok_or_else(|| "sessionId 必填".to_string())?;
            let body = serde_json::json!({ "sessionId": sid }).to_string();
            let (code, resp) =
                crate::extensions::http_post_json_local(host, port, "/v1/session/cancel", &body)?;
            if code != 200 {
                return Err(format!("PanelLive session/cancel HTTP {code}: {resp}"));
            }
            if let Some(s) = sched {
                s.mark_live_stopped(&envelope.group_id);
            }
            ("Live session cancel accepted (stop→cancel)".into(), Some(sid))
        }
        "live.transcribe.result" => {
            validate_events_payload(&envelope.payload)?;
            (
                "Transcript text accepted (WS only; not written to chat)".into(),
                envelope.session_id.clone(),
            )
        }
        "live.synthesize.request" => {
            let port = upstream_port()?;
            let text = envelope
                .payload
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "synthesize 需要 payload.text".to_string())?;
            let mut body = serde_json::json!({ "text": text });
            if let Some(sid) = &envelope.session_id {
                body["sessionId"] = Value::String(sid.clone());
            }
            let (code, resp) = crate::extensions::http_post_json_local(
                host,
                port,
                "/v1/tts/mock?format=json",
                &body.to_string(),
            )?;
            if code != 200 {
                return Err(format!("PanelLive tts/mock HTTP {code}: {resp}"));
            }
            // Do not echo audioBase64 back on A2A control plane.
            (
                "TTS request forwarded to PanelLive (audio stays on media plane)".into(),
                envelope.session_id.clone(),
            )
        }
        other => return Err(format!("未处理 skill：{other}")),
    };

    Ok(A2aDispatchResult {
        accepted: true,
        skill: envelope.skill.clone(),
        session_id,
        message: msg,
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
        let err = dispatch_live_skill(&env, None).unwrap_err();
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
        let ok = dispatch_live_skill(&env, None).unwrap();
        assert!(ok.accepted);
    }

    #[test]
    fn rejects_unknown_skill() {
        assert!(validate_live_skill("live.audio.upload").is_err());
    }
}
