# Photon (光子) ⚡️

![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)
![License](https://img.shields.io/badge/License-MIT-blue.svg)
![Build](https://img.shields.io/badge/Build-Passing-green.svg)

**Photon** 是一个基于 **Rust** 构建的高性能、低延迟、事件驱动（Event-Driven）的量化交易系统。

它旨在解决传统 Python 交易系统在实盘高频场景下的延迟抖动与并发安全问题，利用 Rust 的零成本抽象（Zero-cost abstractions）和内存安全性，提供竞速级别的交易执行能力。

---

## 🏗 系统架构

Photon 采用经典的事件驱动架构，核心组件通过异步消息总线（Event Bus）进行通信。

```text
[ 交易所 API ]  <-- WebSocket/FIX -->  [ Data Feed (Ingestion) ]
                                            |
                                            v
                                     [ Event Bus / Channel ]  <-- 核心消息总线
                                            |
                       +--------------------+---------------------+
                       |                    |                     |
                 [ Strategy ]         [ Risk Manager ]      [ Data Recorder ]
                       |                    |                     |
                       +---------+----------+                     v
                                 |                          [ Database ]
                           [ Execution OMS ]
                                 |
                                 v
                           [ 交易所 API ]