//! Tests for environment restoration in `CliWorld`.

mod world;
use mockable::MockEnv;
use world::CliWorld;

#[test]
fn drop_restores_path() {
    let original = std::env::var("PATH").unwrap_or_default();
    {
        let mut env = MockEnv::new();
        let original_clone = original.clone();
        env.expect_raw()
            .withf(|key| key == "PATH")
            .returning(move |_| Ok(original_clone.clone()));
        let mut world = CliWorld::default();
        world.env = Box::new(env);
        world.original_path = Some(world.env.raw("PATH").expect("retrieve PATH").into());
        unsafe {
            std::env::set_var("PATH", "temp-path");
        }
    }
    assert_eq!(
        std::env::var("PATH").expect("read PATH after drop"),
        original
    );
}
