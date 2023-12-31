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
    impl SubsystemBase for TestSubsystem {
        fn periodic(&self, period: Duration) {
            println!("Periodic: {period:?}");
        }
    }
    impl Subsystem for TestSubsystem {
        const SUID: SubsystemSUID = 11;

        fn construct() -> Self {
            Self {
                integer: 0,
                string: String::new(),
                boolean: false,
            }
        }

        fn log(&self) {
        }
    }

    let mut manager = CommandManager::new();
    let subsystem = SubsystemCell::<TestSubsystem>::generate(&mut manager);

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
        //subsystems impl copy so we can easily move them into closures
        .end(move |_| println!("Cmd End for subsystem: {:?}", subsystem.name()))
        .build()
        .schedule();

    manager.run();
}
