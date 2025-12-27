pub mod enums;
pub mod error;
pub mod market;
pub mod oms;
pub mod primitive;
pub mod time;

// 导出让外部使用
pub use enums::*;
pub use oms::*;
pub use primitive::*;

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
