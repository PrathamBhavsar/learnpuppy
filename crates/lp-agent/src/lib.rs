//! Agent runtime for LearnPuppy.

pub mod extract;
pub mod loops;
pub mod workbench;

#[cfg(test)]
mod tests {
    #[test]
    fn smoke_test() {
        assert_eq!(2 + 2, 4);
    }
}
