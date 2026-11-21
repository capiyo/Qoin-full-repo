//! Deterministic dataset identity and conservative data-quality classification.
use serde::Serialize;
use sha2::{Digest,Sha256};

#[derive(Serialize)]
struct DatasetIdentity<'a,T:Serialize>{schema_version:u32,request:&'a T}

pub struct DatasetDigest(Sha256);
impl DatasetDigest {
    pub fn new<T:Serialize>(request:&T)->Self {
        let mut digest=Sha256::new();
        let identity=serde_json::to_vec(&DatasetIdentity{schema_version:1,request}).expect("serializable dataset request");
        digest.update(identity);
        digest.update([0]);
        Self(digest)
    }
    pub fn update(&mut self,event:&[u8]) {
        self.0.update((event.len() as u64).to_be_bytes());
        self.0.update(event);
    }
    pub fn finish(self)->String {format!("sha256:{}",hex::encode(self.0.finalize()))}
}
pub fn quality_status(events:i64,invalid:i64,gaps:i64)->&'static str {
    if events==0 {"empty"} else if invalid>0||gaps>0 {"degraded"} else {"observed_clean"}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dataset_digest_is_deterministic_and_content_sensitive(){
        let request=serde_json::json!({"symbol":"BTCUSDT","venue":"binance_usdm","from":1,"to":2});
        let mut a=DatasetDigest::new(&request);a.update(b"event-a");a.update(b"event-b");
        let mut b=DatasetDigest::new(&request);b.update(b"event-a");b.update(b"event-b");
        let mut c=DatasetDigest::new(&request);c.update(b"event-a");c.update(b"event-c");
        let ah=a.finish();let bh=b.finish();let ch=c.finish();
        assert_eq!(ah,bh);assert_ne!(bh,ch);
    }
    #[test]
    fn detected_quality_issues_are_never_marked_clean(){
        assert_eq!(quality_status(1,0,0),"observed_clean");
        assert_eq!(quality_status(0,0,0),"empty");
        assert_eq!(quality_status(1,1,0),"degraded");
        assert_eq!(quality_status(1,0,1),"degraded");
    }
}
