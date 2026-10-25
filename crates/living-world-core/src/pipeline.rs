//! Stateful, deterministic acceptance and export pipeline for generated assets.
use crate::{evaluate_with_theme, validate_theme, Check, Recipe, ThemeContract};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_PROMPT_BYTES: usize = 20_000;
const DEFAULT_TIME_BUDGET_MS: u64 = 20 * 60 * 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JobLimits {
    pub max_attempts: u8,
    pub time_budget_ms: u64,
    pub max_provider_calls: u8,
    pub required_image_hashes: u8,
}

impl Default for JobLimits {
    fn default() -> Self {
        Self { max_attempts: 3, time_budget_ms: DEFAULT_TIME_BUDGET_MS, max_provider_calls: 6, required_image_hashes: 3 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenJobContract {
    pub job_id: String,
    pub prompt: String,
    pub prompt_sha256: String,
    pub theme: ThemeContract,
    pub theme_sha256: String,
    pub created_at_ms: u64,
    pub deadline_ms: u64,
    pub limits: JobLimits,
    pub provenance: Provenance,
    /// Fixed renderer camera extent for coherent scene candidates in this job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_half_extent: Option<f64>,
    /// User-directed refinement starts from a previously accepted asset and its actual captures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refinement: Option<RefinementContract>,
    /// Explicitly consented reference metadata; raw reference bytes are never frozen in the job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<ReferenceContract>,
    pub frozen_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferenceContract {
    pub sha256: String,
    pub mime_type: String,
    pub bytes: usize,
    pub destination: String,
    pub consent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefinementContract {
    pub user_feedback: String,
    pub baseline_recipe: Recipe,
    pub baseline_evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub rehearsal: bool,
    pub provider: String,
    pub model: String,
    pub agent: Option<String>,
    pub source_revision: Option<String>,
    pub source_fingerprint: Option<String>,
}

impl Default for Provenance {
    fn default() -> Self {
        Self { rehearsal: true, provider: "fixture".into(), model: "fixture".into(), agent: None, source_revision: None, source_fingerprint: None }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStatus { Active, Approved, Blocked, Exhausted, Cancelled }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict { Repair, Accept, Blocked, Exhausted }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceStatus { Pass, Fail, Blocked }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ComparisonStatus { Initial, Improved, Unchanged, Regressed, Blocked }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualDefect {
    pub view: u8,
    pub location: String,
    /// Exact frozen visualChecks entry, or a deterministic check id present on the candidate.
    pub criterion: String,
    pub issue: String,
    pub repair: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewComparison {
    pub candidate_hash: String,
    pub image_hashes: Vec<String>,
    pub previous_candidate_hash: Option<String>,
    #[serde(default)]
    pub previous_image_hashes: Vec<String>,
    pub status: ComparisonStatus,
    pub summary: String,
    pub strengths: Vec<String>,
    pub defects: Vec<VisualDefect>,
    pub improvements: Vec<String>,
    pub regressions: Vec<String>,
    #[serde(default)]
    pub simulated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewDecision {
    pub theme_status: EvidenceStatus,
    pub render_status: EvidenceStatus,
    pub findings: Vec<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<ReviewComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    #[serde(rename = "candidateHash", alias = "candidateSha256")]
    pub candidate_sha256: String,
    #[serde(rename = "imageHashes", alias = "imageSha256")]
    pub image_sha256: Vec<String>,
    #[serde(default)]
    pub image_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateRecord {
    pub recipe: Recipe,
    #[serde(rename = "candidateHash", alias = "candidateSha256")]
    pub candidate_sha256: String,
    pub checks: Vec<Check>,
    pub validation_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_evidence: Option<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<ReviewComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPipeline {
    pub contract: FrozenJobContract,
    pub status: PipelineStatus,
    pub attempt_count: u8,
    pub provider_calls: u8,
    pub candidates: Vec<CandidateRecord>,
    pub last_verdict: Option<ReviewVerdict>,
    pub approved: Option<ApprovedAssetPackage>,
    pub approved_package_sha256: Option<String>,
    pub state_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedAssetPackage {
    pub format: String,
    pub schema_version: u8,
    pub asset_type: String,
    pub asset_id: String,
    pub theme_id: String,
    pub theme: ThemeContract,
    pub recipe: Recipe,
    pub manifest: PackageManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManifest {
    pub version: u8,
    pub content_sha256: String,
    pub theme_sha256: String,
    pub created_at: String,
    pub created_at_ms: u64,
    pub engine: String,
    pub contract_version: u8,
    pub checks: Vec<Check>,
    pub evidence: Vec<PackageEvidence>,
    pub rehearsal: bool,
    pub provider: String,
    pub model: String,
    pub agent: Option<String>,
    pub source_revision: Option<String>,
    pub source_fingerprint: Option<String>,
    pub review_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<ReviewComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageEvidence {
    pub view: u8,
    pub url: Option<String>,
    pub sha256: String,
}

impl AssetPipeline {
    pub fn create(theme: ThemeContract, prompt: String, now_ms: u64) -> Result<Self, String> {
        validate_theme(&theme)?;
        if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT_BYTES {
            return Err("Prompt must be non-empty and at most 20,000 bytes".into());
        }
        let limits = JobLimits::default();
        let deadline_ms = now_ms.checked_add(limits.time_budget_ms).ok_or("Job deadline overflow")?;
        let theme_sha256 = sha256_json(&theme)?;
        let prompt_sha256 = sha256(prompt.as_bytes());
        let job_id = format!("job-{}-{}", theme.id, now_ms);
        let mut contract = FrozenJobContract {
            job_id, prompt, prompt_sha256, theme, theme_sha256, created_at_ms: now_ms, deadline_ms, limits,
            provenance: Provenance::default(), frozen_sha256: String::new(),
            scene_half_extent: None, refinement: None, reference: None,
        };
        contract.frozen_sha256 = contract_hash(&contract)?;
        let mut pipeline = Self {
            contract,
            status: PipelineStatus::Active,
            attempt_count: 0,
            provider_calls: 0,
            candidates: Vec::new(),
            last_verdict: None,
            approved: None,
            approved_package_sha256: None,
            state_sha256: String::new(),
        };
        pipeline.refresh_state_hash()?;
        Ok(pipeline)
    }

    /// Set provenance before any provider work begins, then freeze it with the rest of the contract.
    pub fn set_provenance(&mut self, provenance: Provenance) -> Result<(), String> {
        self.check_integrity()?;
        if self.status != PipelineStatus::Active || self.provider_calls != 0 || self.attempt_count != 0 {
            return Err("Provenance is frozen after the first provider call or candidate".into());
        }
        if provenance.provider.trim().is_empty() || provenance.model.trim().is_empty()
            || provenance.provider.len() > 100 || provenance.model.len() > 200
            || provenance.agent.as_ref().is_some_and(|v| v.trim().is_empty() || v.len() > 200)
            || provenance.source_revision.as_ref().is_some_and(|v| v.len() > 200)
            || provenance.source_fingerprint.as_ref().is_some_and(|v| v.len() > 512) {
            return Err("Provider provenance fields must be non-empty and bounded".into());
        }
        let frozen_extent = self.contract.scene_half_extent.or(if !provenance.rehearsal { Some(12.0) } else { None });
        if let (Some(extent), Some(refinement)) = (frozen_extent, self.contract.refinement.as_ref()) {
            if refinement.baseline_recipe.scene.as_ref().is_some_and(|scene| scene.half_extent.to_bits() != extent.to_bits()) {
                return Err("Refinement baseline scene halfExtent must match the frozen camera extent".into());
            }
        }
        self.contract.scene_half_extent = frozen_extent;
        self.contract.provenance = provenance;
        self.contract.frozen_sha256 = contract_hash(&self.contract)?;
        self.refresh_state_hash()?;
        Ok(())
    }

    pub fn set_scene_half_extent(&mut self, extent: f64) -> Result<(), String> {
        self.check_integrity()?;
        self.ensure_unstarted()?;
        if !extent.is_finite() || !(4.0..=16.0).contains(&extent) {
            return Err("Scene halfExtent must be between 4 and 16 metres".into());
        }
        if self.contract.refinement.as_ref().and_then(|r| r.baseline_recipe.scene.as_ref())
            .is_some_and(|scene| scene.half_extent.to_bits() != extent.to_bits()) {
            return Err("Refinement baseline scene halfExtent must match the frozen camera extent".into());
        }
        self.contract.scene_half_extent = Some(extent);
        self.contract.frozen_sha256 = contract_hash(&self.contract)?;
        self.refresh_state_hash()?;
        Ok(())
    }

    pub fn set_refinement(&mut self, refinement: RefinementContract) -> Result<(), String> {
        self.check_integrity()?;
        self.ensure_unstarted()?;
        validate_refinement(&refinement, &self.contract.theme)?;
        if let Some(extent) = self.contract.scene_half_extent {
            let baseline_extent = refinement.baseline_recipe.scene.as_ref().map(|scene| scene.half_extent);
            if baseline_extent.is_some_and(|baseline| baseline.to_bits() != extent.to_bits()) {
                return Err("Refinement baseline scene halfExtent must match the frozen camera extent".into());
            }
        }
        self.contract.refinement = Some(refinement);
        self.contract.frozen_sha256 = contract_hash(&self.contract)?;
        self.refresh_state_hash()?;
        Ok(())
    }

    pub fn set_reference(&mut self, reference: ReferenceContract) -> Result<(), String> {
        self.check_integrity()?;
        self.ensure_unstarted()?;
        validate_reference(&reference)?;
        self.contract.reference = Some(reference);
        self.contract.frozen_sha256 = contract_hash(&self.contract)?;
        self.refresh_state_hash()?;
        Ok(())
    }

    fn ensure_unstarted(&self) -> Result<(), String> {
        if self.status != PipelineStatus::Active || self.provider_calls != 0 || self.attempt_count != 0 {
            return Err("Scene extent and refinement feedback are frozen before provider work".into());
        }
        Ok(())
    }

    /// Charge one provider call before dispatching it. Rejected calls remain charged.
    pub fn charge_call(&mut self, now_ms: u64) -> Result<u8, String> {
        self.check_integrity()?;
        self.ensure_active(now_ms)?;
        if self.provider_calls >= self.contract.limits.max_provider_calls {
            self.status = PipelineStatus::Exhausted;
            self.last_verdict = Some(ReviewVerdict::Exhausted);
            self.refresh_state_hash()?;
            return Err("Provider call budget exhausted".into());
        }
        self.provider_calls += 1;
        self.refresh_state_hash()?;
        Ok(self.provider_calls)
    }

    pub fn can_continue(&self, now_ms: u64) -> Result<bool, String> {
        self.check_integrity()?;
        Ok(self.status == PipelineStatus::Active && now_ms < self.contract.deadline_ms
            && self.attempt_count < self.contract.limits.max_attempts
            && self.provider_calls < self.contract.limits.max_provider_calls)
    }

    /// Whether the current job can process an in-flight result or review.
    /// Unlike `can_continue`, this does not require budget for another attempt or call.
    pub fn can_work(&self, now_ms: u64) -> Result<bool, String> {
        self.check_integrity()?;
        Ok(self.status == PipelineStatus::Active && now_ms < self.contract.deadline_ms)
    }

    /// Record each returned candidate, including invalid candidates, as a bounded attempt.
    pub fn submit_candidate(&mut self, recipe: Recipe, now_ms: u64) -> Result<CandidateRecord, String> {
        self.check_integrity()?;
        self.ensure_active(now_ms)?;
        if self.attempt_count >= self.contract.limits.max_attempts {
            self.status = PipelineStatus::Exhausted;
            self.last_verdict = Some(ReviewVerdict::Exhausted);
            self.refresh_state_hash()?;
            return Err("Candidate attempt limit exhausted".into());
        }
        self.attempt_count += 1;
        let candidate_sha256 = sha256_json(&recipe)?;
        let evaluation = evaluate_candidate(&recipe, &self.contract.theme, self.contract.provenance.rehearsal, self.contract.scene_half_extent);
        let (checks, validation_error) = match evaluation { Ok(checks) => (checks, None), Err(error) => (Vec::new(), Some(error)) };
        let record = CandidateRecord { recipe, candidate_sha256, checks, validation_error, reviewed_evidence: None, comparison: None };
        self.candidates.push(record.clone());
        self.last_verdict = None;
        self.refresh_state_hash()?;
        Ok(record)
    }

    pub fn decide(&mut self, review: ReviewDecision, evidence: Evidence, now_ms: u64) -> Result<ReviewVerdict, String> {
        self.check_integrity()?;
        self.ensure_active(now_ms)?;
        if self.last_verdict == Some(ReviewVerdict::Repair) {
            return Err("A repair verdict requires a new candidate before review".into());
        }
        let Some(candidate) = self.candidates.last().cloned() else { return Err("No candidate is available for review".into()); };
        if evidence.candidate_sha256 != candidate.candidate_sha256 {
            return Err("Evidence candidateSha256 does not match the current candidate".into());
        }
        if !valid_sha256(&evidence.candidate_sha256) || evidence.image_sha256.len() != self.contract.limits.required_image_hashes as usize
            || evidence.image_sha256.iter().any(|hash| !valid_sha256(hash)) {
            return Err("Evidence requires exactly three valid SHA-256 image hashes".into());
        }
        let mut unique = evidence.image_sha256.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != evidence.image_sha256.len() {
            return Err("Evidence image hashes must be distinct".into());
        }
        if !evidence.image_urls.is_empty() && (evidence.image_urls.len() != self.contract.limits.required_image_hashes as usize
            || evidence.image_urls.iter().any(|url| url.trim().is_empty() || url.len() > 2_048)) {
            return Err("Evidence image URLs must contain three bounded, non-empty entries when supplied".into());
        }
        if review.findings.iter().any(|finding| finding.trim().is_empty() || finding.len() > 2_000) || review.findings.len() > 32
            || review.summary.as_ref().is_some_and(|summary| summary.trim().is_empty() || summary.len() > 2_000) {
            return Err("Review findings must be non-empty, bounded entries".into());
        }
        let prior_evidence = self.candidates.iter().rev().nth(1).and_then(|candidate| candidate.reviewed_evidence.clone())
            .or_else(|| if self.attempt_count == 1 { self.contract.refinement.as_ref().map(|r| r.baseline_evidence.clone()) } else { None });
        let deterministic_failure = candidate.validation_error.is_some()
            && (review.theme_status == EvidenceStatus::Fail || review.render_status == EvidenceStatus::Fail);
        if !self.contract.provenance.rehearsal && review.comparison.is_none() && !deterministic_failure {
            return Err("Live reviews require a hash-bound visual comparison".into());
        }
        if let Some(comparison) = &review.comparison {
            validate_comparison(comparison, &evidence, prior_evidence.as_ref(), &candidate.checks, &self.contract.theme)?;
            if comparison.simulated != self.contract.provenance.rehearsal {
                return Err("Review comparison rehearsal flag does not match the frozen job".into());
            }
        }
        if let Some(candidate) = self.candidates.last_mut() {
            candidate.reviewed_evidence = Some(evidence.clone());
            candidate.comparison = review.comparison.clone();
        }
        let blocked = review.theme_status == EvidenceStatus::Blocked || review.render_status == EvidenceStatus::Blocked;
        let comparison = review.comparison.as_ref();
        let prior_recipe = self.candidates.iter().rev().nth(1).map(|prior| &prior.recipe)
            .or_else(|| if self.attempt_count == 1 { self.contract.refinement.as_ref().map(|r| &r.baseline_recipe) } else { None });
        // Independent visual improvement cannot be asserted when the actual visible inputs did not change.
        // Ignore recipe labels/descriptions: only renderer geometry and captured pixels establish a visible delta.
        let visible_noop = prior_recipe.is_some_and(|prior| prior.parts == candidate.recipe.parts)
            || prior_evidence.as_ref().is_some_and(|prior| prior.image_sha256 == evidence.image_sha256);
        let comparison_blocked = comparison.is_some_and(|value| value.status == ComparisonStatus::Blocked);
        let comparison_failed = comparison.is_some_and(|value| {
            !value.defects.is_empty() || visible_noop || if prior_evidence.is_none() { value.status != ComparisonStatus::Initial }
            else { value.status != ComparisonStatus::Improved || value.improvements.is_empty() || !value.regressions.is_empty() }
        });
        let failed = review.theme_status == EvidenceStatus::Fail || review.render_status == EvidenceStatus::Fail
            || !review.findings.is_empty() || candidate.validation_error.is_some()
            || candidate.checks.iter().any(|check| check.status != "pass") || comparison_failed;
        let verdict = if blocked || comparison_blocked {
            self.status = PipelineStatus::Blocked;
            ReviewVerdict::Blocked
        } else if failed {
            if self.attempt_count >= self.contract.limits.max_attempts {
                self.status = PipelineStatus::Exhausted;
                ReviewVerdict::Exhausted
            } else {
                ReviewVerdict::Repair
            }
        } else {
            let package = self.build_package(&candidate, &review, &evidence, now_ms)?;
            self.approved_package_sha256 = Some(sha256_json(&package)?);
            self.approved = Some(package);
            self.status = PipelineStatus::Approved;
            ReviewVerdict::Accept
        };
        self.last_verdict = Some(verdict);
        self.refresh_state_hash()?;
        Ok(verdict)
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        self.check_integrity()?;
        if self.status == PipelineStatus::Approved { return Err("Approved assets cannot be cancelled".into()); }
        self.status = PipelineStatus::Cancelled;
        self.refresh_state_hash()?;
        Ok(())
    }

    /// Approved artifacts are durable and remain exportable after the job deadline.
    pub fn package(&self, _now_ms: u64) -> Result<ApprovedAssetPackage, String> {
        self.check_integrity()?;
        if self.status != PipelineStatus::Approved { return Err("Only an approved asset can be exported".into()); }
        let package = self.approved.clone().ok_or("Approved package is missing")?;
        if package.manifest.theme_sha256 != self.contract.theme_sha256
            || sha256_json(&package.theme)? != package.manifest.theme_sha256
            || sha256_json(&package.recipe)? != package.manifest.content_sha256
            || self.approved_package_sha256.as_deref() != Some(sha256_json(&package)?.as_str()) {
            return Err("Approved package integrity check failed".into());
        }
        Ok(package)
    }

    fn build_package(&self, candidate: &CandidateRecord, review: &ReviewDecision, evidence: &Evidence, now_ms: u64) -> Result<ApprovedAssetPackage, String> {
        let mut checks = candidate.checks.clone();
        checks.push(Check { id: "THEME-02".into(), status: status_text(review.theme_status).into(), label: "Theme visual review".into(), detail: "Reviewed against the frozen editable theme contract.".into() });
        checks.push(Check { id: "LIVE-04".into(), status: status_text(review.render_status).into(), label: "Live render review".into(), detail: review.findings.join("; ") });
        Ok(ApprovedAssetPackage {
            format: "living-empires.asset".into(), schema_version: 1, asset_type: "building".into(),
            asset_id: self.contract.job_id.clone(), theme_id: self.contract.theme.id.clone(), theme: self.contract.theme.clone(), recipe: candidate.recipe.clone(),
            manifest: PackageManifest {
                version: self.attempt_count, content_sha256: candidate.candidate_sha256.clone(), theme_sha256: self.contract.theme_sha256.clone(),
                created_at: timestamp_from_ms(now_ms), created_at_ms: now_ms, engine: "living-world-core/0.1.0".into(), contract_version: 2,
                checks, evidence: evidence.image_sha256.iter().enumerate().map(|(i, sha256)| PackageEvidence {
                    view: (i + 1) as u8, url: evidence.image_urls.get(i).cloned(), sha256: sha256.clone(),
                }).collect(),
                rehearsal: self.contract.provenance.rehearsal,
                provider: self.contract.provenance.provider.clone(),
                model: self.contract.provenance.model.clone(),
                agent: self.contract.provenance.agent.clone(),
                source_revision: self.contract.provenance.source_revision.clone(),
                source_fingerprint: self.contract.provenance.source_fingerprint.clone(),
                review_summary: review.summary.clone(),
                comparison: review.comparison.clone(),
            },
        })
    }

    fn ensure_active(&mut self, now_ms: u64) -> Result<(), String> {
        if self.status != PipelineStatus::Active { return Err(format!("Job is {:?}", self.status)); }
        if now_ms >= self.contract.deadline_ms {
            self.status = PipelineStatus::Exhausted;
            self.last_verdict = Some(ReviewVerdict::Exhausted);
            self.refresh_state_hash()?;
            return Err("Job deadline exceeded".into());
        }
        Ok(())
    }

    fn check_integrity(&self) -> Result<(), String> {
        validate_theme(&self.contract.theme)?;
        if self.contract.scene_half_extent.is_some_and(|extent| !extent.is_finite() || !(4.0..=16.0).contains(&extent)) {
            return Err("Frozen scene halfExtent is invalid".into());
        }
        if let Some(refinement) = &self.contract.refinement { validate_refinement(refinement, &self.contract.theme)?; }
        if let Some(reference) = &self.contract.reference { validate_reference(reference)?; }
        if let (Some(extent), Some(refinement)) = (self.contract.scene_half_extent, self.contract.refinement.as_ref()) {
            if refinement.baseline_recipe.scene.as_ref().is_some_and(|scene| scene.half_extent.to_bits() != extent.to_bits()) {
                return Err("Refinement baseline scene extent differs from frozen renderer extent".into());
            }
        }
        if pipeline_hash(self)? != self.state_sha256
            || self.contract.limits.max_attempts > 3 || self.contract.limits.max_provider_calls > 6
            || self.contract.limits.time_budget_ms > DEFAULT_TIME_BUDGET_MS || self.contract.limits.required_image_hashes != 3
            || contract_hash(&self.contract)? != self.contract.frozen_sha256
            || sha256_json(&self.contract.theme)? != self.contract.theme_sha256
            || sha256(self.contract.prompt.as_bytes()) != self.contract.prompt_sha256
            || self.contract.deadline_ms != self.contract.created_at_ms.saturating_add(self.contract.limits.time_budget_ms)
            || self.attempt_count as usize != self.candidates.len()
            || self.attempt_count > self.contract.limits.max_attempts
            || self.provider_calls > self.contract.limits.max_provider_calls {
            return Err("Frozen job contract or pipeline counters were tampered with".into());
        }
        for (index, candidate) in self.candidates.iter().enumerate() {
            if sha256_json(&candidate.recipe)? != candidate.candidate_sha256 { return Err("Candidate hash integrity check failed".into()); }
            match evaluate_candidate(&candidate.recipe, &self.contract.theme, self.contract.provenance.rehearsal, self.contract.scene_half_extent) {
                Ok(checks) if candidate.validation_error.is_none() && checks == candidate.checks => {}
                Err(error) if candidate.validation_error.as_deref() == Some(error.as_str()) && candidate.checks.is_empty() => {}
                _ => return Err("Candidate evaluation record was tampered with".into()),
            }
            if let Some(evidence) = &candidate.reviewed_evidence {
                if evidence.candidate_sha256 != candidate.candidate_sha256 || !valid_sha256(&evidence.candidate_sha256)
                    || evidence.image_sha256.len() != 3 || evidence.image_sha256.iter().any(|hash| !valid_sha256(hash)) {
                    return Err("Reviewed candidate evidence was tampered with".into());
                }
                if let Some(comparison) = &candidate.comparison {
                    let previous = index.checked_sub(1).and_then(|prior_index| self.candidates.get(prior_index)).and_then(|prior| prior.reviewed_evidence.as_ref())
                        .or_else(|| if index == 0 { self.contract.refinement.as_ref().map(|r| &r.baseline_evidence) } else { None });
                    validate_comparison(comparison, evidence, previous, &candidate.checks, &self.contract.theme)?;
                    if comparison.simulated != self.contract.provenance.rehearsal {
                        return Err("Stored comparison rehearsal flag does not match the frozen job".into());
                    }
                }
            } else if candidate.comparison.is_some() {
                return Err("Candidate comparison exists without reviewed evidence".into());
            }
        }
        if let Some(package) = &self.approved {
            if self.status != PipelineStatus::Approved || package.manifest.rehearsal != self.contract.provenance.rehearsal
                || package.manifest.provider != self.contract.provenance.provider || package.manifest.model != self.contract.provenance.model
                || package.manifest.agent != self.contract.provenance.agent
                || package.manifest.source_revision != self.contract.provenance.source_revision
                || package.manifest.source_fingerprint != self.contract.provenance.source_fingerprint {
                return Err("Approved package provenance was tampered with".into());
            }
            if package.manifest.theme_sha256 != self.contract.theme_sha256
                || sha256_json(&package.theme)? != package.manifest.theme_sha256
                || sha256_json(&package.recipe)? != package.manifest.content_sha256
                || self.approved_package_sha256.as_deref() != Some(sha256_json(package)?.as_str()) {
                return Err("Approved package integrity check failed".into());
            }
        } else if self.approved_package_sha256.is_some() || self.status == PipelineStatus::Approved {
            return Err("Approved package hash is present without a package".into());
        }
        Ok(())
    }

    fn refresh_state_hash(&mut self) -> Result<(), String> {
        self.state_sha256 = pipeline_hash(self)?;
        Ok(())
    }
}

fn validate_comparison(comparison: &ReviewComparison, evidence: &Evidence, previous: Option<&Evidence>, checks: &[Check], theme: &ThemeContract) -> Result<(), String> {
    if comparison.candidate_hash != evidence.candidate_sha256 || comparison.image_hashes != evidence.image_sha256 {
        return Err("Review comparison does not bind the current candidate and ordered image hashes".into());
    }
    let expected_previous = previous.map(|item| item.candidate_sha256.as_str());
    if comparison.previous_candidate_hash.as_deref() != expected_previous
        || comparison.previous_image_hashes != previous.map(|item| item.image_sha256.clone()).unwrap_or_default() {
        return Err("Review comparison does not bind the previously reviewed candidate and evidence".into());
    }
    if comparison.summary.trim().is_empty() || comparison.summary.len() > 2_000
        || comparison.strengths.len() > 16 || comparison.defects.len() > 32
        || comparison.improvements.len() > 16 || comparison.regressions.len() > 16
        || [&comparison.strengths, &comparison.improvements, &comparison.regressions].iter().flat_map(|xs| xs.iter()).any(|s| s.trim().is_empty() || s.len() > 1_000) {
        return Err("Comparison summary and lists must be bounded and non-empty".into());
    }
    for defect in &comparison.defects {
        if !(1..=3).contains(&defect.view) || defect.location.trim().is_empty() || defect.location.len() > 500
            || defect.issue.trim().is_empty() || defect.issue.len() > 1_000 || defect.repair.trim().is_empty() || defect.repair.len() > 1_000
            || defect.criterion.trim().is_empty() || defect.criterion.len() > 500
            || (!theme.visual_checks.iter().any(|criterion| criterion == &defect.criterion) && !checks.iter().any(|check| check.id == defect.criterion)) {
            return Err("Comparison defect must cite a frozen visual check or candidate functional check".into());
        }
    }
    Ok(())
}

fn evaluate_candidate(recipe: &Recipe, theme: &ThemeContract, rehearsal: bool, scene_half_extent: Option<f64>) -> Result<Vec<Check>, String> {
    let checks = evaluate_with_theme(recipe, theme)?;
    if !rehearsal && recipe.scene.is_none() {
        return Err("Live candidates require the complete coherent scene contract".into());
    }
    if let (Some(expected), Some(scene)) = (scene_half_extent, recipe.scene.as_ref()) {
        if scene.half_extent.to_bits() != expected.to_bits() {
            return Err("Candidate scene halfExtent does not match the frozen camera extent".into());
        }
    }
    if let (Some(expected), None) = (scene_half_extent, recipe.scene.as_ref()) {
        if !rehearsal { return Err(format!("Candidate is missing the frozen scene halfExtent {expected}")); }
    }
    Ok(checks)
}

fn validate_refinement(refinement: &RefinementContract, theme: &ThemeContract) -> Result<(), String> {
    if refinement.user_feedback.trim().is_empty() || refinement.user_feedback.len() > 3_000 {
        return Err("Refinement feedback must be non-empty and at most 3,000 bytes".into());
    }
    evaluate_with_theme(&refinement.baseline_recipe, theme)
        .map_err(|_| "Refinement baseline recipe does not match the frozen theme or deterministic contract".to_string())?;
    let evidence = &refinement.baseline_evidence;
    if evidence.candidate_sha256 != sha256_json(&refinement.baseline_recipe)?
        || !valid_sha256(&evidence.candidate_sha256)
        || evidence.image_sha256.len() != 3 || evidence.image_sha256.iter().any(|hash| !valid_sha256(hash))
        || (!evidence.image_urls.is_empty() && (evidence.image_urls.len() != 3
            || evidence.image_urls.iter().any(|url| url.trim().is_empty() || url.len() > 2_048))) {
        return Err("Refinement baseline must bind the exact recipe and three renderer image hashes".into());
    }
    let mut hashes = evidence.image_sha256.clone();
    hashes.sort();
    hashes.dedup();
    if hashes.len() != 3 { return Err("Refinement baseline image hashes must be distinct".into()); }
    Ok(())
}

fn validate_reference(reference: &ReferenceContract) -> Result<(), String> {
    if !valid_sha256(&reference.sha256) || !(1..=2 * 1024 * 1024).contains(&reference.bytes)
        || !["image/png", "image/jpeg"].contains(&reference.mime_type.as_str())
        || reference.destination != "Google Gemini" || !reference.consent {
        return Err("Frozen reference requires explicit consent, a supported image, valid SHA-256, 1–2 MiB, and destination Google Gemini".into());
    }
    Ok(())
}

fn status_text(status: EvidenceStatus) -> &'static str {
    match status { EvidenceStatus::Pass => "pass", EvidenceStatus::Fail => "fail", EvidenceStatus::Blocked => "blocked" }
}

pub fn sha256_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_vec(value).map(|bytes| sha256(&bytes)).map_err(|error| error.to_string())
}

pub fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn valid_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn contract_hash(contract: &FrozenJobContract) -> Result<String, String> {
    let mut value = contract.clone();
    value.frozen_sha256.clear();
    sha256_json(&value)
}

fn pipeline_hash(pipeline: &AssetPipeline) -> Result<String, String> {
    let mut value = pipeline.clone();
    value.state_sha256.clear();
    sha256_json(&value)
}

fn timestamp_from_ms(ms: u64) -> String {
    let seconds = (ms / 1_000) as i64;
    let millis = ms % 1_000;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}
