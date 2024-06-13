use crate::{
    command_request::RequestData, CommandRequest, CommandResponse, KvError, MemTable, Storage,
};
use std::sync::Arc;
use tracing::debug;

mod command_service;

/// 对command的处理的抽象
pub trait CommandService {
    // 处理 Command，返回 Response
    fn execute(self, store: &impl Storage) -> CommandResponse;
}

/// Service 数据结构
pub struct Service<Store = MemTable> {
    inner: Arc<ServiceInner<Store>>,
}

impl<Store> Clone for Service<Store> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Service 内部数据结构
pub struct ServiceInner<Store> {
    store: Store,
}

impl<Store: Storage> Service<Store> {
    pub fn new(store: Store) -> Self {
        Self {
            inner: Arc::new(ServiceInner { store }),
        }
    }

    pub fn execute(&self, cmd: CommandRequest) -> CommandResponse {
        debug!("Got request: {:?}", cmd);
        // TODO: 发送 on_received 事件
        let res = dispatch(cmd, &self.inner.store);
        debug!("Executed response: {:?}", res);
        // TODO: 发送 on_executed 事件

        res
    }
}

pub fn dispatch(cmd: CommandRequest, store: &impl Storage) -> CommandResponse {
    match cmd.request_data {
        Some(RequestData::Hget(param)) => param.execute(store),
        Some(RequestData::Hset(param)) => param.execute(store),
        Some(RequestData::Hdel(param)) => param.execute(store),
        Some(RequestData::Hexist(param)) => param.execute(store),
        Some(RequestData::Hmget(param)) => param.execute(store),
        Some(RequestData::Hmset(param)) => param.execute(store),
        Some(RequestData::Hmdel(param)) => param.execute(store),
        Some(RequestData::Hmexist(param)) => param.execute(store),
        Some(RequestData::Hgetall(param)) => param.execute(store),
        None => KvError::InvaildCommand("Request has no data".into()).into(),
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use crate::{MemTable, Value};

    #[test]
    fn service_should_work() {
        // service结构应至少包含Storage
        let service = Service::new(MemTable::new());

        // service 可以运行在多线程环境下，它的 clone 应该是轻量级的
        let cloned = service.clone();

        // 创建一个线程，在 table 中写入 key, value
        let handle = thread::spawn(move || {
            let res = cloned.execute(CommandRequest::new_hset("table", "key", "value".into()));
            assert_res_ok(res, &[Value::default()], &[]);
        });
        handle.join().unwrap();

        // 在当前线程下读取 table 的 key 返回 value
        let res = service.execute(CommandRequest::new_hget("table", "key"));
        assert_res_ok(res, &["value".into()], &[]);
    }
}

#[cfg(test)]
use crate::{Kvpair, Value};

// 测试成功的返回结果
#[cfg(test)]
fn assert_res_ok(mut res: CommandResponse, values: &[Value], pairs: &[Kvpair]) {
    res.pairs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(res.status, 200);
    assert_eq!(res.message, "");
    assert_eq!(res.values, values);
    assert_eq!(res.pairs, pairs);
}

// 测试失败的返回结果
#[cfg(test)]
fn assert_res_error(res: CommandResponse, code: u32, msg: &str) {
    assert_eq!(res.status, code);
    assert!(res.message.contains(msg));
    assert_eq!(res.values, &[]);
    assert_eq!(res.pairs, &[]);
}
