#[cfg(test)]
mod tests {
    use anyhow::Result;
    use chrono::{DateTime, NaiveDate};
    use quant_core::market::MarketBar;
    use quant_core::{BarPeriod, CurrencyPair, Exchange, Price, Quantity};
    use quant_storage::repository::market_repo;
    use rust_decimal::Decimal;
    use serde::Deserialize;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    use std::path::Path;
    use walkdir::WalkDir;
    // =========================================================================
    // 1. 定义 CSV 原始行结构 (用于反序列化)
    // =========================================================================

    // CSV 示例:
    // exchange symbol open  high  low   close amount      volume   bob                        eob                        type
    // XNAS     AAL    54.28 54.6  53.07 53.91 579582022.1 10756705 2015-01-02 00:00:00-05:00 2015-01-02 00:00:00-05:00 21
    #[derive(Debug, Deserialize)]
    struct RawCsvRow {
        exchange: String,
        symbol: String,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        amount: Decimal, // 成交额
        volume: Decimal, // 成交量 (虽然看起来是整数，用 Decimal 接收更安全)
        bob: String,     // 格式: 2015-01-02 00:00:00-05:00
        eob: String,
        #[serde(rename = "type")]
        trade_type: u8,
    }

    // =========================================================================
    // 2. 辅助转换函数
    // =========================================================================

    /// 解析带时区的日期字符串，提取日期部分
    /// 输入: "2015-01-02 00:00:00-05:00" -> 2015-01-02
    fn parse_date_str(date_str: &str) -> Result<NaiveDate> {
        // 格式包含时区 %z，DateTime::parse_from_str 可以处理
        let dt = DateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S%z")?;
        Ok(dt.date_naive())
    }

    /// 映射交易所代码到枚举
    fn map_exchange(code: &str) -> Exchange {
        match code {
            "XNAS" => Exchange::Nasdaq, // 假设你在 Exchange 枚举里加了 Nasdaq
            "XNYS" => Exchange::Nyse,   // 假设你在 Exchange 枚举里加了 Nyse
            _ => Exchange::Binance,     // 或者 fallback
        }
    }

    // =========================================================================
    // 3. 导入脚本 (Test Case)
    // =========================================================================

    async fn get_test_repo() -> market_repo::MarketDataRepository {
        let pool = quant_storage::repository::common::get_real_pool().await;
        market_repo::MarketDataRepository::new(pool.clone())
    }

    #[tokio::test]
    async fn import_us_stock_data() -> Result<()> {
        // 1. 获取数据库连接
        // 注意：这里复用你之前的 get_test_repo 或直接构
        let repo = get_test_repo().await;

        // 2. 配置根路径
        // 注意 Rust 字符串中 \ 需要转义，或者使用 r"" 原始字符串
        let root_path = r"D:\BaiduNetdiskDownload\1d\output\";

        println!("Starting import from: {}", root_path);

        let mut total_files = 0;
        let mut total_records = 0;

        // 3. 遍历目录
        for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            // 过滤：只处理 .csv 文件
            if path.extension().map_or(false, |ext| ext == "csv") {
                total_files += 1;
                // println!("Processing file: {:?}", path.file_name());

                // 处理单个文件
                let records_in_file = process_csv_file(&repo, path).await?;
                total_records += records_in_file;
            }
        }

        println!("Import finished!");
        println!("Total files processed: {}", total_files);
        println!("Total records inserted: {}", total_records);

        Ok(())
    }

    /// 处理单个 CSV 文件
    async fn process_csv_file(
        repo: &market_repo::MarketDataRepository,
        path: &Path,
    ) -> Result<usize> {
        let mut file = File::open(path)?;

        // 1. BOM 头处理 (保持不变，这很重要)
        let mut bom = [0u8; 3];
        let bytes_read = file.read(&mut bom)?;
        if bytes_read == 3 && bom == [0xEF, 0xBB, 0xBF] {
            // 发现 UTF-8 BOM，跳过
        } else {
            // 没有 BOM，倒带回文件开头
            file.seek(SeekFrom::Start(0))?;
        }

        // 2. 构建 CSV Reader
        // 【关键点】：增加 trim 配置，并准备调试
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b',')       // <--- 如果报错持续，请尝试改成 b','
            .trim(csv::Trim::All)   // <--- 自动去除字段两端的空格，解决 "exchange " 匹配不到的问题
            .from_reader(file);

        // 3. 【DEBUG 核心】：打印解析器看到的表头
        // 这一步会消耗掉 header 行，所以下面 deserialize 会自动从数据行开始
        // 必须 clone 一份，因为 headers() 返回的是借用
        let headers = rdr.headers()?.clone();
        // println!("DEBUG: File {:?} headers: {:?}", path.file_name(), headers);

        // 自检逻辑：如果只解析出一列，且包含逗号，提示用户修改分隔符
        if headers.len() == 1 {
            let header_str = &headers[0];
            if header_str.contains(',') {
                eprintln!("!!! 严重警告 !!!");
                eprintln!("文件 {:?} 看起来像是逗号分隔的 CSV，但当前代码使用的是 Tab 分隔。", path.file_name());
                eprintln!("请将代码中的 .delimiter(b'\\t') 修改为 .delimiter(b',')");
                eprintln!("解析到的错误表头: {:?}", headers);
                return Ok(0);
            }
        }

        // 检查是否存在 exchange 字段 (提前报错，而不是在循环里报错)
        // 这一步是可选的，但能让你更清楚地知道是哪个文件有问题
        if headers.iter().find(|&h| h == "exchange").is_none() {
            eprintln!("错误: 在文件 {:?} 的表头中找不到 'exchange' 字段。", path.file_name());
            eprintln!("实际读到的表头: {:?}", headers);
            // 你可以在这里 return Ok(0) 跳过该文件，或者 panic
        }

        let mut count = 0;

        // 4. 遍历数据行
        for result in rdr.deserialize() {
            let record: RawCsvRow = match result {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Skipping invalid row in {:?}: {}", path, e);
                    continue;
                }
            };

            // --- 数据转换逻辑 (保持不变) ---
            let date = match parse_date_str(&record.bob) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Date parse error: {}", e);
                    continue;
                }
            };

            let exchange = map_exchange(&record.exchange);
            let pair = CurrencyPair::new(&record.symbol, "USD");

            let bar = MarketBar::new(
                exchange,
                pair.to_string(),
                BarPeriod::D1,
                record.trade_type,
                Price(record.open),
                Price(record.high),
                Price(record.low),
                Price(record.close),
                Quantity(record.volume),
                date,
            )?;

            let mut final_bar = bar;
            final_bar.amount = Some(record.amount);

            repo.save(&final_bar).await?;
            count += 1;
        }

        if count > 0 {
            println!("  -> Saved {} records from {:?}", count, path.file_name().unwrap());
        }

        Ok(count)
    }
}
