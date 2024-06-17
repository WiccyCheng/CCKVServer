use crate::{KvError, Kvpair, Storage, StorageIter, Value};
use dashmap::{mapref::one::Ref, DashMap};

/// 使用 DashMap 构建的 MemTable，实现了 Storage trait
#[derive(Clone, Debug, Default)]
pub struct MemTable {
    tables: DashMap<String, DashMap<String, Value>>,
}

impl MemTable {
    // 创建一个缺省的MemTable
    pub fn new() -> Self {
        Self::default()
    }

    // 如果名为 name 的 hash table不存在，则创建，否则返回
    fn get_or_create_table(&self, name: &str) -> Ref<String, DashMap<String, Value>> {
        match self.tables.get(name) {
            Some(table) => table,
            None => {
                // entry()提供比insert()更多的灵活性，它返回一个枚举值Entry,该枚举代表了键在 DashMap 中的位置。
                // 通过 Entry，可以决定如何处理该键的值，例如插入新值、更新已有值或执行其他操作。
                let entry = self.tables.entry(name.to_string()).or_default();
                // downgrade() 方法将一个 Entry 转换为一个 Ref, 允许只读访问
                entry.downgrade()
            }
        }
    }
}

impl Storage for MemTable {
    fn get(&self, table: &str, key: &str) -> Result<Option<Value>, KvError> {
        let table = self.get_or_create_table(table);
        Ok(table.get(key).map(|v| v.value().clone()))
    }

    fn set(
        &self,
        table: &str,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> Result<Option<Value>, KvError> {
        let table = self.get_or_create_table(table);
        Ok(table.insert(key.into(), value.into()))
    }

    fn contains(&self, table: &str, key: &str) -> Result<bool, KvError> {
        let table = self.get_or_create_table(table);
        Ok(table.contains_key(key))
    }

    fn del(&self, table: &str, key: &str) -> Result<Option<Value>, KvError> {
        let table = self.get_or_create_table(table);
        Ok(table.remove(key).map(|(_k, v)| v))
    }

    fn get_all(&self, table: &str) -> Result<Vec<Kvpair>, KvError> {
        let table = self.get_or_create_table(table);
        Ok(table
            .iter()
            .map(|p| Kvpair::new(p.key(), p.value().clone()))
            .collect())
    }

    fn get_iter(&self, table: &str) -> Result<impl Iterator<Item = Kvpair>, KvError> {
        let table = self.get_or_create_table(table).clone();
        Ok(StorageIter::new(table.into_iter()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_or_create_table_should_work() {
        let store = MemTable::new();
        assert!(!store.tables.contains_key("table"));
        store.get_or_create_table("table");
        assert!(store.tables.contains_key("table"));
    }
}
