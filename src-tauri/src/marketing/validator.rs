use super::models::{
    ChannelDraft, ContentBrief, RepositorySnapshot, ValidationFinding, REQUIRED_CHANNELS,
};
use std::collections::{HashMap, HashSet};

fn finding(
    severity: &str,
    code: &str,
    message: impl Into<String>,
    path: impl Into<String>,
) -> ValidationFinding {
    ValidationFinding {
        severity: severity.into(),
        code: code.into(),
        message: message.into(),
        path: path.into(),
    }
}

pub fn validate_brief(
    snapshot: &RepositorySnapshot,
    brief: &ContentBrief,
) -> Vec<ValidationFinding> {
    let mut findings = Vec::new();
    if brief.schema_version != 1 {
        findings.push(finding(
            "error",
            "brief.schema_version",
            "Content Brief schemaVersion 必须为 1。",
            "brief.schemaVersion",
        ));
    }
    if !matches!(
        brief.publishability.as_str(),
        "publish" | "hold" | "no_content"
    ) {
        findings.push(finding(
            "error",
            "brief.publishability",
            "publishability 只能是 publish、hold 或 no_content。",
            "brief.publishability",
        ));
    }
    if brief.publishability == "publish" && brief.updates.is_empty() {
        findings.push(finding(
            "error",
            "brief.empty_updates",
            "可发布 brief 至少需要一条 update。",
            "brief.updates",
        ));
    }
    if brief.reason.trim().is_empty() {
        findings.push(finding(
            "error",
            "brief.missing_reason",
            "Content Brief 必须说明为什么值得或不值得传播。",
            "brief.reason",
        ));
    }
    if brief.publishability == "publish" {
        if brief.core_message.trim().is_empty() || brief.audience.is_empty() {
            findings.push(finding(
                "error",
                "brief.missing_strategy",
                "可发布 brief 必须包含 coreMessage 和 audience。",
                "brief",
            ));
        }
        for channel in REQUIRED_CHANNELS {
            if brief
                .channel_angles
                .get(channel)
                .map(|angle| angle.trim().is_empty())
                .unwrap_or(true)
            {
                findings.push(finding(
                    "error",
                    "brief.missing_channel_angle",
                    format!("Content Brief 缺少 {channel} 渠道角度。"),
                    format!("brief.channelAngles.{channel}"),
                ));
            }
        }
    }
    let evidence = snapshot
        .evidence
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let mut claim_ids = HashSet::new();
    for (index, update) in brief.updates.iter().enumerate() {
        let path = format!("brief.updates[{index}]");
        if update.id.trim().is_empty() || !claim_ids.insert(update.id.as_str()) {
            findings.push(finding(
                "error",
                "brief.claim_id",
                "update id 不能为空且必须唯一。",
                format!("{path}.id"),
            ));
        }
        if update.evidence_refs.is_empty() {
            findings.push(finding(
                "error",
                "brief.missing_evidence",
                "每条 update 必须引用仓库证据。",
                format!("{path}.evidenceRefs"),
            ));
        }
        for reference in &update.evidence_refs {
            if !evidence.contains(reference.as_str()) {
                findings.push(finding(
                    "error",
                    "brief.dangling_evidence",
                    format!("证据引用 {reference} 不存在。"),
                    format!("{path}.evidenceRefs"),
                ));
            }
        }
        if !matches!(
            update.release_state.as_str(),
            "released" | "committed" | "unreleased"
        ) {
            findings.push(finding(
                "error",
                "brief.release_state",
                "releaseState 必须为 released、committed 或 unreleased。",
                format!("{path}.releaseState"),
            ));
        }
        if update.release_state != "unreleased"
            && update.evidence_refs.iter().any(|id| {
                snapshot
                    .evidence
                    .iter()
                    .any(|ev| ev.id == *id && ev.release_state == "unreleased")
            })
        {
            findings.push(finding(
                "error",
                "brief.unreleased_mismatch",
                "引用未提交证据的 update 必须标记为 unreleased。",
                format!("{path}.releaseState"),
            ));
        }
        if update.release_state == "released"
            && update.evidence_refs.iter().any(|id| {
                snapshot
                    .evidence
                    .iter()
                    .any(|ev| ev.id == *id && ev.release_state != "released")
            })
        {
            findings.push(finding(
                "error",
                "brief.released_without_evidence",
                "只有明确标记为 released 的证据才能支持已发布表述。",
                format!("{path}.releaseState"),
            ));
        }
    }
    for (index, proof) in brief.proof_points.iter().enumerate() {
        let path = format!("brief.proofPoints[{index}]");
        if proof.id.trim().is_empty() || !claim_ids.insert(proof.id.as_str()) {
            findings.push(finding(
                "error",
                "brief.claim_id",
                "proof point id 不能为空且必须唯一。",
                format!("{path}.id"),
            ));
        }
        if proof.evidence_refs.is_empty() {
            findings.push(finding(
                "error",
                "brief.missing_evidence",
                "每条 proof point 必须引用仓库证据。",
                format!("{path}.evidenceRefs"),
            ));
        }
        for reference in &proof.evidence_refs {
            if !evidence.contains(reference.as_str()) {
                findings.push(finding(
                    "error",
                    "brief.dangling_evidence",
                    format!("证据引用 {reference} 不存在。"),
                    format!("{path}.evidenceRefs"),
                ));
            }
        }
    }
    findings
}

