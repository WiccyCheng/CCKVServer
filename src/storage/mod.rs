mod memory;
mod rocksdb;
mod sleddb;

pub use memory::MemTable;
pub use rocksdb::RocksDB;
pub use sleddb::SledDb;

use crate::{KvError, Kvpair, Value};

/// 对存储的抽象，我们不关心数据存在哪儿，但需要定义外界如何和存储打交道
pub trait Storage {
    /// 从一个 HashTable 里获取一个 key 的 value
    fn get(&self, table: &str, key: &str) -> Result<Option<Value>, KvError>;
    /// 从一个 HashTable 里设置一个 key 的 value，返回旧的 value
    fn set(
        &self,
        table: &str,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> Result<Option<Value>, KvError>;
    /// 查看 HashTable 中是否有 key
    fn contains(&self, table: &str, key: &str) -> Result<bool, KvError>;
    /// 从 HashTable 中删除一个 key
    fn del(&self, table: &str, key: &str) -> Result<Option<Value>, KvError>;
    /// 遍历 HashTable，返回所有 kv pair（这个接口不好）
    fn get_all(&self, table: &str) -> Result<Vec<Kvpair>, KvError>;
    /// 遍历 HashTable，返回 kv pair 的 Iterator
    fn get_iter(&self, table: &str) -> Result<impl Iterator<Item = Kvpair>, KvError>;
}

//提供 Storage Iterator, 这样trait的实现者只需要把他们的Iterator, 提供给 StorageIter, 并且保证next()传出的类型实现了Into<Kvpair>
pub struct StorageIter<T> {
    data: T,
}

impl<T> StorageIter<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T> Iterator for StorageIter<T>
where
    T: Iterator,
    T::Item: Into<Kvpair>,
{
    type Item = Kvpair;

    fn next(&mut self) -> Option<Self::Item> {
        self.data.next().map(|v| v.into())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn memetable_basic_interface_should_work() {
        let store = MemTable::new();
        test_basi_interface(store);
    }

    #[test]
    fn memtable_iter_should_work() {
        let store = MemTable::new();
        test_get_iter(store);
    }

    #[test]
    fn memetable_get_all_should_work() {
        let store = MemTable::new();
        test_get_all(store);
    }

    #[test]
    fn selddb_basic_interface_should_work() {
        let dir = tempdir().unwrap();
        let store = SledDb::new(dir);
        test_basi_interface(store);
    }

    #[test]
    fn selddb_iter_should_work() {
        let dir = tempdir().unwrap();
        let store = SledDb::new(dir);
        test_get_iter(store);
    }

    #[test]
    fn selddb_get_all_should_work() {
        let dir = tempdir().unwrap();
        let store = SledDb::new(dir);
        test_get_all(store);
    }

    #[test]
    fn rocksdb_basic_interface_should_work() {
        let dir = tempdir().unwrap();
        let store = RocksDB::new(dir);
        test_basi_interface(store);
    }

    #[test]
    fn rocksdb_iter_should_work() {
        let dir = tempdir().unwrap();
        let store = RocksDB::new(dir);
        test_get_iter(store);
    }

    #[test]
    fn rocksdb_get_all_should_work() {
        let dir = tempdir().unwrap();
        let store = RocksDB::new(dir);
        test_get_all(store);
    }

    fn test_basi_interface(store: impl Storage) {
        // 第一次set会创建table，插入key并返回None（之前没值）
        let v = store.set("table", "key", "value");
        assert!(v.unwrap().is_none());
        // 再次set会同样的key会更新，并返回之前的值
        let v1 = store.set("table", "key", "value1");
        assert_eq!(v1.unwrap(), Some("value".into()));

        //get存在的key会得到最新的值
        let v = store.get("table", "key");
        assert_eq!(v.unwrap(), Some("value1".into()));

        //get 不存在的key或者table会返回None
        assert_eq!(None, store.get("table", "not exist key").unwrap());
        assert!(store.get("not exist table", "key").unwrap().is_none());

        //contains 存在的key返回true，否则返回None
        assert_eq!(true, store.contains("table", "key").unwrap());
        assert_eq!(false, store.contains("table", "not exist key").unwrap());
        assert_eq!(false, store.contains("not exist table", "key").unwrap());

        // del 存在的key返回之前的值
        let v = store.del("table", "key");
        assert_eq!(v.unwrap(), Some("value1".into()));

        // del 不存在的 key 或者table返回None
        assert_eq!(None, store.del("table", "not exist key").unwrap());
        assert_eq!(None, store.del("not exist table", "key").unwrap());
    }

    fn test_get_all(store: impl Storage) {
        store.set("table", "key1", "1").unwrap();
        store.set("table", "key2", "2").unwrap();
        let mut data = store.get_all("table").unwrap();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            data,
            vec![Kvpair::new("key1", "1"), Kvpair::new("key2", "2")]
        );
    }

    fn test_get_iter(store: impl Storage) {
        store.set("table", "key1", "1").unwrap();
        store.set("table", "key2", "2").unwrap();
        let mut data: Vec<_> = store.get_iter("table").unwrap().collect();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            data,
            vec![Kvpair::new("key1", "1"), Kvpair::new("key2", "2")]
        );
    }
}
