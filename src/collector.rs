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
struct SnapshotInfoLabels {
    repo_id: String,
    id: String,
    paths: String,
    hostname: String,
    username: String,
    tags: String,
    program_version: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet, Default)]
struct RepositoryInfoLabels {
    name: String,
    repo_id: String,
    version: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet, Default)]
struct SnapshotLables {
    id: String,
}

struct Metrics {
    rustic_repository_info: Family<RepositoryInfoLabels, Gauge>,
    rustic_snapshot_info: Family<SnapshotInfoLabels, Gauge>,
    rustic_snapshot_timestamp: Family<SnapshotLables, Gauge>,
    rustic_snapshot_backup_start_timestamp: Family<SnapshotLables, Gauge>,
    rustic_snapshot_backup_end_timestamp: Family<SnapshotLables, Gauge>,
    rustic_snpashot_backup_duration_seconds: Family<SnapshotLables, Gauge<f64, AtomicU64>>,
    rustic_snapshot_files_total: Family<SnapshotLables, Gauge>,
    rustic_snapshot_size_bytes: Family<SnapshotLables, Gauge>,
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
        let this = self.clone();
        let repository = tokio::task::spawn_blocking(move || {
            let opts = RepositoryOptions::default().password(this.backup.password);
            let backend = BackendOptions::default()
                .repository(this.backup.repository)
                .options(this.backup.options)
                .to_backends()
                .unwrap();
            Repository::new(&opts, &backend)
                .expect("cannot create the repository")
                .open()
                .expect("cannot open the repository")
        })
        .await
        .unwrap();

        let mut state = self.state.lock().unwrap();
        state.repository = Some(repository);
        state.ready = true;
    }

    async fn update_data(self) {
        tokio::task::spawn_blocking(move || {
            let mut state = self.state.lock().unwrap();
            let repository = state.repository.as_ref().unwrap();
            let snapshots = repository
                .update_all_snapshots(state.snapshots.clone())
                .unwrap();
            state.snapshots = snapshots
        })
        .await
        .unwrap();
    }
}

impl Collector for RusticCollector {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), std::fmt::Error> {
        let data = self.state.lock().unwrap();

        //-- Set metrics
        // return if data is not ready
        if !data.ready {
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
            rustic_snpashot_backup_duration_seconds: Family::default(),
            rustic_snapshot_files_total: Family::default(),
            rustic_snapshot_size_bytes: Family::default(),
        };

        // set repository metrics
        metrics
            .rustic_repository_info
            .get_or_create(&RepositoryInfoLabels {
                name: repo.name.to_string(),
                repo_id: repo_config.id.to_string(),
                version: repo_config.version.to_string(),
            })
            .set(1);

        // set snapshot metrics
        for snapshot in &data.snapshots {
            let snapshot_info_labels = SnapshotInfoLabels {
                repo_id: repo_config.id.to_string(),
                id: snapshot.id.to_string(),
                paths: snapshot.paths.to_string(),
                tags: snapshot.tags.to_string(),
                hostname: snapshot.hostname.to_string(),
                username: snapshot.username.to_string(),
                program_version: snapshot.program_version.to_string(),
            };
            let snapshot_labels = SnapshotLables {
                id: snapshot.id.to_string(),
            };

            metrics
                .rustic_snapshot_info
                .get_or_create(&snapshot_info_labels)
                .set(1);

            metrics
                .rustic_snapshot_timestamp
                .get_or_create(&snapshot_labels)
                .set(snapshot.time.timestamp());

            // skip current iteration if snapshot summary having no data
            if snapshot.summary.is_none() {
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
                .set(summary.backup_start.timestamp_micros());

            metrics
                .rustic_snapshot_backup_end_timestamp
                .get_or_create(&snapshot_labels)
                .set(summary.backup_end.timestamp_micros());

            metrics
                .rustic_snpashot_backup_duration_seconds
                .get_or_create(&snapshot_labels)
                .set(
                    (summary.backup_end - summary.backup_start)
                        .num_microseconds()
                        .unwrap() as f64
                        / (1000.0 * 1000.0),
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
        metrics.rustic_snpashot_backup_duration_seconds.encode(
            encoder.encode_descriptor(
                "rustic_snpashot_backup_duration_seconds",
                "Backup duration of a snapshot.",
                None,
                metrics
                    .rustic_snpashot_backup_duration_seconds
                    .metric_type(),
            )?,
        )?;

        Ok(())
    }
}
