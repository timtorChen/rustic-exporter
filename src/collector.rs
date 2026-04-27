use crate::config::Backup;

use arc_swap::ArcSwap;
use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeLabelSet, EncodeMetric},
    metrics::{family::Family, gauge::Gauge},
};
use rustic_backend::BackendOptions;
use rustic_core::{
    repofile::SnapshotFile, NoProgressBars, OpenStatus, Repository, RepositoryOptions,
};
use std::sync::{atomic::AtomicU64, Arc};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("repository is not ready")]
    RepositoryNotReady,
    #[error("snapshot update failed")]
    SnapshotUpdateFailed,
}

#[derive(Debug)]
pub struct RusticCollector {
    backup: Backup,
    interval: u64,
    repository: ArcSwap<Option<Repository<NoProgressBars, OpenStatus>>>,
    snapshots: ArcSwap<Vec<SnapshotFile>>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet, Default)]
struct RepositoryInfoLabels {
    repo_name: String,
    repo_id: String,
    version: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet, Default)]
struct SnapshotInfoLabels {
    repo_name: String,
    repo_id: String,
    snapshot_id: String,
    paths: String,
    hostname: String,
    username: String,
    tags: String,
    program_version: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet, Default)]
struct SnapshotLabels {
    repo_name: String,
    repo_id: String,
    snapshot_id: String,
}

struct Metrics {
    rustic_repository_info: Family<RepositoryInfoLabels, Gauge>,
    rustic_snapshot_info: Family<SnapshotInfoLabels, Gauge>,
    rustic_snapshot_timestamp: Family<SnapshotLabels, Gauge<f64, AtomicU64>>,
    rustic_snapshot_backup_start_timestamp: Family<SnapshotLabels, Gauge<f64, AtomicU64>>,
    rustic_snapshot_backup_end_timestamp: Family<SnapshotLabels, Gauge<f64, AtomicU64>>,
    rustic_snapshot_backup_duration_seconds: Family<SnapshotLabels, Gauge<f64, AtomicU64>>,
    rustic_snapshot_files_total: Family<SnapshotLabels, Gauge>,
    rustic_snapshot_size_bytes: Family<SnapshotLabels, Gauge>,
}

impl RusticCollector {
    pub fn new(backup: Backup, interval: u64) -> Arc<Self> {
        let collector = Arc::new(Self {
            backup,
            interval,
            repository: ArcSwap::new(Arc::new(None)),
            snapshots: ArcSwap::new(Arc::new(Vec::new())),
        });

        tokio::spawn({
            let collector = collector.clone();
            async move {
                loop {
                    let repo_result = Self::set_repository(collector.clone()).await;
                    if repo_result.is_ok() {
                        // successfully set the repository, looply update snapshots
                        loop {
                            let snapshots_result = Self::update_snapshots(collector.clone()).await;
                            if matches!(snapshots_result, Err(CollectorError::RepositoryNotReady)) {
                                // repository become not ready somehow, break the loop and start over
                                break;
                            }
                            tokio::time::sleep(Duration::from_secs(collector.interval)).await;
                        }
                    } else {
                        // failed to set the repository, wait and start over
                        error!(
                            repository = collector.backup.name,
                            "failed to set the repostiroy"
                        );
                        tokio::time::sleep(Duration::from_secs(collector.interval)).await;
                    }
                }
            }
        });
        collector
    }

    async fn set_repository(self: Arc<Self>) -> Result<(), CollectorError> {
        debug!(repository = self.backup.name, "setting repository");
        let opts = match (&self.backup.password, &self.backup.password_file) {
            (Some(password), _) => RepositoryOptions::default().password(password),
            (_, Some(password_file)) => RepositoryOptions::default().password_file(password_file),
            _ => panic!("either password or password_file must be set"),
        };

        let backend = BackendOptions::default()
            .repository(&self.backup.repository)
            .options(self.backup.options.clone())
            .to_backends()
            .map_err(|_| CollectorError::RepositoryNotReady)?;

        let repository = tokio::task::spawn_blocking(move || {
            Repository::new(&opts, &backend).and_then(|repo| repo.open())
        })
        .await
        .map_err(|_| CollectorError::RepositoryNotReady)?
        .map_err(|_| CollectorError::RepositoryNotReady)?;

        self.repository.store(Arc::new(Some(repository)));
        info!(repository = self.backup.name, "repository is ready");
        Ok(())
    }

    async fn update_snapshots(self: Arc<Self>) -> Result<(), CollectorError> {
        debug!(repository = self.backup.name, "updating snapshots");
        let collector: Arc<RusticCollector> = self.clone();

        let snpashots = tokio::task::spawn_blocking({
            move || -> Result<Vec<SnapshotFile>, CollectorError> {
                let repo_guard = collector.repository.load();
                let repo = repo_guard
                    .as_ref()
                    .as_ref()
                    .ok_or(CollectorError::RepositoryNotReady)?;

                let snapshots = repo
                    .update_all_snapshots(collector.snapshots.load().to_vec())
                    .map_err(|_| CollectorError::SnapshotUpdateFailed)?;
                Ok(snapshots)
            }
        })
        .await
        .map_err(|_| CollectorError::SnapshotUpdateFailed)?
        .map_err(|_| CollectorError::SnapshotUpdateFailed)?;

        self.snapshots.swap(Arc::new(snpashots));
        info!(repository = %self.backup.name, "snapshots updated");
        Ok(())
    }
}

