use std::{fmt::Debug, hash::Hash};

pub trait PeerId:
    Eq + PartialEq + Clone + Send + Ord + PartialOrd + Hash + Debug + Sync + 'static
{
    fn generate() -> Self;
}
