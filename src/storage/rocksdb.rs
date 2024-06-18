use std::{path::Path, sync::Arc};

use crate::{KvError, Kvpair, Storage, StorageIter, Value};
use rocksdb::{BoundColumnFamily, Options, DB};

pub struct RocksDB(DB);

impl RocksDB {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self(DB::open_default(path).unwrap())
    }

    pub fn get_or_create_table(&self, name: &str) -> Arc<BoundColumnFamily> {
        if self.0.cf_handle(name).is_none() {
            let _ = self.0.create_cf(name, &Options::default());
        }
        self.0.cf_handle(name).unwrap()
    }
}

impl Storage for RocksDB {
    fn get(&self, table: &str, key: &str) -> Result<Option<Value>, KvError> {
        let cf = self.get_or_create_table(table);
        let result = self.0.get_cf(&cf, key)?.map(|v| v.as_slice().try_into());
        result.transpose()
    }

    fn set(
        &self,
        table: &str,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> Result<Option<Value>, KvError> {
        let cf = self.get_or_create_table(table);
        let key = key.into();
        let value: Vec<u8> = Into::<Value>::into(value).try_into()?;
        let old = self.get(table, &key);
        let _ = self.0.put_cf(&cf, key, value);
        old
    }

    fn contains(&self, table: &str, key: &str) -> Result<bool, KvError> {
        let cf = self.get_or_create_table(table);
        Ok(self.0.key_may_exist_cf(&cf, key))
    }

    fn del(&self, table: &str, key: &str) -> Result<Option<Value>, KvError> {
        let cf = self.get_or_create_table(table);
        let old = self.get(table, key);
        self.0.delete_cf(&cf, key)?;
        old
    }

    fn get_all(&self, table: &str) -> Result<Vec<Kvpair>, KvError> {
        let cf = self.get_or_create_table(table);
        Ok(self
            .0
            .iterator_cf(&cf, rocksdb::IteratorMode::Start)
            .map(|v| v.unwrap().into())
            .collect())
    }

    fn get_iter(&self, table: &str) -> Result<impl Iterator<Item = Kvpair>, KvError> {
        let cf = self.get_or_create_table(table);
        let iter = self.0.iterator_cf(&cf, rocksdb::IteratorMode::Start);
        let iter = StorageIter::new(iter.map(|v| Into::<Kvpair>::into(v.unwrap())));
        Ok(iter)
    }
}
