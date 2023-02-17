
//TODO: Increase consistency of error handling this structure has been created just to have a place to put the errors but not designed yet.
#[derive(Debug)]
pub enum PeerNetError {
    InitializationError(String),
}