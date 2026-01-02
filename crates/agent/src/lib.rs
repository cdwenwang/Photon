pub mod config;
pub mod host;
pub mod llm;
pub mod manager;
pub mod personas;
pub mod store;
pub mod tools;
pub mod types;

pub use types::*;

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
