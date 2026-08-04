use super::PublishedSnapshot;

pub trait PublishedSnapshotReconciler: Send + Sync {
    fn reconcile(&self, snapshot: &PublishedSnapshot);
}
