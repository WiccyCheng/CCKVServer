use crate::*;

impl CommandService for Hget {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        match store.get(&self.table, &self.key) {
            Ok(Some(v)) => v.into(),
            Ok(None) => KvError::NotFound(self.table, self.key).into(),
            Err(e) => e.into(),
        }
    }
}

impl CommandService for Hmget {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        self.keys
            .iter()
            .map(|key| match store.get(&self.table, key) {
                Ok(Some(v)) => v,
                _ => Value::default(),
            })
            .collect::<Vec<_>>()
            .into()
    }
}

impl CommandService for Hgetall {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        match store.get_all(&self.table) {
            Ok(v) => v.into(),
            Err(e) => e.into(),
        }
    }
}

impl CommandService for Hset {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        match self.pair {
            Some(v) => match store.set(&self.table, v.key, v.value.unwrap_or_default()) {
                Ok(Some(v)) => v.into(),
                Ok(None) => Value::default().into(),
                Err(e) => e.into(),
            },
            None => Value::default().into(),
        }
    }
}

impl CommandService for Hmset {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        let pairs = self.pairs;
        let table = self.table;
        pairs
            .into_iter()
            .map(
                |pair| match store.set(&table, pair.key, pair.value.unwrap_or_default()) {
                    Ok(Some(v)) => v,
                    _ => Value::default(),
                },
            )
            .collect::<Vec<_>>()
            .into()
    }
}

impl CommandService for Hdel {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        match store.del(&self.table, &self.key) {
            Ok(Some(v)) => v.into(),
            Ok(None) => Value::default().into(),
            Err(e) => e.into(),
        }
    }
}

impl CommandService for Hmdel {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        self.keys
            .iter()
            .map(|key| match store.del(&self.table, key) {
                Ok(Some(v)) => v,
                _ => Value::default(),
            })
            .collect::<Vec<_>>()
            .into()
    }
}

impl CommandService for Hexist {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        match store.contains(&self.table, &self.key) {
            Ok(v) => Value::from(v).into(),
            Err(e) => e.into(),
        }
    }
}

