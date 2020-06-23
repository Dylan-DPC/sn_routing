// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::BTreeSet,
    fmt::{self, Debug, Formatter},
    iter::FromIterator,
};
use xor_name::{Prefix, XorName};

/// Container that acts as a map whose keys are prefixes.
///
/// It differs from a normal map of `Prefix` -> `T` in a couple of ways:
/// 1. It allows to keep the prefix and the value in the same type which makes it internally more
///    similar to a set of `(Prefix, T)` rather than map of `Prefix` -> `T` while still providing
///    convenient map-like API
/// 2. It automatically prunes redundant entries. That is, when the prefix of an entry is fully
///    covered by other prefixes, that entry is removed. For example, when there is entry with
///    prefix (00) and we insert entries with (000) and (001), the (00) prefix becomes fully
///    covered and is automatically removed.
/// 3. It provides some additional lookup API for convenience (`get_equal_or_ancestor`,
///    `get_matching`, ...)
///
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct PrefixMap<T>(BTreeSet<Entry<T>>)
where
    T: Borrow<Prefix<XorName>>;

impl<T> PrefixMap<T>
where
    T: Borrow<Prefix<XorName>>,
{
    /// Create empty `PrefixMap`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts new entry into the map. Replaces previous entry at the same prefix.
    /// Removes those ancestors of the inserted prefix that are now fully covered by their
    /// descendants.
    /// Does not insert anything if any descendant of the prefix of `entry` is already present in
    /// the map.
    /// Returns the previous entry with the same prefix, if any.
    pub fn insert(&mut self, entry: T) -> Option<T> {
        // Don't insert if any descendant is already present in the map.
        if self.descendants(entry.borrow()).next().is_some() {
            return Some(entry);
        }

        let parent_prefix = entry.borrow().popped();
        let old = self.0.replace(Entry(entry));
        self.prune(parent_prefix);
        old.map(|entry| entry.0)
    }

    /// Removes the entry at `prefix` and returns it, if any.
    pub fn remove(&mut self, prefix: &Prefix<XorName>) -> Option<T> {
        self.0.take(prefix).map(|entry| entry.0)
    }

    /// Get the entry at `prefix`, if any.
    pub fn get(&self, prefix: &Prefix<XorName>) -> Option<&T> {
        self.0.get(prefix).map(|entry| &entry.0)
    }

    /// Get the entry at `prefix` or any of its ancestors. In case of multiple matches, returns the
    /// one with the longest prefix.
    pub fn get_equal_or_ancestor(&self, prefix: &Prefix<XorName>) -> Option<&T> {
        let mut prefix = *prefix;
        loop {
            if let Some(entry) = self.get(&prefix) {
                return Some(entry);
            }

            if prefix.is_empty() {
                return None;
            }

            prefix = prefix.popped();
        }
    }

    /// Get the entry at the prefix that matches `name`. In case of multiple matches, returns the
    /// one with the longest prefix.
    pub fn get_matching(&self, name: &XorName) -> Option<&T> {
        self.0
            .iter()
            .filter(|entry| entry.prefix().matches(name))
            .max_by_key(|entry| entry.prefix().bit_count())
            .map(|entry| &entry.0)
    }

    /// Returns whether the map contain at least one entry whose prefix matches `name`.
    pub fn contains_matching(&self, name: &XorName) -> bool {
        self.0.iter().any(|entry| entry.0.borrow().matches(name))
    }

    /// Returns an iterator over the entries, in order by prefixes.
    pub fn iter(&self) -> impl Iterator<Item = &T> + Clone {
        self.0.iter().map(|entry| &entry.0)
    }

    /// Returns an iterator over the prefixes
    pub fn prefixes(&self) -> impl Iterator<Item = &Prefix<XorName>> + Clone {
        self.0.iter().map(|entry| entry.prefix())
    }

    /// Returns an iterator over all entries whose prefixes are descendants (extensions) of
    /// `prefix`.
    pub fn descendants<'a>(
        &'a self,
        prefix: &'a Prefix<XorName>,
    ) -> impl Iterator<Item = &'a T> + Clone + 'a {
        // TODO: there might be a way to do this in O(logn) using BTreeSet::range
        self.0
            .iter()
            .filter(move |entry| entry.0.borrow().is_extension_of(prefix))
            .map(|entry| &entry.0)
    }

    // Remove `prefix` and any of its ancestors if they are covered by their descendants.
    // For example, if `(00)` and `(01)` are both in the map, we can remove `(0)` and `()`.
    fn prune(&mut self, mut prefix: Prefix<XorName>) {
        // TODO: can this be optimized?

        loop {
            if prefix.is_covered_by(self.descendants(&prefix).map(|entry| entry.borrow())) {
                let _ = self.0.remove(&prefix);
            }

            if prefix.is_empty() {
                break;
            } else {
                prefix = prefix.popped();
            }
        }
    }
}

