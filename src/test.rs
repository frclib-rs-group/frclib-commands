use crate::clone_mv;

#[allow(dead_code, clippy::collection_is_never_read)]
#[test]
fn test_manager() {
    use super::*;
    use std::time::Duration;

    #[derive(Debug, Clone)]
    struct TestCommand {
        name: String,
        start: bool,
        end: bool,
    }
    impl CommandTrait for TestCommand {}

    struct TestSubsystem {
        integer: i32,
        string: String,
        boolean: bool,
    }
    impl Subsystem for TestSubsystem {
        fn construct() -> Self {
            Self {
                integer: 0,
                string: String::new(),
                boolean: false,
            }
        }

        fn periodic(&self, period: Duration) {
            println!("Periodic: {period:?}");
        }

        fn log(&self) {}
    }

    struct DummySubsystem;
    impl Subsystem for DummySubsystem {
        fn construct() -> Self {
            Self
        }
    }

    let mut manager = CommandManager::new();
    let subsystem = SubsystemCell::<TestSubsystem>::generate(&mut manager);
    let dummy_subsystem = SubsystemCell::<DummySubsystem>::generate(&mut manager);

    for _ in 0..5 {
        println!("{:?}", subsystem.suid())
    }

    let command = Command::Custom(Box::new(TestCommand {
        name: "Test".to_owned(),
        start: false,
        end: false,
    }));

    manager.schedule(command);

    let variable: Vec<f64> = Vec::new();
    let variable2: Option<u8> = None;

    CommandBuilder::new()
        .periodic(clone_mv!(
            variable
                >> |_period| {
                    println!("Subsystem name: {:?}", subsystem.name());
                    println!("Cmd Periodic: {variable:?}");
                }
        ))
        .init(clone_mv!(
            variable,
            variable2 >> || println!("Cmd Init: {variable:?} {variable2:?}")
        ))
        .end(move |_| println!("Cmd End for subsystem: {:?}", subsystem.name()))
        .with_subsystems(&[&subsystem, &dummy_subsystem])
        .build()
        .schedule();

    manager.run();
}