impl CommandService for Hmexist {
    fn execute(self, store: &impl Storage) -> CommandResponse {
        self.keys
            .iter()
            .map(|key| match store.contains(&self.table, key) {
                Ok(v) => v.into(),
                _ => Value::default(),
            })
            .collect::<Vec<_>>()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_request::RequestData;

    #[test]
    fn hset_should_work() {
        let store = MemTable::new();
        let cmd = CommandRequest::new_hset("table", "hello", "world");
        let res = dispatch(cmd.clone(), &store);
        // 第一次插入返回之前的值为空
        assert_res_ok(res, &[Value::default()], &[]);

        let res = dispatch(cmd, &store);
        // 再次插入返回之前的值
        assert_res_ok(res, &["world".into()], &[]);
    }

    #[test]
    fn hget_should_work() {
        let store = MemTable::new();
        let cmd = CommandRequest::new_hset("score", "u1", 10);
        dispatch(cmd, &store);
        let cmd = CommandRequest::new_hget("score", "u1");
        let res = dispatch(cmd, &store);
        assert_res_ok(res, &[10.into()], &[]);
    }

    #[test]
    fn hdel_should_work() {
        let store = MemTable::new();
        let cmd = CommandRequest::new_hset("table", "key", 10);
        dispatch(cmd, &store);

        let cmd = CommandRequest::new_hdel("table", "key");
        let res = dispatch(cmd, &store);
        assert_res_ok(res, &[10.into()], &[]);

        let cmd = CommandRequest::new_hexist("table", "key");
        let res = dispatch(cmd, &store);
        assert_res_ok(res, &[false.into()], &[]);
    }

    #[test]
    fn hexist_should_work() {
        let store = MemTable::new();
        // 不存在的 key 应返回 false
        let cmd = CommandRequest::new_hexist("table", "key");
        let res = dispatch(cmd, &store);
        assert_res_ok(res, &[false.into()], &[]);

        // 存在的 key 返回 true
        let cmd = CommandRequest::new_hset("table", "key", 10);
        dispatch(cmd, &store);
        let cmd = CommandRequest::new_hexist("table", "key");
        let res = dispatch(cmd, &store);
        assert_res_ok(res, &[true.into()], &[]);
    }

    #[test]
    fn hget_with_non_exist_should_return_404() {
        let store = MemTable::new();
        let cmd = CommandRequest::new_hget("non exist table", "non exist key");
        let res = dispatch(cmd, &store);
        assert_res_error(res, 404, "Not found");
    }

    #[test]
    fn hgetall_should_work() {
        let store = MemTable::new();
        let cmds = vec![
            CommandRequest::new_hset("score", "u1", 10),
            CommandRequest::new_hset("score", "u2", 9),
            CommandRequest::new_hset("score", "u3", 23),
            CommandRequest::new_hset("score", "u1", 5),
        ];
        for cmd in cmds {
            dispatch(cmd, &store);
        }

        let cmd = CommandRequest::new_hgetall("score");
        let res = dispatch(cmd, &store);
        let pairs = &[
            Kvpair::new("u1", 5),
            Kvpair::new("u2", 9),
            Kvpair::new("u3", 23),
        ];
        assert_res_ok(res, &[], pairs);
    }

    #[test]
    fn hmget_should_work() {
        let store = MemTable::new();
        let cmds = vec![
            CommandRequest::new_hset("table", "key1", 1),
            CommandRequest::new_hset("table", "key2", 2),
            CommandRequest::new_hset("table", "key3", 3),
            CommandRequest::new_hset("table", "key4", 4),
        ];
        for cmd in cmds {
            dispatch(cmd, &store);
        }

        let cmd = CommandRequest::new_hmget(
            "table",
            vec!["key1", "key2", "not exist key", "key3", "key4"],
        );
        let res = dispatch(cmd, &store);
        assert_res_ok(
            res,
            &[
                1.into(),
                2.into(),
                pb::abi::Value { value: None },
                3.into(),
                4.into(),
            ],
            &[],
        );
    }

    #[test]
    fn hmset_should_work() {
        let store = MemTable::new();
        let pairs = &[
            Kvpair::new("key1", 1),
            Kvpair::new("key2", 2),
            Kvpair::new("key3", 3),
            Kvpair::new("key4", 4),
        ];

        let cmd = CommandRequest::new_hmset("table", pairs.into());
        dispatch(cmd, &store);

        let cmd = CommandRequest::new_hgetall("table");
        let res = dispatch(cmd, &store);
        assert_res_ok(res, &[], pairs);
    }

    #[test]
    fn hmdel_should_work() {
        let store = MemTable::new();
        let cmds = vec![
            CommandRequest::new_hset("table", "key1", 1),
            CommandRequest::new_hset("table", "key2", 2),
            CommandRequest::new_hset("table", "key3", 3),
            CommandRequest::new_hset("table", "key4", 4),
        ];
        for cmd in cmds {
            dispatch(cmd, &store);
        }

        let cmd = CommandRequest::new_hmdel("table", vec!["key1", "key2", "key3", "key4"]);
        let res = dispatch(cmd, &store);
        assert_res_ok(res, &[1.into(), 2.into(), 3.into(), 4.into()], &[]);
    }

    #[test]
    fn hmexist_should_work() {
        let store = MemTable::new();
        let cmds = vec![
            CommandRequest::new_hset("table", "key1", 1),
            CommandRequest::new_hset("table", "key3", 3),
        ];
        for cmd in cmds {
            dispatch(cmd, &store);
        }

        let cmd = CommandRequest::new_hmexist("table", vec!["key1", "key2", "key3", "key4"]);
        let res = dispatch(cmd, &store);
        assert_res_ok(
            res,
            &[true.into(), false.into(), true.into(), false.into()],
            &[],
        );
    }

    // 从 Request 中获得 Responese 目前只处理 HGET/HSET/HGETALL
    fn dispatch(cmd: CommandRequest, store: &impl Storage) -> CommandResponse {
        match cmd.request_data.unwrap() {
            RequestData::Hget(v) => v.execute(store),
            RequestData::Hset(v) => v.execute(store),
            RequestData::Hdel(v) => v.execute(store),
            RequestData::Hexist(v) => v.execute(store),
            RequestData::Hmget(v) => v.execute(store),
            RequestData::Hmset(v) => v.execute(store),
            RequestData::Hmdel(v) => v.execute(store),
            RequestData::Hmexist(v) => v.execute(store),
            RequestData::Hgetall(v) => v.execute(store),
        }
    }

    // 测试成功的返回结果
    fn assert_res_ok(mut res: CommandResponse, values: &[Value], pairs: &[Kvpair]) {
        res.pairs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(res.status, 200);
        assert_eq!(res.message, "");
        assert_eq!(res.values, values);
        assert_eq!(res.pairs, pairs);
    }

    // 测试失败的返回结果
    fn assert_res_error(res: CommandResponse, code: u32, msg: &str) {
        assert_eq!(res.status, code);
        assert!(res.message.contains(msg));
        assert_eq!(res.values, &[]);
        assert_eq!(res.pairs, &[]);
    }
}
