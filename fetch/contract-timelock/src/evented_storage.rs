//! Storage wrappers that emit an event on every mutation.
//!
//! Declare storage with the attribute macros instead of a bare `Map`/`Item`:
//!
//! ```ignore
//! #[evented_map(key = "id", value = "message")]
//! const MESSAGE_QUEUE: Map<u64, BridgeMessage> = Map::new("message_queue");
//!
//! #[evented_item(value = "nonce")]
//! const GLOBAL_NONCE: Item<u64> = Item::new("nonce");
//! ```
//!
//! The constants become [`EventedMap`]/[`EventedItem`], which keep the underlying
//! `Map`/`Item` private, so the only mutations are `update` (emits `<name>_updated`)
//! and `remove` (emits `<name>_removed` if the entry existed). The storage namespace
//! and serialization are exactly those of the wrapped `cw-storage-plus` type.
//!
//! Event attribute values are rendered with [`to_event_value`]: JSON, with JSON strings
//! unquoted, so numbers, strings, `Addr` and `Uint128` look like their `to_string()` and
//! structs/enums are JSON objects.

use cosmwasm_std::{from_json, to_json_string, Order, StdResult, Storage};
use cw_fetch_std::events::{IntoEvent, ResponseHandler};
use cw_storage_plus::{Bound, Item, KeyDeserialize, Map, PrimaryKey};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub use evented_storage_macros::{evented_item, evented_map};

type Attributes = Vec<(String, String)>;

struct StorageEvent {
    name: &'static str,
    attributes: Attributes,
}

impl IntoEvent for StorageEvent {
    fn event_name(&self) -> &str {
        self.name
    }

    fn event_attributes(&self) -> Attributes {
        self.attributes.clone()
    }
}

/// Renders a value as an event attribute value without panicking.
pub fn to_event_value<T: Serialize + ?Sized>(value: &T) -> StdResult<String> {
    let json = to_json_string(value)?;
    if json.starts_with('"') {
        from_json(json.as_bytes())
    } else {
        Ok(json)
    }
}