impl<T> Default for PrefixMap<T>
where
    T: Borrow<Prefix<XorName>>,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> Debug for PrefixMap<T>
where
    T: Borrow<Prefix<XorName>> + Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> FromIterator<T> for PrefixMap<T>
where
    T: Borrow<Prefix<XorName>>,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        iter.into_iter().fold(Self::new(), |mut map, entry| {
            let _ = map.insert(entry);
            map
        })
    }
}

impl<T> From<PrefixMap<T>> for BTreeSet<T>
where
    T: Borrow<Prefix<XorName>> + Ord,
{
    fn from(map: PrefixMap<T>) -> Self {
        map.0.into_iter().map(|entry| entry.0).collect()
    }
}

// Wrapper for entries of `PrefixMap` which implements Eq, Ord by delegating them to the prefix.
#[derive(Clone, Serialize, Deserialize)]
struct Entry<T>(T);

impl<T> Entry<T>
where
    T: Borrow<Prefix<XorName>>,
{
    fn prefix(&self) -> &Prefix<XorName> {
        self.0.borrow()
    }
}

impl<T> Borrow<Prefix<XorName>> for Entry<T>
where
    T: Borrow<Prefix<XorName>>,
{
    fn borrow(&self) -> &Prefix<XorName> {
        self.0.borrow()
    }
}

impl<T> PartialEq for Entry<T>
where
    T: Borrow<Prefix<XorName>>,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.borrow().eq(other.0.borrow())
    }
}

impl<T> Eq for Entry<T> where T: Borrow<Prefix<XorName>> {}

impl<T> Ord for Entry<T>
where
    T: Borrow<Prefix<XorName>>,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.borrow().cmp(other.0.borrow())
    }
}

