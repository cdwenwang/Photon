pub mod account;
pub mod enums;
pub mod market;
pub mod oms;
pub mod primitive;
pub mod strategy;
pub mod time;
pub mod validate;

// 导出让外部使用
pub use account::*;
pub use enums::*;
pub use oms::*;
pub use primitive::*;
pub use strategy::*;
pub use time::*;
pub use validate::*;
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
