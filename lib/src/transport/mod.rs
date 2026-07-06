
pub trait TransportTrait{
    const MAX_PAYLOAD_SIZE: usize;
    type ExtractIn;
    type InjectIn;
    type InjectOut;
    async fn extract(input: Self::ExtractIn) -> Option<Vec<u8>>;
    async fn inject(input: Self::InjectIn) -> Option<Self::InjectOut>;
}