//! The segments of ADR 0002. An upload becomes one immutable segment, the
//! encoded objects one after another, and an index that finds each object by
//! its hash with one range read:
//!
//! ```text
//! index = "JJFIDX01" entry*                 sorted by hash, no hash twice
//! entry = hash[32] offset[8] length[4] links[1]   big-endian
//! ```
//!
//! `links` is 1 for a commit or a tree, which names other objects, and 0 for
//! anything else, so a reader can check what a segment reaches without
//! reading its files. The segment's ID is the hash of its bytes, so the
//! index names it too.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::ids;
use crate::ids::ID_LENGTH;

const MAGIC: &[u8] = b"JJFIDX01";
const ENTRY_LENGTH: usize = ID_LENGTH + 8 + 4 + 1;

#[derive(Debug, thiserror::Error)]
#[error("the index of a segment is malformed")]
pub(crate) struct Malformed;

/// Where an object lives in a segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) id: Vec<u8>,
    pub(crate) range: Range<u64>,
    /// Whether the object names other objects.
    pub(crate) links: bool,
}

/// A segment and its index, ready to store.
pub(crate) struct Packed {
    pub(crate) id: Vec<u8>,
    pub(crate) segment: Vec<u8>,
    pub(crate) index: Vec<u8>,
}

/// Packs encoded objects, each named by its hash and marked whether it names
/// other objects, into one segment. An object that repeats is stored once.
///
/// # Errors
///
/// Fails when an object is larger than an entry can address.
pub(crate) fn pack(objects: Vec<(Vec<u8>, bool)>) -> Result<Packed, Malformed> {
    let by_id: BTreeMap<_, _> = objects
        .into_iter()
        .map(|(bytes, links)| (ids::hash(&bytes), (bytes, links)))
        .collect();

    let mut segment = vec![];
    let mut entries = vec![];
    for (id, (bytes, links)) in by_id {
        let start = segment.len() as u64;
        u32::try_from(bytes.len()).map_err(|_| Malformed)?;
        segment.extend_from_slice(&bytes);
        entries.push(Entry {
            id,
            range: start..segment.len() as u64,
            links,
        });
    }

    let mut index = MAGIC.to_vec();
    for entry in &entries {
        index.extend_from_slice(&entry.id);
        index.extend_from_slice(&entry.range.start.to_be_bytes());
        let length = u32::try_from(entry.range.end - entry.range.start).map_err(|_| Malformed)?;
        index.extend_from_slice(&length.to_be_bytes());
        index.push(u8::from(entry.links));
    }

    Ok(Packed {
        id: ids::hash(&segment),
        segment,
        index,
    })
}

/// Reads an index that `pack` wrote.
///
/// # Errors
///
/// Fails on bytes that `pack` wouldn't write.
pub(crate) fn parse(index: &[u8]) -> Result<Vec<Entry>, Malformed> {
    let body = index.strip_prefix(MAGIC).ok_or(Malformed)?;
    if body.len() % ENTRY_LENGTH != 0 {
        return Err(Malformed);
    }

    let entries: Vec<_> = body
        .as_chunks::<ENTRY_LENGTH>()
        .0
        .iter()
        .map(|chunk| {
            let (id, rest) = chunk.split_at(ID_LENGTH);
            let (offset, rest) = rest.split_at(8);
            let (length, links) = rest.split_at(4);
            let start = u64::from_be_bytes(offset.try_into().map_err(|_| Malformed)?);
            let length = u32::from_be_bytes(length.try_into().map_err(|_| Malformed)?);
            let links = match links {
                [0] => false,
                [1] => true,
                _ => return Err(Malformed),
            };
            Ok(Entry {
                id: id.to_vec(),
                range: start..start + u64::from(length),
                links,
            })
        })
        .collect::<Result<_, Malformed>>()?;

    if !entries.is_sorted_by(|a, b| a.id < b.id) {
        return Err(Malformed);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_finds_each_object() {
        let objects = vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()];

        let packed = pack(
            objects
                .iter()
                .map(|bytes| (bytes.clone(), bytes == b"two"))
                .collect(),
        )
        .unwrap();

        let entries = parse(&packed.index).unwrap();
        assert_eq!(entries.len(), 3);
        for object in objects {
            let entry = entries
                .iter()
                .find(|entry| entry.id == ids::hash(&object))
                .unwrap();
            let range = usize::try_from(entry.range.start).unwrap()
                ..usize::try_from(entry.range.end).unwrap();
            assert_eq!(packed.segment[range], object);
            assert_eq!(entry.links, object == b"two");
        }
        assert_eq!(packed.id, ids::hash(&packed.segment));
    }

    #[test]
    fn a_repeated_object_is_stored_once() {
        let packed = pack(vec![(b"same".to_vec(), false), (b"same".to_vec(), false)]).unwrap();

        assert_eq!(parse(&packed.index).unwrap().len(), 1);
        assert_eq!(packed.segment, b"same");
    }

    #[test]
    fn packing_is_deterministic() {
        let a = pack(vec![(b"a".to_vec(), false), (b"b".to_vec(), true)]).unwrap();
        let b = pack(vec![(b"b".to_vec(), true), (b"a".to_vec(), false)]).unwrap();

        assert_eq!(a.id, b.id);
        assert_eq!(a.index, b.index);
    }

    #[test]
    fn parse_refuses_a_foreign_or_cut_index() {
        let packed = pack(vec![(b"a".to_vec(), false)]).unwrap();

        assert!(parse(b"NOTANIDX").is_err());
        assert!(parse(&packed.index[..packed.index.len() - 1]).is_err());
    }

    #[test]
    fn parse_refuses_unsorted_entries() {
        let packed = pack(vec![(b"a".to_vec(), false), (b"b".to_vec(), false)]).unwrap();
        let mut swapped = MAGIC.to_vec();
        swapped.extend_from_slice(&packed.index[MAGIC.len() + ENTRY_LENGTH..]);
        swapped.extend_from_slice(&packed.index[MAGIC.len()..MAGIC.len() + ENTRY_LENGTH]);

        assert!(parse(&swapped).is_err());
    }
}
