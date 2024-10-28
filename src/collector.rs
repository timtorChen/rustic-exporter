use crate::config::Backup;

use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeLabelSet, EncodeMetric},
    metrics::{family::Family, gauge::Gauge},
};
use rustic_backend::BackendOptions;
use rustic_core::{
    repofile::{ConfigFile as RepositoryDetails, SnapshotFile as SnapshotDetails},
    Repository, RepositoryOptions,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, Default)]
struct Data {
    repository: RepositoryDetails,
    snapshots: Vec<SnapshotDetails>,
    ready: bool,
}

#[derive(Clone, Debug)]
pub struct RusticCollector {
    backup: Backup,
    interval: u64,
    data: Arc<Mutex<Data>>,
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
    rustic_snpashot_backup_duration: Family<SnapshotLables, Gauge>,
    rustic_snapshot_files_total: Family<SnapshotLables, Gauge>,
    rustic_snapshot_size_bytes: Family<SnapshotLables, Gauge>,
}

impl RusticCollector {
    pub fn new(backup: Backup, interval: u64) -> Self {
        let collector = Self {
            backup,
            interval,
            data: Arc::new(Mutex::new(Data::default())),
        };
        Self::start(collector.clone());
        collector
    }

    fn start(self) {
        tokio::spawn(async move {
            loop {
                Self::update_data(self.clone()).await;
                tokio::time::sleep(Duration::from_secs(self.interval)).await;
            }
        });
    }

    async fn update_data(self) {
        let opts = RepositoryOptions::default().password(&self.backup.password);
        let backend = BackendOptions::default()
            .repository(&self.backup.repository)
            .options(self.backup.options.clone())
            .to_backends()
            .unwrap();

        tokio::task::spawn_blocking(move || {
            let repository = Repository::new(&opts, &backend)
                .expect("cannot create the repository")
                .open()
                .expect("cannot open the repository");

            let snapshots = repository
                .get_all_snapshots()
                .expect("cannot get snapshots");

            let mut data = self.data.lock().unwrap();
            data.repository = repository.config().clone();
            data.snapshots = snapshots;
            data.ready = true;
        })
        .await
        .unwrap();
    }
}

impl Collector for RusticCollector {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), std::fmt::Error> {
        let metrics = Metrics {
            rustic_repository_info: Family::default(),
            rustic_snapshot_info: Family::default(),
            rustic_snapshot_timestamp: Family::default(),
            rustic_snapshot_backup_end_timestamp: Family::default(),
            rustic_snapshot_backup_start_timestamp: Family::default(),
            rustic_snpashot_backup_duration: Family::default(),
            rustic_snapshot_files_total: Family::default(),
            rustic_snapshot_size_bytes: Family::default(),
        };

        //-- Set metrics
        // return if data is not ready
        let backup_name = self.backup.name.clone();
        let data = self.data.lock().unwrap().clone();
        if !data.ready {
            return Ok(());
        }

        // set repository metrics
        metrics
            .rustic_repository_info
            .get_or_create(&RepositoryInfoLabels {
                name: backup_name,
                repo_id: data.repository.id.to_string(),
                version: data.repository.version.to_string(),
            })
            .set(1);

        // set snapshot metrics
        for snapshot in data.snapshots {
            let snapshot_info_labels = SnapshotInfoLabels {
                repo_id: data.repository.id.to_string(),
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

            let summary = snapshot.summary.unwrap();

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
                .rustic_snpashot_backup_duration
                .get_or_create(&snapshot_labels)
                .set(
                    (summary.backup_end - summary.backup_start)
                        .num_microseconds()
                        .unwrap(),
                );
        }

        //-- Encode
        metrics
            .rustic_repository_info
            .encode(encoder.encode_descriptor(
                "rustic_repository_info",
                "",
                None,
                metrics.rustic_repository_info.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_info
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_info",
                "",
                None,
                metrics.rustic_snapshot_info.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_files_total
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_files_total",
                "",
                None,
                metrics.rustic_snapshot_files_total.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_timestamp
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_timestamp",
                "",
                None,
                metrics.rustic_snapshot_timestamp.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_size_bytes
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_size_bytes",
                "",
                None,
                metrics.rustic_snapshot_size_bytes.metric_type(),
            )?)?;

        metrics
            .rustic_snapshot_backup_start_timestamp
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_backup_start_timestamp",
                "",
                None,
                metrics.rustic_snapshot_backup_start_timestamp.metric_type(),
            )?)?;
        metrics
            .rustic_snapshot_backup_end_timestamp
            .encode(encoder.encode_descriptor(
                "rustic_snapshot_backup_end_timestamp",
                "",
                None,
                metrics.rustic_snapshot_backup_end_timestamp.metric_type(),
            )?)?;
        metrics
            .rustic_snpashot_backup_duration
            .encode(encoder.encode_descriptor(
                "rustic_snpashot_backup_duration",
                "",
                None,
                metrics.rustic_snpashot_backup_duration.metric_type(),
            )?)?;

        Ok(())
    }
}
