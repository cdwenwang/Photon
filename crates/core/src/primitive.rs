use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};
use std::str::FromStr;

// =========================================================================
// Price (价格)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
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
    fn add(self, rhs: Self) -> Self::Output { Price(self.0 + rhs.0) }
}
impl AddAssign for Price {
    fn add_assign(&mut self, rhs: Self) { self.0 += rhs.0; }
}
impl Sub for Price {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output { Price(self.0 - rhs.0) }
}
impl SubAssign for Price {
    fn sub_assign(&mut self, rhs: Self) { self.0 -= rhs.0; }
}

// --- 序列化 (关键修复点) ---
impl Serialize for Price {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Price {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        // ⚠️ 修复：使用 <Type as Trait>::method 语法
        let d = <Decimal as Deserialize>::deserialize(deserializer)?;
        Ok(Price(d))
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}

// =========================================================================
// Quantity (数量)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quantity(pub Decimal);

impl Quantity {
    pub const ZERO: Quantity = Quantity(Decimal::ZERO);
    pub fn from_f64(val: f64) -> Self { Quantity(Decimal::from_f64(val).unwrap_or_default()) }
    pub fn from_str(s: &str) -> Self { Quantity(Decimal::from_str(s).unwrap_or_default()) }
    pub fn is_zero(&self) -> bool { self.0.is_zero() }
}

// --- 运算符 ---
impl Add for Quantity {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output { Quantity(self.0 + rhs.0) }
}
impl AddAssign for Quantity {
    fn add_assign(&mut self, rhs: Self) { self.0 += rhs.0; }
}
impl Sub for Quantity {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output { Quantity(self.0 - rhs.0) }
}
impl SubAssign for Quantity {
    fn sub_assign(&mut self, rhs: Self) { self.0 -= rhs.0; }
}

// --- 序列化 (关键修复点) ---
impl Serialize for Quantity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        // ⚠️ 修复：使用 <Type as Trait>::method 语法
        let d = <Decimal as Deserialize>::deserialize(deserializer)?;
        Ok(Quantity(d))
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}

// --- 混合运算 ---
impl Mul<Quantity> for Price {
    type Output = Decimal;
    fn mul(self, rhs: Quantity) -> Self::Output { self.0 * rhs.0 }
}
impl Mul<Price> for Quantity {
    type Output = Decimal;
    fn mul(self, rhs: Price) -> Self::Output { self.0 * rhs.0 }
}
impl Div<Price> for Decimal {
    type Output = Quantity;
    fn div(self, rhs: Price) -> Self::Output { Quantity(self / rhs.0) }
}
impl Div<Quantity> for Decimal {
    type Output = Price;
    fn div(self, rhs: Quantity) -> Self::Output { Price(self / rhs.0) }
}
impl From<Price> for Decimal {
    fn from(p: Price) -> Self { p.0 }
}
impl From<Quantity> for Decimal {
    fn from(q: Quantity) -> Self { q.0 }
}