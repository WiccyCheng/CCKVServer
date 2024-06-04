mod memory;

pub use memory::MemTable;

use crate::{KvError, Kvpair, Value};

/// 对存储的抽象，我们不关心数据存在哪儿，但需要定义外界如何和存储打交道
pub trait Storage {
    /// 从一个 HashTable 里获取一个 key 的 value
    fn get(&self, table: &str, key: &str) -> Result<Option<Value>, KvError>;
    /// 从一个 HashTable 里设置一个 key 的 value，返回旧的 value
    fn set(&self, table: &str, key: String, value: Value) -> Result<Option<Value>, KvError>;
    /// 查看 HashTable 中是否有 key
    fn contains(&self, table: &str, key: &str) -> Result<bool, KvError>;
    /// 从 HashTable 中删除一个 key
    fn del(&self, table: &str, key: &str) -> Result<Option<Value>, KvError>;
    /// 遍历 HashTable，返回所有 kv pair（这个接口不好）
    fn get_all(&self, table: &str) -> Result<Vec<Kvpair>, KvError>;
    /// 遍历 HashTable，返回 kv pair 的 Iterator
    fn get_iter(&self, table: &str) -> Result<Box<dyn Iterator<Item = Kvpair>>, KvError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memetable_basic_interface_should_work() {
        let store = MemTable::new();
        test_basi_interface(store);
    }

    #[test]
    fn memetable_get_all_should_work() {
        let store = MemTable::new();
        test_get_all(store);
    }

    fn test_basi_interface(store: impl Storage) {
        // 第一次set会创建table，插入key并返回None（之前没值）
        let v = store.set("table", "key".to_string(), "value".into());
        assert!(v.unwrap().is_none());
        // 再次set会同样的key会更新，并返回之前的值
        let v1 = store.set("table", "key".to_string(), "value1".into());
        assert_eq!(v1, Ok(Some("value".into())));

        //get存在的key会得到最新的值
        let v = store.get("table", "key");
        assert_eq!(v, Ok(Some("value1".into())));

        //get 不存在的key或者table会返回None
        assert_eq!(Ok(None), store.get("table", "not exist key"));
        assert!(store.get("not exist table", "key").unwrap().is_none());

        //contains 存在的key返回true，否则返回None
        assert_eq!(Ok(true), store.contains("table", "key"));
        assert_eq!(Ok(false), store.contains("table", "not exist key"));
        assert_eq!(Ok(false), store.contains("not exist table", "key"));

        // del 存在的key返回之前的值
        let v = store.del("table", "key");
        assert_eq!(v, Ok(Some("value1".into())));

        // del 不存在的 key 或者table返回None
        assert_eq!(Ok(None), store.del("table", "not exist key"));
        assert_eq!(Ok(None), store.del("not exist table", "key"));
    }

    fn test_get_all(store: impl Storage) {
        store.set("table", "key1".to_string(), "1".into()).unwrap();
        store.set("table", "key2".to_string(), "2".into()).unwrap();
        let mut data = store.get_all("table").unwrap();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            data,
            vec![
                Kvpair::new("key1", "1".into()),
                Kvpair::new("key2", "2".into())
            ]
        );
    }

    // fn test_get_iter(store: impl Storage) {
    //     store.set("table", "key1".to_string(), "1".into()).unwrap();
    //     store.set("table", "key2".to_string(), "2".into()).unwrap();
    //     let mut data: Vec<_> = store.get_iter("table").unwrap().collect();
    //     data.sort_by(|a, b| a.partial_cmp(b).unwrap());
    //     assert_eq!(
    //         data,
    //         vec![
    //             Kvpair::new("key1", "1".into()),
    //             Kvpair::new("key2", "2".into())
    //         ]
    //     );
    // }
}
