//! `FsJobStore`: one atomically-replaced `jobs/<id>.json` per job.
use crate::jsonl::{be, check_id, create_dir, lock, read_json, write_json};
use lp_core::ports::{DeliveryStatus, JobRecord, JobStatus, JobStore, StoreError, StoreResult};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct FsJobStore {
    dir: PathBuf,
    /// Makes `claim_steers` read-then-mark atomic, so each delivery is claimed once.
    lock: Mutex<()>,
}

impl FsJobStore {
    pub fn open(dir: impl Into<PathBuf>) -> StoreResult<Self> {
        let dir = dir.into();
        create_dir(&dir)?;
        Ok(Self {
            dir,
            lock: Mutex::new(()),
        })
    }

    fn path(&self, id: &str) -> StoreResult<PathBuf> {
        check_id(id)?;
        Ok(self.dir.join(format!("{id}.json")))
    }

    /// Every readable job, oldest first, plus the paths of job files that could not be read.
    pub fn all_report(&self) -> StoreResult<(Vec<JobRecord>, Vec<String>)> {
        let mut jobs = vec![];
        let mut skipped = vec![];
        for entry in std::fs::read_dir(&self.dir).map_err(be)? {
            let path = entry.map_err(be)?.path();
            if path.extension().is_some_and(|e| e == "json") {
                // One unreadable job must not block claiming or recovery of the rest.
                match read_json(&path, "") {
                    Ok(job) => jobs.push(job),
                    Err(_) => skipped.push(path.display().to_string()),
                }
            }
        }
        jobs.sort_by(|a: &JobRecord, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok((jobs, skipped))
    }

    fn all(&self) -> StoreResult<Vec<JobRecord>> {
        let (jobs, skipped) = self.all_report()?;
        for s in skipped {
            tracing::warn!("skipped unreadable job file {s}");
        }
        Ok(jobs)
    }
}

impl JobStore for FsJobStore {
    fn create_job(&self, job: &JobRecord) -> StoreResult<()> {
        let _guard = self.lock.lock().map_err(be)?;
        write_json(&self.path(&job.id)?, job)
    }
    fn update_job(&self, job: &JobRecord) -> StoreResult<()> {
        let _guard = self.lock.lock().map_err(be)?;
        let path = self.path(&job.id)?;
        if !path.exists() {
            return Err(StoreError::NotFound(job.id.to_string()));
        }
        write_json(&path, job)
    }
    fn claim_steers(&self, chat_id: &str) -> StoreResult<Vec<JobRecord>> {
        let _guard = self.lock.lock().map_err(be)?;
        let _file_lock = lock(&self.dir.join("claims"))?;
        let mut claimed = vec![];
        for mut job in self.all()? {
            if job.chat_id == chat_id
                && job.status == JobStatus::Done
                && job.delivery == DeliveryStatus::Pending
            {
                job.delivery = DeliveryStatus::Claimed;
                write_json(&self.path(&job.id)?, &job)?;
                claimed.push(job);
            }
        }
        Ok(claimed)
    }
    fn claimed_jobs(&self) -> StoreResult<Vec<JobRecord>> {
        Ok(self
            .all()?
            .into_iter()
            .filter(|j| j.status == JobStatus::Done && j.delivery == DeliveryStatus::Claimed)
            .collect())
    }
    fn interrupted_jobs(&self) -> StoreResult<Vec<JobRecord>> {
        Ok(self
            .all()?
            .into_iter()
            .filter(|j| j.status == JobStatus::Running)
            .collect())
    }
}
