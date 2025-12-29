use anyhow::anyhow;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sqlx::error::BoxDynError;
use sqlx::mysql::MySqlValueRef;
use sqlx::{Database, Decode, Encode, MySql, Type};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};
use std::str::FromStr;

// =========================================================================
// Price (价格)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Type)]
#[sqlx(transparent)]
pub struct Price(pub Decimal);

impl Price {
    pub const ZERO: Price = Price(Decimal::ZERO);

    pub fn from_f64(val: f64) -> Self {
        Price(Decimal::from_f64(val).unwrap_or_default())
    }

    pub fn from_str(s: &str) -> Self {
        Price(Decimal::from_str(s).unwrap_or_default())
    }

    pub fn to_f64(&self) -> f64 {
        self.0.to_f64().unwrap_or(0.0)
    }
}

// --- 运算符 ---
impl Add for Price {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Price(self.0 + rhs.0)
    }
}
impl AddAssign for Price {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl Sub for Price {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Price(self.0 - rhs.0)
    }
}
impl SubAssign for Price {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

// --- 序列化 (关键修复点) ---
impl Serialize for Price {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Price {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // ⚠️ 修复：使用 <Type as Trait>::method 语法
        let d = <Decimal as Deserialize>::deserialize(deserializer)?;
        Ok(Price(d))
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =========================================================================
// Quantity (数量)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Type)]
#[sqlx(transparent)]
pub struct Quantity(pub Decimal);

impl Quantity {
    pub const ZERO: Quantity = Quantity(Decimal::ZERO);
    pub fn from_f64(val: f64) -> Self {
        Quantity(Decimal::from_f64(val).unwrap_or_default())
    }
    pub fn from_str(s: &str) -> Self {
        Quantity(Decimal::from_str(s).unwrap_or_default())
    }
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

// --- 运算符 ---
impl Add for Quantity {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Quantity(self.0 + rhs.0)
    }
}
impl AddAssign for Quantity {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl Sub for Quantity {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Quantity(self.0 - rhs.0)
    }
}
impl SubAssign for Quantity {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

// --- 序列化 (关键修复点) ---
impl Serialize for Quantity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // ⚠️ 修复：使用 <Type as Trait>::method 语法
        let d = <Decimal as Deserialize>::deserialize(deserializer)?;
        Ok(Quantity(d))
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// --- 混合运算 ---
impl Mul<Quantity> for Price {
    type Output = Decimal;
    fn mul(self, rhs: Quantity) -> Self::Output {
        self.0 * rhs.0
    }
}
impl Mul<Price> for Quantity {
    type Output = Decimal;
    fn mul(self, rhs: Price) -> Self::Output {
        self.0 * rhs.0
    }
}
impl Div<Price> for Decimal {
    type Output = Quantity;
    fn div(self, rhs: Price) -> Self::Output {
        Quantity(self / rhs.0)
    }
}
impl Div<Quantity> for Decimal {
    type Output = Price;
    fn div(self, rhs: Quantity) -> Self::Output {
        Price(self / rhs.0)
    }
}
impl From<Price> for Decimal {
    fn from(p: Price) -> Self {
        p.0
    }
}
impl From<Quantity> for Decimal {
    fn from(q: Quantity) -> Self {
        q.0
    }
}

/// 交易对结构体
/// 将 "BTC/USDT" 结构化为 base="BTC", quote="USDT"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CurrencyPair {
    pub base: String,  // 基础币种 (e.g., BTC)
    pub quote: String, // 计价币种 (e.g., USDT)
}

impl CurrencyPair {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into().to_uppercase(),
            quote: quote.into().to_uppercase(),
        }
    }
}

// 1. 实现 Display: 用于生成 "BTC/USDT" 字符串
impl fmt::Display for CurrencyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

// 2. 实现 FromStr: 用于从字符串解析 "BTC/USDT"
impl FromStr for CurrencyPair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            // 尝试处理没有分隔符的情况，如 "BTCUSDT" (仅作示例，建议统一带分隔符)
            // 这里我们强制要求标准格式
            return Err(anyhow!("Invalid symbol format: {}, expected BASE/QUOTE", s));
        }
        Ok(Self::new(parts[0], parts[1]))
    }
}

// 3. 实现 SQLx 的 Type 接口: 告诉数据库把它当 VARCHAR 处理
impl Type<MySql> for CurrencyPair {
    fn type_info() -> <MySql as Database>::TypeInfo {
        <String as Type<MySql>>::type_info()
    }

    fn compatible(ty: &<MySql as Database>::TypeInfo) -> bool {
        <String as Type<MySql>>::compatible(ty)
    }
}

// 4. 实现 SQLx 的 Encode (写入数据库): 转为 String
impl<'q> Encode<'q, MySql> for CurrencyPair {
    // MySQL 的 buffer 是 Vec<u8>
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> sqlx::encode::IsNull {
        <String as Encode<MySql>>::encode_by_ref(&self.to_string(), buf)
    }
}

// 3. 修复 Decode 实现
impl<'r> Decode<'r, MySql> for CurrencyPair {
    // 使用具体类型 MySqlValueRef<'r>
    fn decode(value: MySqlValueRef<'r>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<MySql>>::decode(value)?;
        Ok(Self::from_str(&s)?)
    }
}
