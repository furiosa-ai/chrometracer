use std::{
    thread::{self},
    time::Duration,
};

#[chrometracer::instrument(fields(name = format!("{}", "hello"), tid = 1), args = "HELLO")]
fn hello() {}

#[chrometracer::instrument(event: "async", fields(name = format!("{}", "bye"), tid = 1))]
fn bye() {}

fn main() {
    let _guard = chrometracer::builder().init();

    let mut handles = vec![];
    for _ in 0..10 {
        handles.push(thread::spawn(|| {
            for _ in 0..10 {
                hello();
                bye();
            }
        }));
    }

    std::thread::sleep(Duration::from_secs(1));
    handles
        .into_iter()
        .for_each(|handle| handle.join().unwrap());
}
