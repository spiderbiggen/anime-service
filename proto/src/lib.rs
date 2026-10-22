pub mod api {
    pub mod v2 {
        #![allow(
            clippy::large_enum_variant,
            clippy::derive_partial_eq_without_eq,
            clippy::double_must_use
        )]
        tonic::include_proto!("api.v2");
    }
}
