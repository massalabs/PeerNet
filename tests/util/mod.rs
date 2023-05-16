#![allow(dead_code)]
use std::{
    fmt::Display,
    thread::{sleep, JoinHandle},
    time::Duration,
};

use massa_signature::KeyPair;
use peernet::{
    error::PeerNetResult,
    messages::MessagesHandler,
    types::{PeerNetHasher, PeerNetId, PeerNetKeyPair, PeerNetPubKey, PeerNetSignature},
};

#[derive(Clone)]
pub struct DefaultMessagesHandler {}

impl MessagesHandler for DefaultMessagesHandler {
    fn deserialize_id<'a, T: PeerNetId>(
        &self,
        data: &'a [u8],
        _peer_id: &T,
    ) -> PeerNetResult<(&'a [u8], u64)> {
        Ok((data, 0))
    }

    fn handle<T: PeerNetId>(&self, _id: u64, _data: &[u8], _peer_id: &T) -> PeerNetResult<()> {
        Ok(())
    }
}

pub fn create_clients(nb_clients: usize, to_ip: &str) -> Vec<JoinHandle<()>> {
    let mut clients = Vec::new();
    for ncli in 0..nb_clients {
        let to_ip = to_ip.to_string();
        let client = std::thread::Builder::new()
            .name(format!("test_client_{}", ncli))
            .spawn(|| {
                let stream = std::net::TcpStream::connect(to_ip).unwrap();
                sleep(Duration::from_secs(100));
                stream.shutdown(std::net::Shutdown::Both).unwrap();
            })
            .expect("Failed to spawn thread test_client");
        sleep(Duration::from_millis(100));
        clients.push(client);
    }
    clients
}

#[derive(Clone, Debug)]
pub struct TestSignature(massa_signature::Signature);

impl TestSignature {
    pub fn new(massa_signature: massa_signature::Signature) -> TestSignature {
        TestSignature(massa_signature)
    }
}

impl PeerNetSignature for TestSignature {
    fn to_bytes(&self) -> Vec<u8> {
        unimplemented!()
    }

    fn from_bytes(bytes: &[u8]) -> PeerNetResult<Self> {
        let data: &[u8; 64] = bytes.try_into().unwrap();

        Ok(TestSignature(
            massa_signature::Signature::from_bytes(data).unwrap(),
        ))
    }
}

impl Display for TestKeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct TestKeyPair(KeyPair);

impl TestKeyPair {
    pub fn generate() -> TestKeyPair {
        TestKeyPair(massa_signature::KeyPair::generate())
    }
}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct TestPubKey(massa_signature::PublicKey);

impl PeerNetPubKey for TestPubKey {
    fn to_bytes(&self) -> &[u8] {
        let bytes = self.0.to_bytes();
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> PeerNetResult<Self> {
        unimplemented!()
        // massa_signature::PublicKey::from_bytes(bytes)
        //     .map(|massa_pubkey| TestPubKey { massa_pubkey })
        //     .map_err(|err| PeerNetError::new(PeerNetError::SignError, "test", err, None))
    }
}

impl PeerNetKeyPair<TestPubKey> for TestKeyPair {
    fn get_public_key(&self) -> TestPubKey {
        //self.0.get_public_key();
        let key = TestPubKey(self.0.get_public_key());
        key

        // self.0.get_public_key()
    }

    fn sign(&self, hasher: &impl PeerNetHasher) -> PeerNetResult<Vec<u8>> {
        let temp = massa_hash::Hash::compute_from(hasher.to_bytes());

        let signature = self.0.sign(&temp).unwrap();

        Ok(signature.to_bytes().to_vec())
    }

    // fn sign(&self, hash: &TestHasher) -> PeerNetResult<TestSignature> {
    //     let massa_signature = self.0.sign(&hash.0).unwrap();
    //     let sign = TestSignature(massa_signature);
    //     Ok(sign)
    // }
}

#[derive(Clone, Debug)]
pub struct TestHasher(massa_hash::Hash);

impl PeerNetHasher for TestHasher {
    fn compute_from(data: &[u8]) -> Self {
        TestHasher(massa_hash::Hash::compute_from(data))
    }

    fn to_bytes(&self) -> &[u8] {
        self.0.to_bytes()
    }
}
// use std::marker::PhantomData;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TestId(Vec<u8>);

impl PeerNetId for TestId {
    fn verify_signature(
        &self,
        hash: &impl PeerNetHasher,
        signature: &impl PeerNetSignature,
    ) -> PeerNetResult<()> {
        unimplemented!()
    }

    fn from_public_key<K: PeerNetPubKey>(key: K) -> Self {
        TestId(key.to_bytes().to_vec())
    }
}
