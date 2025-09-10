#[chrometracer::instrument(fields(name = format!("{}", "hello"), tid = 1))]
async fn hello() {
}

#[tokio::main]
async fn main() {
    let _guard = chrometracer::builder().init();

    let mut handles = vec![];
    for _ in 0..10 {
        handles.push(tokio::spawn(async {
            for _ in 0..10 {
                hello().await;
            }
        }));
    }
}
