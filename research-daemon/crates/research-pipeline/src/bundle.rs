use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use research_core::{AuditReport, Job, Plan, QueryRecord, SourceId, SourceRecord, SubquestionId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::PipelineError;

/// A note file to be written under `notes/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFile {
    /// Path relative to the bundle root.
    pub path: String,
    pub content: String,
}

/// Resumable pipeline checkpoints. The bundle, not the database, owns the
/// fine-grained progress so a killed worker can continue after `research job
/// resume`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    #[serde(default)]
    pub completed_subquestions: Vec<SubquestionId>,
    #[serde(default)]
    pub plan_approved: bool,
}

impl Progress {
    pub fn is_complete(&self, subquestion: SubquestionId) -> bool {
        self.completed_subquestions.contains(&subquestion)
    }

    pub fn mark_complete(&mut self, subquestion: SubquestionId) {
        if !self.is_complete(subquestion) {
            self.completed_subquestions.push(subquestion);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunUsage {
    pub queries: usize,
    pub sources: usize,
    pub claims: usize,
    pub credits: u64,
    #[serde(default)]
    pub llm_tokens: u64,
    #[serde(default)]
    pub llm_cost_usd: Option<f64>,
}

/// Stamped by the daemon after the audit, never by a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunMetadata {
    pub schema_version: u32,
    pub job_id: String,
    pub question: String,
    pub depth: String,
    pub provider: String,
    pub planner: String,
    pub synthesizer: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub llm_provider: Option<String>,
    #[serde(default)]
    pub llm_model: Option<String>,
    #[serde(default)]
    pub prompt_version: Option<String>,
    #[serde(default)]
    pub agent_rounds: u32,
    #[serde(default)]
    pub stop_reason: Option<String>,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub output_dir: String,
    pub usage: RunUsage,
    pub audit: AuditReport,
}

/// Writer and reader for one research bundle directory.
#[derive(Debug, Clone)]
pub struct Bundle {
    root: PathBuf,
}

impl Bundle {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, PipelineError> {
        let root = root.into();
        fs::create_dir_all(root.join("notes"))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn report_path(&self) -> PathBuf {
        self.root.join("report.md")
    }

    /// Write `brief.md` only on the first attempt so resume preserves it.
    pub fn write_brief(&self, job: &Job) -> Result<bool, PipelineError> {
        let path = self.root.join("brief.md");
        if path.exists() {
            return Ok(false);
        }
        write_atomic(&path, crate::render::render_brief(job).as_bytes())?;
        Ok(true)
    }

    pub fn save_plan(&self, plan: &Plan) -> Result<(), PipelineError> {
        let json = serde_json::to_vec_pretty(plan)?;
        write_atomic(
            &self.root.join(format!("plan.v{}.json", plan.version)),
            &json,
        )?;
        write_atomic(
            &self.root.join(format!("plan.v{}.md", plan.version)),
            crate::render::render_plan(plan).as_bytes(),
        )?;
        Ok(())
    }

    pub fn load_plan(&self) -> Result<Option<Plan>, PipelineError> {
        let Some(path) = self.plan_path()? else {
            return Ok(None);
        };
        let bytes = fs::read(path)?;
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    pub fn append_query(&self, query: &QueryRecord) -> Result<(), PipelineError> {
        append_jsonl(&self.root.join("queries.jsonl"), query)
    }

    pub fn append_source(&self, source: &SourceRecord) -> Result<(), PipelineError> {
        append_jsonl(&self.root.join("sources.jsonl"), source)
    }

    pub fn load_queries(&self) -> Result<Vec<QueryRecord>, PipelineError> {
        load_jsonl(&self.root.join("queries.jsonl"))
    }

    /// Load sources, collapsing crash-window duplicates by canonical URL.
    pub fn load_sources(&self) -> Result<Vec<SourceRecord>, PipelineError> {
        let sources: Vec<SourceRecord> = load_jsonl(&self.root.join("sources.jsonl"))?;
        let mut seen = HashSet::new();
        Ok(sources
            .into_iter()
            .filter(|source| seen.insert(source.canonical_url.clone()))
            .collect())
    }

    pub fn write_note(&self, note: &NoteFile) -> Result<(), PipelineError> {
        let path = self.root.join(&note.path);
        let escapes = path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir));
        if escapes || !path.starts_with(&self.root) {
            return Err(PipelineError::Bundle(format!(
                "note path escapes the bundle: {}",
                note.path
            )));
        }
        write_atomic(&path, note.content.as_bytes())
    }

    pub fn source_content(&self, source: &SourceRecord) -> Result<String, PipelineError> {
        Ok(fs::read_to_string(self.root.join(&source.content_path))?)
    }

    pub fn load_contents(
        &self,
        sources: &[SourceRecord],
    ) -> Result<HashMap<SourceId, String>, PipelineError> {
        let mut contents = HashMap::with_capacity(sources.len());
        for source in sources {
            contents.insert(source.id, self.source_content(source)?);
        }
        Ok(contents)
    }

    pub fn load_progress(&self) -> Result<Progress, PipelineError> {
        let path = self.root.join("progress.v1.json");
        if !path.exists() {
            return Ok(Progress::default());
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    pub fn save_progress(&self, progress: &Progress) -> Result<(), PipelineError> {
        let json = serde_json::to_vec_pretty(progress)?;
        write_atomic(&self.root.join("progress.v1.json"), &json)
    }

    pub fn mark_plan_approved(&self) -> Result<(), PipelineError> {
        let mut progress = self.load_progress()?;
        progress.plan_approved = true;
        self.save_progress(&progress)
    }

    pub fn write_report(&self, markdown: &str) -> Result<(), PipelineError> {
        write_atomic(&self.root.join("report.md"), markdown.as_bytes())
    }

    pub fn write_audit(&self, markdown: &str) -> Result<(), PipelineError> {
        write_atomic(&self.root.join("audit.md"), markdown.as_bytes())
    }

    pub fn write_run(&self, metadata: &RunMetadata) -> Result<(), PipelineError> {
        let json = serde_json::to_vec_pretty(metadata)?;
        write_atomic(&self.root.join("run.json"), &json)
    }

    /// Append this bundle to the adjacent `INDEX.md`, once.
    pub fn update_index(&self, status: &str) -> Result<(), PipelineError> {
        let Some(directory) = self.root.file_name().map(|name| name.to_string_lossy()) else {
            return Ok(());
        };
        let index_path = self.root.parent().unwrap_or(&self.root).join("INDEX.md");
        let link = format!("[{directory}]({directory}/report.md)");

        let mut index = match fs::read_to_string(&index_path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                "# Research index\n\n| Date | Project | Question | Status |\n|---|---|---|---|\n"
                    .to_owned()
            }
            Err(err) => return Err(err.into()),
        };

        if index.contains(&link) {
            return Ok(());
        }
        if !index.ends_with('\n') {
            index.push('\n');
        }

        let question = self
            .load_plan()?
            .map(|plan| plan.raw_topic)
            .unwrap_or_default()
            .replace('|', "\\|")
            .replace('\n', " ");
        index.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            Utc::now().format("%Y-%m-%d"),
            link,
            question,
            status
        ));
        write_atomic(&index_path, index.as_bytes())
    }

    fn plan_path(&self) -> Result<Option<PathBuf>, PipelineError> {
        let mut best: Option<(u32, PathBuf)> = None;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(rest) = name.strip_prefix("plan.v") else {
                continue;
            };
            let Some(version) = rest.strip_suffix(".json") else {
                continue;
            };
            let Ok(version) = version.parse::<u32>() else {
                continue;
            };
            if best.as_ref().is_none_or(|(best, _)| version > *best) {
                best = Some((version, entry.path()));
            }
        }
        Ok(best.map(|(_, path)| path))
    }
}

fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<(), PipelineError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(value)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn load_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, PipelineError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };

    // A crash can leave a partial final line. Keep the complete records.
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PipelineError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use research_core::{
        JobBudget, JobId, JobStatus, PlanConstraints, PlanId, ResearchRequest, SourceClass,
        Subquestion,
    };

    fn plan() -> Plan {
        Plan {
            id: PlanId::new(),
            version: 1,
            raw_topic: "A question".to_owned(),
            sharpened_topic: "A question".to_owned(),
            subquestions: vec![Subquestion {
                id: SubquestionId::new(),
                question: "A subquestion?".to_owned(),
                source_class: SourceClass::Reference,
                query_seeds: vec!["seed".to_owned()],
                success_criterion: "enough".to_owned(),
            }],
            constraints: PlanConstraints::default(),
            budget: JobBudget::default(),
            created_at: Utc::now(),
        }
    }

    fn source(subquestion: SubquestionId) -> SourceRecord {
        SourceRecord {
            id: SourceId::new(),
            subquestion_id: subquestion,
            url: "https://example.com/a".to_owned(),
            canonical_url: "https://example.com/a".to_owned(),
            title: "Example".to_owned(),
            publisher: "example.com".to_owned(),
            published_date: None,
            retrieved_at: Utc::now(),
            source_type: SourceClass::Reference,
            content_hash: "hash".to_owned(),
            quality_rationale: "test".to_owned(),
            independence_group: "example.com".to_owned(),
            content_path: "notes/a.md".to_owned(),
            excerpt: "excerpt".to_owned(),
        }
    }

    #[tokio::test]
    async fn bundles_roundtrip_plan_sources_and_progress() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = Bundle::open(dir.path().join("research/test")).unwrap();
        let plan = plan();
        bundle.save_plan(&plan).unwrap();
        assert_eq!(bundle.load_plan().unwrap().unwrap(), plan);

        let source = source(plan.subquestions[0].id);
        bundle.append_source(&source).unwrap();
        bundle.append_source(&source).unwrap();
        assert_eq!(bundle.load_sources().unwrap().len(), 1);
        bundle
            .write_note(&NoteFile {
                path: source.content_path.clone(),
                content: "stored text".to_owned(),
            })
            .unwrap();
        assert_eq!(bundle.source_content(&source).unwrap(), "stored text");

        let mut progress = Progress::default();
        progress.mark_complete(plan.subquestions[0].id);
        bundle.save_progress(&progress).unwrap();
        assert!(
            bundle
                .load_progress()
                .unwrap()
                .is_complete(plan.subquestions[0].id)
        );
        bundle.mark_plan_approved().unwrap();
        assert!(bundle.load_progress().unwrap().plan_approved);
    }

    #[tokio::test]
    async fn index_is_appended_once() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = Bundle::open(dir.path().join("research/2026-11-04-test")).unwrap();
        bundle.save_plan(&plan()).unwrap();

        bundle.update_index("completed").unwrap();
        bundle.update_index("completed").unwrap();

        let index = fs::read_to_string(dir.path().join("research/INDEX.md")).unwrap();
        assert_eq!(index.matches("| completed |").count(), 1);
        assert!(index.contains("A question"));
    }

    #[test]
    fn note_paths_cannot_escape_the_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = Bundle::open(dir.path().join("bundle")).unwrap();
        let result = bundle.write_note(&NoteFile {
            path: "../outside.md".to_owned(),
            content: "no".to_owned(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn brief_is_written_once() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = Bundle::open(dir.path().join("bundle")).unwrap();
        let job = Job {
            id: JobId::new(),
            request: ResearchRequest::new("question"),
            status: JobStatus::Queued,
            output_dir: "bundle".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            lease_owner: None,
            lease_expires_at: None,
            attempts: 0,
            last_error: None,
            idempotency_key: None,
            cancel_requested: false,
        };

        assert!(bundle.write_brief(&job).unwrap());
        assert!(!bundle.write_brief(&job).unwrap());
    }
}
