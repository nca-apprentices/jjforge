//! Converts each jj object to its `store.v1` message and back. An object's ID
//! is the hash of its bytes, so the same object always encodes the same way.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use jj_lib::backend::ChangeId;
use jj_lib::backend::Commit;
use jj_lib::backend::CommitId;
use jj_lib::backend::CopyHistory;
use jj_lib::backend::CopyId;
use jj_lib::backend::FileId;
use jj_lib::backend::MillisSinceEpoch;
use jj_lib::backend::SecureSig;
use jj_lib::backend::Signature;
use jj_lib::backend::SymlinkId;
use jj_lib::backend::Timestamp;
use jj_lib::backend::Tree;
use jj_lib::backend::TreeId;
use jj_lib::backend::TreeValue;
use jj_lib::conflict_labels::ConflictLabels;
use jj_lib::merge::Merge;
use jj_lib::object_id::ObjectId;
use jj_lib::op_store::Operation;
use jj_lib::op_store::OperationId;
use jj_lib::op_store::OperationMetadata;
use jj_lib::op_store::RefTarget;
use jj_lib::op_store::RemoteRef;
use jj_lib::op_store::RemoteRefState;
use jj_lib::op_store::RemoteView;
use jj_lib::op_store::TimestampRange;
use jj_lib::op_store::View;
use jj_lib::op_store::ViewId;
use jj_lib::ref_name::RefNameBuf;
use jj_lib::ref_name::RemoteNameBuf;
use jj_lib::ref_name::WorkspaceNameBuf;
use jj_lib::repo_path::InvalidNewRepoPathError;
use jj_lib::repo_path::RepoPathBuf;
use jj_lib::repo_path::RepoPathComponentBuf;
use jjforge_proto::store::v1 as proto;
use jjforge_proto::store::v1::stored_object::Kind;
use jjforge_proto::store::v1::tree_entry::Value;
use prost::Message;

