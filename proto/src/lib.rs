pub mod api {
    pub mod v2 {
        #![allow(clippy::large_enum_variant)]
        #![allow(clippy::derive_partial_eq_without_eq)]
        tonic::include_proto!("api.v2");
    }
}
