//! Build the small in-memory command list synchronously for each opening.
use nfm_picker_sources::structured::StructuredStreamingSnapshot;
use nfm_search_core::snapshot_store::SnapshotStore;
use std::sync::Arc;

pub(crate) type CommandSource = SnapshotStore<StructuredStreamingSnapshot>;

pub(crate) fn source(rows: Vec<Vec<String>>) -> Result<Arc<CommandSource>, String> {
    nfm_picker_sources::structured::structured_source(
        ["Name", "Type", "KeyBinding", "Description"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        rows,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfm_search_core::store::ItemsSource;
    #[test]
    fn sources_are_ready_immediately_and_old_snapshots_remain_unchanged() {
        let rows = vec![vec![
            "first".into(),
            "Function".into(),
            "Ctrl+A".into(),
            "Original description".into(),
        ]];
        let first = source(rows.clone()).unwrap();
        assert!(first.is_done());
        let mut changed = rows;
        changed[0][0] = "second".into();
        changed[0][3] = "Updated description".into();
        let second = source(changed).unwrap();
        assert!(second.is_done());
        assert_eq!(
            first.snapshot().unwrap().item(0).unwrap().fields["Name"],
            "first"
        );
        assert_eq!(
            second.snapshot().unwrap().item(0).unwrap().fields["Description"],
            "Updated description"
        );
    }
    #[test]
    fn malformed_rows_are_rejected_and_empty_lists_are_ready() {
        assert!(source(vec![vec!["invalid".into()]]).is_err());
        let empty = source(Vec::new()).unwrap();
        assert!(empty.is_done());
        assert_eq!(empty.snapshot().unwrap().len(), 0);
    }
}