impl<T> PartialOrd for Entry<T>
where
    T: Borrow<Prefix<XorName>>,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Debug> Debug for Entry<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng;
    use rand::Rng;
    use xor_name::Prefix;

    #[test]
    fn insert_existing_prefix() {
        let mut map = PrefixMap::new();
        assert_eq!(map.insert((prefix("0"), 1)), None);
        assert_eq!(map.insert((prefix("0"), 2)), Some((prefix("0"), 1)));
        assert_eq!(map.get(&prefix("0")), Some(&(prefix("0"), 2)));
    }

    #[test]
    fn insert_direct_descendants_of_existing_prefix() {
        let mut map = PrefixMap::new();
        assert_eq!(map.insert((prefix("0"), 0)), None);

        // Insert the first sibling. Parent remain in the map.
        assert_eq!(map.insert((prefix("00"), 1)), None);
        assert_eq!(map.get(&prefix("00")), Some(&(prefix("00"), 1)));
        assert_eq!(map.get(&prefix("01")), None);
        assert_eq!(map.get(&prefix("0")), Some(&(prefix("0"), 0)));

        // Insert the other sibling. Parent is removed because it is now fully covered by its
        // descendants.
        assert_eq!(map.insert((prefix("01"), 2)), None);
        assert_eq!(map.get(&prefix("00")), Some(&(prefix("00"), 1)));
        assert_eq!(map.get(&prefix("01")), Some(&(prefix("01"), 2)));
        assert_eq!(map.get(&prefix("0")), None);
    }

    #[test]
    fn insert_indirect_descendants_of_existing_prefix() {
        let mut map = PrefixMap::new();
        assert_eq!(map.insert((prefix("0"), 0)), None);

        assert_eq!(map.insert((prefix("000"), 1)), None);
        assert_eq!(map.get(&prefix("000")), Some(&(prefix("000"), 1)));
        assert_eq!(map.get(&prefix("001")), None);
        assert_eq!(map.get(&prefix("00")), None);
        assert_eq!(map.get(&prefix("01")), None);
        assert_eq!(map.get(&prefix("0")), Some(&(prefix("0"), 0)));

        assert_eq!(map.insert((prefix("001"), 2)), None);
        assert_eq!(map.get(&prefix("000")), Some(&(prefix("000"), 1)));
        assert_eq!(map.get(&prefix("001")), Some(&(prefix("001"), 2)));
        assert_eq!(map.get(&prefix("00")), None);
        assert_eq!(map.get(&prefix("01")), None);
        assert_eq!(map.get(&prefix("0")), Some(&(prefix("0"), 0)));

        assert_eq!(map.insert((prefix("01"), 3)), None);
        assert_eq!(map.get(&prefix("000")), Some(&(prefix("000"), 1)));
        assert_eq!(map.get(&prefix("001")), Some(&(prefix("001"), 2)));
        assert_eq!(map.get(&prefix("00")), None);
        assert_eq!(map.get(&prefix("01")), Some(&(prefix("01"), 3)));
        // (0) is now fully covered and so was removed
        assert_eq!(map.get(&prefix("0")), None);
    }

    #[test]
    fn insert_ancestor_of_existing_prefix() {
        let mut map = PrefixMap::new();
        let _ = map.insert((prefix("00"), 1));

        assert_eq!(map.insert((prefix("0"), 2)), Some((prefix("0"), 2)));
        assert_eq!(map.get(&prefix("0")), None);
        assert_eq!(map.get(&prefix("00")), Some(&(prefix("00"), 1)));
    }

    #[test]
    fn get_equal_or_ancestor() {
        let mut map = PrefixMap::new();
        let _ = map.insert((prefix("0"), 0));
        let _ = map.insert((prefix("10"), 1));

        assert_eq!(
            map.get_equal_or_ancestor(&prefix("0")),
            Some(&(prefix("0"), 0))
        );
        assert_eq!(
            map.get_equal_or_ancestor(&prefix("00")),
            Some(&(prefix("0"), 0))
        );
        assert_eq!(
            map.get_equal_or_ancestor(&prefix("01")),
            Some(&(prefix("0"), 0))
        );

        assert_eq!(map.get_equal_or_ancestor(&prefix("1")), None);
        assert_eq!(
            map.get_equal_or_ancestor(&prefix("10")),
            Some(&(prefix("10"), 1))
        );
        assert_eq!(
            map.get_equal_or_ancestor(&prefix("100")),
            Some(&(prefix("10"), 1))
        );
    }

    #[test]
    fn get_matching() {
        let mut rng = rng::new();

        let mut map = PrefixMap::new();
        let _ = map.insert((prefix("0"), 0));
        let _ = map.insert((prefix("1"), 1));
        let _ = map.insert((prefix("10"), 10));

        assert_eq!(
            map.get_matching(&prefix("0").substituted_in(rng.gen())),
            Some(&(prefix("0"), 0))
        );

        assert_eq!(
            map.get_matching(&prefix("11").substituted_in(rng.gen())),
            Some(&(prefix("1"), 1))
        );

        assert_eq!(
            map.get_matching(&prefix("10").substituted_in(rng.gen())),
            Some(&(prefix("10"), 10))
        );
    }

    fn prefix(s: &str) -> Prefix<XorName> {
        s.parse().unwrap()
    }
}