/// An object of a repository, as jj-lib models it.
#[derive(Debug, PartialEq, Eq)]
pub enum Object {
    File(Vec<u8>),
    Symlink(String),
    Tree(Tree),
    CopyHistory(CopyHistory),
    Commit(Box<Commit>),
    Operation(Box<Operation>),
    View(Box<View>),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the bytes are not a store.v1 object")]
    Decode(#[from] prost::DecodeError),
    #[error("the {0} is missing")]
    Missing(&'static str),
    #[error("the path is not valid")]
    Path(#[from] InvalidNewRepoPathError),
    #[error("a merge needs exactly one more add than removes, and one label per term")]
    Merge,
    #[error("the tree entries are not sorted by name, or a name repeats")]
    Unsorted,
    #[error("{0:?} is not a commit ID in hexadecimal")]
    CommitId(String),
    #[error("store.v1 has no {0}")]
    Unsupported(&'static str),
    #[error("the bytes are not how store.v1 encodes the object")]
    NotCanonical,
}

/// Encodes an object as a `store.v1.StoredObject`.
///
/// # Errors
///
/// Fails on the git parts of jj, which `store.v1` leaves out, as ADR 0001
/// decides: a git submodule, git refs, and git heads.
pub fn encode(object: &Object) -> Result<Vec<u8>, Error> {
    Ok(stored_object(object)?.encode_to_vec())
}

fn stored_object(object: &Object) -> Result<proto::StoredObject, Error> {
    let kind = match object {
        Object::File(content) => Kind::File(proto::File {
            content: content.clone(),
        }),
        Object::Symlink(target) => Kind::Symlink(proto::Symlink {
            target: target.clone(),
        }),
        Object::Tree(tree) => Kind::Tree(tree_to_proto(tree)?),
        Object::CopyHistory(history) => Kind::CopyHistory(copy_history_to_proto(history)),
        Object::Commit(commit) => Kind::Commit(commit_to_proto(commit)),
        Object::Operation(operation) => Kind::Operation(operation_to_proto(operation)),
        Object::View(view) => Kind::View(view_to_proto(view)?),
    };
    Ok(proto::StoredObject { kind: Some(kind) })
}

/// Decodes a `store.v1.StoredObject`.
///
/// # Errors
///
/// Fails when the bytes aren't a `StoredObject`, when it breaks a rule of
/// jj-lib, such as a tree whose entries aren't sorted, or when `encode` would
/// give the object other bytes, and so another ID.
pub fn decode(bytes: &[u8]) -> Result<Object, Error> {
    let message = proto::StoredObject::decode(bytes)?;
    // The store sets an operation's segments, which jj-lib never sees, so
    // the check carries them over.
    let segment_ids = match &message.kind {
        Some(Kind::Operation(operation)) => operation.segment_ids.clone(),
        _ => vec![],
    };

    let object = object_from_proto(message)?;

    let mut canonical = stored_object(&object)?;
    if let Some(Kind::Operation(operation)) = &mut canonical.kind {
        operation.segment_ids = segment_ids;
    }
    if canonical.encode_to_vec() != bytes {
        return Err(Error::NotCanonical);
    }

    Ok(object)
}

fn object_from_proto(message: proto::StoredObject) -> Result<Object, Error> {
    let kind = message.kind.ok_or(Error::Missing("object kind"))?;
    Ok(match kind {
        Kind::File(file) => Object::File(file.content),
        Kind::Symlink(symlink) => Object::Symlink(symlink.target),
        Kind::Tree(tree) => Object::Tree(tree_from_proto(tree)?),
        Kind::CopyHistory(history) => Object::CopyHistory(copy_history_from_proto(history)?),
        Kind::Commit(commit) => Object::Commit(Box::new(commit_from_proto(commit)?)),
        Kind::Operation(operation) => Object::Operation(Box::new(operation_from_proto(operation)?)),
        Kind::View(view) => Object::View(Box::new(view_from_proto(view)?)),
    })
}

/// Splits a merge into its adds and its removes.
fn terms<T>(merge: &Merge<T>, to_bytes: impl Fn(&T) -> Vec<u8>) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    (
        merge.adds().map(&to_bytes).collect(),
        merge.removes().map(&to_bytes).collect(),
    )
}

/// Joins adds and removes into a merge, which jj-lib would panic on if they
/// didn't match.
fn merge<T>(
    adds: Vec<Vec<u8>>,
    removes: Vec<Vec<u8>>,
    from_bytes: impl Fn(Vec<u8>) -> T,
) -> Result<Merge<T>, Error> {
    if adds.len() != removes.len() + 1 {
        return Err(Error::Merge);
    }

    Ok(Merge::from_removes_adds(
        removes.into_iter().map(&from_bytes),
        adds.into_iter().map(&from_bytes),
    ))
}

/// An absent term of a ref target is stored as empty bytes.
fn ref_target_to_proto(target: &RefTarget) -> proto::RefTarget {
    let (adds, removes) = terms(target, |id| {
        id.as_ref().map(ObjectId::to_bytes).unwrap_or_default()
    });
    proto::RefTarget { adds, removes }
}

fn ref_target_from_proto(message: proto::RefTarget) -> Result<RefTarget, Error> {
    merge(message.adds, message.removes, |bytes| {
        (!bytes.is_empty()).then(|| CommitId::new(bytes))
    })
}

fn signature_to_proto(signature: &Signature) -> proto::Signature {
    proto::Signature {
        name: signature.name.clone(),
        email: signature.email.clone(),
        timestamp_millis: signature.timestamp.timestamp.0,
        tz_offset_minutes: signature.timestamp.tz_offset,
    }
}

fn signature_from_proto(
    message: Option<proto::Signature>,
    field: &'static str,
) -> Result<Signature, Error> {
    let message = message.ok_or(Error::Missing(field))?;
    Ok(Signature {
        name: message.name,
        email: message.email,
        timestamp: Timestamp {
            timestamp: MillisSinceEpoch(message.timestamp_millis),
            tz_offset: message.tz_offset_minutes,
        },
    })
}

/// Leaves out `predecessors`, which jj deprecates. The operation's
/// `commit_predecessors` records the same.
fn commit_to_proto(commit: &Commit) -> proto::Commit {
    let (adds, removes) = terms(&commit.root_tree, ObjectId::to_bytes);
    let conflict_labels = if commit.conflict_labels.is_resolved() {
        vec![]
    } else {
        commit.conflict_labels.iter().cloned().collect()
    };

    proto::Commit {
        change_id: commit.change_id.to_bytes(),
        parent_ids: commit.parents.iter().map(ObjectId::to_bytes).collect(),
        root_tree: Some(proto::TreeMerge { adds, removes }),
        author: Some(signature_to_proto(&commit.author)),
        committer: Some(signature_to_proto(&commit.committer)),
        description: commit.description.clone(),
        signature: commit
            .secure_sig
            .as_ref()
            .map(|secure_sig| secure_sig.sig.clone())
            .unwrap_or_default(),
        conflict_labels,
    }
}

fn commit_from_proto(mut message: proto::Commit) -> Result<Commit, Error> {
    // The signed data is the commit encoded without its signature.
    let sig = std::mem::take(&mut message.signature);
    let secure_sig = (!sig.is_empty()).then(|| SecureSig {
        data: message.encode_to_vec(),
        sig,
    });

    let root_tree = message.root_tree.ok_or(Error::Missing("root tree"))?;
    let root_tree = merge(root_tree.adds, root_tree.removes, TreeId::new)?;
    let labels = message.conflict_labels;
    if !labels.is_empty() && labels.len() != root_tree.iter().len() {
        return Err(Error::Merge);
    }

    Ok(Commit {
        parents: message.parent_ids.into_iter().map(CommitId::new).collect(),
        predecessors: vec![],
        root_tree,
        conflict_labels: ConflictLabels::from_vec(labels).into_merge(),
        change_id: ChangeId::new(message.change_id),
        description: message.description,
        author: signature_from_proto(message.author, "author")?,
        committer: signature_from_proto(message.committer, "committer")?,
        secure_sig,
    })
}

fn tree_to_proto(tree: &Tree) -> Result<proto::Tree, Error> {
    let entries = tree
        .entries()
        .map(|entry| {
            let value = match entry.value() {
                TreeValue::File {
                    id,
                    executable,
                    copy_id,
                } => Value::File(proto::FileRef {
                    id: id.to_bytes(),
                    executable: *executable,
                    copy_id: copy_id.to_bytes(),
                }),
                TreeValue::Symlink(id) => Value::SymlinkId(id.to_bytes()),
                TreeValue::Tree(id) => Value::TreeId(id.to_bytes()),
                TreeValue::GitSubmodule(_) => return Err(Error::Unsupported("git submodule")),
            };
            Ok(proto::TreeEntry {
                name: entry.name().as_internal_str().to_owned(),
                value: Some(value),
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(proto::Tree { entries })
}

fn tree_from_proto(message: proto::Tree) -> Result<Tree, Error> {
    let entries: Vec<_> = message
        .entries
        .into_iter()
        .map(|entry| {
            let value = match entry.value.ok_or(Error::Missing("tree entry value"))? {
                Value::File(file) => TreeValue::File {
                    id: FileId::new(file.id),
                    executable: file.executable,
                    copy_id: CopyId::new(file.copy_id),
                },
                Value::TreeId(id) => TreeValue::Tree(TreeId::new(id)),
                Value::SymlinkId(id) => TreeValue::Symlink(SymlinkId::new(id)),
            };
            Ok((RepoPathComponentBuf::new(entry.name)?, value))
        })
        .collect::<Result<_, Error>>()?;

    if !entries.is_sorted_by(|(a, _), (b, _)| a < b) {
        return Err(Error::Unsorted);
    }

    Ok(Tree::from_sorted_entries(entries))
}

fn copy_history_to_proto(history: &CopyHistory) -> proto::CopyHistory {
    proto::CopyHistory {
        current_path: history.current_path.as_internal_file_string().to_owned(),
        parent_ids: history.parents.iter().map(ObjectId::to_bytes).collect(),
        salt: history.salt.clone(),
    }
}

fn copy_history_from_proto(message: proto::CopyHistory) -> Result<CopyHistory, Error> {
    Ok(CopyHistory {
        current_path: RepoPathBuf::from_internal_string(message.current_path)?,
        parents: message.parent_ids.into_iter().map(CopyId::new).collect(),
        salt: message.salt,
    })
}

/// Leaves `segment_ids` empty, since the store sets them, not jj-lib.
fn operation_to_proto(operation: &Operation) -> proto::Operation {
    let metadata = &operation.metadata;
    let commit_predecessors = operation
        .commit_predecessors
        .iter()
        .flatten()
        .map(|(id, predecessors)| {
            let ids = predecessors.iter().map(ObjectId::to_bytes).collect();
            (id.hex(), proto::CommitIds { ids })
        })
        .collect();

    proto::Operation {
        parent_ids: operation.parents.iter().map(ObjectId::to_bytes).collect(),
        view_id: operation.view_id.to_bytes(),
        start_time_millis: metadata.time.start.timestamp.0,
        end_time_millis: metadata.time.end.timestamp.0,
        description: metadata.description.clone(),
        attributes: metadata.attributes.clone(),
        commit_predecessors,
        segment_ids: vec![],
        hostname: metadata.hostname.clone(),
        username: metadata.username.clone(),
        is_snapshot: metadata.is_snapshot,
        workspace_name: metadata
            .workspace_name
            .as_ref()
            .map(|name| name.as_str().to_owned()),
        start_tz_offset_minutes: metadata.time.start.tz_offset,
        end_tz_offset_minutes: metadata.time.end.tz_offset,
        stores_commit_predecessors: operation.commit_predecessors.is_some(),
    }
}

fn operation_from_proto(message: proto::Operation) -> Result<Operation, Error> {
    let commit_predecessors = message
        .stores_commit_predecessors
        .then(|| {
            message
                .commit_predecessors
                .into_iter()
                .map(|(hex, predecessors)| {
                    let id = CommitId::try_from_hex(&hex).ok_or(Error::CommitId(hex))?;
                    Ok((
                        id,
                        predecessors.ids.into_iter().map(CommitId::new).collect(),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, Error>>()
        })
        .transpose()?;

    let time = TimestampRange {
        start: Timestamp {
            timestamp: MillisSinceEpoch(message.start_time_millis),
            tz_offset: message.start_tz_offset_minutes,
        },
        end: Timestamp {
            timestamp: MillisSinceEpoch(message.end_time_millis),
            tz_offset: message.end_tz_offset_minutes,
        },
    };

    Ok(Operation {
        view_id: ViewId::new(message.view_id),
        parents: message
            .parent_ids
            .into_iter()
            .map(OperationId::new)
            .collect(),
        metadata: OperationMetadata {
            time,
            description: message.description,
            hostname: message.hostname,
            username: message.username,
            is_snapshot: message.is_snapshot,
            workspace_name: message.workspace_name.map(WorkspaceNameBuf::from),
            attributes: message.attributes,
        },
        commit_predecessors,
    })
}

fn view_to_proto(view: &View) -> Result<proto::View, Error> {
    if !view.git_refs.is_empty() {
        return Err(Error::Unsupported("git refs"));
    }
    if !view.git_heads.is_empty() {
        return Err(Error::Unsupported("git heads"));
    }

    // jj keeps the heads in a hash set, whose order differs from run to run.
    let mut head_ids: Vec<_> = view.head_ids.iter().map(ObjectId::to_bytes).collect();
    head_ids.sort();

    Ok(proto::View {
        head_ids,
        bookmarks: ref_targets_to_proto(&view.local_bookmarks),
        tags: ref_targets_to_proto(&view.local_tags),
        remote_views: view
            .remote_views
            .iter()
            .map(|(name, remote)| (name.as_str().to_owned(), remote_view_to_proto(remote)))
            .collect(),
        working_copy_commit_ids: view
            .wc_commit_ids
            .iter()
            .map(|(name, id)| (name.as_str().to_owned(), id.to_bytes()))
            .collect(),
    })
}

fn view_from_proto(message: proto::View) -> Result<View, Error> {
    let remote_views = message
        .remote_views
        .into_iter()
        .map(|(name, remote)| Ok((RemoteNameBuf::from(name), remote_view_from_proto(remote)?)))
        .collect::<Result<_, Error>>()?;

    Ok(View {
        head_ids: message.head_ids.into_iter().map(CommitId::new).collect(),
        local_bookmarks: ref_targets_from_proto(message.bookmarks)?,
        local_tags: ref_targets_from_proto(message.tags)?,
        remote_views,
        git_refs: BTreeMap::new(),
        git_heads: BTreeMap::new(),
        wc_commit_ids: message
            .working_copy_commit_ids
            .into_iter()
            .map(|(name, id)| (WorkspaceNameBuf::from(name), CommitId::new(id)))
            .collect(),
    })
}

fn ref_targets_to_proto(
    refs: &BTreeMap<RefNameBuf, RefTarget>,
) -> BTreeMap<String, proto::RefTarget> {
    refs.iter()
        .map(|(name, target)| (name.as_str().to_owned(), ref_target_to_proto(target)))
        .collect()
}

fn ref_targets_from_proto(
    refs: BTreeMap<String, proto::RefTarget>,
) -> Result<BTreeMap<RefNameBuf, RefTarget>, Error> {
    refs.into_iter()
        .map(|(name, target)| Ok((RefNameBuf::from(name), ref_target_from_proto(target)?)))
        .collect()
}

fn remote_view_to_proto(remote: &RemoteView) -> proto::RemoteView {
    proto::RemoteView {
        bookmarks: remote_refs_to_proto(&remote.bookmarks),
        tags: remote_refs_to_proto(&remote.tags),
        tracked_bookmarks: tracked(&remote.bookmarks),
        tracked_tags: tracked(&remote.tags),
    }
}

fn remote_view_from_proto(message: proto::RemoteView) -> Result<RemoteView, Error> {
    Ok(RemoteView {
        bookmarks: remote_refs_from_proto(message.bookmarks, message.tracked_bookmarks)?,
        tags: remote_refs_from_proto(message.tags, message.tracked_tags)?,
    })
}

fn remote_refs_to_proto(
    refs: &BTreeMap<RefNameBuf, RemoteRef>,
) -> BTreeMap<String, proto::RefTarget> {
    refs.iter()
        .map(|(name, remote_ref)| {
            (
                name.as_str().to_owned(),
                ref_target_to_proto(&remote_ref.target),
            )
        })
        .collect()
}

fn remote_refs_from_proto(
    refs: BTreeMap<String, proto::RefTarget>,
    tracked: Vec<String>,
) -> Result<BTreeMap<RefNameBuf, RemoteRef>, Error> {
    let tracked: BTreeSet<_> = tracked.into_iter().collect();
    refs.into_iter()
        .map(|(name, target)| {
            let state = if tracked.contains(&name) {
                RemoteRefState::Tracked
            } else {
                RemoteRefState::New
            };
            let target = ref_target_from_proto(target)?;
            Ok((RefNameBuf::from(name), RemoteRef { target, state }))
        })
        .collect()
}

/// The names of the tracked refs, sorted, since the map is.
fn tracked(refs: &BTreeMap<RefNameBuf, RemoteRef>) -> Vec<String> {
    refs.iter()
        .filter(|(_, remote_ref)| remote_ref.is_tracked())
        .map(|(name, _)| name.as_str().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn commit_id(n: u8) -> CommitId {
        CommitId::new(vec![n; 32])
    }

    fn tree_id(n: u8) -> TreeId {
        TreeId::new(vec![n; 32])
    }

    fn signature() -> Signature {
        Signature {
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
            timestamp: Timestamp {
                timestamp: MillisSinceEpoch(1_760_000_000_000),
                tz_offset: 120,
            },
        }
    }

    fn commit() -> Commit {
        Commit {
            parents: vec![commit_id(1), commit_id(2)],
            predecessors: vec![],
            root_tree: Merge::resolved(tree_id(3)),
            conflict_labels: Merge::resolved(String::new()),
            change_id: ChangeId::new(vec![4; 16]),
            description: "Add the store\n".to_owned(),
            author: signature(),
            committer: signature(),
            secure_sig: None,
        }
    }

    fn view() -> View {
        let remote = RemoteView {
            bookmarks: BTreeMap::from([
                (
                    "main".into(),
                    RemoteRef {
                        target: RefTarget::normal(commit_id(1)),
                        state: RemoteRefState::Tracked,
                    },
                ),
                (
                    "wip".into(),
                    RemoteRef {
                        target: RefTarget::normal(commit_id(2)),
                        state: RemoteRefState::New,
                    },
                ),
            ]),
            tags: BTreeMap::from([(
                "v1".into(),
                RemoteRef {
                    target: RefTarget::normal(commit_id(1)),
                    state: RemoteRefState::Tracked,
                },
            )]),
        };

        View {
            head_ids: HashSet::from([commit_id(1), commit_id(2), commit_id(3)]),
            local_bookmarks: BTreeMap::from([
                ("main".into(), RefTarget::normal(commit_id(1))),
                // Conflicted, with an absent term.
                (
                    "feature".into(),
                    RefTarget::from_vec(vec![Some(commit_id(2)), None, Some(commit_id(3))]),
                ),
            ]),
            local_tags: BTreeMap::from([("v1".into(), RefTarget::normal(commit_id(1)))]),
            remote_views: BTreeMap::from([("origin".into(), remote)]),
            git_refs: BTreeMap::new(),
            git_heads: BTreeMap::new(),
            wc_commit_ids: BTreeMap::from([("default".into(), commit_id(3))]),
        }
    }

    fn round_trip(object: &Object) -> Object {
        decode(&encode(object).unwrap()).unwrap()
    }

    #[test]
    fn file_and_symlink_return_equal() {
        for object in [
            Object::File(b"hello\n".to_vec()),
            Object::Symlink("../a".to_owned()),
        ] {
            assert_eq!(round_trip(&object), object);
        }
    }

    #[test]
    fn tree_returns_equal() {
        let file = TreeValue::File {
            id: FileId::new(vec![1; 32]),
            executable: true,
            copy_id: CopyId::new(vec![2; 32]),
        };
        let entries = vec![
            (RepoPathComponentBuf::new("README.md").unwrap(), file),
            (
                RepoPathComponentBuf::new("link").unwrap(),
                TreeValue::Symlink(SymlinkId::new(vec![3; 32])),
            ),
            (
                RepoPathComponentBuf::new("src").unwrap(),
                TreeValue::Tree(tree_id(4)),
            ),
        ];
        let object = Object::Tree(Tree::from_sorted_entries(entries));

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn tree_with_git_submodule_fails() {
        let entries = vec![(
            RepoPathComponentBuf::new("vendor").unwrap(),
            TreeValue::GitSubmodule(commit_id(1)),
        )];

        let result = encode(&Object::Tree(Tree::from_sorted_entries(entries)));

        assert!(matches!(result, Err(Error::Unsupported("git submodule"))));
    }

    #[test]
    fn unsorted_tree_fails_to_decode() {
        let entry = |name: &str| proto::TreeEntry {
            name: name.to_owned(),
            value: Some(Value::TreeId(vec![1; 32])),
        };
        let tree = proto::Tree {
            entries: vec![entry("b"), entry("a")],
        };
        let bytes = proto::StoredObject {
            kind: Some(Kind::Tree(tree)),
        }
        .encode_to_vec();

        assert!(matches!(decode(&bytes), Err(Error::Unsorted)));
    }

    #[test]
    fn copy_history_returns_equal() {
        let object = Object::CopyHistory(CopyHistory {
            current_path: RepoPathBuf::from_internal_string("src/lib.rs").unwrap(),
            parents: vec![CopyId::new(vec![1; 32]), CopyId::new(vec![2; 32])],
            salt: vec![9; 8],
        });

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn commit_returns_equal() {
        let object = Object::Commit(Box::new(commit()));

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn conflicted_commit_returns_equal() {
        let mut commit = commit();
        commit.root_tree = Merge::from_vec(vec![tree_id(1), tree_id(2), tree_id(3)]);
        commit.conflict_labels = Merge::from_vec(vec![
            "left".to_owned(),
            "base".to_owned(),
            "right".to_owned(),
        ]);
        let object = Object::Commit(Box::new(commit));

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn signed_commit_returns_equal() {
        let mut commit = commit();
        commit.secure_sig = Some(SecureSig {
            data: commit_to_proto(&commit).encode_to_vec(),
            sig: b"signature".to_vec(),
        });
        let object = Object::Commit(Box::new(commit));

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn commit_drops_predecessors() {
        let mut commit = commit();
        commit.predecessors = vec![commit_id(9)];

        let Object::Commit(decoded) = round_trip(&Object::Commit(Box::new(commit))) else {
            panic!("not a commit");
        };

        assert_eq!(decoded.predecessors, vec![]);
    }

    #[test]
    fn root_operation_returns_equal() {
        let object = Object::Operation(Box::new(Operation::make_root(ViewId::new(vec![1; 32]))));

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn operation_returns_equal() {
        let at = |millis, tz_offset| Timestamp {
            timestamp: MillisSinceEpoch(millis),
            tz_offset,
        };
        let operation = Operation {
            view_id: ViewId::new(vec![1; 32]),
            parents: vec![OperationId::new(vec![2; 32]), OperationId::new(vec![3; 32])],
            metadata: OperationMetadata {
                time: TimestampRange {
                    start: at(1_760_000_000_000, 120),
                    end: at(1_760_000_001_000, -300),
                },
                description: "rebase".to_owned(),
                hostname: "laptop".to_owned(),
                username: "ada".to_owned(),
                is_snapshot: true,
                workspace_name: Some("default".into()),
                attributes: BTreeMap::from([("jjforge.principal".to_owned(), "ada".to_owned())]),
            },
            commit_predecessors: Some(BTreeMap::from([
                (commit_id(4), vec![commit_id(5)]),
                (commit_id(6), vec![]),
            ])),
        };
        let object = Object::Operation(Box::new(operation));

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn operation_without_predecessors_returns_equal() {
        let mut operation = Operation::make_root(ViewId::new(vec![1; 32]));
        operation.commit_predecessors = None;
        let object = Object::Operation(Box::new(operation));

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn view_returns_equal() {
        let object = Object::View(Box::new(view()));

        assert_eq!(round_trip(&object), object);
    }

    #[test]
    fn view_encodes_its_heads_in_one_order() {
        let encodings: HashSet<_> = (0..16)
            .map(|_| encode(&Object::View(Box::new(view()))).unwrap())
            .collect();

        assert_eq!(encodings.len(), 1);
    }

    #[test]
    fn view_with_git_refs_fails() {
        let mut view = view();
        view.git_refs
            .insert("refs/heads/main".into(), RefTarget::normal(commit_id(1)));

        let result = encode(&Object::View(Box::new(view)));

        assert!(matches!(result, Err(Error::Unsupported("git refs"))));
    }

    fn stored(kind: Kind) -> Vec<u8> {
        proto::StoredObject { kind: Some(kind) }.encode_to_vec()
    }

    #[test]
    fn view_with_unsorted_heads_fails_to_decode() {
        let view = proto::View {
            head_ids: vec![vec![2; 32], vec![1; 32], vec![1; 32]],
            ..Default::default()
        };

        assert!(matches!(
            decode(&stored(Kind::View(view))),
            Err(Error::NotCanonical)
        ));
    }

    #[test]
    fn tracked_name_without_ref_fails_to_decode() {
        let remote = proto::RemoteView {
            tracked_bookmarks: vec!["ghost".to_owned()],
            ..Default::default()
        };
        let view = proto::View {
            head_ids: vec![vec![1; 32]],
            remote_views: BTreeMap::from([("origin".to_owned(), remote)]),
            ..Default::default()
        };

        assert!(matches!(
            decode(&stored(Kind::View(view))),
            Err(Error::NotCanonical)
        ));
    }

    #[test]
    fn operation_with_segments_decodes() {
        let operation = Operation::make_root(ViewId::new(vec![1; 32]));
        let mut message = operation_to_proto(&operation);
        message.segment_ids = vec![vec![7; 32]];

        let decoded = decode(&stored(Kind::Operation(message))).unwrap();

        assert_eq!(decoded, Object::Operation(Box::new(operation)));
    }
}
