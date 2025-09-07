use crossbeam_channel::Sender;
use crossbeam_queue::ArrayQueue;
use derive_builder::Builder;
use std::{
    cell::RefCell,
    fs::File,
    io::{BufWriter, Write},
    thread::{self, JoinHandle},
    time::Instant,
};

#[derive(Debug)]
pub struct SlimEvent {
    pub name: &'static str,
    pub from: std::time::Duration,
    pub to: std::time::Duration,
    pub is_async: bool,
    pub tid: u64,
}

impl SlimEvent {
    fn write_json<W>(self, writer: &mut W)
    where
        W: std::io::Write,
    {
        let pid = std::process::id();
        let json = if self.is_async {
            let begin = self.from.as_nanos() as f64 / 1000.0;
            let end = self.to.as_nanos() as f64 / 1000.0;
            format!("{{\"name\":\"{}\",\"ts\":{},\"pid\":{},\"tid\":{},\"id\":{},\"ph\":\"b\",\"cat\":\"async\"}},\n{{\"name\":\"{}\",\"ts\":{},\"pid\":{},\"tid\":{},\"id\":{},\"ph\":\"e\",\"cat\":\"async\"}}", self.name, begin, pid, self.tid, self.from.as_nanos(), self.name, end, pid, self.tid, self.from.as_nanos())
        } else {
            let ts = self.from.as_nanos() as f64 / 1000.0;
            let dur = (self.to.as_nanos() - self.from.as_nanos()) as f64 / 1000.0;
            format!(
                "{{\"name\":\"{}\",\"ts\":{},\"dur\":{},\"pid\":{},\"tid\":{},\"ph\":\"X\"}}",
                self.name,
                ts,
                dur,
                std::process::id(),
                self.tid
            )
        };
        writer.write_all(json.as_bytes()).unwrap();
    }
}

thread_local! {
    static CURRENT: RefCell<Option<ChromeTracer>> = RefCell::new(None);
}

static mut GLOBAL: Option<ChromeTracer> = None;

#[derive(Builder, Clone)]
#[builder(custom_constructor, build_fn(private, name = "_build"))]
pub struct ChromeTracer {
    #[builder(default = "Instant::now()")]
    pub start: Instant,

    #[builder(setter(skip))]
    sender: Option<Sender<ChromeTracerMessage>>,

    #[builder(default = "std::thread::current().id().as_u64().into()")]
    pub tid: u64,
}

#[allow(clippy::large_enum_variant)]
enum ChromeTracerMessage {
    ChromeEvent(SlimEvent),
    Terminate,
}

pub struct ChromeTracerGuard {
    sender: Sender<ChromeTracerMessage>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for ChromeTracerGuard {
    fn drop(&mut self) {
        self.sender.send(ChromeTracerMessage::Terminate).unwrap();
        self.handle.take().map(JoinHandle::join).unwrap().unwrap();
    }
}

impl ChromeTracerBuilder {
    pub fn init(&self) -> ChromeTracerGuard {
        CURRENT.with(|c| {
            if unsafe { GLOBAL.is_some() } {
                panic!("Unable to intialize ChromeTracer. A chrometracer already been set");
            } else {
                let mut tracer = self._build().expect("All required fields were initialized");
                let guard = tracer.init();

                unsafe { GLOBAL = Some(tracer.clone()) };
                *c.borrow_mut() = Some(tracer);

                guard
            }
        })
    }
}

pub fn builder() -> ChromeTracerBuilder {
    ChromeTracerBuilder::create_empty()
}

impl ChromeTracer {
    fn init(&mut self) -> ChromeTracerGuard {
        let (sender, receiver) = crossbeam_channel::unbounded();
        self.sender = Some(sender.clone());

        let handle = Some(thread::spawn(move || {
            let mut writer = BufWriter::new(File::create("trace.json").unwrap());
            let queue = ArrayQueue::new(1);

            writer.write_all(b"[\n").unwrap();

            while let Ok(ChromeTracerMessage::ChromeEvent(event)) = receiver.recv() {
                if let Some(e) = queue.force_push(event) {
                    e.write_json(&mut writer);
                    writer.write_all(b",\n").unwrap();
                };
            }

            if let Some(e) = queue.pop() {
                e.write_json(&mut writer);
                writer.write_all(b"\n").unwrap();
            }

            writer.write_all(b"]").unwrap();
        }));

        ChromeTracerGuard { sender, handle }
    }

    #[inline]
    pub fn trace(&self, event: SlimEvent) {
        let _ = self
            .sender
            .as_ref()
            .map(|sender| sender.send(ChromeTracerMessage::ChromeEvent(event)));
    }
}

#[inline]
pub fn current<T, F>(mut f: F) -> T
where
    F: FnMut(Option<&ChromeTracer>) -> T,
{
    CURRENT.with(|c| {
        let mut tracer = c.borrow_mut();
        if tracer.is_none() {
            *tracer = unsafe { GLOBAL.clone() };
            tracer
                .as_mut()
                .map(|t| t.tid = std::thread::current().id().as_u64().into());
        }

        f(tracer.as_ref())
    })
}

pub struct Span {
    pub name: &'static str,
    pub tid: Option<u64>,
    pub from: std::time::Duration,
    pub is_async: bool,
}

impl Drop for Span {
    fn drop(&mut self) {
        crate::event!(name: self.name, tid: self.tid, from: self.from, is_async: self.is_async);
    }
}

#[macro_export]
macro_rules! span {
    (name: $name:expr, is_async: $is_async: expr) => {
        $crate::Span {
            name: $name,
            tid: None,
            from: chrometracer::current(|tracer| tracer.map(|t| t.start.elapsed()))
                .unwrap_or_default(),
            is_async: $is_async,
        }
    };
    (name: $name:expr, tid: $tid:expr, is_async: $is_async: expr) => {
        $crate::Span {
            name: $name,
            tid: Some($tid as u64),
            from: chrometracer::current(|tracer| tracer.map(|t| t.start.elapsed()))
                .unwrap_or_default(),
            is_async: $is_async,
        }
    };
}

#[macro_export]
macro_rules! event {
    (name: $name:expr, tid: $tid:expr, from: $from:expr, is_async: $is_async:expr) => {
        $crate::current(|tracer| {
            if let Some(tracer) = tracer {
                let event = $crate::SlimEvent {
                    name: $name,
                    from: $from,
                    to: tracer.start.elapsed(),
                    is_async: $is_async,
                    tid: $tid.unwrap_or(tracer.tid),
                };

                tracer.trace(event);
            }
        })
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn event() {
        let _guard = crate::builder().init();

        event!(name: "hello", tid: None, from: std::time::Duration::from_secs(1), is_async: true);
    }

    #[test]
    fn without_init() {
        event!(name: "hello", tid: None, from: std::time::Duration::from_secs(1), is_async: false);
    }
}