pub fn validate_drafts(
    snapshot: &RepositorySnapshot,
    brief: &ContentBrief,
    drafts: &[ChannelDraft],
) -> Vec<ValidationFinding> {
    let mut findings = validate_brief(snapshot, brief);
    let mut claim_states = HashMap::new();
    for update in &brief.updates {
        claim_states.insert(update.id.as_str(), update.release_state.as_str());
    }
    for proof in &brief.proof_points {
        let state = if proof.evidence_refs.iter().any(|id| {
            snapshot
                .evidence
                .iter()
                .any(|evidence| evidence.id == *id && evidence.release_state == "unreleased")
        }) {
            "unreleased"
        } else if proof.evidence_refs.iter().all(|id| {
            snapshot
                .evidence
                .iter()
                .any(|evidence| evidence.id == *id && evidence.release_state == "released")
        }) {
            "released"
        } else {
            "committed"
        };
        claim_states.insert(proof.id.as_str(), state);
    }
    let mut seen = HashSet::new();
    for (index, draft) in drafts.iter().enumerate() {
        let path = format!("drafts[{index}]");
        if !REQUIRED_CHANNELS.contains(&draft.channel.as_str()) {
            findings.push(finding(
                "error",
                "draft.channel",
                format!("未知渠道 {}。", draft.channel),
                format!("{path}.channel"),
            ));
            continue;
        }
        if !seen.insert(draft.channel.as_str()) {
            findings.push(finding(
                "error",
                "draft.duplicate_channel",
                format!("渠道 {} 重复。", draft.channel),
                format!("{path}.channel"),
            ));
        }
        if draft.body.trim().is_empty() {
            findings.push(finding(
                "error",
                "draft.empty",
                "渠道正文不能为空。",
                format!("{path}.body"),
            ));
        }
        if draft.claim_refs.is_empty() {
            findings.push(finding(
                "error",
                "draft.missing_claim_refs",
                "每个渠道草稿必须列出使用的 claimRefs。",
                format!("{path}.claimRefs"),
            ));
        }
        for reference in &draft.claim_refs {
            if !claim_states.contains_key(reference.as_str()) {
                findings.push(finding(
                    "error",
                    "draft.dangling_claim",
                    format!("claim 引用 {reference} 不存在。"),
                    format!("{path}.claimRefs"),
                ));
            }
        }
        let combined = format!("{}\n{}", draft.title, draft.body);
        for phrase in &snapshot.config.banned_phrases {
            if !phrase.is_empty() && combined.contains(phrase) {
                findings.push(finding(
                    "error",
                    "style.banned_phrase",
                    format!("包含禁用营销表达「{phrase}」。"),
                    format!("{path}.body"),
                ));
            }
        }
        let absolute_phrases = [
            "100%",
            "绝对",
            "保证",
            "彻底",
            "从不",
            "永远",
            "zero risk",
            "guaranteed",
        ];
        for phrase in absolute_phrases {
            if combined
                .to_ascii_lowercase()
                .contains(&phrase.to_ascii_lowercase())
            {
                findings.push(finding(
                    "error",
                    "style.absolute_claim",
                    format!("包含无法由仓库证据证明的绝对化表达「{phrase}」。"),
                    format!("{path}.body"),
                ));
            }
        }
        let has_unreleased = draft
            .claim_refs
            .iter()
            .any(|id| claim_states.get(id.as_str()) == Some(&"unreleased"));
        if has_unreleased {
            let release_claims = [
                "已发布",
                "正式发布",
                "现已支持",
                "现已上线",
                "now available",
                "released today",
            ];
            for phrase in release_claims {
                if combined
                    .to_ascii_lowercase()
                    .contains(&phrase.to_ascii_lowercase())
                {
                    findings.push(finding(
                        "error",
                        "fact.unreleased_as_released",
                        format!("未提交内容不能表述为「{phrase}」。"),
                        format!("{path}.body"),
                    ));
                }
            }
        }
        if draft.channel == "x" && draft.body.chars().count() > 280 {
            findings.push(finding(
                "error",
                "channel.x_length",
                "X / Twitter 单条正文超过 280 字符。",
                format!("{path}.body"),
            ));
        }
        if draft.channel == "xiaohongshu" && draft.body.chars().count() > 1_500 {
            findings.push(finding(
                "warning",
                "channel.xiaohongshu_length",
                "小红书正文超过 MVP 建议的 1500 字符。",
                format!("{path}.body"),
            ));
        }
    }
    for channel in REQUIRED_CHANNELS {
        if !seen.contains(channel) {
            findings.push(finding(
                "error",
                "draft.missing_channel",
                format!("缺少 {channel} 草稿。"),
                "drafts",
            ));
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketing::{ContentUpdate, Evidence, MarketingConfig, ProofPoint};
    use std::collections::BTreeMap;

    fn fixture() -> (RepositorySnapshot, ContentBrief) {
        let snapshot = RepositorySnapshot {
            schema_version: 1,
            repository_root: ".".into(),
            base_ref: Some("v1".into()),
            head_ref: "abc".into(),
            source_mode: "committed".into(),
            commits: vec![],
            changed_files: vec![],
            uncommitted_files: vec![],
            evidence: vec![Evidence {
                id: "ev-001".into(),
                kind: "commit".into(),
                source: "abc".into(),
                excerpt: "feat".into(),
                content_hash: "h".into(),
                release_state: "committed".into(),
            }],
            config: MarketingConfig {
                banned_phrases: vec!["革命性".into()],
                ..Default::default()
            },
            truncated: false,
            collected_at: 1,
        };
        let brief = ContentBrief {
            schema_version: 1,
            campaign_id: "c".into(),
            publishability: "publish".into(),
            reason: "有用户价值".into(),
            audience: vec!["开发者".into()],
            core_message: "新增能力".into(),
            updates: vec![ContentUpdate {
                id: "up-1".into(),
                title: "能力".into(),
                summary: "事实".into(),
                user_value: "价值".into(),
                evidence_refs: vec!["ev-001".into()],
                release_state: "committed".into(),
            }],
            proof_points: vec![ProofPoint {
                id: "proof-1".into(),
                text: "提交".into(),
                evidence_refs: vec!["ev-001".into()],
            }],
            do_not_claim: vec![],
            channel_angles: REQUIRED_CHANNELS
                .iter()
                .map(|channel| (channel.to_string(), "角度".to_string()))
                .collect::<BTreeMap<_, _>>(),
        };
        (snapshot, brief)
    }

    #[test]
    fn dangling_evidence_blocks_brief() {
        let (snapshot, mut brief) = fixture();
        brief.updates[0].evidence_refs = vec!["missing".into()];
        assert!(validate_brief(&snapshot, &brief)
            .iter()
            .any(|f| f.code == "brief.dangling_evidence"));
    }

    #[test]
    fn banned_phrase_and_missing_channels_block_drafts() {
        let (snapshot, brief) = fixture();
        let drafts = vec![ChannelDraft {
            channel: "x".into(),
            title: "革命性更新".into(),
            body: "正文".into(),
            claim_refs: vec!["up-1".into()],
        }];
        let findings = validate_drafts(&snapshot, &brief, &drafts);
        assert!(findings.iter().any(|f| f.code == "style.banned_phrase"));
        assert!(findings.iter().any(|f| f.code == "draft.missing_channel"));
    }

    #[test]
    fn committed_evidence_cannot_be_labeled_released() {
        let (snapshot, mut brief) = fixture();
        brief.updates[0].release_state = "released".into();
        assert!(validate_brief(&snapshot, &brief)
            .iter()
            .any(|finding| finding.code == "brief.released_without_evidence"));
    }

    #[test]
    fn unreleased_proof_point_cannot_be_announced_as_available() {
        let (mut snapshot, brief) = fixture();
        snapshot.evidence[0].release_state = "unreleased".into();
        let mut drafts = REQUIRED_CHANNELS
            .iter()
            .map(|channel| ChannelDraft {
                channel: channel.to_string(),
                title: "更新".into(),
                body: "开发中的变化".into(),
                claim_refs: vec!["proof-1".into()],
            })
            .collect::<Vec<_>>();
        drafts[0].body = "现已上线".into();
        assert!(validate_drafts(&snapshot, &brief, &drafts)
            .iter()
            .any(|finding| finding.code == "fact.unreleased_as_released"));
    }
}
