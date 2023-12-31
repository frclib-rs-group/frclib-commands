
use std::{cell::RefCell, sync::atomic::{AtomicBool, Ordering}, rc::Rc};

use crate::conditions::Condition;



#[allow(dead_code, clippy::collection_is_never_read)]
#[test]
fn test_manager() {
    use crate::clone_mv;
    use super::*;
    use std::time::Duration;
    thread_local! {
        static TEST_MARKERS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    }

    fn add_marker(marker: &str) {
        TEST_MARKERS.with(|markers| {
            markers.borrow_mut().push(marker.to_owned());
        });
    }

    #[derive(Debug, Clone)]
    struct TestCommand {
        name: String,
        start: bool,
        end: bool,
    }
    impl CommandTrait for TestCommand {
        fn is_finished(&mut self) -> bool {
            add_marker("custom_command_is_finished");
            true
        }
    }

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

        fn periodic(&self, _: Duration) {
            add_marker("subsystem_periodic");
        }

        fn log(&self) {
            add_marker("subsystem_log");
        }
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

    assert_eq!(subsystem.suid(), subsystem.suid());

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
                    add_marker("cmd_periodic");
                    variable.push(0.0);
                }
        ))
        .init(clone_mv!(
            variable,
            variable2 >> || {
                add_marker("cmd_init");
                variable.push(0.0);
                variable2.replace(0);
            })
        )
        .end(move |_| {
            add_marker("cmd_end");
        })
        .is_finished(|| {
            add_marker("cmd_is_finished");
            true
        })
        .with_subsystems(&[&subsystem, &dummy_subsystem])
        .build()
        .schedule();

    let inner_cond = Rc::new(AtomicBool::new(false));
    let cond = Condition::new(
        clone_mv!(
            inner_cond >> || {
                add_marker("cond_eval");
                inner_cond.load(Ordering::Relaxed)
            }
        )
    );

    cond.on_true(
        CommandBuilder::new()
            .init(move || {
                add_marker("cond_sched_init");
            })
            .build()
    );

    manager.run();

    inner_cond.store(true, Ordering::Relaxed);

    manager.run();

    macro_rules! assert_marker {
        ($marker:expr) => {
            assert!(
                TEST_MARKERS.with(|markers| {
                    markers.borrow().contains(&$marker.to_owned())
                }),
                "Marker {} not found",
                $marker
            );
        };
    }

    assert_marker!("custom_command_is_finished");
    assert_marker!("subsystem_periodic");
    // assert_marker!("subsystem_log"); TODO
    assert_marker!("cmd_init");
    assert_marker!("cmd_periodic");
    assert_marker!("cmd_end");
    assert_marker!("cmd_is_finished");
    assert_marker!("cond_eval");
    assert_marker!("cond_sched_init");
}
