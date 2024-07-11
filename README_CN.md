# CCKVServer
这个项目的初衷是为了练习从零开始构建一个 KV 服务器。支持基本的[存储功能和发布订阅功能](#supported-commands)

# Architecture
![Overview of Server Architecture](./Architecture.png)

# Feature
- **存储**：支持多种存储后端，包括基于内存的[DashMap](https://github.com/xacrimon/dashmap)，嵌入式的[rocksdb](https://github.com/rust-rocksdb/rust-rocksdb)、[sled](https://github.com/spacejam/sled)
- **加密协议**：支持Tls协议或[Noise协议](https://noiseprotocol.org/noise.html)（目前，Noise协议仅支持NN模式）
- **多路复用**：支持[Yamux协议](https://github.com/hashicorp/yamux/blob/master/spec.md)或[Quic协议](https://quicwg.org/)
- **监控和测量**：由[opentelemetry](https://opentelemetry.io/)和[jaeger](https://www.jaegertracing.io/)集成
- **自定义的帧数据封装格式**：每个数据帧的包头占四字节，包含长度、是否压缩及压缩格式等信息

# Usage
### Profile
服务端和客户端会读取由tool/gen_config.rs生成的配置文件
```plaintext
~/$ cargo run --bin gen_config -- -h
help generating client and server config

Usage: gen_config [OPTIONS] --protocol <PROTOCOL>

Options:
  -p, --protocol <PROTOCOL>          Tls files are required when using Quic and Tls.
                                     You may use tools/gen_cert.rs to generate these files. [possible values: tls, noise, quic]
  -a, --addr <ADDR>                  [default: 127.0.0.1:1973]
      --enable-jaeger                
      --enable-log-file              
      --log-level <LOG_LEVEL>        [default: info]
      --log-path <LOG_PATH>          [default: /tmp/kv-log]
      --log-rotation <LOG_ROTATION>  [default: daily] [possible values: hourly, daily, never]
      --storage <STORAGE>            [default: memtable]
  -h, --help                         Print help
```
### Running Server
```sh
cargo run --bin kvs
```
### Running Client
```sh
cargo run --bin kvc
```
#### Supported Commands
* **get**  获取指定键的值。
* **getall**  获取所有键的值。
* **mget**  获取多个指定键的值。
* **set**  设置指定键的值。如果键已经存在，则更新其值。
* **mset**  设置多个键的值。如果键已经存在，则更新其值。
* **del**  删除指定键及其对应的值。
* **mdel**  删除多个指定键及其对应的值。
* **exist**  检查指定键是否存在。
* **mexist**  检查多个指定键是否存在。
* **Publish**  发布消息到指定频道。
* **Subscribe**  订阅指定频道以接收消息。
* **Unsubscribe**  取消订阅指定频道。
# Benchmark
### Pubsub
在100个订阅者的背景下，对Publish操作进行了基准测试。结果如下：
```plaintext
publishing              time:   [76.341 µs 76.824 µs 77.390 µs]
                        change: [-0.6231% +0.9935% +2.7754%] (p = 0.29 > 0.05)
```
服务器每次发布操作的平均时间为76.341微秒，即每秒大约可处理13,097次发布操作

# 🚧TODO🚧
- 使用消息队列（例如 Kafka、RabbitMQ 或 Redis Pub/Sub）替换 Subscribe 功能实现（目前用[Tokio channel](https://docs.rs/tokio/latest/tokio/sync/mpsc/fn.channel.html)）
- 为[DashMap](https://github.com/xacrimon/dashmap)实现哈希分片机制
- 扩展Noise协议的支持模式，增加对XX、IK等更多模式的支持。
- 增加集群模式的支持
- 修复Noise协议与Yamux协议的冲突，目前由Noise协议构建的数据流在放入Yamux中会碰到EOF