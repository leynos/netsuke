//! Compile-fail mirror of an invalid runtime import from the `build.rs` slice.

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

use cli::discovery;

fn main() {}
