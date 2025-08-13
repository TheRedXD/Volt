use std::time::Duration;
use std::sync::{Arc, LazyLock, Mutex};

macro_rules! generate_timings {
    ($($name:ident),*) => {
        struct SharedTimings {
            $(
                $name: Duration,
            )*
        }

        static SHARED_TIMINGS: LazyLock<Arc<Mutex<SharedTimings>>> =
            LazyLock::new(|| Arc::new(Mutex::new(SharedTimings {
                $(
                $name: Duration::ZERO,
                )*
            }
        )));

        $(
            pastey::paste! {
                #[allow(dead_code, reason = "this is a debugging tool")]
                pub fn [<get_ $name _time>]() -> Duration {
                    SHARED_TIMINGS.lock().unwrap().$name
                }

                #[allow(dead_code, reason = "this is a debugging tool")]
                pub fn [<set_ $name _time>](time: Duration) {
                    SHARED_TIMINGS.lock().unwrap().$name = time;
                }
            }
        )*

        #[allow(dead_code, reason = "this is a debugging tool")]
        pub fn show_timings(ctx: &egui::Context, window_name: &str) {
            egui::Window::new(window_name)
                .collapsible(false)
                .show(ctx, |ui| {
                    $(
                        pastey::paste! {
                            ui.label(format!("{}: {:?}", stringify!($name), [<get_ $name _time>]()));
                        }
                    )*
                });
        }
    };
}

generate_timings!(render);
