use crate::{error::PeerNetError, peer_id::PeerId, transports::Endpoint};

fn handshake(endpoint: Endpoint) -> Result<(PeerId, Endpoint), PeerNetError> {
    Ok(())
}
