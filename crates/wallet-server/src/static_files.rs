use include_dir::{include_dir, Dir};

pub static FRONTEND_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/frontend-dist");
