//! Compile-pass mirror of the `build.rs` CLI module composition root.

mod cli {
    //! Minimal declaration-only model of the build-script CLI slice.

    pub mod config {
        //! Configuration schema stand-in.
    }

    mod validation {
        //! Validation helper stand-in.
    }

    mod help {
        //! Help-schema stand-in.
    }

    mod command {
        //! Command-schema stand-in.
    }
}

fn main() {}