/// Converts a map key into event attributes, one per key component.
/// Any serializable key is a single attribute; tuple (composite) keys can also be
/// split into one attribute per element by giving one name per element.
pub trait EventKey<const N: usize> {
    fn push_attributes(
        &self,
        names: &[&'static str; N],
        attributes: &mut Attributes,
    ) -> StdResult<()>;
}

impl<T: Serialize> EventKey<1> for T {
    fn push_attributes(
        &self,
        [name]: &[&'static str; 1],
        attributes: &mut Attributes,
    ) -> StdResult<()> {
        attributes.push((name.to_string(), to_event_value(self)?));
        Ok(())
    }
}

macro_rules! impl_composite_event_key {
    ($arity:literal: $($element:ident $index:tt),+) => {
        impl<$($element: Serialize),+> EventKey<$arity> for ($($element,)+) {
            fn push_attributes(&self, names: &[&'static str; $arity], attributes: &mut Attributes) -> StdResult<()> {
                $(attributes.push((names[$index].to_string(), to_event_value(&self.$index)?));)+
                Ok(())
            }
        }
    };
}

impl_composite_event_key!(2: A 0, B 1);
impl_composite_event_key!(3: A 0, B 1, C 2);

/// A `Map<K, V>` whose mutations emit events. Declare it with [`evented_map`].
pub struct EventedMap<K, V, const N: usize> {
    map: Map<K, V>,
    updated_event: &'static str,
    removed_event: &'static str,
    key_names: [&'static str; N],
    value_name: Option<&'static str>,
}

impl<K, V, const N: usize> EventedMap<K, V, N> {
    pub const fn new(
        map: Map<K, V>,
        updated_event: &'static str,
        removed_event: &'static str,
        key_names: [&'static str; N],
        value_name: Option<&'static str>,
    ) -> Self {
        Self {
            map,
            updated_event,
            removed_event,
            key_names,
            value_name,
        }
    }
}

impl<'a, K, V, const N: usize> EventedMap<K, V, N>
where
    K: PrimaryKey<'a>,
    V: Serialize + DeserializeOwned,
{
    /// Saves `value` under `key` and emits the updated event.
    /// On error, neither the storage nor the response is modified.
    pub fn update<C>(
        &self,
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler<C>,
        key: K,
        value: &V,
    ) -> StdResult<()>
    where
        K: EventKey<N>,
    {
        // Attributes are built before saving, so an attribute serialization failure
        // leaves storage untouched.
        let mut attributes = Vec::with_capacity(N + 1);
        key.push_attributes(&self.key_names, &mut attributes)?;
        if let Some(value_name) = self.value_name {
            attributes.push((value_name.to_string(), to_event_value(value)?));
        }

        self.map.save(storage, key, value)?;
        response_handler.add_event(StorageEvent {
            name: self.updated_event,
            attributes,
        });
        Ok(())
    }

    /// Removes the entry under `key`. Emits the removed event and returns `true` only
    /// if the entry existed.
    pub fn remove<C>(
        &self,
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler<C>,
        key: K,
    ) -> StdResult<bool>
    where
        K: EventKey<N>,
    {
        let mut attributes = Vec::with_capacity(N);
        key.push_attributes(&self.key_names, &mut attributes)?;

        let path = self.map.key(key);
        if !path.has(storage) {
            return Ok(false);
        }
        path.remove(storage);
        response_handler.add_event(StorageEvent {
            name: self.removed_event,
            attributes,
        });
        Ok(true)
    }

    pub fn load(&self, storage: &dyn Storage, key: K) -> StdResult<V> {
        self.map.load(storage, key)
    }

    pub fn may_load(&self, storage: &dyn Storage, key: K) -> StdResult<Option<V>> {
        self.map.may_load(storage, key)
    }

    pub fn has(&self, storage: &dyn Storage, key: K) -> bool {
        self.map.has(storage, key)
    }
}

impl<'a, K, V, const N: usize> EventedMap<K, V, N>
where
    K: PrimaryKey<'a> + KeyDeserialize,
    V: Serialize + DeserializeOwned,
{
    pub fn range<'c>(
        &self,
        storage: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, V)>> + 'c>
    where
        V: 'c,
        K::Output: 'static,
    {
        self.map.range(storage, min, max, order)
    }

    pub fn keys<'c>(
        &self,
        storage: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<K::Output>> + 'c>
    where
        V: 'c,
        K::Output: 'static,
    {
        self.map.keys(storage, min, max, order)
    }
}

/// An `Item<V>` whose mutations emit events. Declare it with [`evented_item`].
pub struct EventedItem<V> {
    item: Item<V>,
    updated_event: &'static str,
    removed_event: &'static str,
    value_name: &'static str,
}

impl<V> EventedItem<V> {
    pub const fn new(
        item: Item<V>,
        updated_event: &'static str,
        removed_event: &'static str,
        value_name: &'static str,
    ) -> Self {
        Self {
            item,
            updated_event,
            removed_event,
            value_name,
        }
    }
}

impl<V> EventedItem<V>
where
    V: Serialize + DeserializeOwned,
{
    /// Saves `value` and emits the updated event.
    /// On error, neither the storage nor the response is modified.
    pub fn update<C>(
        &self,
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler<C>,
        value: &V,
    ) -> StdResult<()> {
        let attributes = vec![(self.value_name.to_string(), to_event_value(value)?)];

        self.item.save(storage, value)?;
        response_handler.add_event(StorageEvent {
            name: self.updated_event,
            attributes,
        });
        Ok(())
    }

    /// Removes the item. Emits the removed event and returns `true` only if it existed.
    pub fn remove<C>(
        &self,
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler<C>,
    ) -> bool {
        if !self.item.exists(storage) {
            return false;
        }
        self.item.remove(storage);
        response_handler.add_event(StorageEvent {
            name: self.removed_event,
            attributes: vec![],
        });
        true
    }

    pub fn load(&self, storage: &dyn Storage) -> StdResult<V> {
        self.item.load(storage)
    }

    pub fn may_load(&self, storage: &dyn Storage) -> StdResult<Option<V>> {
        self.item.may_load(storage)
    }

    pub fn exists(&self, storage: &dyn Storage) -> bool {
        self.item.exists(storage)
    }
}

/// Flattens the events of a response into `(name, attributes)` for assertions.
#[cfg(test)]
pub(crate) fn events(response_handler: ResponseHandler) -> Vec<(String, Attributes)> {
    response_handler
        .into_response()
        .events
        .into_iter()
        .map(|event| {
            let attributes = event
                .attributes
                .into_iter()
                .map(|attribute| (attribute.key, attribute.value))
                .collect();
            (event.ty, attributes)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_schema::cw_serde;
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{Addr, Uint128};
    use serde::ser::{Error as _, SerializeStruct};
    use serde::{Deserialize, Serializer};
    use std::cell::Cell;

    #[cw_serde]
    enum Kind {
        Simple,
        Nested { depth: u8 },
    }

    #[cw_serde]
    struct Complex {
        owner: Addr,
        amount: Uint128,
        kind: Kind,
    }

    /// Fails its `fail_on_call`-th serialization (1 = event attribute, 2 = storage).
    #[derive(Deserialize)]
    struct Flaky {
        tag: u32,
        #[serde(skip)]
        fail_on_call: u32,
        #[serde(skip)]
        calls: Cell<u32>,
    }

    impl Flaky {
        fn new(tag: u32, fail_on_call: u32) -> Self {
            Self {
                tag,
                fail_on_call,
                calls: Cell::new(0),
            }
        }
    }

    impl Serialize for Flaky {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.calls.set(self.calls.get() + 1);
            if self.calls.get() == self.fail_on_call {
                return Err(S::Error::custom("serialization failed"));
            }
            let mut state = serializer.serialize_struct("Flaky", 1)?;
            state.serialize_field("tag", &self.tag)?;
            state.end()
        }
    }

    #[evented_map(key = "id", value = "data")]
    const TEST_MAP: Map<u64, Complex> = Map::new("test_map");

    #[evented_map(key = ("role", "address"))]
    const ROLES: Map<(&str, &Addr), ()> = Map::new("roles");

    #[evented_map(key = "id", value = "value")]
    const FLAKY_MAP: Map<u64, Flaky> = Map::new("flaky_map");

    #[evented_item(value = "config")]
    const TEST_ITEM: Item<Complex> = Item::new("test_item");

    #[evented_item(value = "count", updated = "count_set", removed = "count_cleared")]
    const RENAMED_ITEM: Item<u32> = Item::new("renamed_item");

    #[evented_item(value = "value")]
    const FLAKY_ITEM: Item<Flaky> = Item::new("flaky_item");

    fn complex(amount: u128) -> Complex {
        Complex {
            owner: Addr::unchecked("fetch1owner"),
            amount: Uint128::new(amount),
            kind: Kind::Nested { depth: 2 },
        }
    }

    fn event(name: &str, attributes: &[(&str, &str)]) -> (String, Attributes) {
        let attributes = attributes
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        (name.to_string(), attributes)
    }

    #[test]
    fn map_update_overwrite_and_remove() {
        let mut deps = mock_dependencies();
        let storage = deps.as_mut().storage;
        let mut response_handler = ResponseHandler::default();

        TEST_MAP
            .update(storage, &mut response_handler, 1, &complex(10))
            .unwrap();
        assert_eq!(TEST_MAP.load(storage, 1).unwrap(), complex(10));

        TEST_MAP
            .update(storage, &mut response_handler, 1, &complex(20))
            .unwrap();
        assert_eq!(TEST_MAP.load(storage, 1).unwrap(), complex(20));

        assert!(TEST_MAP.remove(storage, &mut response_handler, 1).unwrap());
        assert!(!TEST_MAP.has(storage, 1));
        assert_eq!(TEST_MAP.may_load(storage, 1).unwrap(), None);

        let first = to_json_string(&complex(10)).unwrap();
        let second = to_json_string(&complex(20)).unwrap();
        assert_eq!(
            events(response_handler),
            vec![
                event("test_map_updated", &[("id", "1"), ("data", &first)]),
                event("test_map_updated", &[("id", "1"), ("data", &second)]),
                event("test_map_removed", &[("id", "1")]),
            ]
        );
    }

    #[test]
    fn map_remove_nonexistent_is_silent() {
        let mut deps = mock_dependencies();
        let storage = deps.as_mut().storage;
        TEST_MAP
            .update(storage, &mut ResponseHandler::default(), 1, &complex(10))
            .unwrap();

        let mut response_handler = ResponseHandler::default();
        assert!(!TEST_MAP.remove(storage, &mut response_handler, 2).unwrap());

        assert!(events(response_handler).is_empty());
        assert_eq!(TEST_MAP.load(storage, 1).unwrap(), complex(10));
    }

    #[test]
    fn map_read_methods_do_not_emit_events() {
        let mut deps = mock_dependencies();
        let storage = deps.as_mut().storage;
        for id in [3, 1, 2] {
            TEST_MAP
                .update(
                    storage,
                    &mut ResponseHandler::default(),
                    id,
                    &complex(id.into()),
                )
                .unwrap();
        }

        let entries: Vec<_> = TEST_MAP
            .range(
                storage,
                Some(Bound::inclusive(2u64)),
                None,
                Order::Ascending,
            )
            .collect::<StdResult<_>>()
            .unwrap();
        assert_eq!(entries, vec![(2, complex(2)), (3, complex(3))]);

        let keys: Vec<_> = TEST_MAP
            .keys(storage, None, None, Order::Descending)
            .collect::<StdResult<_>>()
            .unwrap();
        assert_eq!(keys, vec![3, 2, 1]);
        assert!(TEST_MAP.load(storage, 4).is_err());
    }

    #[test]
    fn composite_key_attributes() {
        let mut deps = mock_dependencies();
        let storage = deps.as_mut().storage;
        let mut response_handler = ResponseHandler::default();
        let address = Addr::unchecked("fetch1member");

        ROLES
            .update(storage, &mut response_handler, ("admin", &address), &())
            .unwrap();
        assert!(ROLES.has(storage, ("admin", &address)));
        let keys: Vec<_> = ROLES
            .keys(storage, None, None, Order::Ascending)
            .collect::<StdResult<_>>()
            .unwrap();
        assert_eq!(keys, vec![("admin".to_string(), address.clone())]);

        assert!(ROLES
            .remove(storage, &mut response_handler, ("admin", &address))
            .unwrap());
        assert!(!ROLES
            .remove(storage, &mut response_handler, ("admin", &address))
            .unwrap());

        assert_eq!(
            events(response_handler),
            vec![
                event(
                    "roles_updated",
                    &[("role", "admin"), ("address", "fetch1member")]
                ),
                event(
                    "roles_removed",
                    &[("role", "admin"), ("address", "fetch1member")]
                ),
            ]
        );
    }

    #[test]
    fn item_update_and_remove() {
        let mut deps = mock_dependencies();
        let storage = deps.as_mut().storage;
        let mut response_handler = ResponseHandler::default();

        assert!(!TEST_ITEM.exists(storage));
        TEST_ITEM
            .update(storage, &mut response_handler, &complex(1))
            .unwrap();
        assert_eq!(TEST_ITEM.load(storage).unwrap(), complex(1));

        assert!(TEST_ITEM.remove(storage, &mut response_handler));
        assert_eq!(TEST_ITEM.may_load(storage).unwrap(), None);
        assert!(!TEST_ITEM.remove(storage, &mut response_handler));

        let json = to_json_string(&complex(1)).unwrap();
        assert_eq!(
            events(response_handler),
            vec![
                event("test_item_updated", &[("config", &json)]),
                event("test_item_removed", &[]),
            ]
        );
    }

    #[test]
    fn event_name_overrides() {
        let mut deps = mock_dependencies();
        let storage = deps.as_mut().storage;
        let mut response_handler = ResponseHandler::default();

        RENAMED_ITEM
            .update(storage, &mut response_handler, &5)
            .unwrap();
        RENAMED_ITEM.remove(storage, &mut response_handler);

        assert_eq!(
            events(response_handler),
            vec![
                event("count_set", &[("count", "5")]),
                event("count_cleared", &[])
            ]
        );
    }

    #[test]
    fn event_value_rendering() {
        assert_eq!(to_event_value(&42u64).unwrap(), "42");
        assert_eq!(to_event_value(&-3i32).unwrap(), "-3");
        assert_eq!(to_event_value(&true).unwrap(), "true");
        assert_eq!(to_event_value("say \"hi\"\n").unwrap(), "say \"hi\"\n");
        assert_eq!(
            to_event_value(&Addr::unchecked("fetch1abc")).unwrap(),
            "fetch1abc"
        );
        assert_eq!(to_event_value(&Uint128::new(123)).unwrap(), "123");
        assert_eq!(to_event_value(&Kind::Simple).unwrap(), "simple");
        assert_eq!(to_event_value(&None::<u64>).unwrap(), "null");
        assert_eq!(
            to_event_value(&complex(5)).unwrap(),
            r#"{"owner":"fetch1owner","amount":"5","kind":{"nested":{"depth":2}}}"#
        );
    }

    #[test]
    fn map_failures_leave_storage_and_events_untouched() {
        let mut deps = mock_dependencies();
        let storage = deps.as_mut().storage;
        FLAKY_MAP
            .update(
                storage,
                &mut ResponseHandler::default(),
                1,
                &Flaky::new(1, 0),
            )
            .unwrap();

        // Failing while building the event, then while saving to storage.
        for fail_on_call in [1, 2] {
            let mut response_handler = ResponseHandler::default();
            for id in [1, 2] {
                let value = Flaky::new(2, fail_on_call);
                assert!(FLAKY_MAP
                    .update(storage, &mut response_handler, id, &value)
                    .is_err());
            }
            assert!(events(response_handler).is_empty());
            assert_eq!(FLAKY_MAP.load(storage, 1).unwrap().tag, 1);
            assert!(!FLAKY_MAP.has(storage, 2));
        }
    }

    #[test]
    fn item_failures_leave_storage_and_events_untouched() {
        let mut deps = mock_dependencies();
        let storage = deps.as_mut().storage;

        for fail_on_call in [1, 2] {
            let mut response_handler = ResponseHandler::default();
            assert!(FLAKY_ITEM
                .update(storage, &mut response_handler, &Flaky::new(2, fail_on_call))
                .is_err());
            assert!(events(response_handler).is_empty());
            assert!(!FLAKY_ITEM.exists(storage));
        }

        FLAKY_ITEM
            .update(storage, &mut ResponseHandler::default(), &Flaky::new(1, 0))
            .unwrap();
        for fail_on_call in [1, 2] {
            let mut response_handler = ResponseHandler::default();
            assert!(FLAKY_ITEM
                .update(storage, &mut response_handler, &Flaky::new(2, fail_on_call))
                .is_err());
            assert!(events(response_handler).is_empty());
            assert_eq!(FLAKY_ITEM.load(storage).unwrap().tag, 1);
        }
    }
}
