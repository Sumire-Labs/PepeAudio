//! Process-private media root, hard quota, and janitor assembly.

use std::{path::Path, path::PathBuf, sync::Arc, time::Duration};

use pepeaudio_config::{BotRuntimeConfig, ToolConfig};
use pepeaudio_media::{JanitorPolicy, ManagedDownloadJanitor, ManagedMediaLeaseRegistry};

use crate::BotError;

pub(crate) struct PreparedMedia {
    pub(crate) tools: ToolConfig,
    pub(crate) leases: ManagedMediaLeaseRegistry,
    pub(crate) janitor: Arc<ManagedDownloadJanitor>,
}

pub(crate) async fn prepare_managed_media(
    runtime: &BotRuntimeConfig,
) -> Result<PreparedMedia, BotError> {
    // The quota and deletion registry are process-local, so every shard
    // process receives a private subtree even when the base volume is shared.
    let tools = instance_media_tools(runtime);
    let policy = media_janitor_policy(runtime);
    let maximum_bytes = runtime.player.max_managed_media_bytes.get();
    let leases = ManagedMediaLeaseRegistry::new_with_capacity(
        &tools.upload_directory,
        maximum_bytes,
        policy.max_entries_per_scan,
    )
    .await
    .map_err(|_| BotError::MediaAdapter)?;
    if let Some(usage) = leases.capacity_usage() {
        tracing::info!(
            used_bytes = usage.used_bytes,
            reserved_bytes = usage.reserved_bytes,
            maximum_bytes = usage.maximum_bytes,
            managed_files = usage.managed_files,
            maximum_entries = usage.maximum_entries,
            "managed media hard quota initialized"
        );
    }
    let janitor =
        ManagedDownloadJanitor::new_with_registry(&tools.upload_directory, policy, leases.clone())
            .await
            .map(Arc::new)
            .map_err(|_| BotError::MediaAdapter)?;
    janitor.run().await.map_err(|_| BotError::MediaAdapter)?;
    Ok(PreparedMedia {
        tools,
        leases,
        janitor,
    })
}

fn media_janitor_policy(runtime: &BotRuntimeConfig) -> JanitorPolicy {
    JanitorPolicy {
        minimum_object_retention: Duration::from_mins(5),
        object_ttl: Duration::from_hours(7 * 24),
        max_total_bytes: runtime.player.max_managed_media_bytes.get(),
        ..JanitorPolicy::default()
    }
}

fn instance_media_tools(runtime: &BotRuntimeConfig) -> ToolConfig {
    let mut tools = runtime.tools.clone();
    tools.upload_directory = instance_media_root(&tools.upload_directory, &runtime.instance_id);
    tools
}

fn instance_media_root(base: &Path, instance_id: &str) -> PathBuf {
    // BotRuntimeConfig restricts instance IDs to a single safe path segment.
    base.join(instance_id)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::instance_media_root;

    #[test]
    fn media_root_is_private_to_the_process_instance() {
        assert_eq!(
            instance_media_root(Path::new("/managed/uploads"), "shards-0-4"),
            Path::new("/managed/uploads/shards-0-4")
        );
    }
}
