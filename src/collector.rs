use crate::config::Backup;

use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeLabelSet, EncodeMetric},
    metrics::{family::Family, gauge::Gauge},
};
use rustic_backend::BackendOptions;
use rustic_core::{
    repofile::SnapshotFile, NoProgressBars, OpenStatus, Repository, RepositoryOptions,
};
use std::sync::{atomic::AtomicU64, Arc, Mutex};
use std::time::Duration;
use tracing::{debug, error, info, warn};

#[derive(Debug, Default)]
struct State {
    ready: bool,
    repository: Option<Repository<NoProgressBars, OpenStatus>>,
    snapshots: Vec<SnapshotFile>,
}

#[derive(Clone, Debug)]
pub struct RusticCollector {
    backup: Backup,
    interval: u64,
    state: Arc<Mutex<State>>,
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
    pub fn new(backup: Backup, interval: u64) -> Self {
        let collector = Self {
            backup,
            interval,
            state: Arc::new(Mutex::new(State::default())),
        };
        Self::start(collector.clone());
        collector
    }

    fn start(self) {
        tokio::spawn(async move {
            Self::set_repository(self.clone()).await;
            loop {
                Self::update_data(self.clone()).await;
                tokio::time::sleep(Duration::from_secs(self.interval)).await;
            }
        });
    }

    async fn set_repository(self) {
        let opts = match (self.backup.password, self.backup.password_file) {
            (Some(password), _) => RepositoryOptions::default().password(password),
            (_, Some(password_file)) => RepositoryOptions::default().password_file(password_file),
            _ => panic!("Either password or password_file must be set"),
        };

        let backend_result = BackendOptions::default()
            .repository(self.backup.repository)
            .options(self.backup.options)
            .to_backends();

        let backend = match backend_result {
            Ok(backend) => backend,
            Err(_) => {
                error!("Unable to set the backend, repository {}", self.backup.name);
                return;
            }
        };

        let repository_result = tokio::task::spawn_blocking(move || {
            Repository::new(&opts, &backend).and_then(|repo| repo.open())
        })
        .await;

        let repository = match repository_result {
            Ok(Ok(repo)) => repo,
            Ok(Err(_)) => {
                error!("Unable to open the repository: {}", self.backup.name);
                return;
            }
            Err(_) => return,
        };

        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.repository = Some(repository);
        state.ready = true;
        info!("Repository is ready, repository: {}", self.backup.name);
    }

    async fn update_data(self) {
        debug!("Updating metrics, repository: {}", self.backup.name);
        let backup_name = self.backup.name.clone();

        let result = tokio::task::spawn_blocking(move || {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let repository = state.repository.as_ref().unwrap();
            match repository.update_all_snapshots(state.snapshots.clone()) {
                Ok(snapshots) => state.snapshots = snapshots,
                Err(_err) => error!("Unable to update snapshot, repository: {}", backup_name),
            };
        })
        .await;
        match result {
            Ok(()) => debug!(
                "Successfully updated metrics, repository: {}",
                self.backup.name
            ),
            Err(_err) => error!("Failed to update metrics, repository: {}", self.backup.name),
        }
    }
}

impl Collector for RusticCollector {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), std::fmt::Error> {
        let data = match self.state.lock() {
            Ok(data) => data,
            Err(poisoned) => poisoned.into_inner(),
        };

        //-- Set metrics
        // return if repository is not ready
        if !data.ready {
            warn!(
                "Repository is not ready yet, repository: {}",
                self.backup.name
            );
            return Ok(());
        }

        let repo = data.repository.as_ref().unwrap();
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
        for snapshot in &data.snapshots {
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
                    "Snapshot summary has no data, repository: {}, snapshot_id: {} ",
                    self.backup.name,
                    snapshot.id.to_string()
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
