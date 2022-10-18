use std::sync::Arc;
use rust_box::std_ext::StdExt;

fn main() {
    std::env::set_var("RUST_LOG", "std-ext=info");
    env_logger::init();

    test_std_arc();
}

fn test_std_arc() {
    let arc_a = Arc::new(1);
    let arc_b = 1.arc();
    assert_eq!(arc_a, arc_b);
}