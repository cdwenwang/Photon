
# Photon (光子) ⚡️

![Language](https://img.shields.io/badge/Language-Rust-orange.svg)
![AI-Agent](https://img.shields.io/badge/AI-Multi--Agent-blueviolet.svg)
![License](https://img.shields.io/badge/License-MIT-blue.svg)
![Build](https://img.shields.io/badge/Build-Passing-green.svg)

**Photon** 是一个基于 **Rust** 构建的 **AI 驱动型** 高性能量化交易系统。

它融合了传统高频交易系统的 **低延迟与并发安全**（基于 Rust 零成本抽象），以及现代大模型（LLM）的 **逻辑推理与泛化分析能力**。通过 "Manager Agent" 编排的多智能体网络，Photon 能够在毫秒级执行交易的同时，进行秒级的深度市场分析与舆情监控。

---

## 🏗 系统架构：快慢双轨设计

Photon 采用 **"Fast Path / Slow Path"** 的异步解耦架构，确保 AI 的推理延迟不会阻塞核心交易循环。

```mermaid
sequenceDiagram
    participant Market as 市场(Exchange)
    participant Feed as Feed(眼睛)
    participant DB as Database/Cache
    participant Agent as AI Agent(大脑)
    participant Risk as Risk(安全阀)
    participant Exec as Execution(手)

    %% 1. 数据摄入 (Feed 模块的工作)
    Market->>Feed: WebSocket 推送海量数据
    Feed->>DB: 清洗并存储 (K线/指标)
    
    %% 2. 思考与决策 (Agent 模块的工作)
    loop 定时分析
        Agent->>DB: 查询: "过去4小时成交量是多少?" (Tool)
        DB-->>Agent: 返回: 聚合后的数据
        Agent->>Agent: LLM 推理: "看涨，建议买入"
    end

    %% 3. 执行指令 (风险控制与执行)
    Agent->>Risk: 请求: "买入 1.0 BTC"
    
    alt 风控通过
        Risk->>Exec: 转发指令
        Exec->>Market: API 下单 (签名/重试)
        Market-->>Exec: 订单确认
        Exec-->>Agent: 执行成功
    else 风控拒绝
        Risk-->>Agent: 拒绝: "超过最大持仓限制!"
    end
```

---

## 🧠 AI Agent 架构：Manager Agent 调度模式

Photon 引入了 `crates/agent` 模块，实现了基于 **Function Calling** 和 **Semantic Routing (语义路由)** 的多 Agent 协作系统。

### 1. 调度层 (Orchestration Layer)
核心是一个 **Manager Agent (Coordinator)**。它不直接干活，而是作为系统的“大脑”：
*   **任务感知**：接收来自系统的非结构化需求（如“分析当前 ETH 市场情绪”）。
*   **智能路由**：根据注册表中各个 Agent 的 `System Prompt` 和能力描述，将任务动态分发给最合适的专家。

### 2. 专家层 (Expert Layer)
专精于特定领域的 Agent，由 Manager 调度：
*   **🕵️ Researcher (研究员)**：擅长信息搜集。
    *   *能力*：调用 `Google Search API` 搜索突发新闻、Twitter 舆情。
*   **📊 Analyst (分析师)**：擅长数据挖掘。
    *   *能力*：调用 `Market DB Tool` 查询历史 K 线、计算技术指标。
*   **🛡️ Risk Guardian (风控官)**：擅长风险评估。
    *   *能力*：评估当前仓位风险敞口，否决高风险信号。

### 3. 工具层 (Tool Layer)
封装了原子能力的 Rust Trait，供 Agent 调用外部世界：
*   **Web Tools**: Google Search, News API.
*   **Data Tools**: SQLx Database Query, Redis Cache Access.
*   **Compute Tools**: TA-Lib Indicator Calculation.

---

## 📂 项目结构

```text
Photon/
├── Cargo.toml                # Workspace 配置
├── services/
│   └── engine/               # [主程序] 负责启动 Fast Path 和 Smart Path
├── crates/
│   ├── agent/                # [NEW] AI Agent 核心库
│   │   ├── src/
│   │   │   ├── llm/          # 底层 LLM 接口 (OpenAI/DeepSeek)
│   │   │   ├── tools/        # 工具箱 (GoogleSearchTool, MarketDbTool)
│   │   │   ├── personas/     # 专家 Agent 定义 (Researcher, Analyst)
│   │   │   └── manager.rs    # 智能路由器实现
│   ├── core/                 # 基础数据结构 (Order, Trade, Symbol)
│   ├── feed/                 # 行情接入 (Binance, OKX WebSocket)
│   ├── strategy/             # 传统策略引擎 (RSI, Grid, Maker)
│   ├── execution/            # 订单执行与 OMS
│   ├── storage/              # 数据库持久化 (TimescaleDB/Redis)
│   └── risk/                 # 预设风控规则
└── .config/
    └── nextest.toml          # 测试配置
```

---

## 🚀 核心特性

### 1. 极速交易 (High Performance)
*   **Rust 异步运行时**：基于 `Tokio` 构建，充分利用多核性能。
*   **无锁编程**：关键路径使用 `Channels` 通信，避免锁竞争。
*   **精确计算**：全链路使用 `rust_decimal`，杜绝浮点数精度误差。

### 2. 智能决策 (AI Capabilities)
*   **语义路由**：Manager Agent 能够理解自然语言指令，动态协调多个 Agent 协同工作。
*   **外部感知**：集成了 Google Search 等工具，让交易策略能感知链下世界的突发新闻。
*   **可解释性**：所有 AI 决策均保留完整的 "思考链 (Chain of Thought)" 日志。

### 3. 工程化完备
*   **Workspace 管理**：清晰的模块化分层。
*   **测试驱动**：集成 `cargo-nextest`，支持单元测试与集成测试生成 JUnit 报告。
*   **数据库集成**：`SQLx` + `Postgres` 处理时序数据，`Redis` 处理热缓存。

---

## 🛠️ 快速开始

### 前置要求
*   Rust (Latest Stable)
*   PostgreSQL & Redis
*   OpenAI / DeepSeek API Key (用于 AI Agent)
*   Google Custom Search API Key (用于 Researcher Agent)

### 配置环境变量
在项目根目录创建 `.env`：
```bash
DATABASE_URL=postgres://user:pass@localhost/photon
REDIS_URL=redis://localhost:6379
OPENAI_API_KEY=sk-xxxxxx
GOOGLE_API_KEY=xxxxxx
GOOGLE_CX_ID=xxxxxx
```

### 运行测试
```bash
# 运行全量测试并生成报告
cargo nextest run --workspace --profile ci
```

### 启动系统
```bash
# 启动主引擎
cargo run -p quant-engine
```

---

## 🧩 Agent 开发示例

如何向系统添加一个新的 **"Twitter 情绪分析师"**？

**1. 定义工具** (`crates/agent/src/tools/twitter.rs`):
实现 `Tool` Trait，封装 Twitter API 调用逻辑。

**2. 定义专家** (`crates/agent/src/personas/social_analyst.rs`):
创建一个包含 Twitter 工具的 Agent，设定 System Prompt 为 *"你是一个专注于社交媒体情绪分析的专家..."*。

**3. 注册到 Manager** (`services/engine/src/main.rs`):

```rust
let twitter_tool = TwitterTool::new(...);
let social_agent = Agent::new("SocialAnalyst", "分析推特舆情...").with_tool(twitter_tool);

// 注册给 Manager，Manager 会自动根据你的描述进行路由
manager.register_agent(social_agent);
```

**4. 触发**:
当系统通过定时器发送任务 *"看看现在推特上关于 DOGE 的讨论热度"* 时，Manager 会自动将任务路由给 `SocialAnalyst`。

---

## 📄 License

MIT License
```