impl Collector for RusticCollector {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), std::fmt::Error> {
        let repo_guard = self.repository.load();
        let repo = match repo_guard.as_ref() {
            Some(repo) => repo,
            None => {
                warn!(repository = self.backup.name, "repository is not ready yet",);
                return Ok(());
            }
        };

        let repo_config = repo.config();
        let metrics = Metrics {
            rustic_repository_info: Family::default(),
            rustic_snapshot_info: Family::default(),
            rustic_snapshot_timestamp: Family::default(),
            rustic_snapshot_backup_end_timestamp: Family::default(),
            rustic_snapshot_backup_start_timestamp: Family::default(),
            rustic_snapshot_backup_duration_seconds: Family::default(),
            rustic_snapshot_files_total: Family::default(),
            rustic_snapshot_size_bytes: Family::default(),
        };

        // set repository metrics
        metrics
            .rustic_repository_info
            .get_or_create(&RepositoryInfoLabels {
                repo_name: self.backup.name.clone(),
                repo_id: repo_config.id.to_string(),
                version: repo_config.version.to_string(),
            })
            .set(1);

        // set snapshot metrics
        for snapshot in &**self.snapshots.load() {
            let snapshot_info_labels = SnapshotInfoLabels {
                repo_name: self.backup.name.clone(),
                repo_id: repo_config.id.to_string(),
                snapshot_id: snapshot.id.to_string(),
                paths: snapshot.paths.to_string(),
                tags: snapshot.tags.to_string(),
                hostname: snapshot.hostname.to_string(),
                username: snapshot.username.to_string(),
                program_version: snapshot.program_version.to_string(),
            };

            let snapshot_labels = SnapshotLabels {
                repo_name: self.backup.name.clone(),
                repo_id: repo_config.id.to_string(),
                snapshot_id: snapshot.id.to_string(),
            };

            metrics
                .rustic_snapshot_info
                .get_or_create(&snapshot_info_labels)
                .set(1);

            metrics
                .rustic_snapshot_timestamp
                .get_or_create(&snapshot_labels)
                .set(snapshot.time.timestamp_micros() as f64 / (10f64.powf(6.0)));

            // skip current iteration if snapshot summary having no data
            if snapshot.summary.is_none() {
                warn!(
                    repository = self.backup.name,
                    snapshot_id = snapshot.id.to_string(),
                    "snapshot summary has no data",
                );
                continue;
            }

            let summary = snapshot.summary.as_ref().unwrap();

            metrics
                .rustic_snapshot_files_total
                .get_or_create(&snapshot_labels)
                .set(summary.total_files_processed as i64);

            metrics
                .rustic_snapshot_size_bytes
                .get_or_create(&snapshot_labels)
                .set(summary.total_bytes_processed as i64);

            metrics
                .rustic_snapshot_backup_start_timestamp
                .get_or_create(&snapshot_labels)
                .set(summary.backup_start.timestamp_micros() as f64 / (10f64.powf(6.0)));

            metrics
                .rustic_snapshot_backup_end_timestamp
                .get_or_create(&snapshot_labels)
                .set(summary.backup_end.timestamp_micros() as f64 / (10f64.powf(6.0)));

            metrics
                .rustic_snapshot_backup_duration_seconds
                .get_or_create(&snapshot_labels)
                .set(
                    (summary.backup_end - summary.backup_start)
                        .num_microseconds()
                        .unwrap() as f64
                        / (10f64.powf(6.0)),
                );
        }

        //-- Encode
        metrics
            .rustic_repository_info
            .encode(encoder.encode_descriptor(
                "rustic_repository_info",
                "Repository information.",
                None,
                metrics.rustic_repository_info.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_info
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_info",
                "Snapshot inforamation.",
                None,
                metrics.rustic_snapshot_info.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_files_total
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_files_total",
                "Total files in a snapshot.",
                None,
                metrics.rustic_snapshot_files_total.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_timestamp
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_timestamp",
                "Snapshot creation time in unix timestamp.",
                None,
                metrics.rustic_snapshot_timestamp.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_size_bytes
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_size_bytes",
                "Snapshot size in bytes.",
                None,
                metrics.rustic_snapshot_size_bytes.metric_type(),
            )?)?;

        metrics
            .rustic_snapshot_backup_start_timestamp
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_backup_start_timestamp",
                "Backup start time of a snapshot in unix timestamp.",
                None,
                metrics.rustic_snapshot_backup_start_timestamp.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_backup_end_timestamp
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_backup_end_timestamp",
                "Backup finished time of a snapshot in unix timestamp.",
                None,
                metrics.rustic_snapshot_backup_end_timestamp.metric_type(),
            )?)?;
        metrics.rustic_snapshot_backup_duration_seconds.encode(
            encoder.encode_descriptor(
                "rustic_snapshot_backup_duration_seconds",
                "Backup duration of a snapshot.",
                None,
                metrics
                    .rustic_snapshot_backup_duration_seconds
                    .metric_type(),
            )?,
        )?;

        Ok(())
    }
}
